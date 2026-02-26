//! Database query functions for user sessions.

use anyhow::Context;
use rand::Rng;
use sqlx::PgPool;

use super::models::UserSession;
use anyhow::Result;

/// Session lifetime: 7 days (in seconds).
pub const SESSION_DURATION_SECS: u64 = 7 * 24 * 3600;

/// Generate a cryptographically random 32-byte hex token.
fn generate_token() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Create a new session for a user with the given duration.
pub async fn create_session(
    pool: &PgPool,
    user_id: i64,
    duration: std::time::Duration,
) -> Result<UserSession> {
    let token = generate_token();
    let duration_secs = duration.as_secs() as i64;

    sqlx::query_as::<_, UserSession>(
        r#"
        INSERT INTO user_sessions (id, user_id, expires_at)
        VALUES ($1, $2, now() + make_interval(secs => $3::double precision))
        RETURNING *
        "#,
    )
    .bind(&token)
    .bind(user_id)
    .bind(duration_secs as f64)
    .fetch_one(pool)
    .await
    .context("failed to create session")
}

/// Fetch a session by token, only if it has not expired.
pub async fn get_session(pool: &PgPool, token: &str) -> Result<Option<UserSession>> {
    sqlx::query_as::<_, UserSession>(
        "SELECT * FROM user_sessions WHERE id = $1 AND expires_at > now()",
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .context("failed to get session")
}

/// Update the last-active timestamp and extend session expiry (sliding window).
pub async fn touch_session(pool: &PgPool, token: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE user_sessions
        SET last_active_at = now(),
            expires_at = now() + make_interval(secs => $2::double precision)
        WHERE id = $1
        "#,
    )
    .bind(token)
    .bind(SESSION_DURATION_SECS as f64)
    .execute(pool)
    .await
    .context("failed to touch session")?;
    Ok(())
}

/// Delete a session by token.
pub async fn delete_session(pool: &PgPool, token: &str) -> Result<()> {
    sqlx::query("DELETE FROM user_sessions WHERE id = $1")
        .bind(token)
        .execute(pool)
        .await
        .context("failed to delete session")?;
    Ok(())
}

/// Delete all sessions for a user. Returns the number of sessions deleted.
#[allow(dead_code)] // Available for admin user-deletion flow
pub async fn delete_user_sessions(pool: &PgPool, user_id: i64) -> Result<u64> {
    let result = sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .context("failed to delete user sessions")?;
    Ok(result.rows_affected())
}

/// Count active (non-expired) sessions.
pub async fn count_active(pool: &PgPool) -> Result<i64> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM user_sessions WHERE expires_at > now()")
            .fetch_one(pool)
            .await?;
    Ok(count)
}

/// Delete all expired sessions. Returns the number of sessions cleaned up.
pub async fn cleanup_expired(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query("DELETE FROM user_sessions WHERE expires_at <= now()")
        .execute(pool)
        .await
        .context("failed to cleanup expired sessions")?;
    Ok(result.rows_affected())
}
