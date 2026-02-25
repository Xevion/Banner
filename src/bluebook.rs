//! BlueBook (bluebook.utsa.edu) course evaluation scraper.
//!
//! BlueBook is an ASP.NET WebForms application that requires stateful
//! ViewState/EventValidation round-tripping and cookie-based sessions.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use html_scraper::{Html, Selector};
use std::sync::LazyLock;
use std::time::Duration;
use tracing::{debug, info, warn};

use sqlx::PgPool;

use crate::banner::models::terms::{Season, Term};
use crate::data::bluebook::{
    BlueBookEvaluation, batch_upsert_bluebook_evaluations, get_all_subject_scrape_times,
    get_subject_max_terms, mark_subject_scraped,
};

#[allow(dead_code)]
const BASE_URL: &str = "https://bluebook.utsa.edu/Default.aspx";

/// Re-scrape interval for subjects with evaluations in a recent term (within ~2 years).
const RECENT_SUBJECT_INTERVAL: Duration = Duration::from_secs(14 * 24 * 3600);

/// Re-scrape interval for subjects with only old evaluations or zero evaluations.
const HISTORICAL_SUBJECT_INTERVAL: Duration = Duration::from_secs(90 * 24 * 3600);

/// BlueBook-specific season representation.
///
/// BlueBook distinguishes Summer I and Summer II, which Banner collapses
/// into a single "Summer" (code "30"). We parse the distinction internally
/// for fidelity, then collapse when converting to a [`Term`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlueBookSeason {
    Spring,
    SummerI,
    SummerII,
    Fall,
}

impl BlueBookSeason {
    /// Parse a BlueBook season prefix string.
    fn from_bluebook_str(s: &str) -> Option<Self> {
        match s {
            "Spr" | "Spring" => Some(Self::Spring),
            "Sum" | "Sum I" => Some(Self::SummerI),
            "Sum II" => Some(Self::SummerII),
            "Fall" => Some(Self::Fall),
            _ => None,
        }
    }

    /// Collapse to the canonical Banner [`Season`].
    fn to_season(self) -> Season {
        match self {
            Self::Spring => Season::Spring,
            Self::SummerI | Self::SummerII => Season::Summer,
            Self::Fall => Season::Fall,
        }
    }
}

/// Convert a BlueBook term string (e.g. `"Spr 2026"`) to a Banner [`Term`].
///
/// Summer I and Summer II are both collapsed to `Season::Summer`.
#[allow(dead_code)]
fn normalize_term(bluebook_term: &str) -> Option<Term> {
    let parts: Vec<&str> = bluebook_term.trim().rsplitn(2, ' ').collect();
    if parts.len() != 2 {
        warn!(term = bluebook_term, "Unrecognized BlueBook term format");
        return None;
    }

    let year: u32 = match parts[0].parse() {
        Ok(y) => y,
        Err(_) => {
            warn!(term = bluebook_term, "Failed to parse year");
            return None;
        }
    };
    let season_str = parts[1];

    let bb_season = match BlueBookSeason::from_bluebook_str(season_str) {
        Some(s) => s,
        None => {
            warn!(
                term = bluebook_term,
                season = season_str,
                "Unrecognized BlueBook season prefix"
            );
            return None;
        }
    };

    Some(Term {
        year,
        season: bb_season.to_season(),
    })
}

/// All form field values extracted from an ASP.NET WebForms page.
/// WebForms requires the complete set of fields to be round-tripped on every POST.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct FormFields(Vec<(String, String)>);

const TERM_FILTER_RADIO: &str = "ctl00$MainContent$mainContent1$CourseTermSelectRBL";

impl FormFields {
    /// Check whether the form contains a specific named field.
    fn has(&self, name: &str) -> bool {
        self.0.iter().any(|(n, _)| n == name)
    }
}

/// A subject entry from the BlueBook ComboBox.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SubjectEntry {
    /// Subject code, e.g. "CS"
    code: String,
    /// Full display text, e.g. "Computer Science (CS)"
    display_text: String,
    /// 0-based index in the ComboBox <li> list (needed for the HiddenField value)
    combo_index: usize,
}

/// Returns true if a subject should be scraped this cycle.
///
/// Subjects never seen before always scrape. Otherwise the interval depends on
/// whether the subject has recent evaluations: 14 days for recent subjects,
/// 90 days for historical ones.
fn needs_scrape(
    last_scraped: Option<DateTime<Utc>>,
    max_term: Option<&str>,
    current_term_code: &str,
    force: bool,
) -> bool {
    if force {
        return true;
    }
    let Some(last) = last_scraped else {
        return true;
    };
    let interval = if is_recent_subject(max_term, current_term_code) {
        RECENT_SUBJECT_INTERVAL
    } else {
        HISTORICAL_SUBJECT_INTERVAL
    };
    (Utc::now() - last).to_std().unwrap_or(interval) >= interval
}

/// Returns true if the subject has evaluations within ~2 years of the current term.
///
/// Term codes are formatted as YYYYSS (e.g. 202620 = Spring 2026).
/// "Recent" is defined as max_term year >= current year - 2.
fn is_recent_subject(max_term: Option<&str>, current_term_code: &str) -> bool {
    let Some(mt) = max_term else {
        return false;
    };
    if mt.len() < 4 || current_term_code.len() < 4 {
        return false;
    }
    let max_year: u32 = mt[..4].parse().unwrap_or(0);
    let curr_year: u32 = current_term_code[..4].parse().unwrap_or(0);
    max_year + 2 >= curr_year
}

/// Client for scraping BlueBook course evaluations.
#[allow(dead_code)]
pub(crate) struct BlueBookClient {
    http: reqwest::Client,
    delay: Duration,
}

