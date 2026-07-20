//! Reusable embed pagination for Discord commands.
//!
//! Callers hand over a fully-fetched slice of items and a per-page renderer; this module
//! chunks the slice, builds the navigation row, and drives the interaction loop until the
//! timeout, at which point the buttons are stripped from the message.

use crate::bot::{Context, Error};
use serenity::all::{
    ButtonStyle, ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
    CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use std::time::Duration;

/// How long the navigation buttons stay live without a press.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Which navigation button was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAction {
    First,
    Prev,
    Next,
    Last,
}

impl PageAction {
    /// Stable token used inside the component custom ID.
    fn token(self) -> &'static str {
        match self {
            PageAction::First => "first",
            PageAction::Prev => "prev",
            PageAction::Next => "next",
            PageAction::Last => "last",
        }
    }

    /// Build the custom ID for this action, scoped to a single command invocation.
    ///
    /// The invocation ID prefix is what keeps a different invocation's buttons from
    /// colliding with these; presses are additionally matched against the invoking user.
    pub fn encode(self, invocation_id: u64) -> String {
        format!("{invocation_id}:page:{}", self.token())
    }

    /// Parse a custom ID back into an action, rejecting IDs from other invocations.
    pub fn decode(custom_id: &str, invocation_id: u64) -> Option<Self> {
        let rest = custom_id.strip_prefix(&custom_id_prefix(invocation_id))?;
        match rest {
            "first" => Some(PageAction::First),
            "prev" => Some(PageAction::Prev),
            "next" => Some(PageAction::Next),
            "last" => Some(PageAction::Last),
            _ => None,
        }
    }

    /// Apply this action to the current page index, clamped to the page range.
    pub fn apply(self, page: usize, page_count: usize) -> usize {
        let last = page_count.saturating_sub(1);
        match self {
            PageAction::First => 0,
            PageAction::Prev => page.saturating_sub(1),
            PageAction::Next => (page + 1).min(last),
            PageAction::Last => last,
        }
    }
}

/// Custom ID namespace for one command invocation.
fn custom_id_prefix(invocation_id: u64) -> String {
    format!("{invocation_id}:page:")
}

/// Which navigation buttons are clickable for a given page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonStates {
    pub first: bool,
    pub prev: bool,
    pub next: bool,
    pub last: bool,
}

impl ButtonStates {
    /// Backwards controls are live off page one, forwards controls off the final page.
    pub fn for_page(page: usize, page_count: usize) -> Self {
        let has_prev = page > 0;
        let has_next = page + 1 < page_count;
        ButtonStates {
            first: has_prev,
            prev: has_prev,
            next: has_next,
            last: has_next,
        }
    }

    fn enabled(self, action: PageAction) -> bool {
        match action {
            PageAction::First => self.first,
            PageAction::Prev => self.prev,
            PageAction::Next => self.next,
            PageAction::Last => self.last,
        }
    }
}

/// Position of the rendered page within the whole result set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageInfo {
    /// Zero-based page index.
    pub index: usize,
    pub page_count: usize,
    /// Total matches reported by the source, which may exceed the items actually fetched.
    pub total_results: usize,
}

impl PageInfo {
    /// Footer line shown under every page.
    pub fn footer_text(self) -> String {
        let noun = if self.total_results == 1 {
            "result"
        } else {
            "results"
        };
        format!(
            "Page {} of {} ({} total {noun})",
            self.index + 1,
            self.page_count.max(1),
            self.total_results
        )
    }
}

/// Number of pages needed to show `item_count` items, never less than one.
pub fn page_count(item_count: usize, per_page: usize) -> usize {
    if per_page == 0 {
        return 1;
    }
    item_count.div_ceil(per_page).max(1)
}

/// The slice of items belonging to `page`, empty when the page is out of range.
pub fn page_slice<T>(items: &[T], page: usize, per_page: usize) -> &[T] {
    if per_page == 0 {
        return &[];
    }
    let start = page.saturating_mul(per_page).min(items.len());
    let end = start.saturating_add(per_page).min(items.len());
    &items[start..end]
}

/// Build the navigation row for a page, scoped to one invocation.
fn action_row(page: usize, page_count: usize, invocation_id: u64) -> CreateActionRow {
    let states = ButtonStates::for_page(page, page_count);
    let buttons = [
        (PageAction::First, "First"),
        (PageAction::Prev, "Previous"),
        (PageAction::Next, "Next"),
        (PageAction::Last, "Last"),
    ]
    .into_iter()
    .map(|(action, label)| {
        CreateButton::new(action.encode(invocation_id))
            .label(label)
            .style(ButtonStyle::Secondary)
            .disabled(!states.enabled(action))
    })
    .collect();

    CreateActionRow::Buttons(buttons)
}

