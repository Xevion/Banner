//! Autocomplete functions for Discord slash command parameters.

use crate::bot::Context;
use poise::serenity_prelude as serenity;

/// Autocomplete for the subject parameter.
///
/// Filters reference cache entries where code or description contains the
/// partial input (case-insensitive). Returns up to 25 choices formatted as
/// "CS - Computer Science" with the subject code as the value.
pub async fn autocomplete_subject(
    ctx: Context<'_>,
    partial: &str,
) -> serenity::CreateAutocompleteResponse {
    let cache = ctx.data().app_state.reference_cache.read().await;
    let entries = cache.entries_for_category("subject");
    let partial_lower = partial.to_lowercase();

    let choices = entries
        .into_iter()
        .filter(|(code, desc)| {
            partial_lower.is_empty()
                || code.to_lowercase().contains(&partial_lower)
                || desc.to_lowercase().contains(&partial_lower)
        })
        .take(25)
        .map(|(code, desc)| {
            serenity::AutocompleteChoice::new(format!("{code} - {desc}"), code.to_owned())
        })
        .collect();

    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}

/// Autocomplete for the term parameter.
///
/// Filters reference cache entries where code or description contains the
/// partial input (case-insensitive). Returns up to 25 choices formatted as
/// "Spring 2026 (202620)" with the term code as the value.
pub async fn autocomplete_term(
    ctx: Context<'_>,
    partial: &str,
) -> serenity::CreateAutocompleteResponse {
    let cache = ctx.data().app_state.reference_cache.read().await;
    let entries = cache.entries_for_category("term");
    let partial_lower = partial.to_lowercase();

    let choices = entries
        .into_iter()
        .filter(|(code, desc)| {
            partial_lower.is_empty()
                || code.to_lowercase().contains(&partial_lower)
                || desc.to_lowercase().contains(&partial_lower)
        })
        .take(25)
        .map(|(code, desc)| {
            serenity::AutocompleteChoice::new(format!("{desc} ({code})"), code.to_owned())
        })
        .collect();

    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}
