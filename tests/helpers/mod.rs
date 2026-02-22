#![allow(dead_code)]

use banner::banner::Course;
use banner::banner::models::Term;
use banner::banner::models::meetings::{MeetingTime, MeetingTimeResponse};
use banner::data::models::{ScrapePriority, TargetType};
use chrono::Utc;
use sqlx::PgPool;
use std::str::FromStr;

/// Build a test `Course` (Banner API model) with sensible defaults.
///
/// Only the fields used by `batch_upsert_courses` need meaningful values;
/// the rest are filled with harmless placeholders.
pub fn make_course(
    crn: &str,
    term: &str,
    subject: &str,
    course_number: &str,
    title: &str,
    (enrollment, max_enrollment, wait_count, wait_capacity): (i32, i32, i32, i32),
) -> Course {
    Course {
        id: 0,
        term: term.to_owned(),
        term_desc: String::new(),
        course_reference_number: crn.to_owned(),
        part_of_term: "1".to_owned(),
        course_number: course_number.to_owned(),
        subject: subject.to_owned(),
        subject_description: subject.to_owned(),
        sequence_number: "001".to_owned(),
        campus_description: "Main Campus".to_owned(),
        schedule_type_description: Some("Lecture".to_owned()),
        course_title: title.to_owned(),
        credit_hours: Some(3),
        maximum_enrollment: max_enrollment,
        enrollment,
        seats_available: max_enrollment - enrollment,
        wait_capacity: Some(wait_capacity),
        wait_count: Some(wait_count),
        cross_list: None,
        cross_list_capacity: None,
        cross_list_count: None,
        cross_list_available: None,
        credit_hour_high: None,
        credit_hour_low: None,
        credit_hour_indicator: None,
        open_section: enrollment < max_enrollment,
        link_identifier: None,
        is_section_linked: false,
        subject_course: format!("{subject}{course_number}"),
        reserved_seat_summary: None,
        instructional_method: Some("FF".to_owned()),
        instructional_method_description: Some("Face to Face".to_owned()),
        section_attributes: vec![],
        faculty: vec![],
        meetings_faculty: vec![],
    }
}

/// Builder for constructing `MeetingTimeResponse` objects with sensible defaults.
///
/// Produces meeting times suitable for `batch_upsert_courses` tests without
/// requiring callers to fill in every field on the Banner API model.
pub struct MeetingTimeBuilder {
    term: String,
    crn: String,
    monday: bool,
    tuesday: bool,
    wednesday: bool,
    thursday: bool,
    friday: bool,
    saturday: bool,
    sunday: bool,
    begin_time: Option<String>,
    end_time: Option<String>,
    building: Option<String>,
    building_description: Option<String>,
    room: Option<String>,
    start_date: String,
    end_date: String,
    meeting_type: String,
    meeting_schedule_type: String,
}

impl Default for MeetingTimeBuilder {
    fn default() -> Self {
        Self {
            term: "202620".to_owned(),
            crn: "00000".to_owned(),
            monday: false,
            tuesday: false,
            wednesday: false,
            thursday: false,
            friday: false,
            saturday: false,
            sunday: false,
            begin_time: None,
            end_time: None,
            building: None,
            building_description: None,
            room: None,
            start_date: "01/20/2026".to_owned(),
            end_date: "05/13/2026".to_owned(),
            meeting_type: "FF".to_owned(),
            meeting_schedule_type: "AFF".to_owned(),
        }
    }
}

impl MeetingTimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set which days of the week this meeting occurs on.
    ///
    /// Days are ordered `[mon, tue, wed, thu, fri, sat, sun]`.
    pub fn days(mut self, [mon, tue, wed, thu, fri, sat, sun]: [bool; 7]) -> Self {
        self.monday = mon;
        self.tuesday = tue;
        self.wednesday = wed;
        self.thursday = thu;
        self.friday = fri;
        self.saturday = sat;
        self.sunday = sun;
        self
    }

    /// Set begin/end times in HHMM format (e.g. "0900", "0950").
    pub fn time(mut self, begin_hhmm: &str, end_hhmm: &str) -> Self {
        self.begin_time = Some(begin_hhmm.to_owned());
        self.end_time = Some(end_hhmm.to_owned());
        self
    }

    /// Set building code and room number.
    pub fn location(mut self, building: &str, room: &str) -> Self {
        self.building = Some(building.to_owned());
        self.building_description = Some(building.to_owned());
        self.room = Some(room.to_owned());
        self
    }

    /// Set start/end dates in MM/DD/YYYY format.
    pub fn dates(mut self, start_mmddyyyy: &str, end_mmddyyyy: &str) -> Self {
        self.start_date = start_mmddyyyy.to_owned();
        self.end_date = end_mmddyyyy.to_owned();
        self
    }

    /// Consume the builder and produce a `MeetingTimeResponse`.
    pub fn build(self) -> MeetingTimeResponse {
        MeetingTimeResponse {
            category: Some("01".to_owned()),
            class: "net.hedtech.banner.general.overall.SectionMeetingTimeDecorator".to_owned(),
            course_reference_number: self.crn.clone(),
            faculty: vec![],
            meeting_time: MeetingTime {
                start_date: self.start_date,
                end_date: self.end_date,
                begin_time: self.begin_time,
                end_time: self.end_time,
                category: "01".to_owned(),
                class: "net.hedtech.banner.general.overall.SectionMeetingTime".to_owned(),
                monday: self.monday,
                tuesday: self.tuesday,
                wednesday: self.wednesday,
                thursday: self.thursday,
                friday: self.friday,
                saturday: self.saturday,
                sunday: self.sunday,
                room: self.room,
                term: Term::from_str(&self.term).expect("valid term code"),
                building: self.building,
                building_description: self.building_description,
                campus: Some("11".to_owned()),
                campus_description: Some("Main Campus".to_owned()),
                course_reference_number: self.crn,
                credit_hour_session: None,
                hours_week: Some(0.0),
                meeting_schedule_type: self.meeting_schedule_type,
                meeting_type: Some(self.meeting_type.clone()),
                meeting_type_description: Some("Face to Face".to_owned()),
            },
            term: self.term,
        }
    }
}

/// Return a copy of `course` with its `meetings_faculty` replaced.
pub fn with_meetings(mut course: Course, meetings: Vec<MeetingTimeResponse>) -> Course {
    course.meetings_faculty = meetings;
    course
}

/// Insert a scrape job row directly via SQL, returning the generated ID.
pub async fn insert_scrape_job(
    pool: &PgPool,
    target_type: TargetType,
    payload: serde_json::Value,
    priority: ScrapePriority,
    locked: bool,
    retry_count: i32,
    max_retries: i32,
) -> i32 {
    let locked_at = if locked { Some(Utc::now()) } else { None };

    let (id,): (i32,) = sqlx::query_as(
        "INSERT INTO scrape_jobs (target_type, target_payload, priority, execute_at, locked_at, retry_count, max_retries)
         VALUES ($1, $2, $3, NOW(), $4, $5, $6)
         RETURNING id",
    )
    .bind(target_type)
    .bind(payload)
    .bind(priority)
    .bind(locked_at)
    .bind(retry_count)
    .bind(max_retries)
    .fetch_one(pool)
    .await
    .expect("insert_scrape_job failed");

    id
}