/// Send `items` as paginated embeds and drive the navigation buttons until they time out.
///
/// `render` receives the items for the current page plus its position and returns the embed
/// body; the footer is applied here. Only presses from the invoking user are honored. Returns
/// once the timeout elapses, at which point the buttons are removed from the message.
pub async fn paginate<T, F>(
    ctx: Context<'_>,
    items: &[T],
    per_page: usize,
    total_results: usize,
    render: F,
) -> Result<(), Error>
where
    F: Fn(&[T], PageInfo) -> CreateEmbed,
{
    let invocation_id = ctx.id();
    let author_id = ctx.author().id;
    let page_count = page_count(items.len(), per_page);

    let build = |page: usize| {
        let info = PageInfo {
            index: page,
            page_count,
            total_results,
        };
        render(page_slice(items, page, per_page), info)
            .footer(CreateEmbedFooter::new(info.footer_text()))
    };

    let mut page = 0;
    let mut reply = poise::CreateReply::default().embed(build(page));
    if page_count > 1 {
        reply = reply.components(vec![action_row(page, page_count, invocation_id)]);
    }
    let handle = ctx.send(reply).await?;

    if page_count <= 1 {
        return Ok(());
    }

    let prefix = custom_id_prefix(invocation_id);
    while let Some(press) = ComponentInteractionCollector::new(ctx)
        .filter({
            let prefix = prefix.clone();
            move |press| press.data.custom_id.starts_with(&prefix)
        })
        .timeout(DEFAULT_TIMEOUT)
        .await
    {
        // Buttons belong to the invoker; anyone else silently gets nothing.
        if press.user.id != author_id {
            continue;
        }
        let Some(action) = PageAction::decode(&press.data.custom_id, invocation_id) else {
            continue;
        };

        page = action.apply(page, page_count);
        press
            .create_response(
                ctx.serenity_context(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(build(page))
                        .components(vec![action_row(page, page_count, invocation_id)]),
                ),
            )
            .await?;
    }

    handle
        .edit(
            ctx,
            poise::CreateReply::default()
                .embed(build(page))
                .components(vec![]),
        )
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::check;

    #[test]
    fn page_count_rounds_up() {
        check!(page_count(0, 5) == 1);
        check!(page_count(1, 5) == 1);
        check!(page_count(5, 5) == 1);
        check!(page_count(6, 5) == 2);
        check!(page_count(50, 5) == 10);
    }

    #[test]
    fn page_count_never_divides_by_zero() {
        check!(page_count(10, 0) == 1);
    }

    #[test]
    fn page_slice_chunks_in_order() {
        let items: Vec<usize> = (0..12).collect();
        check!(page_slice(&items, 0, 5) == [0, 1, 2, 3, 4]);
        check!(page_slice(&items, 1, 5) == [5, 6, 7, 8, 9]);
        check!(page_slice(&items, 2, 5) == [10, 11]);
    }

    #[test]
    fn page_slice_out_of_range_is_empty() {
        let items: Vec<usize> = (0..12).collect();
        check!(page_slice(&items, 3, 5).is_empty());
        check!(page_slice(&items, usize::MAX, 5).is_empty());
    }

    #[test]
    fn footer_reports_one_based_page_and_total() {
        let first = PageInfo {
            index: 0,
            page_count: 3,
            total_results: 42,
        };
        check!(first.footer_text() == "Page 1 of 3 (42 total results)");

        let last = PageInfo {
            index: 2,
            page_count: 3,
            total_results: 42,
        };
        check!(last.footer_text() == "Page 3 of 3 (42 total results)");
    }

    #[test]
    fn footer_singularizes_a_lone_result() {
        let info = PageInfo {
            index: 0,
            page_count: 1,
            total_results: 1,
        };
        check!(info.footer_text() == "Page 1 of 1 (1 total result)");
    }

    #[test]
    fn buttons_disabled_at_the_edges() {
        let first = ButtonStates::for_page(0, 3);
        check!(!first.first);
        check!(!first.prev);
        check!(first.next);
        check!(first.last);

        let middle = ButtonStates::for_page(1, 3);
        check!(middle.first);
        check!(middle.prev);
        check!(middle.next);
        check!(middle.last);

        let last = ButtonStates::for_page(2, 3);
        check!(last.first);
        check!(last.prev);
        check!(!last.next);
        check!(!last.last);
    }

    #[test]
    fn buttons_all_disabled_on_a_single_page() {
        let expected = ButtonStates {
            first: false,
            prev: false,
            next: false,
            last: false,
        };
        check!(ButtonStates::for_page(0, 1) == expected);
    }

    #[test]
    fn custom_id_round_trips() {
        for action in [
            PageAction::First,
            PageAction::Prev,
            PageAction::Next,
            PageAction::Last,
        ] {
            let encoded = action.encode(1234);
            check!(PageAction::decode(&encoded, 1234) == Some(action));
        }
    }

    #[test]
    fn custom_id_encodes_the_invocation_id() {
        check!(PageAction::Next.encode(987) == "987:page:next");
    }

    #[test]
    fn custom_id_from_another_invocation_is_rejected() {
        let encoded = PageAction::Next.encode(1234);
        check!(PageAction::decode(&encoded, 5678) == None);
    }

    #[test]
    fn unrelated_custom_ids_are_rejected() {
        check!(PageAction::decode("1234:page:bogus", 1234) == None);
        check!(PageAction::decode("1234:other:next", 1234) == None);
        check!(PageAction::decode("next", 1234) == None);
    }

    #[test]
    fn actions_clamp_to_page_bounds() {
        check!(PageAction::Prev.apply(0, 3) == 0);
        check!(PageAction::Next.apply(2, 3) == 2);
        check!(PageAction::First.apply(2, 3) == 0);
        check!(PageAction::Last.apply(0, 3) == 2);
        check!(PageAction::Next.apply(0, 3) == 1);
        check!(PageAction::Prev.apply(2, 3) == 1);
    }
}
