//! BlueBook (bluebook.utsa.edu) course evaluation scraper.
//!
//! BlueBook is an ASP.NET WebForms application that requires stateful
//! ViewState/EventValidation round-tripping and cookie-based sessions.

use anyhow::{Context, Result};
use html_scraper::{Html, Selector};
use std::sync::LazyLock;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::banner::models::terms::{Season, Term};
use crate::data::bluebook::BlueBookEvaluation;

#[allow(dead_code)]
const BASE_URL: &str = "https://bluebook.utsa.edu/Default.aspx";

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
pub fn normalize_term(bluebook_term: &str) -> Option<Term> {
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

/// Client for scraping BlueBook course evaluations.
#[allow(dead_code)]
pub struct BlueBookClient {
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
    pub fn new() -> Self {
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

            // Skip buttons and image inputs — they're only sent when clicked
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

        let event_target =
            format!("ctl00$MainContent$mainContent1$CourseTermSelectRBL${radio_index}");
        let params = Self::build_postback(
            fields,
            &event_target,
            &[("ctl00$MainContent$mainContent1$CourseTermSelectRBL", filter)],
        );

        let resp = self
            .http
            .post(BASE_URL)
            .form(&params)
            .send()
            .await
            .context("Failed to switch BlueBook term filter")?;

        let body = resp.text().await?;
        let html = Html::parse_document(&body);
        let new_fields = Self::extract_form_fields(&html)?;

        Ok((html, new_fields))
    }

    /// POST to navigate to the next page of results.
    async fn next_page(&self, fields: &FormFields, top: bool) -> Result<(Html, FormFields)> {
        tokio::time::sleep(self.delay).await;

        let suffix = if top { "TOP" } else { "BOTTOM" };
        // Page navigation uses image buttons — the event target includes coordinates
        let event_target = format!("ctl00$MainContent$mainContent1$PagerImgBtn_Next{suffix}");
        let params = Self::build_postback(fields, &event_target, &[]);

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

            // Course.Section: "CS 1083.001" — extract course_number and section
            let course_section = cells[2].trim();
            let (course_number, section) = match Self::parse_course_section(course_section, subject)
            {
                Some(pair) => pair,
                None => {
                    warn!(
                        raw = course_section,
                        "Failed to parse course.section from header"
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
    /// Returns `None` if the subject prefix doesn't match or the course number
    /// doesn't start with a digit (rejects garbage like "ISA 1234" when subject is "IS").
    fn parse_course_section(raw: &str, subject: &str) -> Option<(String, String)> {
        // Strip "{subject} " prefix. We require the space to avoid false matches
        // when one subject code is a prefix of another (e.g. "IS" vs "ISA").
        let with_space = format!("{subject} ");
        let without_prefix = raw.strip_prefix(&with_space)?.trim();

        let (course_number, section) = without_prefix.split_once('.')?;
        let course_number = course_number.trim();
        let section = section.trim();

        // Course numbers must start with a digit (e.g. "1083", "3343")
        if !course_number.starts_with(|c: char| c.is_ascii_digit()) {
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

    /// Scrape all subjects and return collected evaluation records.
    ///
    /// Searches each subject with the PAST term filter and paginates through all pages.
    pub async fn scrape_all(&self) -> Result<Vec<BlueBookEvaluation>> {
        let (subjects, initial_fields) = self.fetch_subjects().await?;
        let mut all_evals = Vec::new();

        for subject in &subjects {
            info!(
                code = subject.code.as_str(),
                "Scraping BlueBook evaluations"
            );

            // Search for the subject
            let (_html, fields) = match self.search_subject(subject, &initial_fields).await {
                Ok(result) => result,
                Err(e) => {
                    warn!(code = subject.code.as_str(), error = %e, "Failed to search subject, skipping");
                    continue;
                }
            };

            // Switch to PAST courses to get completed evaluations
            let (html, mut fields) = match self.switch_term_filter("PAST", &fields).await {
                Ok(result) => result,
                Err(e) => {
                    warn!(code = subject.code.as_str(), error = %e, "Failed to switch to PAST filter, skipping");
                    continue;
                }
            };

            // Parse first page
            let page_evals = Self::parse_evaluations(&html, &subject.code);
            all_evals.extend(page_evals);

            // Paginate through remaining pages
            let total_pages = Self::parse_page_info(&html)
                .map(|(_, total)| total)
                .unwrap_or(1);

            for page in 2..=total_pages {
                debug!(
                    code = subject.code.as_str(),
                    page, total_pages, "Fetching page"
                );

                match self.next_page(&fields, true).await {
                    Ok((page_html, new_fields)) => {
                        fields = new_fields;
                        let page_evals = Self::parse_evaluations(&page_html, &subject.code);
                        all_evals.extend(page_evals);
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

            debug!(
                code = subject.code.as_str(),
                total = all_evals.len(),
                "Finished subject"
            );
        }

        info!(total = all_evals.len(), "BlueBook scrape complete");
        Ok(all_evals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // --- BlueBookSeason ---

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

    // --- normalize_term ---

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

    // --- parse_rating_cell ---

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

    // --- parse_course_section ---

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
    fn test_parse_course_section_mismatched_subject_rejected() {
        // "ISA 1234.001" doesn't match subject "IS" — prefix "IS " doesn't
        // match "ISA ", so this correctly returns None rather than producing
        // garbage like ("ISA 1234", "001").
        assert_eq!(
            BlueBookClient::parse_course_section("ISA 1234.001", "IS"),
            None
        );
    }

    #[test]
    fn test_parse_course_section_exact_subject_match() {
        assert_eq!(
            BlueBookClient::parse_course_section("IS 1234.001", "IS"),
            Some(("1234".to_string(), "001".to_string()))
        );
    }

    // --- extract_form_fields ---

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

    // --- build_postback ---

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

    // --- parse_page_info ---

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

    // --- parse_subjects ---

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

    // --- parse_evaluations (synthetic HTML) ---

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

        // Second entry has no ratings → skipped
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

    // --- parse_department ---

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

    // --- parse_evaluations_from_dump (requires manual data generation) ---

    /// Parse the previously dumped page 7 to validate the parser extracts real data.
    /// Requires running `dump_past_evaluations` first to generate the fixture.
    #[test]
    #[ignore]
    fn test_parse_evaluations_from_dump() {
        let path = std::path::Path::new("target/bluebook_dump/search_CS_past_page7.html");
        let body = std::fs::read_to_string(path).expect("Run dump_past_evaluations first");
        let html = Html::parse_document(&body);
        let evals = BlueBookClient::parse_evaluations(&html, "CS");

        println!("Parsed {} evaluations from page 7:", evals.len());
        for eval in &evals {
            println!(
                "  {} {}.{} {} (CRN {}) by {} — inst:{:?} ({:?} resp), course:{:?} ({:?} resp)",
                eval.term,
                eval.course_number,
                eval.section,
                eval.subject,
                eval.crn,
                eval.instructor_name,
                eval.instructor_rating,
                eval.instructor_response_count,
                eval.course_rating,
                eval.course_response_count,
            );
        }

        assert!(!evals.is_empty(), "Should parse at least one evaluation");

        // Verify a known entry from the test output: CS 1083.01T, Sum 2025, instructor rating 3.9
        let cs1083 = evals
            .iter()
            .find(|e| e.course_number == "1083" && e.section == "01T");
        assert!(cs1083.is_some(), "Should find CS 1083.01T");
        let cs1083 = cs1083.unwrap();
        assert_eq!(cs1083.instructor_rating, Some(3.9));
        assert_eq!(cs1083.instructor_response_count, Some(17));
        assert_eq!(cs1083.course_rating, Some(3.9));
        assert_eq!(cs1083.course_response_count, Some(17));
        assert_eq!(
            cs1083.department.as_deref(),
            Some("Department of Computer Science"),
            "Department should be extracted from the detail pane"
        );

        // All CS evaluations should have the same department
        for eval in &evals {
            assert_eq!(
                eval.department.as_deref(),
                Some("Department of Computer Science"),
                "All CS evals should have department, but {} {} is missing it",
                eval.course_number,
                eval.section
            );
        }
    }

    /// Fetch the BlueBook landing page and dump raw HTML to target/bluebook_dump/.
    /// Run with: cargo test -p banner bluebook::tests::dump_landing_page -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn dump_landing_page() {
        let client = BlueBookClient::new();
        let (subjects, fields) = client
            .fetch_subjects()
            .await
            .expect("Failed to fetch subjects");

        let dump_dir = std::path::Path::new("target/bluebook_dump");
        std::fs::create_dir_all(dump_dir).expect("Failed to create dump dir");

        println!("Extracted {} form fields", fields.0.len());
        for (name, value) in &fields.0 {
            let display_val = if value.len() > 60 {
                format!("{}... ({} chars)", &value[..60], value.len())
            } else {
                value.clone()
            };
            println!("  {name} = {display_val}");
        }

        println!(
            "\nFound {} subjects (first 20): {:?}",
            subjects.len(),
            &subjects[..subjects.len().min(20)]
        );
    }

    /// Search CS with PAST filter and expand first accordion pane to see rating data.
    /// Run with: cargo test -p banner bluebook::tests::dump_past_evaluations -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn dump_past_evaluations() {
        let client = BlueBookClient::new();
        let (subjects, fields) = client
            .fetch_subjects()
            .await
            .expect("Failed to fetch subjects");

        let dump_dir = std::path::Path::new("target/bluebook_dump");
        std::fs::create_dir_all(dump_dir).expect("Failed to create dump dir");

        let subject = subjects
            .iter()
            .find(|s| s.code == "CS")
            .expect("CS not found");
        println!("Searching for {}", subject.code);

        // Search for current first (to get results page)
        let (_html, fields) = client
            .search_subject(subject, &fields)
            .await
            .expect("Failed to search subject");

        // Switch to PAST
        println!("Switching to PAST courses...");
        let (html, fields) = client
            .switch_term_filter("PAST", &fields)
            .await
            .expect("Failed to switch to PAST");

        let body = html.html();
        let path = dump_dir.join("search_CS_past.html");
        std::fs::write(&path, &body).expect("Failed to write HTML");
        println!(
            "Wrote PAST results to {} ({} bytes)",
            path.display(),
            body.len()
        );

        // Check page count
        let page_re = regex::Regex::new(r"(\d+) of (\d+)").unwrap();
        if let Some(caps) = page_re.captures(&body) {
            println!("Page {} of {}", &caps[1], &caps[2]);
        }

        // Show first few accordion headers
        let header_sel = Selector::parse("table.infoTable").unwrap();
        let td_sel = Selector::parse("td").unwrap();

        for (i, table) in html.select(&header_sel).enumerate().take(5) {
            let cells: Vec<String> = table
                .select(&td_sel)
                .map(|td| td.text().collect::<String>().trim().to_string())
                .collect();
            println!("Pane {i}: {cells:?}");
        }

        // Check if any pane already has evaluation data visible
        let eval_sel = Selector::parse("div.accordionDetailPane").unwrap();
        for (i, pane) in html.select(&eval_sel).enumerate().take(3) {
            let text = pane.text().collect::<String>();
            let text = text.trim();
            if !text.is_empty() {
                println!("\nDetail pane {i} text (first 500 chars):");
                println!("{}", &text[..text.len().min(500)]);
            }
        }

        // Page forward until we find entries with actual evaluation data (not "n/a")
        let inst_eval_sel = Selector::parse("span[id*='InstEval']").unwrap();
        let mut current_fields = fields;
        let mut found_evals = false;

        for page in 2..=10 {
            println!("\nNavigating to page {page}...");
            let (page_html, new_fields) = client
                .next_page(&current_fields, true)
                .await
                .expect("Failed to navigate page");
            current_fields = new_fields;

            // Check if any InstEval span has content other than "n/a" or a future date
            for eval_span in page_html.select(&inst_eval_sel) {
                let text = eval_span.text().collect::<String>();
                let text = text.trim();
                if !text.is_empty() && text != "n/a" {
                    found_evals = true;
                    let id = eval_span.attr("id").unwrap_or("?");
                    println!("  Found non-n/a evaluation: id={id} text='{text}'");
                }
            }

            if found_evals {
                // Dump this page with actual evaluations
                let body = page_html.html();
                let path = dump_dir.join(format!("search_CS_past_page{page}.html"));
                std::fs::write(&path, &body).expect("Failed to write HTML");
                println!(
                    "Wrote page {page} to {} ({} bytes)",
                    path.display(),
                    body.len()
                );

                // Show the header rows on this page
                for (i, table) in page_html.select(&header_sel).enumerate().take(10) {
                    let cells: Vec<String> = table
                        .select(&td_sel)
                        .map(|td| td.text().collect::<String>().trim().to_string())
                        .collect();
                    println!("  Pane {i}: {cells:?}");
                }

                // Check what's in the detail panes on this page
                let detail_sel = Selector::parse("div.accordionDetailPane").unwrap();
                for (i, pane) in page_html.select(&detail_sel).enumerate().take(3) {
                    let inner = pane.inner_html();
                    if inner.contains("survey")
                        || inner.contains("Survey")
                        || inner.contains("rating")
                        || inner.contains("Rating")
                    {
                        println!(
                            "\nDetail pane {i} contains survey/rating content (first 2000 chars):"
                        );
                        println!("{}", &inner[..inner.len().min(2000)]);
                    }
                }

                break;
            }

            // Check what semester we're on now
            for table in page_html.select(&header_sel).take(1) {
                let cells: Vec<String> = table
                    .select(&td_sel)
                    .map(|td| td.text().collect::<String>().trim().to_string())
                    .collect();
                println!("  First row on page {page}: {cells:?}");
            }
        }

        if !found_evals {
            println!("\nDid not find completed evaluations in first 10 pages");
        }
    }

    /// Fetch subjects, then search one subject and dump the results page.
    /// Run with: cargo test -p banner bluebook::tests::dump_subject_search -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn dump_subject_search() {
        let client = BlueBookClient::new();
        let (subjects, fields) = client
            .fetch_subjects()
            .await
            .expect("Failed to fetch subjects");

        println!(
            "Found {} subjects: {:?}",
            subjects.len(),
            &subjects[..subjects.len().min(20)]
        );

        let dump_dir = std::path::Path::new("target/bluebook_dump");
        std::fs::create_dir_all(dump_dir).expect("Failed to create dump dir");

        // Search for CS (or first available subject)
        let subject = subjects
            .iter()
            .find(|s| s.code == "CS")
            .unwrap_or_else(|| subjects.first().expect("No subjects found"));
        println!(
            "\nSearching for subject: {} ({}) [index={}]",
            subject.code, subject.display_text, subject.combo_index
        );

        let (html, _new_fields) = client
            .search_subject(subject, &fields)
            .await
            .expect("Failed to search subject");

        let body = html.html();
        let path = dump_dir.join(format!("search_{}.html", subject.code));
        std::fs::write(&path, &body).expect("Failed to write HTML");
        println!("Wrote search results to {}", path.display());
        println!("Body length: {} bytes", body.len());

        // Inspect result structure
        let table_sel = Selector::parse("table").unwrap();
        let tr_sel = Selector::parse("tr").unwrap();
        let td_sel = Selector::parse("td").unwrap();
        let th_sel = Selector::parse("th").unwrap();

        for table in html.select(&table_sel) {
            let id = table.attr("id").unwrap_or("(no id)");
            let class = table.attr("class").unwrap_or("(no class)");
            let rows: Vec<_> = table.select(&tr_sel).collect();
            println!("\nTable id={id} class={class} rows={}", rows.len());

            // Print headers if present
            if let Some(first_row) = rows.first() {
                let headers: Vec<String> = first_row
                    .select(&th_sel)
                    .map(|th| th.text().collect::<String>().trim().to_string())
                    .collect();
                if !headers.is_empty() {
                    println!("  Headers: {headers:?}");
                }
            }

            // Print first few data rows
            for (i, row) in rows.iter().take(5).enumerate() {
                let cells: Vec<String> = row
                    .select(&td_sel)
                    .map(|td| {
                        let text = td.text().collect::<String>().trim().to_string();
                        if text.len() > 80 {
                            format!("{}...", &text[..80])
                        } else {
                            text
                        }
                    })
                    .collect();
                if !cells.is_empty() {
                    println!("  Row {i}: {cells:?}");
                }
            }
        }

        // Also look for accordion/panel structures
        for sel_str in [
            "div.accordion",
            "div[id*='Accordion']",
            "div[id*='Panel']",
            "div[id*='pane']",
        ] {
            if let Ok(sel) = Selector::parse(sel_str) {
                let count = html.select(&sel).count();
                if count > 0 {
                    println!("\nFound {count} elements matching '{sel_str}'");
                }
            }
        }

        // Try parsing evaluations with current logic
        let evals = BlueBookClient::parse_evaluations(&html, &subject.code);
        println!(
            "\nParsed {len} evaluations with current logic",
            len = evals.len()
        );
        for eval in evals.iter().take(3) {
            println!("  {eval:?}");
        }
    }
}
