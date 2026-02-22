//! Query builder for Banner API course searches.

use std::collections::HashMap;

use chrono::{NaiveTime, Timelike};

/// Builder for constructing Banner API search queries.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    subject: Option<String>,
    title: Option<String>,
    keywords: Option<Vec<String>>,
    course_reference_number: Option<String>,
    open_only: Option<bool>,
    term_part: Option<Vec<String>>,
    campus: Option<Vec<String>>,
    instructional_method: Option<Vec<String>>,
    attributes: Option<Vec<String>>,
    instructor: Option<Vec<u64>>,
    start_time: Option<NaiveTime>,
    end_time: Option<NaiveTime>,
    min_credits: Option<i32>,
    max_credits: Option<i32>,
    offset: i32,
    max_results: i32,
    course_number_low: Option<i32>,
    course_number_high: Option<i32>,
}

#[allow(dead_code)]
impl SearchQuery {
    /// Creates a new SearchQuery with default values
    pub fn new() -> Self {
        Self {
            max_results: 8,
            offset: 0,
            ..Default::default()
        }
    }

    /// Sets the subject for the query
    pub fn subject<S: Into<String>>(mut self, subject: S) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Sets the title for the query
    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the course reference number (CRN) for the query
    pub fn course_reference_number<S: Into<String>>(mut self, crn: S) -> Self {
        self.course_reference_number = Some(crn.into());
        self
    }

