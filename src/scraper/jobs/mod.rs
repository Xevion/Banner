pub mod subject;

use crate::banner::BannerApi;
use crate::data::DbContext;
use crate::data::models::{TargetPayload, TargetType, UpsertCounts};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during job parsing
#[derive(Debug, Error)]
pub enum JobParseError {
    #[error("Payload shape does not match target type: {0:?}")]
    PayloadMismatch(TargetType),
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
        payload: TargetPayload,
    ) -> Result<Self, JobParseError> {
        match (target_type, payload) {
            (TargetType::Subject, TargetPayload::Subject(job)) => Ok(JobType::Subject(job)),
            (TargetType::Subject, _) => Err(JobParseError::PayloadMismatch(TargetType::Subject)),
            (other, _) => Err(JobParseError::UnsupportedTargetType(other)),
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
    use crate::data::models::{SingleCrnTarget, SubjectTarget};
    use serde_json::json;

    fn subject_payload(subject: &str) -> TargetPayload {
        TargetPayload::Subject(SubjectTarget {
            subject: subject.to_string(),
            term: Some("202620".to_string()),
        })
    }

    #[test]
    fn test_from_target_subject_valid() {
        let result =
            JobType::from_target_type_and_payload(TargetType::Subject, subject_payload("CS"));
        assert!(matches!(result, Ok(JobType::Subject(_))));
    }

    #[test]
    fn test_from_target_subject_empty_string() {
        let result =
            JobType::from_target_type_and_payload(TargetType::Subject, subject_payload(""));
        assert!(matches!(result, Ok(JobType::Subject(_))));
    }

    #[test]
    fn test_from_target_subject_rejects_other_payload_shape() {
        let payload = TargetPayload::SingleCrn(SingleCrnTarget {
            crn: "12345".to_string(),
            term: None,
        });
        let result = JobType::from_target_type_and_payload(TargetType::Subject, payload);
        assert!(matches!(result, Err(JobParseError::PayloadMismatch(_))));
    }

    #[test]
    fn test_payload_missing_required_field_fails_to_parse() {
        assert!(serde_json::from_value::<TargetPayload>(json!({})).is_err());
        assert!(serde_json::from_value::<TargetPayload>(json!(null)).is_err());
        assert!(serde_json::from_value::<TargetPayload>(json!({"subject": 123})).is_err());
    }

    #[test]
    fn test_payload_shapes_round_trip() {
        let payload: TargetPayload =
            serde_json::from_value(json!({"subject": "CS", "term": "202620"})).unwrap();
        assert_eq!(payload.subject(), Some("CS"));
        assert_eq!(payload.term(), Some("202620"));

        let payload: TargetPayload = serde_json::from_value(json!({"crn": "12345"})).unwrap();
        assert!(matches!(payload, TargetPayload::SingleCrn(_)));

        let payload: TargetPayload =
            serde_json::from_value(json!({"subject": "CS", "low": 1000, "high": 1999})).unwrap();
        assert!(matches!(payload, TargetPayload::CourseRange(_)));
    }

    #[test]
    fn test_from_target_unsupported_variants() {
        let unsupported = [
            TargetType::CourseRange,
            TargetType::CrnList,
            TargetType::SingleCrn,
        ];
        for target_type in unsupported {
            let result = JobType::from_target_type_and_payload(target_type, subject_payload("CS"));
            assert!(
                matches!(result, Err(JobParseError::UnsupportedTargetType(_))),
                "expected UnsupportedTargetType for {target_type:?}"
            );
        }
    }

    #[test]
    fn test_job_parse_error_display() {
        let mismatch_err = JobType::from_target_type_and_payload(
            TargetType::Subject,
            TargetPayload::SingleCrn(SingleCrnTarget {
                crn: "12345".to_string(),
                term: None,
            }),
        )
        .unwrap_err();
        let display = mismatch_err.to_string();
        assert!(
            display.contains("does not match target type"),
            "got: {display}"
        );

        let unsupported_err =
            JobType::from_target_type_and_payload(TargetType::CrnList, subject_payload("CS"))
                .unwrap_err();
        let display = unsupported_err.to_string();
        assert!(
            display.contains("Unsupported target type"),
            "got: {display}"
        );
    }
}
