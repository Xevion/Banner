pub mod subject;

use crate::banner::BannerApi;
use crate::data::DbContext;
use crate::data::models::{TargetType, UpsertCounts};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during job parsing
#[derive(Debug, Error)]
pub enum JobParseError {
    #[error("Invalid JSON in job payload: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Unsupported target type: {0:?}")]
    UnsupportedTargetType(TargetType),
}

/// Errors that can occur during job processing
#[derive(Debug, Error)]
pub enum JobError {
    #[error("Recoverable error: {0}")]
    Recoverable(#[source] anyhow::Error),
    #[error("Unrecoverable error: {0}")]
    Unrecoverable(#[source] anyhow::Error),
}

/// Common trait interface for all job types
#[async_trait::async_trait]
pub trait Job: Send + Sync {
    /// Process the job with the given API client and database context.
    /// Returns upsert effectiveness counts on success.
    async fn process(&self, banner_api: &BannerApi, db: &DbContext) -> Result<UpsertCounts>;

    /// Get a human-readable description of the job
    #[allow(dead_code)]
    fn description(&self) -> String;
}

/// Main job enum that dispatches to specific job implementations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobType {
    Subject(subject::SubjectJob),
}

impl JobType {
    /// Create a job from the target type and payload
    pub fn from_target_type_and_payload(
        target_type: TargetType,
        payload: serde_json::Value,
    ) -> Result<Self, JobParseError> {
        match target_type {
            TargetType::Subject => {
                let subject_job: subject::SubjectJob =
                    serde_json::from_value(payload).map_err(JobParseError::InvalidJson)?;
                Ok(JobType::Subject(subject_job))
            }
            _ => Err(JobParseError::UnsupportedTargetType(target_type)),
        }
    }

    /// Convert to a Job trait object
    pub fn boxed(self) -> Box<dyn Job> {
        match self {
            JobType::Subject(job) => Box::new(job),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Valid dispatch ---

    #[test]
    fn test_from_target_subject_valid() {
        let result =
            JobType::from_target_type_and_payload(TargetType::Subject, json!({"subject": "CS"}));
        assert!(matches!(result, Ok(JobType::Subject(_))));
    }

    #[test]
    fn test_from_target_subject_empty_string() {
        let result =
            JobType::from_target_type_and_payload(TargetType::Subject, json!({"subject": ""}));
        assert!(matches!(result, Ok(JobType::Subject(_))));
    }

    // --- Invalid JSON ---

    #[test]
    fn test_from_target_subject_missing_field() {
        let result = JobType::from_target_type_and_payload(TargetType::Subject, json!({}));
        assert!(matches!(result, Err(JobParseError::InvalidJson(_))));
    }

    #[test]
    fn test_from_target_subject_wrong_type() {
        let result =
            JobType::from_target_type_and_payload(TargetType::Subject, json!({"subject": 123}));
        assert!(matches!(result, Err(JobParseError::InvalidJson(_))));
    }

    #[test]
    fn test_from_target_subject_null_payload() {
        let result = JobType::from_target_type_and_payload(TargetType::Subject, json!(null));
        assert!(matches!(result, Err(JobParseError::InvalidJson(_))));
    }

    // --- Unsupported target types ---

    #[test]
    fn test_from_target_unsupported_variants() {
        let unsupported = [
            TargetType::CourseRange,
            TargetType::CrnList,
            TargetType::SingleCrn,
        ];
        for target_type in unsupported {
            let result =
                JobType::from_target_type_and_payload(target_type, json!({"subject": "CS"}));
            assert!(
                matches!(result, Err(JobParseError::UnsupportedTargetType(_))),
                "expected UnsupportedTargetType for {target_type:?}"
            );
        }
    }

    // --- Error Display ---

    #[test]
    fn test_job_parse_error_display() {
        let invalid_json_err =
            JobType::from_target_type_and_payload(TargetType::Subject, json!(null)).unwrap_err();
        let display = invalid_json_err.to_string();
        assert!(display.contains("Invalid JSON"), "got: {display}");

        let unsupported_err =
            JobType::from_target_type_and_payload(TargetType::CrnList, json!({})).unwrap_err();
        let display = unsupported_err.to_string();
        assert!(
            display.contains("Unsupported target type"),
            "got: {display}"
        );
    }
}
