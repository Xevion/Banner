//! Generic key-value persistence for application state across restarts.
//!
//! Backed by the `app_kv` UNLOGGED table. Used for scheduler timestamps,
//! bot command fingerprints, and other ephemeral state that should survive
//! normal restarts but is safe to lose on DB crash recovery.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Retrieve a value by key, or `None` if not present.
pub async fn get(pool: &PgPool, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar!("SELECT value FROM app_kv WHERE key = $1", key)
        .fetch_optional(pool)
        .await
}

/// Insert or update a key-value pair.
pub async fn set(pool: &PgPool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO app_kv (key, value)
        VALUES ($1, $2)
        ON CONFLICT (key)
        DO UPDATE SET value = EXCLUDED.value, updated_at = now()
        "#,
        key,
        value,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Retrieve a persisted UTC timestamp, or `None` if absent or unparseable.
pub async fn get_timestamp(pool: &PgPool, key: &str) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    let value = get(pool, key).await?;
    Ok(value.and_then(|v| DateTime::parse_from_rfc3339(&v).ok().map(|dt| dt.to_utc())))
}

/// Persist a UTC timestamp under the given key.
pub async fn set_timestamp(pool: &PgPool, key: &str, ts: DateTime<Utc>) -> Result<(), sqlx::Error> {
    set(pool, key, &ts.to_rfc3339()).await
}
