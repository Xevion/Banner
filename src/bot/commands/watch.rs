//! Course watch commands: /watch, /unwatch, /watches

use crate::banner::Term;
use crate::bot::{Context, Error};
use crate::data::courses::get_id_by_crn;
use crate::data::watches::{self, WatchType};

/// Watch type choices for Discord slash command parameters.
#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum WatchTypeChoice {
    #[name = "Seats Available"]
    SeatsAvailable,
    #[name = "Waitlist Open"]
    WaitlistOpen,
    #[name = "Any Change"]
    AnyChange,
}

impl From<WatchTypeChoice> for WatchType {
    fn from(c: WatchTypeChoice) -> Self {
        match c {
            WatchTypeChoice::SeatsAvailable => WatchType::SeatsAvailable,
            WatchTypeChoice::WaitlistOpen => WatchType::WaitlistOpen,
            WatchTypeChoice::AnyChange => WatchType::AnyChange,
        }
    }
}

fn watch_type_label(watch_type: &str) -> &'static str {
    match watch_type {
        "seats_available" => "Seats Available",
        "waitlist_open" => "Waitlist Open",
        "any_change" => "Any Change",
        _ => "Unknown",
    }
}

/// Watch a course and receive a DM when the specified condition is met.
#[poise::command(slash_command, prefix_command)]
pub async fn watch(
    ctx: Context<'_>,
    #[description = "Course Reference Number (CRN)"] crn: String,
    #[description = "Term code (e.g. 202620 -- defaults to current)"] term: Option<String>,
    #[description = "What to watch for (default: Seats Available)"] watch_type: Option<
        WatchTypeChoice,
    >,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let pool = &ctx.data().app_state.db_pool;
    let term_code = term.unwrap_or_else(|| Term::get_current().inner().to_string());
    let watch_type = WatchType::from(watch_type.unwrap_or(WatchTypeChoice::SeatsAvailable));

    let course_id = match get_id_by_crn(pool, &term_code, &crn).await? {
        Some(id) => id,
        None => {
            ctx.say(format!(
                "No course found with CRN **{}** in term **{}**.",
                crn, term_code
            ))
            .await?;
            return Ok(());
        }
    };

    let author = ctx.author();
    let discord_user_id = author.id.get() as i64;
    let discord_username = author.tag();

    watches::ensure_user(pool, discord_user_id, &discord_username).await?;

    let is_new = watches::upsert_watch(pool, discord_user_id, course_id, &watch_type).await?;

    let label = watch_type_label(watch_type.as_str());
    if is_new {
        ctx.say(format!(
            "Watch set! I'll DM you when **{}** is triggered for CRN **{}** (term {}).",
            label, crn, term_code
        ))
        .await?;
    } else {
        ctx.say(format!(
            "You're already watching CRN **{}** (term {}) for **{}**. Watch reactivated.",
            crn, term_code, label
        ))
        .await?;
    }

    Ok(())
}

/// Remove a course watch.
#[poise::command(slash_command, prefix_command)]
pub async fn unwatch(
    ctx: Context<'_>,
    #[description = "Course Reference Number (CRN)"] crn: String,
    #[description = "Term code (e.g. 202620 -- defaults to current)"] term: Option<String>,
    #[description = "Which watch to remove (default: all)"] watch_type: Option<WatchTypeChoice>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let pool = &ctx.data().app_state.db_pool;
    let term_code = term.unwrap_or_else(|| Term::get_current().inner().to_string());
    let discord_user_id = ctx.author().id.get() as i64;

    let course_id = match get_id_by_crn(pool, &term_code, &crn).await? {
        Some(id) => id,
        None => {
            ctx.say(format!(
                "No course found with CRN **{}** in term **{}**.",
                crn, term_code
            ))
            .await?;
            return Ok(());
        }
    };

    let removed = if let Some(choice) = watch_type {
        let wt = WatchType::from(choice);
        let deleted = watches::delete_watch(pool, discord_user_id, course_id, &wt).await?;
        if deleted { 1u64 } else { 0 }
    } else {
        watches::delete_all_watches_for_course(pool, discord_user_id, course_id).await?
    };

    if removed == 0 {
        ctx.say(format!(
            "No active watch found for CRN **{}** (term {}).",
            crn, term_code
        ))
        .await?;
    } else {
        ctx.say(format!(
            "Removed {} watch(es) for CRN **{}** (term {}).",
            removed, crn, term_code
        ))
        .await?;
    }

    Ok(())
}

/// List your active course watches.
#[poise::command(slash_command, prefix_command)]
pub async fn watches(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let pool = &ctx.data().app_state.db_pool;
    let discord_user_id = ctx.author().id.get() as i64;

    let items = watches::list_active_watches(pool, discord_user_id).await?;

    if items.is_empty() {
        ctx.say("You have no active course watches. Use `/watch` to add one.")
            .await?;
        return Ok(());
    }

    let lines: Vec<String> = items
        .iter()
        .map(|w| {
            let last_notified = w
                .notified_at
                .map(|t| format!(", last notified <t:{}:R>", t.timestamp()))
                .unwrap_or_default();
            format!(
                "**{} {} - {}** (CRN {}, term {}) -- {}{}",
                w.subject,
                w.course_number,
                w.title,
                w.crn,
                w.term_code,
                watch_type_label(&w.watch_type),
                last_notified,
            )
        })
        .collect();

    let body = lines.join("\n");
    ctx.say(format!("Your active watches ({}):\n{}", items.len(), body))
        .await?;
    Ok(())
}
