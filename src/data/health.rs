//! Database health check query.

use anyhow::Result;
use sqlx::PgPool;

/// Verify the database connection is alive.
pub async fn ping(pool: &PgPool) -> Result<()> {
    sqlx::query_scalar!(r#"SELECT 1 AS "one!""#).fetch_one(pool).await?;
    Ok(())
}
