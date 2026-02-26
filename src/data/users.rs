//! Database query functions for users.

use anyhow::Context;
use sqlx::PgPool;

use super::models::User;
use anyhow::Result;

/// Insert a new user or update username/avatar on conflict.
pub async fn upsert_user(
    pool: &PgPool,
    discord_id: i64,
    username: &str,
    avatar_hash: Option<&str>,
) -> Result<User> {
    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (discord_id, discord_username, discord_avatar_hash)
        VALUES ($1, $2, $3)
        ON CONFLICT (discord_id) DO UPDATE
            SET discord_username = EXCLUDED.discord_username,
                discord_avatar_hash = EXCLUDED.discord_avatar_hash,
                updated_at = now()
        RETURNING *
        "#,
    )
    .bind(discord_id)
    .bind(username)
    .bind(avatar_hash)
    .fetch_one(pool)
    .await
    .context("failed to upsert user")
}

/// Fetch a user by Discord ID.
pub async fn get_user(pool: &PgPool, discord_id: i64) -> Result<Option<User>> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE discord_id = $1")
        .bind(discord_id)
        .fetch_optional(pool)
        .await
        .context("failed to get user")
}

/// List all users ordered by creation date (newest first).
pub async fn list_users(pool: &PgPool) -> Result<Vec<User>> {
    sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
        .context("failed to list users")
}

/// Count all registered users.
pub async fn count_all(pool: &PgPool) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// Set the admin flag for a user, returning the updated user if found.
pub async fn set_admin(pool: &PgPool, discord_id: i64, is_admin: bool) -> Result<Option<User>> {
    sqlx::query_as::<_, User>(
        r#"
        UPDATE users
        SET is_admin = $2, updated_at = now()
        WHERE discord_id = $1
        RETURNING *
        "#,
    )
    .bind(discord_id)
    .bind(is_admin)
    .fetch_optional(pool)
    .await
    .context("failed to set admin status")
}

/// Ensure a seed admin exists. Upserts with `is_admin = true` and a placeholder
/// username that will be replaced on first OAuth login.
pub async fn ensure_seed_admin(pool: &PgPool, discord_id: i64) -> Result<User> {
    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (discord_id, discord_username, is_admin)
        VALUES ($1, 'seed-admin', true)
        ON CONFLICT (discord_id) DO UPDATE
            SET is_admin = true,
                updated_at = now()
        RETURNING *
        "#,
    )
    .bind(discord_id)
    .fetch_one(pool)
    .await
    .context("failed to ensure seed admin")
}
