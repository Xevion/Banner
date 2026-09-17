//! Test databases cloned from a template that is migrated once per machine.
//!
//! `#[sqlx::test]` creates a database per test and replays every migration into
//! it, which costs about a second each. Cloning a migrated template instead, and
//! emptying that clone between runs, holds the cost flat as migrations accumulate.

use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Guards template creation, which several test processes may reach at once.
const TEMPLATE_LOCK: i64 = 0x6261_6e6e_6572;

/// Holds the schema digest a clone was born with, so drift can be detected.
const STAMP_TABLE: &str = "_test_schema";

/// Every column in the public schema, ignoring the two bookkeeping tables.
const SCHEMA_DIGEST: &str = "SELECT md5(coalesce(string_agg(sig, ',' ORDER BY sig), '')) \
     FROM (SELECT table_name || ':' || column_name || ':' || data_type AS sig \
             FROM information_schema.columns \
            WHERE table_schema = 'public' \
              AND table_name NOT IN ('_sqlx_migrations', '_test_schema')) c";

/// Empty every table, leaving the schema and the bookkeeping tables alone.
const TRUNCATE_ALL: &str = r"
DO $$
DECLARE tables text;
BEGIN
    SELECT string_agg(format('%I.%I', schemaname, tablename), ', ')
      INTO tables
      FROM pg_tables
     WHERE schemaname = 'public'
       AND tablename NOT IN ('_sqlx_migrations', '_test_schema');
    IF tables IS NOT NULL THEN
        EXECUTE 'TRUNCATE TABLE ' || tables || ' RESTART IDENTITY CASCADE';
    END IF;
END $$;
";

