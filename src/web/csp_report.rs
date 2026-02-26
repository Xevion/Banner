//! CSP violation reporting endpoint.
//!
//! Receives `Content-Security-Policy-Report-Only` browser reports and logs
//! them at `warn` level for tuning before enforcing the policy.
//!
//! Browsers send CSP reports with `Content-Type: application/csp-report`,
//! not `application/json`, so we accept raw bytes and deserialize manually.

use axum::body::Bytes;
use axum::http::StatusCode;
use serde::Deserialize;

/// Browser CSP violation report payload.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
struct CspReport {
    document_uri: Option<String>,
    violated_directive: Option<String>,
    effective_directive: Option<String>,
    blocked_uri: Option<String>,
    source_file: Option<String>,
    line_number: Option<u32>,
    column_number: Option<u32>,
    original_policy: Option<String>,
    disposition: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CspReportWrapper {
    csp_report: CspReport,
}

/// `POST /api/csp-report` — receives CSP violation reports from browsers.
///
/// Accepts both `application/csp-report` and `application/json` content types.
pub(crate) async fn csp_report(body: Bytes) -> StatusCode {
    let wrapper: CspReportWrapper = match serde_json::from_slice(&body) {
        Ok(w) => w,
        Err(e) => {
            tracing::debug!(error = %e, "Malformed CSP report");
            return StatusCode::BAD_REQUEST;
        }
    };

    let r = &wrapper.csp_report;

    tracing::warn!(
        document_uri = r.document_uri.as_deref().unwrap_or("-"),
        violated_directive = r.violated_directive.as_deref().unwrap_or("-"),
        effective_directive = r.effective_directive.as_deref().unwrap_or("-"),
        blocked_uri = r.blocked_uri.as_deref().unwrap_or("-"),
        source_file = r.source_file.as_deref().unwrap_or("-"),
        line_number = r.line_number.unwrap_or(0),
        column_number = r.column_number.unwrap_or(0),
        disposition = r.disposition.as_deref().unwrap_or("-"),
        "CSP violation"
    );

    StatusCode::NO_CONTENT
}
