//! Database operations for course watches (notification subscriptions).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::fmt;
use std::str::FromStr;

/// What condition to watch for on a course.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchType {
    SeatsAvailable,
    WaitlistOpen,
    AnyChange,
}

impl WatchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SeatsAvailable => "seats_available",
            Self::WaitlistOpen => "waitlist_open",
            Self::AnyChange => "any_change",
        }
    }
}

impl fmt::Display for WatchType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WatchType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "seats_available" => Ok(Self::SeatsAvailable),
            "waitlist_open" => Ok(Self::WaitlistOpen),
            "any_change" => Ok(Self::AnyChange),
            _ => Err(anyhow::anyhow!("unknown watch type: {}", s)),
        }
    }
}

/// A watch entry joined with course info, for listing.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WatchListItem {
    pub watch_type: String,
    pub notified_at: Option<DateTime<Utc>>,
    pub crn: String,
    pub term_code: String,
    pub subject: String,
    pub course_number: String,
    pub title: String,
}

/// A watch that has been triggered and should receive a notification.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TriggeredWatch {
    pub watch_id: i32,
    pub discord_user_id: i64,
    pub watch_type: String,
    pub crn: String,
    pub term_code: String,
    pub subject: String,
    pub course_number: String,
    pub title: String,
    pub enrollment: i32,
    pub max_enrollment: i32,
    pub wait_count: i32,
    pub wait_capacity: i32,
}

/// Upsert a minimal user record so the FK on course_watches is satisfied.
///
/// Discord bot users may not have logged in via the web, so we create a thin
/// record from the information available in the bot context.
pub async fn ensure_user(
    pool: &PgPool,
    discord_user_id: i64,
    discord_username: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO users (discord_id, discord_username)
        VALUES ($1, $2)
        ON CONFLICT (discord_id) DO UPDATE
            SET discord_username = EXCLUDED.discord_username,
                updated_at = NOW()
        "#,
    )
    .bind(discord_user_id)
    .bind(discord_username)
    .execute(pool)
    .await
    .context("failed to upsert user")?;
    Ok(())
}

/// Create or reactivate a watch. Returns true if newly created, false if it already existed.
pub async fn upsert_watch(
    pool: &PgPool,
    discord_user_id: i64,
    course_id: i32,
    watch_type: &WatchType,
) -> Result<bool> {
    // xmax = 0 means the row was just inserted; non-zero means it was updated.
    let row: (bool,) = sqlx::query_as(
        r#"
        INSERT INTO course_watches (discord_user_id, course_id, watch_type)
        VALUES ($1, $2, $3)
        ON CONFLICT (discord_user_id, course_id, watch_type)
        DO UPDATE SET active = TRUE, notified_at = NULL
        RETURNING (xmax::text::bigint = 0) AS is_new
        "#,
    )
    .bind(discord_user_id)
    .bind(course_id)
    .bind(watch_type.as_str())
    .fetch_one(pool)
    .await
    .context("failed to upsert watch")?;
    Ok(row.0)
}

/// Delete a specific watch. Returns true if a watch was found and deleted.
pub async fn delete_watch(
    pool: &PgPool,
    discord_user_id: i64,
    course_id: i32,
    watch_type: &WatchType,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        DELETE FROM course_watches
        WHERE discord_user_id = $1 AND course_id = $2 AND watch_type = $3
        "#,
    )
    .bind(discord_user_id)
    .bind(course_id)
    .bind(watch_type.as_str())
    .execute(pool)
    .await
    .context("failed to delete watch")?;
    Ok(result.rows_affected() > 0)
}

/// Delete all watches for a user on a specific course. Returns count deleted.
pub async fn delete_all_watches_for_course(
    pool: &PgPool,
    discord_user_id: i64,
    course_id: i32,
) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM course_watches
        WHERE discord_user_id = $1 AND course_id = $2
        "#,
    )
    .bind(discord_user_id)
    .bind(course_id)
    .execute(pool)
    .await
    .context("failed to delete watches for course")?;
    Ok(result.rows_affected())
}

/// List all active watches for a user with course info.
pub async fn list_active_watches(
    pool: &PgPool,
    discord_user_id: i64,
) -> Result<Vec<WatchListItem>> {
    let items = sqlx::query_as::<_, WatchListItem>(
        r#"
        SELECT
            cw.watch_type,
            cw.notified_at,
            c.crn,
            c.term_code,
            c.subject,
            c.course_number,
            c.title
        FROM course_watches cw
        JOIN courses c ON c.id = cw.course_id
        WHERE cw.discord_user_id = $1
          AND cw.active = TRUE
        ORDER BY c.subject, c.course_number, c.crn, cw.watch_type
        "#,
    )
    .bind(discord_user_id)
    .fetch_all(pool)
    .await
    .context("failed to list active watches")?;
    Ok(items)
}

/// Find all watches that should fire given the set of changed course IDs.
///
/// Applies a 15-minute cooldown via `notified_at`. Each parameter is the set of
/// course IDs that changed in the relevant way:
/// - `enrollment_changed_ids`: courses where enrollment or max_enrollment changed
/// - `waitlist_changed_ids`: courses where wait_count or wait_capacity changed
/// - `any_change_ids`: courses with any non-initial field change
pub async fn find_triggered_watches(
    pool: &PgPool,
    enrollment_changed_ids: &[i32],
    waitlist_changed_ids: &[i32],
    any_change_ids: &[i32],
) -> Result<Vec<TriggeredWatch>> {
    let watches = sqlx::query_as::<_, TriggeredWatch>(
        r#"
        SELECT
            cw.id AS watch_id,
            cw.discord_user_id,
            cw.watch_type,
            c.crn,
            c.term_code,
            c.subject,
            c.course_number,
            c.title,
            c.enrollment,
            c.max_enrollment,
            c.wait_count,
            c.wait_capacity
        FROM course_watches cw
        JOIN courses c ON c.id = cw.course_id
        WHERE cw.active = TRUE
          AND (cw.notified_at IS NULL OR cw.notified_at < NOW() - INTERVAL '15 minutes')
          AND (
            (cw.watch_type = 'seats_available'
                AND c.id = ANY($1::int4[])
                AND c.max_enrollment > c.enrollment)
            OR
            (cw.watch_type = 'waitlist_open'
                AND c.id = ANY($2::int4[])
                AND c.wait_count < c.wait_capacity)
            OR
            (cw.watch_type = 'any_change'
                AND c.id = ANY($3::int4[]))
          )
        "#,
    )
    .bind(enrollment_changed_ids)
    .bind(waitlist_changed_ids)
    .bind(any_change_ids)
    .fetch_all(pool)
    .await
    .context("failed to find triggered watches")?;
    Ok(watches)
}

/// Update `notified_at` to NOW() for a watch after a notification is sent.
pub async fn mark_notified(pool: &PgPool, watch_id: i32) -> Result<()> {
    sqlx::query("UPDATE course_watches SET notified_at = NOW() WHERE id = $1")
        .bind(watch_id)
        .execute(pool)
        .await
        .context("failed to mark watch as notified")?;
    Ok(())
}
