//! Course search command implementation.

use crate::banner::{Course, SearchQuery, Term};
use crate::bot::autocomplete::{autocomplete_subject, autocomplete_term};
use crate::bot::pagination::{self, PageInfo};
use crate::bot::{Context, Error};
use anyhow::anyhow;
use regex::Regex;
use serenity::all::CreateEmbed;
use std::sync::LazyLock;
use tracing::info;

static RANGE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{1,4})-(\d{1,4})?").unwrap());
static WILDCARD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)(x+)").unwrap());

/// Courses rendered per embed page.
const RESULTS_PER_PAGE: usize = 5;

/// Results fetched up front, then paged through client-side.
const MAX_FETCHED_RESULTS: i32 = 50;

/// UTSA blue, used for the search result embeds.
const EMBED_COLOR: u32 = 0x0C_2340;

/// Search for courses with various filters
#[poise::command(slash_command, prefix_command)]
pub async fn search(
    ctx: Context<'_>,
    #[description = "Subject (e.g. CS, MAT, ENG)"]
    #[autocomplete = "autocomplete_subject"]
    subject: Option<String>,
    #[description = "Term (defaults to current)"]
    #[autocomplete = "autocomplete_term"]
    term: Option<String>,
    #[description = "Course title (exact, use autocomplete)"] title: Option<String>,
    #[description = "Course code (e.g. 3743, 3000-3999, 3xxx, 3000-)"] code: Option<String>,
    #[description = "Maximum number of results"] max: Option<i32>,
    #[description = "Keywords in title or description (space separated)"] keywords: Option<String>,
) -> Result<(), Error> {
    // Defer the response since this might take a while
    ctx.defer().await?;

    // Build the search query -- no default credit filter so all courses are visible
    let mut query = SearchQuery::new();

    if let Some(subject) = subject {
        query = query.subject(subject);
    }

    if let Some(title) = title {
        query = query.title(title);
    }

    if let Some(code) = code {
        let (low, high) = parse_course_code(&code)?;
        query = query.course_numbers(low, high);
    }

    if let Some(keywords) = keywords {
        let keyword_list: Vec<String> =
            keywords.split_whitespace().map(|s| s.to_string()).collect();
        query = query.keywords(keyword_list);
    }

    query = query.max_results(
        max.unwrap_or(MAX_FETCHED_RESULTS)
            .clamp(1, MAX_FETCHED_RESULTS),
    );

    let term = term.unwrap_or_else(|| Term::get_current().inner().to_string());
    let search_result = ctx
        .data()
        .app_state
        .banner_api
        .search(&term, &query, "subjectDescription", false)
        .await?;

    let courses = search_result.data.unwrap_or_default();
    if courses.is_empty() {
        ctx.say("No courses found with the specified criteria.")
            .await?;
        return Ok(());
    }

    // total_count is the server-side match count, which can exceed what was fetched.
    let total_results = search_result.total_count.max(courses.len() as i32) as usize;

    pagination::paginate(
        ctx,
        &courses,
        RESULTS_PER_PAGE,
        total_results,
        build_page_embed,
    )
    .await?;

    info!("search command completed");
    Ok(())
}

/// Render one page of courses as a single embed with a field per course.
fn build_page_embed(courses: &[Course], _info: PageInfo) -> CreateEmbed {
    courses.iter().fold(
        CreateEmbed::new().title("Course Search").color(EMBED_COLOR),
        |embed, course| embed.field(course.display_title(), course_summary(course), false),
    )
}

/// Multi-line field body describing a single section.
fn course_summary(course: &Course) -> String {
    format!(
        "{} | CRN `{}`\n{} | {}\n{}",
        course.primary_instructor_name(),
        course.course_reference_number,
        format_enrollment(course.enrollment, course.maximum_enrollment),
        format_waitlist(course.wait_count, course.wait_capacity),
        format_meetings(course),
    )
}

/// Seat usage, e.g. "12/30 seats".
fn format_enrollment(enrollment: i32, maximum: i32) -> String {
    if maximum <= 0 {
        return format!("{enrollment} enrolled");
    }
    format!("{enrollment}/{maximum} seats")
}

/// Waitlist usage, falling back when Banner omits the counts (older terms).
fn format_waitlist(count: Option<i32>, capacity: Option<i32>) -> String {
    match (count, capacity) {
        (Some(count), Some(capacity)) if capacity > 0 => format!("Waitlist {count}/{capacity}"),
        (Some(count), _) if count > 0 => format!("Waitlist {count}"),
        _ => "No waitlist".to_string(),
    }
}

/// Meeting days and times, joined when a section meets on several schedules.
fn format_meetings(course: &Course) -> String {
    let entries: Vec<String> = course
        .meetings_faculty
        .iter()
        .map(|meeting| {
            let info = meeting.schedule_info();
            let days = info.days_string().unwrap_or_else(|| "TBA".to_string());
            match &info.time_range {
                Some(range) => format!("{days} {}", range.format_12hr()),
                None => days,
            }
        })
        .collect();

    if entries.is_empty() {
        "Meeting times TBA".to_string()
    } else {
        entries.join(" / ")
    }
}

