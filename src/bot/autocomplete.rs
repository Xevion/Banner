//! Autocomplete functions for Discord slash command parameters.

use crate::bot::Context;
use poise::serenity_prelude as serenity;

/// Autocomplete for the subject parameter.
///
/// Filters reference cache entries where code or description contains the
/// partial input (case-insensitive). Returns up to 25 choices formatted as
/// "CS - Computer Science" with the subject code as the value.
pub async fn autocomplete_subject<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = serenity::AutocompleteChoice> + 'a {
    let cache = ctx.data().app_state.reference_cache.read().await;
    let entries = cache.entries_for_category("subject");
    let partial_lower = partial.to_lowercase();

    entries
        .into_iter()
        .filter(move |(code, desc)| {
            partial_lower.is_empty()
                || code.to_lowercase().contains(&partial_lower)
                || desc.to_lowercase().contains(&partial_lower)
        })
        .take(25)
        .map(|(code, desc)| {
            serenity::AutocompleteChoice::new(format!("{code} - {desc}"), code.to_owned())
        })
        .collect::<Vec<_>>()
        .into_iter()
}

/// Autocomplete for the term parameter.
///
/// Filters reference cache entries where code or description contains the
/// partial input (case-insensitive). Returns up to 25 choices formatted as
/// "Spring 2026 (202620)" with the term code as the value.
pub async fn autocomplete_term<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = serenity::AutocompleteChoice> + 'a {
    let cache = ctx.data().app_state.reference_cache.read().await;
    let entries = cache.entries_for_category("term");
    let partial_lower = partial.to_lowercase();

    entries
        .into_iter()
        .filter(move |(code, desc)| {
            partial_lower.is_empty()
                || code.to_lowercase().contains(&partial_lower)
                || desc.to_lowercase().contains(&partial_lower)
        })
        .take(25)
        .map(|(code, desc)| {
            serenity::AutocompleteChoice::new(format!("{desc} ({code})"), code.to_owned())
        })
        .collect::<Vec<_>>()
        .into_iter()
}