/// FNV-1a, so the template name depends only on the migrations themselves.
fn digest(parts: impl IntoIterator<Item = Vec<u8>>) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in part {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

/// Name the template after the migration set, so editing one builds a new template
/// rather than silently reusing a stale schema.
fn template_name() -> String {
    let checksums = MIGRATOR.iter().map(|m| m.checksum.to_vec());
    format!("banner_tpl_{:016x}", digest(checksums))
}

fn maintenance_options() -> PgConnectOptions {
    // dotenvy, matching what sqlx::test does, so a bare cargo nextest run still works.
    let url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set to run db tests");
    PgConnectOptions::from_str(&url)
        .expect("DATABASE_URL is not a valid postgres connection string")
        .database("postgres")
}

async fn database_exists(conn: &mut PgConnection, name: &str) -> bool {
    sqlx::query_scalar::<_, i32>("SELECT 1 FROM pg_database WHERE datname = $1")
        .bind(name)
        .fetch_optional(conn)
        .await
        .expect("failed to look up database")
        .is_some()
}

async fn run_statement(conn: &mut PgConnection, sql: String, context: &str) {
    sqlx::query(AssertSqlSafe(sql))
        .execute(conn)
        .await
        .unwrap_or_else(|e| panic!("{context}: {e}"));
}

/// Drop every template and clone left behind by an earlier schema.
///
/// Only reached when a new template is built, so the cost lands once per
/// migration change rather than once per test.
async fn discard_other_generations(conn: &mut PgConnection, keep: &str) {
    let generation = &keep["banner_tpl_".len()..];
    let stale: Vec<String> = sqlx::query_scalar(
        "SELECT datname FROM pg_database \
         WHERE (datname LIKE 'banner_tpl%' OR datname LIKE 'banner_test%') \
           AND datname <> $1 \
           AND datname NOT LIKE $2",
    )
    .bind(keep)
    .bind(format!("banner_test_{generation}%"))
    .fetch_all(&mut *conn)
    .await
    .expect("failed to list stale test databases");

    for name in stale {
        run_statement(
            &mut *conn,
            format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"),
            "failed to drop a stale test database",
        )
        .await;
    }
}

/// Record the migrated schema's digest inside the template itself.
async fn stamp_schema(conn: &mut PgConnection) {
    sqlx::query(AssertSqlSafe(format!(
        "CREATE TABLE {STAMP_TABLE} (digest text NOT NULL)"
    )))
    .execute(&mut *conn)
    .await
    .expect("failed to create the schema stamp");

    sqlx::query(AssertSqlSafe(format!(
        "INSERT INTO {STAMP_TABLE} (digest) {SCHEMA_DIGEST}"
    )))
    .execute(conn)
    .await
    .expect("failed to record the schema stamp");
}

/// Create the migrated template if no other process has already done so.
///
/// The lock is taken only when the template looks absent. Taking it every time
/// would funnel every test in the run through one global mutex.
async fn ensure_template(conn: &mut PgConnection, template: &str) {
    if database_exists(&mut *conn, template).await {
        return;
    }

    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(TEMPLATE_LOCK)
        .execute(&mut *conn)
        .await
        .expect("failed to take the template lock");

    // Another process may have won the race while this one waited.
    if !database_exists(&mut *conn, template).await {
        // Migrate under a scratch name and rename at the end. A reader that sees
        // the real name must see a finished schema, not one mid-migration.
        let building = format!("{template}_building");
        run_statement(
            &mut *conn,
            format!("DROP DATABASE IF EXISTS \"{building}\" WITH (FORCE)"),
            "failed to clear the scratch template",
        )
        .await;
        run_statement(
            &mut *conn,
            format!("CREATE DATABASE \"{building}\""),
            "failed to create the scratch template",
        )
        .await;

        let mut fresh = PgConnection::connect_with(&maintenance_options().database(&building))
            .await
            .expect("failed to connect to the template database");
        MIGRATOR
            .run(&mut fresh)
            .await
            .expect("failed to migrate the template database");
        stamp_schema(&mut fresh).await;
        fresh.close().await.expect("failed to close the template connection");

        run_statement(
            &mut *conn,
            format!("ALTER DATABASE \"{building}\" RENAME TO \"{template}\""),
            "failed to publish the template database",
        )
        .await;
        discard_other_generations(&mut *conn, template).await;
    }

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(TEMPLATE_LOCK)
        .execute(conn)
        .await
        .expect("failed to release the template lock");
}

/// True when the clone still has the schema it was cloned with.
///
/// A test is free to drop or alter a table, and emptying rows would not put it
/// back; such a clone has to be thrown away rather than reused.
async fn schema_intact(pool: &PgPool) -> bool {
    let stamped: Option<String> =
        sqlx::query_scalar(AssertSqlSafe(format!("SELECT digest FROM {STAMP_TABLE} LIMIT 1")))
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let Some(stamped) = stamped else {
        return false;
    };

    let current: Option<String> = sqlx::query_scalar(SCHEMA_DIGEST)
        .fetch_one(pool)
        .await
        .expect("failed to read the current schema");
    current.as_deref() == Some(stamped.as_str())
}

async fn pool_for(name: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(5)
        .connect_with(maintenance_options().database(name))
        .await
        .expect("failed to connect to the test database")
}

/// A pool on a database of this test's own, holding the migrated schema and no rows.
///
/// `key` must be unique per test; [`test_db`] derives one from the call site.
/// The database is cloned once and then reused between runs, emptied rather than
/// recreated: cloning is a file copy per test, and 114 of those at once costs
/// more than truncating tables that are already empty.
pub async fn connect(key: &str) -> PgPool {
    let template = template_name();
    // The template's hash is in the name, so a migration change cannot reuse a
    // clone made from the previous schema.
    let name = format!(
        "banner_test_{}_{:016x}",
        &template["banner_tpl_".len()..],
        digest([key.as_bytes().to_vec()])
    );

    let mut conn = PgConnection::connect_with(&maintenance_options())
        .await
        .expect("failed to connect to the maintenance database");
    ensure_template(&mut conn, &template).await;

    let mut reused = None;
    if database_exists(&mut conn, &name).await {
        let pool = pool_for(&name).await;
        if schema_intact(&pool).await {
            reused = Some(pool);
        } else {
            pool.close().await;
            run_statement(
                &mut conn,
                format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)"),
                "failed to discard a modified test database",
            )
            .await;
        }
    }

    let pool = if let Some(pool) = reused {
        sqlx::query(TRUNCATE_ALL)
            .execute(&pool)
            .await
            .expect("failed to empty the test database");
        pool
    } else {
        run_statement(
            &mut conn,
            format!("CREATE DATABASE \"{name}\" TEMPLATE \"{template}\""),
            "failed to clone the template database",
        )
        .await;
        pool_for(&name).await
    };

    conn.close().await.expect("failed to close the maintenance connection");
    pool
}

/// A pool on a database of this call site's own, cloned from the template.
macro_rules! test_db {
    () => {
        $crate::helpers::db::connect(concat!(module_path!(), ":", line!()))
    };
}

pub(crate) use test_db;
