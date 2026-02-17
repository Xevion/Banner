//! Standardized API error responses.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use ts_rs::TS;

/// Machine-readable error code for API responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ApiErrorCode {
    NotFound,
    BadRequest,
    Conflict,
    InternalError,
    InvalidTerm,
    InvalidRange,
    Unauthorized,
    Forbidden,
    NoTerms,
}

/// Standardized error response for all API endpoints.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApiError {
    /// Machine-readable error code
    pub code: ApiErrorCode,
    /// Human-readable error message
    pub message: String,
    /// Optional additional details (validation errors, field info, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::NotFound, message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::BadRequest, message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::InternalError, message)
    }

    pub fn invalid_term(term: impl std::fmt::Display) -> Self {
        Self::new(ApiErrorCode::InvalidTerm, format!("Invalid term: {}", term))
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::Conflict, message)
    }

    fn status_code(&self) -> StatusCode {
        match self.code {
            ApiErrorCode::NotFound => StatusCode::NOT_FOUND,
            ApiErrorCode::BadRequest
            | ApiErrorCode::InvalidTerm
            | ApiErrorCode::InvalidRange
            | ApiErrorCode::NoTerms => StatusCode::BAD_REQUEST,
            ApiErrorCode::Conflict => StatusCode::CONFLICT,
            ApiErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ApiErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        (status, Json(self)).into_response()
    }
}

/// Convert `(StatusCode, String)` tuple errors to ApiError
impl From<(StatusCode, String)> for ApiError {
    fn from((status, message): (StatusCode, String)) -> Self {
        let code = match status {
            StatusCode::NOT_FOUND => ApiErrorCode::NotFound,
            StatusCode::BAD_REQUEST => ApiErrorCode::BadRequest,
            StatusCode::CONFLICT => ApiErrorCode::Conflict,
            StatusCode::UNAUTHORIZED => ApiErrorCode::Unauthorized,
            StatusCode::FORBIDDEN => ApiErrorCode::Forbidden,
            _ => ApiErrorCode::InternalError,
        };
        Self::new(code, message)
    }
}

/// Helper for converting database errors to ApiError
pub fn db_error(context: &str, error: anyhow::Error) -> ApiError {
    tracing::error!(error = %error, context = context, "Database error");
    ApiError::internal_error(format!("{} failed", context))
}