#[allow(dead_code)]
impl Default for BlueBookClient {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl BlueBookClient {
    pub(crate) fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .cookie_store(true)
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build reqwest client"),
            delay: Duration::from_millis(1500),
        }
    }

    /// Extract all form input values from an HTML response.
    /// WebForms requires all fields to be round-tripped on every POST.
    fn extract_form_fields(html: &Html) -> Result<FormFields> {
        let input_sel = Selector::parse("input").unwrap();
        let mut fields = Vec::new();
        let mut has_viewstate = false;

        for input in html.select(&input_sel) {
            let name = match input.attr("name") {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };
            let input_type = input.attr("type").unwrap_or("text").to_lowercase();

            // Skip buttons and image inputs -- they're only sent when clicked
            if input_type == "submit" || input_type == "image" || input_type == "button" {
                continue;
            }

            // Radio buttons and checkboxes: only include if checked
            if (input_type == "radio" || input_type == "checkbox")
                && input.attr("checked").is_none()
            {
                continue;
            }

            let value = input.attr("value").unwrap_or_default().to_string();
            if name == "__VIEWSTATE" {
                has_viewstate = true;
            }
            fields.push((name.to_string(), value));
        }

        if !has_viewstate {
            // Log the title and first part of the page to diagnose server responses
            let title = html
                .select(&Selector::parse("title").unwrap())
                .next()
                .map(|t| t.text().collect::<String>());
            let body_text: String = html.root_element().text().take(500).collect();
            let body_preview: String = body_text.chars().take(300).collect();
            warn!(
                ?title,
                body_preview,
                input_count = fields.len(),
                "No __VIEWSTATE found in response"
            );
            anyhow::bail!("No __VIEWSTATE found in response");
        }

        Ok(FormFields(fields))
    }

    /// Build POST params: clone all form fields, then set __EVENTTARGET and override specified fields.
    fn build_postback(
        fields: &FormFields,
        event_target: &str,
        overrides: &[(&str, &str)],
    ) -> Vec<(String, String)> {
        let mut params = fields.0.clone();

        // Set __EVENTTARGET (should already exist, but override it)
        let mut found_target = false;
        for (name, value) in &mut params {
            if name == "__EVENTTARGET" {
                *value = event_target.to_string();
                found_target = true;
                break;
            }
        }
        if !found_target {
            params.push(("__EVENTTARGET".to_string(), event_target.to_string()));
        }

        // Apply overrides
        for &(key, val) in overrides {
            if let Some(existing) = params.iter_mut().find(|(n, _)| n == key) {
                existing.1 = val.to_string();
            } else {
                params.push((key.to_string(), val.to_string()));
            }
        }

        params
    }

    /// GET the initial page and extract the list of available subjects.
    async fn fetch_subjects(&self) -> Result<(Vec<SubjectEntry>, FormFields)> {
        let resp = self
            .http
            .get(BASE_URL)
            .send()
            .await
            .context("Failed to GET BlueBook page")?;

        let body = resp.text().await?;
        let html = Html::parse_document(&body);
        let fields = Self::extract_form_fields(&html)?;
        let subjects = Self::parse_subjects(&html);

        info!(count = subjects.len(), "Fetched BlueBook subject list");
        Ok((subjects, fields))
    }

    /// Extract subject entries from the ComboBox `<li>` list on the landing page.
    ///
    /// BlueBook uses an AJAX ComboBox where subjects are `<li>` items like
    /// `"Computer Science (CS)"`. The parenthesized code is extracted as the subject code,
    /// and the 0-based `<li>` index is preserved (needed for the HiddenField POST value).
    fn parse_subjects(html: &Html) -> Vec<SubjectEntry> {
        let li_sel = Selector::parse(
            "#ctl00_MainContentSearchQuery_searchCriteriaEntry_CourseSubjectCombo_OptionList > li",
        )
        .unwrap();

        static CODE_RE: LazyLock<regex::Regex> =
            LazyLock::new(|| regex::Regex::new(r"\(([^)]+)\)\s*$").unwrap());

        let mut subjects = Vec::new();

        for (combo_index, li) in html.select(&li_sel).enumerate() {
            let text = li.text().collect::<String>();
            let text = text.trim().to_string();
            if text.is_empty() {
                continue;
            }
            if let Some(caps) = CODE_RE.captures(&text) {
                subjects.push(SubjectEntry {
                    code: caps[1].to_string(),
                    display_text: text,
                    combo_index,
                });
            }
        }

        subjects
    }

    /// POST a search filtered by subject, returning the response HTML and updated form fields.
    async fn search_subject(
        &self,
        subject: &SubjectEntry,
        fields: &FormFields,
    ) -> Result<(Html, FormFields)> {
        tokio::time::sleep(self.delay).await;

        let index_str = subject.combo_index.to_string();
        let params = Self::build_postback(
            fields,
            "ctl00$MainContentSearchQuery$searchCriteriaEntry$SearchBtn",
            &[
                (
                    "ctl00$MainContentSearchQuery$searchCriteriaEntry$CourseSubjectCombo$TextBox",
                    &subject.display_text,
                ),
                (
                    "ctl00$MainContentSearchQuery$searchCriteriaEntry$CourseSubjectCombo$HiddenField",
                    &index_str,
                ),
            ],
        );

        let resp = self
            .http
            .post(BASE_URL)
            .form(&params)
            .send()
            .await
            .context("Failed to POST BlueBook search")?;

        let body = resp.text().await?;
        let html = Html::parse_document(&body);
        let new_fields = Self::extract_form_fields(&html)?;

        Ok((html, new_fields))
    }

    /// POST to switch the term filter (CURRENT/ALL/PAST/FUTURE) and return the updated page.
    async fn switch_term_filter(
        &self,
        filter: &str,
        fields: &FormFields,
    ) -> Result<(Html, FormFields)> {
        tokio::time::sleep(self.delay).await;

        let radio_index = match filter {
            "CURRENT" => "0",
            "ALL" => "1",
            "PAST" => "2",
            "FUTURE" => "3",
            _ => anyhow::bail!("Unknown term filter: {filter}"),
        };

        let event_target = format!("{TERM_FILTER_RADIO}${radio_index}");
        let params = Self::build_postback(fields, &event_target, &[(TERM_FILTER_RADIO, filter)]);

        let resp = self
            .http
            .post(BASE_URL)
            .form(&params)
            .send()
            .await
            .context("Failed to switch BlueBook term filter")?;

        let status = resp.status();
        let body = resp.text().await?;
        debug!(
            filter,
            status = %status,
            body_len = body.len(),
            "switch_term_filter response"
        );
        let html = Html::parse_document(&body);
        let new_fields = Self::extract_form_fields(&html)?;

        Ok((html, new_fields))
    }

    /// POST to navigate to the next page of results.
    async fn next_page(&self, fields: &FormFields, top: bool) -> Result<(Html, FormFields)> {
        tokio::time::sleep(self.delay).await;

        // Pager buttons are `<input type="image">`, which submit as `name.x=N&name.y=N`
        // coordinates rather than via __EVENTTARGET. __EVENTTARGET must be empty.
        let suffix = if top { "TOP" } else { "" };
        let button_name = format!("ctl00$MainContent$mainContent1$PagerImgBtn_Next{suffix}");

        let mut params = fields.0.clone();
        // Clear __EVENTTARGET -- image buttons don't use it
        if let Some(et) = params.iter_mut().find(|(n, _)| n == "__EVENTTARGET") {
            et.1 = String::new();
        }
        // Send image button click coordinates
        params.push((format!("{button_name}.x"), "10".to_string()));
        params.push((format!("{button_name}.y"), "10".to_string()));

        let resp = self
            .http
            .post(BASE_URL)
            .form(&params)
            .send()
            .await
            .context("Failed to navigate BlueBook page")?;

        let body = resp.text().await?;
        let html = Html::parse_document(&body);
        let new_fields = Self::extract_form_fields(&html)?;

        Ok((html, new_fields))
    }

    /// Parse evaluation records from accordion panes on an HTML page.
    ///
    /// Each accordion has a master pane (header) with `table.infoTable`:
    ///   [Sem/Yr, CRN, Course.Section, Title, Instructor,
    ///    InstructorEval ("3.9 / 5.0\n17 students responded" or "n/a"),
    ///    Textbooks, Syllabus,
    ///    CourseEval (same format)]
    ///
    /// Each master pane is immediately followed by a detail pane (expanded content)
    /// containing department, college, campus, schedule, and description.
    /// The detail pane is hidden via CSS (`display:none`) but fully present in the HTML.
    fn parse_evaluations(html: &Html, subject: &str) -> Vec<BlueBookEvaluation> {
        let header_sel = Selector::parse("div.accordionMasterPane").unwrap();
        let detail_sel = Selector::parse("div.accordionDetailPane").unwrap();
        let table_sel = Selector::parse("table.infoTable").unwrap();
        let td_sel = Selector::parse("td").unwrap();
        let mut evals = Vec::new();

        let masters: Vec<_> = html.select(&header_sel).collect();
        let details: Vec<_> = html.select(&detail_sel).collect();

        for (i, pane) in masters.iter().enumerate() {
            let table = match pane.select(&table_sel).next() {
                Some(t) => t,
                None => continue,
            };

            let cells: Vec<String> = table
                .select(&td_sel)
                .map(|td| td.text().collect::<String>())
                .collect();

            // Need at least 9 cells: SemYr, CRN, Course.Section, Title, Instructor, InstEval, Textbooks, Syllabus, CourseEval
            if cells.len() < 9 {
                continue;
            }

            let raw_term = cells[0].trim();
            let term = match normalize_term(raw_term) {
                Some(t) => t,
                None => continue,
            };

            let crn = cells[1].trim().to_string();
            if crn.is_empty() {
                warn!("Empty CRN in BlueBook accordion header, skipping");
                continue;
            }

            // Course.Section: "CS 1083.001" -- extract course_number and section
            let course_section = cells[2].trim();
            let (course_number, section) = match Self::parse_course_section(course_section, subject)
            {
                Some(pair) => pair,
                None => {
                    warn!(
                        raw = course_section,
                        subject, "Failed to parse course.section from header"
                    );
                    continue;
                }
            };

            let instructor_name = cells[4].trim().to_string();
            if instructor_name.is_empty() {
                continue;
            }

            let (instructor_rating, instructor_response_count) =
                Self::parse_rating_cell(cells[5].trim());
            let (course_rating, course_response_count) = Self::parse_rating_cell(cells[8].trim());

            // Skip rows with no evaluation data at all
            if instructor_rating.is_none() && course_rating.is_none() {
                continue;
            }

            let department = details
                .get(i)
                .and_then(|detail| Self::parse_department(*detail));

            evals.push(BlueBookEvaluation {
                subject: subject.to_string(),
                course_number,
                section,
                crn,
                term: term.to_string(),
                instructor_name,
                instructor_rating,
                instructor_response_count,
                course_rating,
                course_response_count,
                department,
            });
        }

        evals
    }

    /// Extract the department from an accordion detail pane.
    ///
    /// The detail pane contains `<span class="contentHeaderSpan">Dept:</span>`
    /// followed by the department text (e.g. "Department of Computer Science").
    /// This text appears in both surveyed and non-surveyed pane variants.
    fn parse_department(detail_pane: html_scraper::ElementRef<'_>) -> Option<String> {
        static DEPT_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
            regex::Regex::new(r"Dept:\s*(.+?)\s*(?:College:|Partial Term:)").unwrap()
        });

        let text: String = detail_pane
            .text()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        DEPT_RE
            .captures(&text)
            .map(|caps| caps[1].trim().to_string())
    }

    /// Parse a course.section string like "CS 1083.001" into ("1083", "001").
    ///
    /// Returns `None` if the format is unrecognized or the course number
    /// doesn't start with a digit.
    ///
    /// The `subject` parameter is accepted for call-site clarity but is NOT used
    /// to validate the display prefix. BlueBook's ComboBox subject codes don't
    /// always match the prefix shown in the accordion cell (e.g., ComboBox "MTC"
    /// -> display prefix "MAT"), so prefix matching would silently drop valid rows.
    fn parse_course_section(raw: &str, subject: &str) -> Option<(String, String)> {
        // Normalize all Unicode whitespace (including non-breaking spaces \u{00A0})
        // to regular ASCII spaces. Some BlueBook HTML cells use non-breaking spaces.
        let normalized: String = raw
            .split(|c: char| c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let raw = normalized.as_str();

        // Format: "DISPLAY_PREFIX COURSE_NUMBER.SECTION"
        // Skip the display prefix (which may differ from the ComboBox subject code)
        // by splitting on the first space.
        let Some((display_prefix, rest)) = raw.split_once(' ') else {
            debug!(
                normalized = raw,
                subject, "parse_course_section: no space separator"
            );
            return None;
        };

        if display_prefix != subject {
            debug!(
                normalized = raw,
                subject,
                display_prefix,
                "parse_course_section: display prefix differs from ComboBox subject code"
            );
        }

        let Some((course_number, section)) = rest.split_once('.') else {
            debug!(
                normalized = raw,
                subject, rest, "parse_course_section: no dot separator"
            );
            return None;
        };
        let course_number = course_number.trim();
        let section = section.trim();

        // Course numbers must start with a digit (e.g. "1083", "3343")
        if !course_number.starts_with(|c: char| c.is_ascii_digit()) {
            debug!(
                normalized = raw,
                subject,
                course_number,
                "parse_course_section: course_number does not start with digit"
            );
            return None;
        }

        Some((course_number.to_string(), section.to_string()))
    }

    /// Parse a rating cell like "3.9 / 5.0\n17 students responded" into (rating, response_count).
    /// Returns (None, None) for "n/a" or unrecognized formats.
    fn parse_rating_cell(text: &str) -> (Option<f32>, Option<i32>) {
        static RESPONSE_RE: LazyLock<regex::Regex> =
            LazyLock::new(|| regex::Regex::new(r"(\d+)\s+students?\s+responded").unwrap());

        if text == "n/a" || text.is_empty() {
            return (None, None);
        }

        let rating = text
            .split('/')
            .next()
            .and_then(|s| s.trim().parse::<f32>().ok());

        let response_count = RESPONSE_RE
            .captures(text)
            .and_then(|caps| caps[1].parse::<i32>().ok());

        (rating, response_count)
    }

    /// Extract current page number and total pages from the pager text (e.g. "3 of 47").
    fn parse_page_info(html: &Html) -> Option<(u32, u32)> {
        static PAGE_RE: LazyLock<regex::Regex> =
            LazyLock::new(|| regex::Regex::new(r"(\d+)\s+of\s+(\d+)").unwrap());

        let pager_sel = Selector::parse("#ctl00_MainContent_mainContent1_topPagerPnl").unwrap();
        let pager = html.select(&pager_sel).next()?;
        let text = pager.text().collect::<String>();
        let caps = PAGE_RE.captures(text.trim())?;
        Some((caps[1].parse().ok()?, caps[2].parse().ok()?))
    }

    /// Scrape all subjects and upsert evaluations to the database per-subject.
    ///
    /// Searches each subject with the PAST term filter, paginates through all
    /// pages, and upserts immediately after each subject completes. Returns the
    /// total number of evaluations upserted.
    ///
    /// When `force` is true all subjects are scraped regardless of timestamps.
    pub(crate) async fn scrape_all(&self, db_pool: &PgPool, force: bool) -> Result<u32> {
        let (subjects, initial_fields) = self.fetch_subjects().await?;

        let scrape_times = get_all_subject_scrape_times(db_pool)
            .await
            .unwrap_or_default();
        let max_terms = get_subject_max_terms(db_pool).await.unwrap_or_default();
        let current_term_code = Term::get_current().inner().to_string();

        let total = subjects.len();
        let eligible_subjects: Vec<_> = subjects
            .iter()
            .filter(|s| {
                let last = scrape_times.get(&s.code).copied();
                let max_term = max_terms.get(&s.code).map(|t| t.as_str());
                needs_scrape(last, max_term, &current_term_code, force)
            })
            .collect();
        let eligible = eligible_subjects.len();
        let skipped_interval = total - eligible;

        info!(
            total,
            eligible,
            skipped = skipped_interval,
            "BlueBook incremental scrape starting"
        );

        let subject_count = eligible;
        let mut total_evals = 0u32;
        let mut skipped_no_radio = 0u32;
        let mut skipped_errors = 0u32;

        for (i, subject) in eligible_subjects.iter().enumerate() {
            let progress = i + 1;

            // Search for the subject (drop Html before next await -- Html is !Send)
            let fields = match self.search_subject(subject, &initial_fields).await {
                Ok((_html, fields)) => fields,
                Err(e) => {
                    warn!(
                        code = subject.code.as_str(),
                        progress, subject_count,
                        error = %e,
                        "Failed to search subject, skipping"
                    );
                    skipped_errors += 1;
                    continue;
                }
            };

            // Subjects with no results don't render the term filter radio buttons.
            // The response contains TotalRows=0 and a "Revise your search criteria"
            // message. Attempting to POST a PAST switch would fail with ASP.NET
            // EventValidation rejection, so detect this early and skip.
            // Mark as scraped so these subjects are not retried every cycle.
            if !fields.has(TERM_FILTER_RADIO) {
                info!(
                    code = subject.code.as_str(),
                    progress, subject_count, "Skipped (no results)"
                );
                skipped_no_radio += 1;
                if let Err(e) = mark_subject_scraped(&subject.code, db_pool).await {
                    warn!(
                        code = subject.code.as_str(),
                        error = %e,
                        "Failed to record scrape timestamp for empty subject"
                    );
                }
                continue;
            }

            // Switch to PAST courses to get completed evaluations
            let mut subject_evals = Vec::new();
            let (total_pages, mut fields) = match self.switch_term_filter("PAST", &fields).await {
                Ok((html, fields)) => {
                    let page_evals = Self::parse_evaluations(&html, &subject.code);
                    subject_evals.extend(page_evals);
                    let total_pages = Self::parse_page_info(&html)
                        .map(|(_, total)| total)
                        .unwrap_or(1);
                    (total_pages, fields)
                }
                Err(e) => {
                    warn!(
                        code = subject.code.as_str(),
                        progress, subject_count,
                        error = %e,
                        "Failed to switch to PAST filter, skipping"
                    );
                    skipped_errors += 1;
                    continue;
                }
            };

            // Paginate through remaining pages
            for page in 2..=total_pages {
                debug!(
                    code = subject.code.as_str(),
                    page, total_pages, "Fetching page"
                );

                match self.next_page(&fields, true).await {
                    Ok((page_html, new_fields)) => {
                        fields = new_fields;
                        let page_evals = Self::parse_evaluations(&page_html, &subject.code);
                        subject_evals.extend(page_evals);
                    }
                    Err(e) => {
                        warn!(
                            code = subject.code.as_str(),
                            page,
                            error = %e,
                            "Failed to fetch page, stopping pagination"
                        );
                        break;
                    }
                }
            }

            let subject_eval_count = subject_evals.len() as u32;

            // Upsert immediately so data is available without waiting for the full scrape
            if !subject_evals.is_empty()
                && let Err(e) = batch_upsert_bluebook_evaluations(&subject_evals, db_pool).await
            {
                warn!(
                    code = subject.code.as_str(),
                    evals = subject_eval_count,
                    error = %e,
                    "Failed to upsert evaluations for subject"
                );
                skipped_errors += 1;
                continue;
            }

            total_evals += subject_eval_count;

            if let Err(e) = mark_subject_scraped(&subject.code, db_pool).await {
                warn!(
                    code = subject.code.as_str(),
                    error = %e,
                    "Failed to record subject scrape timestamp"
                );
            }

            info!(
                code = subject.code.as_str(),
                progress,
                subject_count,
                pages = total_pages,
                evals = subject_eval_count,
                total_evals,
                "Scraped subject"
            );
        }

        info!(
            total_evals,
            subjects = subject_count,
            skipped_no_results = skipped_no_radio,
            skipped_errors,
            "BlueBook scrape complete"
        );
        Ok(total_evals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::EnvFilter;

    /// Initialize tracing for tests that need log output.
    fn init_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("banner::bluebook=debug")),
            )
            .with_test_writer()
            .try_init();
    }

    /// (term, crn, course_section, title, instructor, inst_eval, course_eval, department)
    type AccordionEntry<'a> = (
        &'a str,
        &'a str,
        &'a str,
        &'a str,
        &'a str,
        &'a str,
        &'a str,
        Option<&'a str>,
    );

    fn term(year: u32, season: Season) -> Term {
        Term { year, season }
    }

    #[test]
    fn test_bluebook_season_from_str() {
        assert_eq!(
            BlueBookSeason::from_bluebook_str("Spr"),
            Some(BlueBookSeason::Spring)
        );
        assert_eq!(
            BlueBookSeason::from_bluebook_str("Spring"),
            Some(BlueBookSeason::Spring)
        );
        assert_eq!(
            BlueBookSeason::from_bluebook_str("Sum"),
            Some(BlueBookSeason::SummerI)
        );
        assert_eq!(
            BlueBookSeason::from_bluebook_str("Sum I"),
            Some(BlueBookSeason::SummerI)
        );
        assert_eq!(
            BlueBookSeason::from_bluebook_str("Sum II"),
            Some(BlueBookSeason::SummerII)
        );
        assert_eq!(
            BlueBookSeason::from_bluebook_str("Fall"),
            Some(BlueBookSeason::Fall)
        );
        assert_eq!(BlueBookSeason::from_bluebook_str("Winter"), None);
        assert_eq!(BlueBookSeason::from_bluebook_str(""), None);
    }

    #[test]
    fn test_bluebook_season_summer_collapses() {
        assert_eq!(BlueBookSeason::SummerI.to_season(), Season::Summer);
        assert_eq!(BlueBookSeason::SummerII.to_season(), Season::Summer);
    }

    #[test]
    fn test_normalize_term_spring() {
        assert_eq!(normalize_term("Spr 2026"), Some(term(2026, Season::Spring)));
    }

    #[test]
    fn test_normalize_term_fall() {
        assert_eq!(normalize_term("Fall 2025"), Some(term(2025, Season::Fall)));
    }

    #[test]
    fn test_normalize_term_summer_i() {
        assert_eq!(
            normalize_term("Sum I 2026"),
            Some(term(2026, Season::Summer))
        );
    }

    #[test]
    fn test_normalize_term_summer_ii_collapses_to_summer() {
        assert_eq!(
            normalize_term("Sum II 2026"),
            Some(term(2026, Season::Summer))
        );
    }

    #[test]
    fn test_normalize_term_summer_bare() {
        assert_eq!(normalize_term("Sum 2025"), Some(term(2025, Season::Summer)));
    }

    #[test]
    fn test_normalize_term_spring_full() {
        assert_eq!(
            normalize_term("Spring 2025"),
            Some(term(2025, Season::Spring))
        );
    }

    #[test]
    fn test_normalize_term_unknown() {
        assert_eq!(normalize_term("Winter 2025"), None);
    }

    #[test]
    fn test_normalize_term_empty() {
        assert_eq!(normalize_term(""), None);
    }

    #[test]
    fn test_normalize_term_whitespace() {
        assert_eq!(
            normalize_term("  Fall 2025  "),
            Some(term(2025, Season::Fall))
        );
    }

    #[test]
    fn test_normalize_term_invalid_year() {
        assert_eq!(normalize_term("Fall abcd"), None);
    }

    #[test]
    fn test_parse_rating_cell_with_rating() {
        let (rating, count) = BlueBookClient::parse_rating_cell("3.9 / 5.0\n17 students responded");
        assert_eq!(rating, Some(3.9));
        assert_eq!(count, Some(17));
    }

    #[test]
    fn test_parse_rating_cell_na() {
        let (rating, count) = BlueBookClient::parse_rating_cell("n/a");
        assert_eq!(rating, None);
        assert_eq!(count, None);
    }

    #[test]
    fn test_parse_rating_cell_empty() {
        let (rating, count) = BlueBookClient::parse_rating_cell("");
        assert_eq!(rating, None);
        assert_eq!(count, None);
    }

    #[test]
    fn test_parse_rating_cell_rating_without_response_count() {
        let (rating, count) = BlueBookClient::parse_rating_cell("4.2 / 5.0");
        assert_eq!(rating, Some(4.2));
        assert_eq!(count, None);
    }

    #[test]
    fn test_parse_rating_cell_singular_student() {
        let (rating, count) = BlueBookClient::parse_rating_cell("5.0 / 5.0\n1 student responded");
        assert_eq!(rating, Some(5.0));
        assert_eq!(count, Some(1));
    }

    #[test]
    fn test_parse_course_section_standard() {
        assert_eq!(
            BlueBookClient::parse_course_section("CS 1083.001", "CS"),
            Some(("1083".to_string(), "001".to_string()))
        );
    }

    #[test]
    fn test_parse_course_section_letter_section() {
        assert_eq!(
            BlueBookClient::parse_course_section("CS 3343.01T", "CS"),
            Some(("3343".to_string(), "01T".to_string()))
        );
    }

    #[test]
    fn test_parse_course_section_no_dot() {
        assert_eq!(BlueBookClient::parse_course_section("CS 1083", "CS"), None);
    }

    #[test]
    fn test_parse_course_section_display_prefix_differs_from_subject() {
        // The display prefix in the accordion cell may differ from the ComboBox
        // subject code (e.g., code "IS" -> display "ISA", code "MTC" -> display "MAT").
        // We no longer reject on mismatch -- the page is subject-scoped by BlueBook
        // and we trust its contents. Course number and section are still parsed.
        assert_eq!(
            BlueBookClient::parse_course_section("ISA 1234.001", "IS"),
            Some(("1234".to_string(), "001".to_string()))
        );
    }

    #[test]
    fn test_parse_course_section_exact_subject_match() {
        assert_eq!(
            BlueBookClient::parse_course_section("IS 1234.001", "IS"),
            Some(("1234".to_string(), "001".to_string()))
        );
    }

    /// BlueBook lists some subjects twice in the ComboBox under different internal
    /// codes. E.g., "Mathematics (MAT)" and "Mathematics (MTC)" both exist, but
    /// accordion cells always display the prefix "MAT". Scraping with code "MTC"
    /// must still parse "MAT 1043.06B" successfully.
    #[test]
    fn test_parse_course_section_combobox_code_differs_from_display_prefix() {
        assert_eq!(
            BlueBookClient::parse_course_section("MAT 1043.06B", "MTC"),
            Some(("1043".to_string(), "06B".to_string()))
        );
    }

    /// BlueBook HTML sometimes uses non-breaking spaces (\u{00A0}) between the
    /// subject code and course number. strip_prefix with a regular space fails.
    #[test]
    fn test_parse_course_section_non_breaking_space() {
        assert_eq!(
            BlueBookClient::parse_course_section("MAT\u{00A0}1213.001", "MAT"),
            Some(("1213".to_string(), "001".to_string()))
        );
    }

    /// Non-breaking space also appears in parse_evaluations via the HTML cell text.
    #[test]
    fn test_parse_evaluations_non_breaking_space_in_course_section() {
        // Simulate the BlueBook HTML where course section uses \u{00A0} instead of a space
        let html_str = format!(
            r#"<html><body>
            <div class="accordionMasterPane">
                <table class="infoTable"><tr>
                    <td>Fall 2025</td>
                    <td>12345</td>
                    <td>MAT{nbsp}1213.001</td>
                    <td>Calculus I</td>
                    <td>Smith, John</td>
                    <td>4.5 / 5.0\n30 students responded</td>
                    <td>n/a</td>
                    <td>n/a</td>
                    <td>4.0 / 5.0\n30 students responded</td>
                </tr></table>
            </div>
            <div class="accordionDetailPane" style="display:none;"></div>
            </body></html>"#,
            nbsp = '\u{00A0}'
        );
        let html = Html::parse_document(&html_str);
        let evals = BlueBookClient::parse_evaluations(&html, "MAT");
        assert_eq!(
            evals.len(),
            1,
            "Should parse MAT course with non-breaking space"
        );
        assert_eq!(evals[0].course_number, "1213");
        assert_eq!(evals[0].section, "001");
    }

    /// Alphanumeric sections (ON1, ON2, 09A, 901) seen in production logs must parse.
    #[test]
    fn test_parse_course_section_alphanumeric_sections() {
        for section in &["ON1", "ON2", "09A", "10A", "01B", "901"] {
            let raw = format!("MAT\u{00A0}1023.{section}");
            assert_eq!(
                BlueBookClient::parse_course_section(&raw, "MAT"),
                Some(("1023".to_string(), section.to_string())),
                "Failed to parse section: {section}"
            );
        }
    }

    #[test]
    fn test_extract_form_fields_basic() {
        let html_str = r#"<html><body><form>
            <input type="hidden" name="__VIEWSTATE" value="abc123" />
            <input type="hidden" name="__EVENTTARGET" value="" />
            <input type="text" name="search" value="hello" />
        </form></body></html>"#;
        let html = Html::parse_document(html_str);
        let fields = BlueBookClient::extract_form_fields(&html).unwrap();
        assert_eq!(fields.0.len(), 3);
        assert!(
            fields
                .0
                .iter()
                .any(|(n, v)| n == "__VIEWSTATE" && v == "abc123")
        );
        assert!(fields.0.iter().any(|(n, _)| n == "__EVENTTARGET"));
        assert!(fields.0.iter().any(|(n, v)| n == "search" && v == "hello"));
    }

    #[test]
    fn test_extract_form_fields_skips_buttons() {
        let html_str = r#"<html><body><form>
            <input type="hidden" name="__VIEWSTATE" value="x" />
            <input type="submit" name="btn" value="Go" />
            <input type="image" name="img" value="pic" />
            <input type="button" name="btn2" value="Click" />
        </form></body></html>"#;
        let html = Html::parse_document(html_str);
        let fields = BlueBookClient::extract_form_fields(&html).unwrap();
        assert_eq!(fields.0.len(), 1);
        assert_eq!(fields.0[0].0, "__VIEWSTATE");
    }

    #[test]
    fn test_extract_form_fields_checkbox_checked_only() {
        let html_str = r#"<html><body><form>
            <input type="hidden" name="__VIEWSTATE" value="x" />
            <input type="checkbox" name="opt1" value="a" checked="checked" />
            <input type="checkbox" name="opt2" value="b" />
        </form></body></html>"#;
        let html = Html::parse_document(html_str);
        let fields = BlueBookClient::extract_form_fields(&html).unwrap();
        assert!(fields.0.iter().any(|(n, _)| n == "opt1"));
        assert!(!fields.0.iter().any(|(n, _)| n == "opt2"));
    }

    #[test]
    fn test_extract_form_fields_no_viewstate() {
        let html_str = r#"<html><body><form>
            <input type="hidden" name="other" value="x" />
        </form></body></html>"#;
        let html = Html::parse_document(html_str);
        assert!(BlueBookClient::extract_form_fields(&html).is_err());
    }

    #[test]
    fn test_build_postback_sets_event_target() {
        let fields = FormFields(vec![
            ("__VIEWSTATE".to_string(), "vs".to_string()),
            ("__EVENTTARGET".to_string(), "".to_string()),
            ("field1".to_string(), "val1".to_string()),
        ]);
        let params = BlueBookClient::build_postback(&fields, "my_target", &[]);
        let target = params.iter().find(|(n, _)| n == "__EVENTTARGET").unwrap();
        assert_eq!(target.1, "my_target");
    }

    #[test]
    fn test_build_postback_applies_overrides() {
        let fields = FormFields(vec![
            ("__VIEWSTATE".to_string(), "vs".to_string()),
            ("__EVENTTARGET".to_string(), "".to_string()),
            ("field1".to_string(), "old".to_string()),
        ]);
        let params = BlueBookClient::build_postback(
            &fields,
            "target",
            &[("field1", "new"), ("field2", "added")],
        );
        let f1 = params.iter().find(|(n, _)| n == "field1").unwrap();
        assert_eq!(f1.1, "new");
        let f2 = params.iter().find(|(n, _)| n == "field2").unwrap();
        assert_eq!(f2.1, "added");
    }

    #[test]
    fn test_build_postback_adds_missing_event_target() {
        let fields = FormFields(vec![("__VIEWSTATE".to_string(), "vs".to_string())]);
        let params = BlueBookClient::build_postback(&fields, "my_target", &[]);
        assert!(
            params
                .iter()
                .any(|(n, v)| n == "__EVENTTARGET" && v == "my_target")
        );
    }

    #[test]
    fn test_parse_page_info_found() {
        let html_str = r#"<html><body>
            <div id="ctl00_MainContent_mainContent1_topPagerPnl">3 of 47</div>
        </body></html>"#;
        let html = Html::parse_document(html_str);
        assert_eq!(BlueBookClient::parse_page_info(&html), Some((3, 47)));
    }

    #[test]
    fn test_parse_page_info_missing() {
        let html_str = "<html><body></body></html>";
        let html = Html::parse_document(html_str);
        assert_eq!(BlueBookClient::parse_page_info(&html), None);
    }

    #[test]
    fn test_parse_subjects_standard() {
        let html_str = r#"<html><body>
            <ul id="ctl00_MainContentSearchQuery_searchCriteriaEntry_CourseSubjectCombo_OptionList">
                <li></li>
                <li>Computer Science (CS)</li>
                <li>Mathematics (MAT)</li>
                <li>Information Systems &amp; Cyber Security (IS)</li>
            </ul>
        </body></html>"#;
        let html = Html::parse_document(html_str);
        let subjects = BlueBookClient::parse_subjects(&html);

        assert_eq!(subjects.len(), 3);

        assert_eq!(subjects[0].code, "CS");
        assert_eq!(subjects[0].display_text, "Computer Science (CS)");
        assert_eq!(subjects[0].combo_index, 1);

        assert_eq!(subjects[1].code, "MAT");
        assert_eq!(subjects[1].combo_index, 2);

        assert_eq!(subjects[2].code, "IS");
        assert_eq!(subjects[2].combo_index, 3);
    }

    #[test]
    fn test_parse_subjects_skips_empty_items() {
        let html_str = r#"<html><body>
            <ul id="ctl00_MainContentSearchQuery_searchCriteriaEntry_CourseSubjectCombo_OptionList">
                <li></li>
                <li>  </li>
                <li>Computer Science (CS)</li>
            </ul>
        </body></html>"#;
        let html = Html::parse_document(html_str);
        let subjects = BlueBookClient::parse_subjects(&html);

        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].code, "CS");
        // combo_index preserves the original position, including empty items
        assert_eq!(subjects[0].combo_index, 2);
    }

    #[test]
    fn test_parse_subjects_skips_no_code() {
        let html_str = r#"<html><body>
            <ul id="ctl00_MainContentSearchQuery_searchCriteriaEntry_CourseSubjectCombo_OptionList">
                <li>Some text without code</li>
                <li>Computer Science (CS)</li>
            </ul>
        </body></html>"#;
        let html = Html::parse_document(html_str);
        let subjects = BlueBookClient::parse_subjects(&html);

        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].code, "CS");
    }

    #[test]
    fn test_parse_subjects_empty_list() {
        let html_str = "<html><body></body></html>";
        let html = Html::parse_document(html_str);
        let subjects = BlueBookClient::parse_subjects(&html);
        assert!(subjects.is_empty());
    }

    /// Build a minimal BlueBook accordion HTML page for testing.
    /// Each entry is (term, crn, course_section, title, instructor, inst_eval, course_eval, department).
    fn build_accordion_html(entries: &[AccordionEntry<'_>]) -> String {
        let mut html = String::from("<html><body>");
        for (term, crn, course_section, title, instructor, inst_eval, course_eval, dept) in entries
        {
            // Master pane (header)
            html.push_str(&format!(
                r#"<div class="accordionMasterPane">
                    <table class="infoTable"><tr>
                        <td>{term}</td>
                        <td>{crn}</td>
                        <td>{course_section}</td>
                        <td>{title}</td>
                        <td>{instructor}</td>
                        <td>{inst_eval}</td>
                        <td>n/a</td>
                        <td>n/a</td>
                        <td>{course_eval}</td>
                    </tr></table>
                </div>"#
            ));
            // Detail pane (expanded content, hidden via CSS)
            let dept_html = dept.unwrap_or("Unknown Department");
            html.push_str(&format!(
                r#"<div class="accordionDetailPane" style="display:none;">
                    <div>
                        <span class="contentHeaderSpan">Dept:</span> {dept_html}<br>
                        <span class="contentHeaderSpan">College:</span> College of Sciences<br>
                    </div>
                </div>"#
            ));
        }
        html.push_str("</body></html>");
        html
    }

    #[test]
    fn test_parse_evaluations_with_ratings() {
        let html_str = build_accordion_html(&[(
            "Sum 2025",
            "33601",
            "CS 1083.01T",
            "Intro to CS I CS",
            "Gomez Morales, Mauricio Alejandro",
            "3.9 / 5.0\n17 students responded",
            "3.9 / 5.0\n17 students responded",
            Some("Department of Computer Science"),
        )]);
        let html = Html::parse_document(&html_str);
        let evals = BlueBookClient::parse_evaluations(&html, "CS");

        assert_eq!(evals.len(), 1);
        let eval = &evals[0];
        assert_eq!(eval.subject, "CS");
        assert_eq!(eval.course_number, "1083");
        assert_eq!(eval.section, "01T");
        assert_eq!(eval.crn, "33601");
        assert_eq!(eval.term, "202530");
        assert_eq!(eval.instructor_name, "Gomez Morales, Mauricio Alejandro");
        assert_eq!(eval.instructor_rating, Some(3.9));
        assert_eq!(eval.instructor_response_count, Some(17));
        assert_eq!(eval.course_rating, Some(3.9));
        assert_eq!(eval.course_response_count, Some(17));
        assert_eq!(
            eval.department.as_deref(),
            Some("Department of Computer Science")
        );
    }

    #[test]
    fn test_parse_evaluations_skips_no_rating_rows() {
        let html_str = build_accordion_html(&[(
            "Fall 2025",
            "19697",
            "CS 1063.001",
            "Intro to Comp Programming I",
            "Long, Byron Lindsay",
            "n/a",
            "n/a",
            Some("Department of Computer Science"),
        )]);
        let html = Html::parse_document(&html_str);
        let evals = BlueBookClient::parse_evaluations(&html, "CS");

        assert!(
            evals.is_empty(),
            "Should skip rows where both ratings are n/a"
        );
    }

    #[test]
    fn test_parse_evaluations_partial_ratings() {
        let html_str = build_accordion_html(&[(
            "Spr 2025",
            "12345",
            "CS 3343.001",
            "Design Analysis of Algorithms",
            "Smith, John",
            "4.5 / 5.0\n30 students responded",
            "n/a",
            Some("Department of Computer Science"),
        )]);
        let html = Html::parse_document(&html_str);
        let evals = BlueBookClient::parse_evaluations(&html, "CS");

        assert_eq!(evals.len(), 1);
        let eval = &evals[0];
        assert_eq!(eval.instructor_rating, Some(4.5));
        assert_eq!(eval.instructor_response_count, Some(30));
        assert_eq!(eval.course_rating, None);
        assert_eq!(eval.course_response_count, None);
    }

    #[test]
    fn test_parse_evaluations_multiple_panes() {
        let html_str = build_accordion_html(&[
            (
                "Sum 2025",
                "33601",
                "CS 1083.01T",
                "Intro to CS I",
                "Gomez, M",
                "3.9 / 5.0\n17 students responded",
                "3.9 / 5.0\n17 students responded",
                Some("Department of Computer Science"),
            ),
            (
                "Fall 2025",
                "19697",
                "CS 1063.001",
                "Intro to Programming",
                "Long, B",
                "n/a",
                "n/a",
                None,
            ),
            (
                "Sum 2025",
                "31355",
                "CS 1173.01T",
                "Data Analysis",
                "Rutherford, L",
                "4.2 / 5.0\n49 students responded",
                "3.9 / 5.0\n49 students responded",
                Some("Department of Computer Science"),
            ),
        ]);
        let html = Html::parse_document(&html_str);
        let evals = BlueBookClient::parse_evaluations(&html, "CS");

        // Second entry has no ratings -> skipped
        assert_eq!(evals.len(), 2);
        assert_eq!(evals[0].crn, "33601");
        assert_eq!(evals[1].crn, "31355");
        assert_eq!(evals[1].instructor_rating, Some(4.2));
        assert_eq!(evals[1].course_rating, Some(3.9));
    }

    #[test]
    fn test_parse_evaluations_fewer_than_9_cells() {
        let html_str = r#"<html><body>
            <div class="accordionMasterPane">
                <table class="infoTable"><tr>
                    <td>Fall 2025</td><td>12345</td><td>CS 1083.001</td>
                </tr></table>
            </div>
            <div class="accordionDetailPane" style="display:none;"></div>
        </body></html>"#;
        let html = Html::parse_document(html_str);
        let evals = BlueBookClient::parse_evaluations(&html, "CS");
        assert!(evals.is_empty(), "Should skip rows with fewer than 9 cells");
    }

    #[test]
    fn test_parse_evaluations_empty_crn() {
        let html_str = build_accordion_html(&[(
            "Sum 2025",
            "",
            "CS 1083.01T",
            "Intro to CS",
            "Smith, J",
            "4.0 / 5.0\n10 students responded",
            "4.0 / 5.0\n10 students responded",
            None,
        )]);
        let html = Html::parse_document(&html_str);
        let evals = BlueBookClient::parse_evaluations(&html, "CS");
        assert!(evals.is_empty(), "Should skip rows with empty CRN");
    }

    #[test]
    fn test_parse_evaluations_empty_instructor() {
        let html_str = build_accordion_html(&[(
            "Sum 2025",
            "33601",
            "CS 1083.01T",
            "Intro to CS",
            "",
            "4.0 / 5.0\n10 students responded",
            "4.0 / 5.0\n10 students responded",
            None,
        )]);
        let html = Html::parse_document(&html_str);
        let evals = BlueBookClient::parse_evaluations(&html, "CS");
        assert!(evals.is_empty(), "Should skip rows with empty instructor");
    }

    #[test]
    fn test_parse_department_standard() {
        let html_str = r#"<html><body><div class="detail">
            <span class="contentHeaderSpan">Dept:</span> Department of Computer Science<br>
            <span class="contentHeaderSpan">College:</span> College of Sciences<br>
        </div></body></html>"#;
        let html = Html::parse_document(html_str);
        let detail_sel = Selector::parse("div.detail").unwrap();
        let detail = html.select(&detail_sel).next().unwrap();
        assert_eq!(
            BlueBookClient::parse_department(detail),
            Some("Department of Computer Science".to_string())
        );
    }

    #[test]
    fn test_parse_department_with_partial_term() {
        let html_str = r#"<html><body><div class="detail">
            <span class="contentHeaderSpan">Dept:</span> Department of
                Computer Science
            <br>
            <span class="contentHeaderSpan">Partial Term:</span> T - Ten-week Summer<br>
            <span class="contentHeaderSpan">College:</span> College of Sciences<br>
        </div></body></html>"#;
        let html = Html::parse_document(html_str);
        let detail_sel = Selector::parse("div.detail").unwrap();
        let detail = html.select(&detail_sel).next().unwrap();
        assert_eq!(
            BlueBookClient::parse_department(detail),
            Some("Department of Computer Science".to_string())
        );
    }

    #[test]
    fn test_parse_department_missing() {
        let html_str = r#"<html><body><div class="detail">
            <span class="contentHeaderSpan">College:</span> College of Sciences<br>
        </div></body></html>"#;
        let html = Html::parse_document(html_str);
        let detail_sel = Selector::parse("div.detail").unwrap();
        let detail = html.select(&detail_sel).next().unwrap();
        assert_eq!(BlueBookClient::parse_department(detail), None);
    }

    /// Verify that searching CS and switching to PAST produces evaluations across
    /// multiple pages (the core bug was broken pagination via image button handling).
    #[tokio::test]
    #[ignore = "requires network access to bluebook.utsa.edu"]
    async fn test_live_cs_past_pagination() {
        init_tracing();

        let client = BlueBookClient::new();
        let (subjects, initial_fields) = client.fetch_subjects().await.unwrap();
        let cs = subjects
            .iter()
            .find(|s| s.code == "CS")
            .expect("CS subject must exist");

        // Search CS
        let (_html, fields) = client.search_subject(cs, &initial_fields).await.unwrap();
        assert!(
            fields.has(TERM_FILTER_RADIO),
            "CS search should render term filter radio"
        );

        // Switch to PAST
        let (html, mut fields) = client.switch_term_filter("PAST", &fields).await.unwrap();
        let page1_evals = BlueBookClient::parse_evaluations(&html, "CS");
        let (current_page, total_pages) =
            BlueBookClient::parse_page_info(&html).expect("PAST results should have pager");
        eprintln!(
            "Page {current_page}/{total_pages}: {} evals",
            page1_evals.len()
        );
        assert!(total_pages > 1, "CS PAST should have multiple pages");

        let mut all_evals = page1_evals;

        // Pages 1-~8 are Fall 2025 (n/a evals). Spr 2025 data with real ratings
        // starts around page 9-10. Paginate enough to reach them.
        let pages_to_check = 12.min(total_pages);
        for page in 2..=pages_to_check {
            let (page_html, new_fields) = client.next_page(&fields, true).await.unwrap();
            fields = new_fields;
            let page_evals = BlueBookClient::parse_evaluations(&page_html, "CS");
            // Log the semester of the first accordion to track pagination progress
            let semester_sel = Selector::parse("span[id*='SemYrLbl']").unwrap();
            let first_semester: String = page_html
                .select(&semester_sel)
                .next()
                .map(|el| el.text().collect())
                .unwrap_or_default();
            eprintln!(
                "Page {page}/{total_pages}: {} evals (semester: {first_semester})",
                page_evals.len()
            );
            all_evals.extend(page_evals);
        }

        eprintln!(
            "Total evaluations from {pages_to_check} pages: {}",
            all_evals.len()
        );
        assert!(
            !all_evals.is_empty(),
            "Should have evaluations from CS PAST pages (page 1 may be n/a Fall 2025, but later pages have Spr 2025+ data)"
        );

        // Verify evaluation data quality
        for eval in all_evals.iter().take(3) {
            eprintln!(
                "  {} {} {}.{} | {} | inst={:?} course={:?}",
                eval.term,
                eval.crn,
                eval.subject,
                eval.course_number,
                eval.instructor_name,
                eval.instructor_rating,
                eval.course_rating
            );
        }
    }

    /// Verify that BAN (no current-term results) is detected early via missing radio.
    #[tokio::test]
    #[ignore = "requires network access to bluebook.utsa.edu"]
    async fn test_live_ban_no_term_filter() {
        init_tracing();

        let client = BlueBookClient::new();
        let (subjects, initial_fields) = client.fetch_subjects().await.unwrap();
        let ban = subjects
            .iter()
            .find(|s| s.code == "BAN")
            .expect("BAN subject must exist");

        let (_html, fields) = client.search_subject(ban, &initial_fields).await.unwrap();
        assert!(
            !fields.has(TERM_FILTER_RADIO),
            "BAN search should NOT have term filter radio (no current-term results)"
        );
        eprintln!("BAN correctly detected as having no term filter");
    }

    /// Test the full scrape_all flow to see how many evaluations are collected.
    /// Requires a running PostgreSQL database.
    #[tokio::test]
    #[ignore = "requires network access to bluebook.utsa.edu and database; runs full scrape"]
    async fn test_live_scrape_all() {
        init_tracing();

        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for this test");
        let db_pool = sqlx::PgPool::connect(&database_url).await.unwrap();

        let client = BlueBookClient::new();
        let total = client.scrape_all(&db_pool, false).await.unwrap();
        eprintln!("Total evaluations upserted: {total} (0 means all subjects failed)",);
        assert!(
            total > 0,
            "Should collect at least some evaluations from the live site"
        );
    }
}