/// Parse course code input (e.g, "3743", "3000-3999", "3xxx", "3000-")
fn parse_course_code(input: &str) -> Result<(i32, i32), Error> {
    let input = input.trim();

    // Handle range format (e.g, "3000-3999")
    if input.contains('-') {
        if let Some(captures) = RANGE_RE.captures(input) {
            let low: i32 = captures[1].parse()?;
            let high = if captures.get(2).is_some() {
                captures[2].parse()?
            } else {
                9999 // Open-ended range
            };

            if low > high {
                return Err(anyhow!("Invalid range: low value greater than high value"));
            }

            if low < 1000 || high > 9999 {
                return Err(anyhow!("Course codes must be between 1000 and 9999"));
            }

            return Ok((low, high));
        }
        return Err(anyhow!("Invalid range format"));
    }

    // Handle wildcard format (e.g, "34xx")
    if input.contains('x') {
        if input.len() != 4 {
            return Err(anyhow!("Wildcard format must be exactly 4 characters"));
        }

        if let Some(captures) = WILDCARD_RE.captures(input) {
            let prefix: i32 = captures[1].parse()?;
            let x_count = captures[2].len();

            let low = prefix * 10_i32.pow(x_count as u32);
            let high = low + 10_i32.pow(x_count as u32) - 1;

            if low < 1000 || high > 9999 {
                return Err(anyhow!("Course codes must be between 1000 and 9999"));
            }

            return Ok((low, high));
        }
        return Err(anyhow!("Invalid wildcard format"));
    }

    // Handle single course code
    if input.len() == 4 {
        let code: i32 = input.parse()?;
        if !(1000..=9999).contains(&code) {
            return Err(anyhow!("Course codes must be between 1000 and 9999"));
        }
        return Ok((code, code));
    }

    Err(anyhow!("Invalid course code format"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::check;

    #[test]
    fn test_parse_single_code() {
        assert_eq!(parse_course_code("3743").unwrap(), (3743, 3743));
    }

    #[test]
    fn test_parse_single_code_boundaries() {
        assert_eq!(parse_course_code("1000").unwrap(), (1000, 1000));
        assert_eq!(parse_course_code("9999").unwrap(), (9999, 9999));
    }

    #[test]
    fn test_parse_single_code_below_range() {
        assert!(parse_course_code("0999").is_err());
    }

    #[test]
    fn test_parse_single_code_wrong_length() {
        assert!(parse_course_code("123").is_err());
    }

    #[test]
    fn test_parse_single_code_non_numeric() {
        assert!(parse_course_code("abcd").is_err());
    }

    #[test]
    fn test_parse_single_code_trimmed() {
        assert_eq!(parse_course_code("  3743  ").unwrap(), (3743, 3743));
    }

    #[test]
    fn test_parse_range_full() {
        assert_eq!(parse_course_code("3000-3999").unwrap(), (3000, 3999));
    }

    #[test]
    fn test_parse_range_same() {
        assert_eq!(parse_course_code("3000-3000").unwrap(), (3000, 3000));
    }

    #[test]
    fn test_parse_range_open() {
        assert_eq!(parse_course_code("3000-").unwrap(), (3000, 9999));
    }

    #[test]
    fn test_parse_range_inverted() {
        assert!(parse_course_code("5000-3000").is_err());
    }

    #[test]
    fn test_parse_range_below_1000() {
        assert!(parse_course_code("500-999").is_err());
    }

    #[test]
    fn test_parse_range_above_9999() {
        assert!(parse_course_code("9000-10000").is_err());
    }

    #[test]
    fn test_parse_range_full_valid() {
        assert_eq!(parse_course_code("1000-9999").unwrap(), (1000, 9999));
    }

    #[test]
    fn test_parse_wildcard_one_x() {
        assert_eq!(parse_course_code("300x").unwrap(), (3000, 3009));
    }

    #[test]
    fn test_parse_wildcard_two_x() {
        assert_eq!(parse_course_code("30xx").unwrap(), (3000, 3099));
    }

    #[test]
    fn test_parse_wildcard_three_x() {
        assert_eq!(parse_course_code("3xxx").unwrap(), (3000, 3999));
    }

    #[test]
    fn test_parse_wildcard_9xxx() {
        assert_eq!(parse_course_code("9xxx").unwrap(), (9000, 9999));
    }

    #[test]
    fn test_parse_wildcard_wrong_length() {
        assert!(parse_course_code("3xxxx").is_err());
    }

    #[test]
    fn test_parse_wildcard_0xxx() {
        assert!(parse_course_code("0xxx").is_err());
    }

    #[test]
    fn format_enrollment_shows_seat_usage() {
        check!(format_enrollment(12, 30) == "12/30 seats");
        check!(format_enrollment(0, 30) == "0/30 seats");
        check!(format_enrollment(30, 30) == "30/30 seats");
    }

    #[test]
    fn format_enrollment_without_a_capacity() {
        check!(format_enrollment(7, 0) == "7 enrolled");
    }

    #[test]
    fn format_waitlist_shows_usage() {
        check!(format_waitlist(Some(3), Some(10)) == "Waitlist 3/10");
        check!(format_waitlist(Some(0), Some(10)) == "Waitlist 0/10");
    }

    #[test]
    fn format_waitlist_without_capacity() {
        check!(format_waitlist(Some(4), None) == "Waitlist 4");
        check!(format_waitlist(Some(0), None) == "No waitlist");
        check!(format_waitlist(None, Some(10)) == "No waitlist");
        check!(format_waitlist(None, None) == "No waitlist");
    }
}
