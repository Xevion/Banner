//! Terms command implementation.

use crate::banner::{BannerTerm, Term};
use crate::bot::{Context, Error};
use tracing::info;

/// List available terms or search for a specific term
#[poise::command(slash_command, prefix_command)]
pub async fn terms(
    ctx: Context<'_>,
    #[description = "Term to search for"] search: Option<String>,
    #[description = "Page number"] page: Option<i32>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let search_term = search.unwrap_or_default();
    let page_number = page.unwrap_or(1).max(1);
    let max_results = 10;

    let terms = ctx
        .data()
        .app_state
        .banner_api
        .get_terms(&search_term, page_number, max_results)
        .await?;

    let response = if terms.is_empty() {
        "No terms found.".to_string()
    } else {
        let current_term_code = Term::get_current().inner().to_string();
        terms
            .iter()
            .map(|term| format_term(term, &current_term_code))
            .collect::<Vec<_>>()
            .join("\n")
    };

    ctx.say(response).await?;
    info!("terms command completed");
    Ok(())
}

fn format_term(term: &BannerTerm, current_term_code: &str) -> String {
    let is_current = if term.code == current_term_code {
        " (current)"
    } else {
        ""
    };
    let is_archived = if term.is_archived() {
        " (archived)"
    } else {
        ""
    };
    format!(
        "- `{}`: {}{}{}",
        term.code, term.description, is_current, is_archived
    )
}