    /// Sets the keywords for the query
    pub fn keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = Some(keywords);
        self
    }

    /// Adds a keyword to the query
    pub fn keyword<S: Into<String>>(mut self, keyword: S) -> Self {
        match &mut self.keywords {
            Some(keywords) => keywords.push(keyword.into()),
            None => self.keywords = Some(vec![keyword.into()]),
        }
        self
    }

    /// Sets whether to search for open courses only
    pub fn open_only(mut self, open_only: bool) -> Self {
        self.open_only = Some(open_only);
        self
    }

    /// Sets the term part for the query
    pub fn term_part(mut self, term_part: Vec<String>) -> Self {
        self.term_part = Some(term_part);
        self
    }

    /// Sets the campuses for the query
    pub fn campus(mut self, campus: Vec<String>) -> Self {
        self.campus = Some(campus);
        self
    }

    /// Sets the instructional methods for the query
    pub fn instructional_method(mut self, instructional_method: Vec<String>) -> Self {
        self.instructional_method = Some(instructional_method);
        self
    }

    /// Sets the attributes for the query
    pub fn attributes(mut self, attributes: Vec<String>) -> Self {
        self.attributes = Some(attributes);
        self
    }

    /// Sets the instructors for the query
    pub fn instructor(mut self, instructor: Vec<u64>) -> Self {
        self.instructor = Some(instructor);
        self
    }

    /// Sets the start time for the query
    pub fn start_time(mut self, start_time: NaiveTime) -> Self {
        self.start_time = Some(start_time);
        self
    }

    /// Sets the end time for the query
    pub fn end_time(mut self, end_time: NaiveTime) -> Self {
        self.end_time = Some(end_time);
        self
    }

    /// Sets the credit range for the query
    pub fn credits(mut self, low: i32, high: i32) -> Self {
        self.min_credits = Some(low);
        self.max_credits = Some(high);
        self
    }

    /// Sets the minimum credits for the query
    pub fn min_credits(mut self, value: i32) -> Self {
        self.min_credits = Some(value);
        self
    }

    /// Sets the maximum credits for the query
    pub fn max_credits(mut self, value: i32) -> Self {
        self.max_credits = Some(value);
        self
    }

    /// Sets the course number range for the query
    pub fn course_numbers(mut self, low: i32, high: i32) -> Self {
        self.course_number_low = Some(low);
        self.course_number_high = Some(high);
        self
    }

    /// Sets the offset for pagination
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = offset;
        self
    }

    /// Sets the maximum number of results to return
    /// Clamped to a maximum of 500 to prevent excessive API load
    pub fn max_results(mut self, max_results: i32) -> Self {
        self.max_results = max_results.clamp(1, 500);
        self
    }

    /// Gets the subject field
    pub fn get_subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    /// Gets the max_results field
    pub fn get_max_results(&self) -> i32 {
        self.max_results
    }

    /// Converts the query into URL parameters for the Banner API
    pub fn to_params(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();

        for field in self.fields() {
            match field {
                QueryField::Single {
                    param_key, value, ..
                } => {
                    params.insert(param_key.to_string(), value);
                }
                QueryField::Time { prefix, time, .. } => {
                    let (hour, minute, meridiem) = format_time_parameter(time);
                    params.insert(format!("select_{prefix}_hour"), hour);
                    params.insert(format!("select_{prefix}_min"), minute);
                    params.insert(format!("select_{prefix}_ampm"), meridiem);
                }
            }
        }

        params
    }

    /// Returns the list of active query fields for serialization.
    ///
    /// Both `to_params()` and `Display` consume this, so adding a field
    /// once here covers both serializations.
    fn fields(&self) -> Vec<QueryField> {
        let mut fields = Vec::new();

        if let Some(ref subject) = self.subject {
            fields.push(QueryField::single(
                "txt_subject",
                "subject",
                subject.clone(),
            ));
        }
        if let Some(ref title) = self.title {
            fields.push(QueryField::single(
                "txt_courseTitle",
                "title",
                title.trim().to_string(),
            ));
        }
        if let Some(ref crn) = self.course_reference_number {
            fields.push(QueryField::single(
                "txt_courseReferenceNumber",
                "crn",
                crn.clone(),
            ));
        }
        if let Some(ref keywords) = self.keywords {
            fields.push(QueryField::single(
                "txt_keywordlike",
                "keywords",
                keywords.join(" "),
            ));
        }
        if self.open_only == Some(true) {
            fields.push(QueryField::single(
                "chk_open_only",
                "openOnly",
                "true".to_string(),
            ));
        }
        if let Some(ref term_part) = self.term_part {
            fields.push(QueryField::single(
                "txt_partOfTerm",
                "termPart",
                term_part.join(","),
            ));
        }
        if let Some(ref campus) = self.campus {
            fields.push(QueryField::single("txt_campus", "campus", campus.join(",")));
        }
        if let Some(ref instructional_method) = self.instructional_method {
            fields.push(QueryField::single(
                "txt_instructionalMethod",
                "instructionalMethod",
                instructional_method.join(","),
            ));
        }
        if let Some(ref attributes) = self.attributes {
            fields.push(QueryField::single(
                "txt_attribute",
                "attributes",
                attributes.join(","),
            ));
        }
        if let Some(ref instructor) = self.instructor {
            let value = instructor
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            fields.push(QueryField::single("txt_instructor", "instructor", value));
        }
        if let Some(start_time) = self.start_time {
            fields.push(QueryField::Time {
                prefix: "start",
                display_name: "startTime",
                time: start_time,
            });
        }
        if let Some(end_time) = self.end_time {
            fields.push(QueryField::Time {
                prefix: "end",
                display_name: "endTime",
                time: end_time,
            });
        }
        if let Some(min_credits) = self.min_credits {
            fields.push(QueryField::single(
                "txt_credithourlow",
                "minCredits",
                min_credits.to_string(),
            ));
        }
        if let Some(max_credits) = self.max_credits {
            fields.push(QueryField::single(
                "txt_credithourhigh",
                "maxCredits",
                max_credits.to_string(),
            ));
        }
        if let Some(low) = self.course_number_low {
            fields.push(QueryField::single(
                "txt_course_number_range",
                "courseNumberLow",
                low.to_string(),
            ));
        }
        if let Some(high) = self.course_number_high {
            fields.push(QueryField::single(
                "txt_course_number_range_to",
                "courseNumberHigh",
                high.to_string(),
            ));
        }
        fields.push(QueryField::single(
            "pageOffset",
            "offset",
            self.offset.to_string(),
        ));
        fields.push(QueryField::single(
            "pageMaxSize",
            "maxResults",
            self.max_results.to_string(),
        ));

        fields
    }
}

