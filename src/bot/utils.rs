//! Bot command utilities.

use crate::banner::{Course, Term};
use crate::bot::Context;
use anyhow::Result;
use tracing::error;

/// Gets a course by its CRN for the current term.
pub async fn get_course_by_crn(ctx: &Context<'_>, crn: i32) -> Result<Course> {
    let app_state = &ctx.data().app_state;

    // Get current term dynamically
    let current_term_status = Term::get_current();
    let term = current_term_status.inner();

    // Fetch live course data from database via AppState
    app_state
        .get_course_or_fetch(&term.to_string(), &crn.to_string())
        .await
        .map_err(|e| {
            error!(error = %e, crn = %crn, "failed to fetch course data");
            e
        })
}
