use serde::{Deserialize, Serialize};

use super::meetings::FacultyItem;
use super::meetings::MeetingTimeResponse;

/// Course section attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionAttribute {
    pub class: String,
    pub course_reference_number: String,
    pub code: String,
    pub description: String,
    pub term_code: String,
    #[serde(rename = "isZTCAttribute")]
    pub is_ztc_attribute: bool,
}

/// Represents a single course returned from a search
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub id: i32,
    pub term: String,
    pub term_desc: String,
    pub course_reference_number: String,
    pub part_of_term: String,
    pub course_number: String,
    pub subject: String,
    pub subject_description: String,
    pub sequence_number: String,
    pub campus_description: String,
    pub schedule_type_description: Option<String>,
    pub course_title: String,
    pub credit_hours: Option<f64>,
    pub maximum_enrollment: i32,
    pub enrollment: i32,
    pub seats_available: i32,
    pub wait_capacity: Option<i32>, // null for older/archived courses
    pub wait_count: Option<i32>,    // null for older/archived courses
    pub cross_list: Option<String>,
    pub cross_list_capacity: Option<i32>,
    pub cross_list_count: Option<i32>,
    pub cross_list_available: Option<i32>,
    pub credit_hour_high: Option<f64>,
    pub credit_hour_low: Option<f64>,
    pub credit_hour_indicator: Option<String>,
    pub open_section: bool,
    pub link_identifier: Option<String>,
    pub is_section_linked: bool,
    pub subject_course: String,
    pub reserved_seat_summary: Option<String>,
    pub instructional_method: Option<String>,
    pub instructional_method_description: Option<String>,
    pub section_attributes: Vec<SectionAttribute>,
    #[serde(default)]
    pub faculty: Vec<FacultyItem>,
    #[serde(default)]
    pub meetings_faculty: Vec<MeetingTimeResponse>,
}

impl Course {
    /// Returns the course title in the format "SUBJ #### - Course Title"
    pub fn display_title(&self) -> String {
        format!(
            "{} {} - {}",
            self.subject, self.course_number, self.course_title
        )
    }

    /// Returns the name of the primary instructor, or "Unknown" if not available
    pub fn primary_instructor_name(&self) -> &str {
        self.faculty
            .first()
            .map(|f| f.display_name.as_str())
            .unwrap_or("Unknown")
    }
}