/// A single field in a search query, used to unify `to_params()` and `Display`.
enum QueryField {
    /// A simple key-value field.
    Single {
        param_key: &'static str,
        display_name: &'static str,
        value: String,
    },
    /// A time field that expands to three params (hour, min, ampm).
    Time {
        prefix: &'static str,
        display_name: &'static str,
        time: NaiveTime,
    },
}

impl QueryField {
    fn single(param_key: &'static str, display_name: &'static str, value: String) -> Self {
        Self::Single {
            param_key,
            display_name,
            value,
        }
    }
}

/// Formats a NaiveTime into hour, minute, and meridiem strings for Banner API.
///
/// Uses 12-hour format: midnight = 12:00 AM, noon = 12:00 PM.
fn format_time_parameter(time: NaiveTime) -> (String, String, String) {
    let hours = time.hour();
    let minutes = time.minute();

    let meridiem = if hours >= 12 { "PM" } else { "AM" };
    let hour_12 = match hours % 12 {
        0 => 12,
        h => h,
    };

    (
        hour_12.to_string(),
        minutes.to_string(),
        meridiem.to_string(),
    )
}

impl std::fmt::Display for SearchQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self
            .fields()
            .into_iter()
            .map(|field| match field {
                QueryField::Single {
                    display_name,
                    value,
                    ..
                } => format!("{display_name}={value}"),
                QueryField::Time {
                    display_name, time, ..
                } => {
                    let (hour, minute, meridiem) = format_time_parameter(time);
                    format!("{display_name}={hour}:{minute}:{meridiem}")
                }
            })
            .collect();

        write!(f, "{}", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let q = SearchQuery::new();
        assert_eq!(q.get_max_results(), 8);
        assert!(q.get_subject().is_none());
        let params = q.to_params();
        assert_eq!(params.get("pageMaxSize").unwrap(), "8");
        assert_eq!(params.get("pageOffset").unwrap(), "0");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_subject_param() {
        let params = SearchQuery::new().subject("CS").to_params();
        assert_eq!(params.get("txt_subject").unwrap(), "CS");
    }

    #[test]
    fn test_title_trims_whitespace() {
        let params = SearchQuery::new().title("  Intro to CS  ").to_params();
        assert_eq!(params.get("txt_courseTitle").unwrap(), "Intro to CS");
    }

    #[test]
    fn test_crn_param() {
        let params = SearchQuery::new()
            .course_reference_number("12345")
            .to_params();
        assert_eq!(params.get("txt_courseReferenceNumber").unwrap(), "12345");
    }

    #[test]
    fn test_keywords_joined_with_spaces() {
        let params = SearchQuery::new()
            .keyword("data")
            .keyword("science")
            .to_params();
        assert_eq!(params.get("txt_keywordlike").unwrap(), "data science");
    }

    #[test]
    fn test_keywords_vec() {
        let params = SearchQuery::new()
            .keywords(vec!["machine".into(), "learning".into()])
            .to_params();
        assert_eq!(params.get("txt_keywordlike").unwrap(), "machine learning");
    }

    #[test]
    fn test_open_only() {
        let params = SearchQuery::new().open_only(true).to_params();
        assert_eq!(params.get("chk_open_only").unwrap(), "true");

        // open_only(false) should NOT set the param
        let params2 = SearchQuery::new().open_only(false).to_params();
        assert!(!params2.contains_key("chk_open_only"));
    }

    #[test]
    fn test_credits_range() {
        let params = SearchQuery::new().credits(3, 6).to_params();
        assert_eq!(params.get("txt_credithourlow").unwrap(), "3");
        assert_eq!(params.get("txt_credithourhigh").unwrap(), "6");
    }

    #[test]
    fn test_course_number_range() {
        let params = SearchQuery::new().course_numbers(3000, 3999).to_params();
        assert_eq!(params.get("txt_course_number_range").unwrap(), "3000");
        assert_eq!(params.get("txt_course_number_range_to").unwrap(), "3999");
    }

    #[test]
    fn test_pagination() {
        let params = SearchQuery::new().offset(20).max_results(10).to_params();
        assert_eq!(params.get("pageOffset").unwrap(), "20");
        assert_eq!(params.get("pageMaxSize").unwrap(), "10");
    }

    #[test]
    fn test_format_time_9am() {
        let (h, m, mer) = format_time_parameter(NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        assert_eq!(h, "9");
        assert_eq!(m, "0");
        assert_eq!(mer, "AM");
    }

    #[test]
    fn test_format_time_noon() {
        let (h, m, mer) = format_time_parameter(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        assert_eq!(h, "12");
        assert_eq!(m, "0");
        assert_eq!(mer, "PM");
    }

    #[test]
    fn test_format_time_1pm() {
        let (h, m, mer) = format_time_parameter(NaiveTime::from_hms_opt(13, 0, 0).unwrap());
        assert_eq!(h, "1");
        assert_eq!(m, "0");
        assert_eq!(mer, "PM");
    }

    #[test]
    fn test_format_time_930am() {
        let (h, m, mer) = format_time_parameter(NaiveTime::from_hms_opt(9, 30, 0).unwrap());
        assert_eq!(h, "9");
        assert_eq!(m, "30");
        assert_eq!(mer, "AM");
    }

    #[test]
    fn test_format_time_midnight() {
        let (h, m, mer) = format_time_parameter(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        assert_eq!(h, "12");
        assert_eq!(m, "0");
        assert_eq!(mer, "AM");
    }

    #[test]
    fn test_time_params_in_query() {
        let params = SearchQuery::new()
            .start_time(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
            .end_time(NaiveTime::from_hms_opt(17, 0, 0).unwrap())
            .to_params();
        assert_eq!(params.get("select_start_hour").unwrap(), "9");
        assert_eq!(params.get("select_start_ampm").unwrap(), "AM");
        assert_eq!(params.get("select_end_hour").unwrap(), "5");
        assert_eq!(params.get("select_end_ampm").unwrap(), "PM");
    }

    #[test]
    fn test_multi_value_params() {
        let params = SearchQuery::new()
            .campus(vec!["MAIN".into(), "DT".into()])
            .attributes(vec!["HONORS".into()])
            .instructor(vec![1001, 1002])
            .to_params();
        assert_eq!(params.get("txt_campus").unwrap(), "MAIN,DT");
        assert_eq!(params.get("txt_attribute").unwrap(), "HONORS");
        assert_eq!(params.get("txt_instructor").unwrap(), "1001,1002");
    }

    #[test]
    fn test_display_minimal() {
        let display = SearchQuery::new().to_string();
        assert_eq!(display, "offset=0, maxResults=8");
    }

    #[test]
    fn test_display_with_fields() {
        let display = SearchQuery::new()
            .subject("CS")
            .open_only(true)
            .max_results(10)
            .to_string();
        assert!(display.contains("subject=CS"));
        assert!(display.contains("openOnly=true"));
        assert!(display.contains("maxResults=10"));
    }

    #[test]
    fn test_instructional_method_param() {
        let params = SearchQuery::new()
            .instructional_method(vec!["ONLINE".into(), "HYBRID".into()])
            .to_params();
        assert_eq!(
            params.get("txt_instructionalMethod").unwrap(),
            "ONLINE,HYBRID"
        );
    }

    #[test]
    fn test_instructional_method_display() {
        let display = SearchQuery::new()
            .instructional_method(vec!["ONLINE".into()])
            .to_string();
        assert!(display.contains("instructionalMethod=ONLINE"));
    }

    #[test]
    fn test_crn_display() {
        let display = SearchQuery::new()
            .course_reference_number("12345")
            .to_string();
        assert!(display.contains("crn=12345"));
    }

    #[test]
    fn test_full_query_param_count() {
        let params = SearchQuery::new()
            .subject("CS")
            .title("Intro")
            .course_reference_number("12345")
            .keyword("programming")
            .open_only(true)
            .instructional_method(vec!["ONLINE".into()])
            .credits(3, 4)
            .course_numbers(1000, 1999)
            .offset(0)
            .max_results(25)
            .to_params();
        // subject, title, crn, keyword, open_only, instructional_method,
        // min_credits, max_credits, course_number_range, course_number_range_to,
        // pageOffset, pageMaxSize = 12
        assert_eq!(params.len(), 12);
    }
}
