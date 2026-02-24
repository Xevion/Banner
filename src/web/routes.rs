//! Web API endpoints for Banner bot monitoring and metrics.

use axum::{
    Extension, Router,
    extract::{Path, Query, State},
    http::HeaderValue,
    response::{Json, Response},
    routing::{get, post, put},
};

use axum::extract::Request;

use crate::data::course_types::{CreditHours, CrossList, Enrollment, RmpRating, SectionLink};
use crate::data::reference_types::{
    Attribute, Campus, FilterValue, InstructionalMethod, PartOfTerm,
};

/// Wraps a JSON response with a `Cache-Control` header.
fn with_cache_control<T: serde::Serialize>(value: T, header: &'static str) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static(header),
    );
    response
}

/// Convert a raw Banner code to its typed filter string for a given reference category.
fn code_to_filter_value(category: &str, code: &str, description: Option<&str>) -> String {
    match category {
        "instructional_method" => InstructionalMethod::from_code(code)
            .map(|m| m.to_filter_str().to_owned())
            .unwrap_or_else(|_| format!("raw:{code}")),
        "campus" => Campus::from_code(code, description)
            .to_filter_str()
            .into_owned(),
        "attribute" => Attribute::from_code(code, description)
            .to_filter_str()
            .into_owned(),
        "part_of_term" => PartOfTerm::from_code(code, description)
            .to_filter_str()
            .into_owned(),
        _ => format!("raw:{code}"),
    }
}
use crate::data;
use crate::data::models;
use crate::web::admin;
use crate::web::auth::{self, AuthConfig};
use crate::web::calendar;
use crate::web::error::{ApiError, ApiErrorCode, db_error};
use crate::web::instructors;
use crate::web::stream;
use crate::web::timeline;
#[cfg(feature = "embed-assets")]
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
use ts_rs::TS;

use crate::state::AppState;
use crate::state::ServiceStatus;
use crate::web::middleware::request_id::RequestIdLayer;
use tower_http::{compression::CompressionLayer, timeout::TimeoutLayer};
use tracing::{error, trace, warn};

#[cfg(feature = "embed-assets")]
use crate::web::assets::try_serve_asset_with_encoding;

/// Creates the web server router
pub fn create_router(app_state: AppState, auth_config: AuthConfig) -> Router {
    let api_router = Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/metrics", get(metrics))
        .route("/courses/search", get(search_courses))
        .route("/courses/{term}/{crn}", get(get_course))
        .route(
            "/courses/{term}/{subject}/{course_number}/sections",
            get(get_related_sections),
        )
        .route(
            "/courses/{term}/{crn}/calendar.ics",
            get(calendar::course_ics),
        )
        .route("/courses/{term}/{crn}/gcal", get(calendar::course_gcal))
        .route("/reference/{category}", get(get_reference))
        .route("/search-options", get(get_search_options))
        .route("/suggest", get(suggest))
        .route("/instructors", get(instructors::list_instructors))
        .route("/instructors/{slug}", get(instructors::get_instructor))
        .route(
            "/instructors/{slug}/sections",
            get(instructors::get_instructor_sections),
        )
        .route("/timeline", post(timeline::timeline))
        .route("/ws", get(stream::stream_ws))
        .with_state(app_state.clone());

    let auth_router = Router::new()
        .route("/auth/login", get(auth::auth_login))
        .route("/auth/callback", get(auth::auth_callback))
        .route("/auth/logout", post(auth::auth_logout))
        .route("/auth/me", get(auth::auth_me))
        .layer(Extension(auth_config))
        .with_state(app_state.clone());

    let admin_router = Router::new()
        .route("/admin/status", get(admin::admin_status))
        .route("/admin/users", get(admin::list_users))
        .route(
            "/admin/users/{discord_id}/admin",
            put(admin::set_user_admin),
        )
        .route("/admin/scrape-jobs", get(admin::list_scrape_jobs))
        .route("/admin/audit-log", get(admin::list_audit_log))
        .route("/admin/instructors", get(admin::rmp::list_instructors))
        .route("/admin/instructors/{id}", get(admin::rmp::get_instructor))
        .route(
            "/admin/instructors/{id}/match",
            post(admin::rmp::match_instructor),
        )
        .route(
            "/admin/instructors/{id}/reject-candidate",
            post(admin::rmp::reject_candidate),
        )
        .route(
            "/admin/instructors/{id}/reject-all",
            post(admin::rmp::reject_all),
        )
        .route(
            "/admin/instructors/{id}/unmatch",
            post(admin::rmp::unmatch_instructor),
        )
        .route("/admin/rmp/rescore", post(admin::rmp::rescore))
        .route("/admin/scraper/stats", get(admin::scraper::scraper_stats))
        .route(
            "/admin/scraper/timeseries",
            get(admin::scraper::scraper_timeseries),
        )
        .route(
            "/admin/scraper/subjects",
            get(admin::scraper::scraper_subjects),
        )
        .route(
            "/admin/scraper/subjects/{subject}",
            get(admin::scraper::scraper_subject_detail),
        )
        .route("/admin/bluebook/sync", post(admin::bluebook::sync_bluebook))
        .route("/admin/bluebook/links", get(admin::bluebook::list_links))
        .route("/admin/bluebook/links/{id}", get(admin::bluebook::get_link))
        .route(
            "/admin/bluebook/links/{id}/approve",
            post(admin::bluebook::approve_link),
        )
        .route(
            "/admin/bluebook/links/{id}/reject",
            post(admin::bluebook::reject_link),
        )
        .route(
            "/admin/bluebook/links/{id}/assign",
            post(admin::bluebook::assign_link),
        )
        .route("/admin/bluebook/match", post(admin::bluebook::run_matching))
        .route("/admin/terms", get(admin::terms::list_terms))
        .route("/admin/terms/sync", post(admin::terms::sync_terms))
        .route(
            "/admin/terms/{code}/enable",
            post(admin::terms::enable_term),
        )
        .route(
            "/admin/terms/{code}/disable",
            post(admin::terms::disable_term),
        )
        .with_state(app_state.clone());

    let router = Router::new()
        .nest("/api", api_router)
        .nest("/api", auth_router)
        .nest("/api", admin_router)
        .fallback(ssr_fallback)
        .with_state(app_state);

    router.layer((
        // Outermost: per-request ULID span + severity-proportional response logging.
        RequestIdLayer,
        // Compress API responses (gzip/brotli/zstd). Pre-compressed static
        // assets already have Content-Encoding set, so tower-http skips them.
        CompressionLayer::new()
            .zstd(true)
            .br(true)
            .gzip(true)
            .quality(tower_http::CompressionLevel::Fastest),
        TimeoutLayer::new(Duration::from_secs(60)),
    ))
}

/// SSR fallback: try embedded static assets first, then proxy to the SSR server.
async fn ssr_fallback(State(state): State<AppState>, request: Request) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path();
    let query = uri.query();
    let headers = request.headers().clone();

    // Try serving embedded static assets (production only)
    #[cfg(feature = "embed-assets")]
    {
        if let Some(response) = try_serve_asset_with_encoding(path, &headers) {
            return response;
        }

        // SvelteKit assets under _app/ that don't exist are a hard 404
        let trimmed = path.trim_start_matches('/');
        if trimmed.starts_with("_app/") || trimmed.starts_with("assets/") {
            return (StatusCode::NOT_FOUND, "Asset not found").into_response();
        }
    }

    // Proxy to the downstream SSR server
    crate::web::proxy::proxy_to_ssr(&state, &method, path, query, headers).await
}

/// Health check endpoint
async fn health() -> Json<Value> {
    trace!("health check requested");
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ServiceInfo {
    name: String,
    status: ServiceStatus,
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct StatusResponse {
    status: ServiceStatus,
    version: String,
    commit: String,
    services: BTreeMap<String, ServiceInfo>,
}

/// Status endpoint showing bot and system status
async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let mut services = BTreeMap::new();

    for (name, svc_status) in state.service_statuses.all() {
        services.insert(
            name.clone(),
            ServiceInfo {
                name,
                status: svc_status,
            },
        );
    }

    let overall_status = if services
        .values()
        .any(|s| matches!(s.status, ServiceStatus::Error))
    {
        ServiceStatus::Error
    } else if !services.is_empty()
        && services
            .values()
            .all(|s| matches!(s.status, ServiceStatus::Active | ServiceStatus::Connected))
    {
        ServiceStatus::Active
    } else if services.is_empty() {
        ServiceStatus::Disabled
    } else {
        ServiceStatus::Active
    };

    Json(StatusResponse {
        status: overall_status,
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: env!("GIT_COMMIT_HASH").to_string(),
        services,
    })
}

/// Metrics endpoint for monitoring
async fn metrics(
    State(state): State<AppState>,
    Query(params): Query<MetricsParams>,
) -> Result<Json<MetricsResponse>, ApiError> {
    let limit = params.limit.clamp(1, 5000);

    let range_str = params.range.as_deref().unwrap_or("24h");
    let duration = match range_str {
        "1h" => chrono::Duration::hours(1),
        "6h" => chrono::Duration::hours(6),
        "24h" => chrono::Duration::hours(24),
        "7d" => chrono::Duration::days(7),
        "30d" => chrono::Duration::days(30),
        _ => {
            return Err(ApiError::new(
                ApiErrorCode::InvalidRange,
                format!("Invalid range '{range_str}'. Valid: 1h, 6h, 24h, 7d, 30d"),
            ));
        }
    };
    let since = chrono::Utc::now() - duration;

    let course_id = if let Some(id) = params.course_id {
        Some(id)
    } else if let (Some(term), Some(crn)) = (params.term.as_deref(), params.crn.as_deref()) {
        use crate::banner::models::terms::Term;
        let resolved = Term::resolve_to_code(term).unwrap_or_else(|| term.to_string());
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT id FROM courses WHERE term_code = $1 AND crn = $2")
                .bind(&resolved)
                .bind(crn)
                .fetch_optional(&state.db_pool)
                .await
                .map_err(|e| db_error("Course lookup for metrics", e.into()))?;
        row.map(|(id,)| id)
    } else {
        None
    };

    let metrics: Vec<(i32, i32, chrono::DateTime<chrono::Utc>, i32, i32, i32)> =
        if let Some(cid) = course_id {
            sqlx::query_as(
                "SELECT id, course_id, timestamp, enrollment, wait_count, seats_available \
             FROM course_metrics \
             WHERE course_id = $1 AND timestamp >= $2 \
             ORDER BY timestamp DESC \
             LIMIT $3",
            )
            .bind(cid)
            .bind(since)
            .bind(limit)
            .fetch_all(&state.db_pool)
            .await
        } else {
            sqlx::query_as(
                "SELECT id, course_id, timestamp, enrollment, wait_count, seats_available \
             FROM course_metrics \
             WHERE timestamp >= $1 \
             ORDER BY timestamp DESC \
             LIMIT $2",
            )
            .bind(since)
            .bind(limit)
            .fetch_all(&state.db_pool)
            .await
        }
        .map_err(|e| db_error("Metrics query", e.into()))?;

    let count = metrics.len();
    let metrics_entries: Vec<MetricEntry> = metrics
        .into_iter()
        .map(
            |(id, course_id, timestamp, enrollment, wait_count, seats_available)| MetricEntry {
                id,
                course_id,
                timestamp: timestamp.to_rfc3339(),
                enrollment,
                wait_count,
                seats_available,
            },
        )
        .collect();

    Ok(Json(MetricsResponse {
        metrics: metrics_entries,
        count,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricEntry {
    pub id: i32,
    pub course_id: i32,
    pub timestamp: String,
    pub enrollment: i32,
    pub wait_count: i32,
    pub seats_available: i32,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricsResponse {
    pub metrics: Vec<MetricEntry>,
    pub count: usize,
    pub timestamp: String,
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    #[serde(default = "default_metrics_limit")]
    pub limit: i32,
}

fn default_metrics_limit() -> i32 {
    500
}

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchParams {
    pub term: String,
    #[serde(default)]
    pub subject: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "q")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "course_number_low")]
    pub course_number_low: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "course_number_high")]
    pub course_number_high: Option<i32>,
    #[serde(default, alias = "open_only")]
    pub open_only: bool,
    #[serde(default, alias = "instructional_method")]
    #[ts(type = "Array<string>")]
    pub instructional_method: Vec<FilterValue<InstructionalMethod>>,
    #[serde(default)]
    #[ts(type = "Array<string>")]
    pub campus: Vec<FilterValue<Campus>>,
    #[serde(default = "default_limit")]
    pub limit: i32,
    #[serde(default)]
    pub offset: i32,
    #[serde(skip_serializing_if = "Option::is_none", alias = "sort_by")]
    pub sort_by: Option<SortColumn>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "sort_dir")]
    pub sort_dir: Option<SortDirection>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "wait_count_max")]
    pub wait_count_max: Option<i32>,
    #[serde(default)]
    pub days: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "time_start")]
    pub time_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "time_end")]
    pub time_end: Option<String>,
    #[serde(default, alias = "part_of_term")]
    #[ts(type = "Array<string>")]
    pub part_of_term: Vec<FilterValue<PartOfTerm>>,
    #[serde(default)]
    #[ts(type = "Array<string>")]
    pub attributes: Vec<FilterValue<Attribute>>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "credit_hour_min")]
    pub credit_hour_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "credit_hour_max")]
    pub credit_hour_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructor: Option<String>,
}

use crate::data::courses::{CourseSuggestion, InstructorSuggestion, SortColumn, SortDirection};

fn default_limit() -> i32 {
    25
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CourseResponse {
    crn: String,
    subject: String,
    course_number: String,
    title: String,
    term_code: String,
    term_slug: String,
    sequence_number: Option<String>,
    instructional_method: Option<InstructionalMethod>,
    /// Raw instructional method code, included when parsing fails (Tier 1 fallback).
    #[serde(skip_serializing_if = "Option::is_none")]
    instructional_method_code: Option<String>,
    campus: Option<Campus>,
    enrollment: Enrollment,
    credit_hours: Option<CreditHours>,
    cross_list: Option<CrossList>,
    section_link: Option<SectionLink>,
    part_of_term: Option<PartOfTerm>,
    meeting_times: Vec<models::DbMeetingTime>,
    attributes: Vec<Attribute>,
    is_async_online: bool,
    /// Best display-ready location: physical room ("MH 2.206"), "Online", or campus fallback.
    primary_location: Option<String>,
    /// Whether a physical (non-INT) building was found in meeting times.
    has_physical_location: bool,
    primary_instructor_id: Option<i32>,
    instructors: Vec<InstructorResponse>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstructorResponse {
    instructor_id: i32,
    banner_id: String,
    display_name: String,
    first_name: Option<String>,
    last_name: Option<String>,
    email: Option<String>,
    slug: Option<String>,
    is_primary: bool,
    rmp: Option<RmpRating>,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchResponse {
    courses: Vec<CourseResponse>,
    total_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CodeDescription {
    code: String,
    description: String,
    /// Typed filter string for query params (e.g. "Online.Async", "Main", "raw:XYZ").
    filter_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TermResponse {
    code: String,
    slug: String,
    description: String,
}

/// Response for the consolidated search-options endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchOptionsResponse {
    pub terms: Vec<TermResponse>,
    pub subjects: Vec<CodeDescription>,
    pub reference: SearchOptionsReference,
    pub ranges: data::courses::FilterRanges,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchOptionsReference {
    pub instructional_methods: Vec<CodeDescription>,
    pub campuses: Vec<CodeDescription>,
    pub parts_of_term: Vec<CodeDescription>,
    pub attributes: Vec<CodeDescription>,
}

#[derive(Debug, Deserialize)]
pub struct SearchOptionsParams {
    pub term: Option<String>,
}

const RMP_CONFIDENCE_THRESHOLD: i32 = 7;

/// Build a `CourseResponse` from a DB course with pre-fetched instructor details.
pub fn build_course_response(
    course: &models::Course,
    instructors: Vec<models::CourseInstructorDetail>,
) -> CourseResponse {
    let instructors: Vec<InstructorResponse> = instructors
        .into_iter()
        .map(|i| {
            let rmp = i.rmp_legacy_id.map(|legacy_id| {
                let (avg_rating, num_ratings) =
                    crate::data::course_types::sanitize_rmp_ratings(i.avg_rating, i.num_ratings);
                let is_confident = num_ratings.is_some_and(|n| n >= RMP_CONFIDENCE_THRESHOLD);
                RmpRating {
                    avg_rating,
                    num_ratings,
                    legacy_id,
                    is_confident,
                }
            });
            InstructorResponse {
                instructor_id: i.instructor_id,
                banner_id: i.banner_id,
                display_name: i.display_name,
                first_name: i.first_name,
                last_name: i.last_name,
                email: i.email,
                slug: i.slug,
                is_primary: i.is_primary,
                rmp,
            }
        })
        .collect();

    let primary_instructor_id = instructors
        .iter()
        .find(|i| i.is_primary)
        .or(instructors.first())
        .map(|i| i.instructor_id);

    let meeting_times: Vec<models::DbMeetingTime> =
        serde_json::from_value(course.meeting_times.clone())
            .map_err(|e| {
                error!(
                    course_id = course.id,
                    crn = %course.crn,
                    term = %course.term_code,
                    %e,
                    "Failed to deserialize meeting_times JSONB"
                );
                e
            })
            .unwrap_or_default();

    let attributes: Vec<Attribute> =
        serde_json::from_value::<Vec<String>>(course.attributes.clone())
            .map_err(|e| {
                error!(
                    course_id = course.id,
                    crn = %course.crn,
                    term = %course.term_code,
                    %e,
                    "Failed to deserialize attributes JSONB"
                );
                e
            })
            .unwrap_or_default()
            .into_iter()
            .map(|code| Attribute::from_code(&code, None))
            .collect();

    let (instructional_method, instructional_method_code) = match &course.instructional_method {
        Some(code) => match InstructionalMethod::from_code(code) {
            Ok(method) => (Some(method), None),
            Err(_) => {
                warn!(
                    crn = %course.crn,
                    term = %course.term_code,
                    %code,
                    "Unknown instructional method code"
                );
                (None, Some(code.clone()))
            }
        },
        None => (None, None),
    };

    let campus = course
        .campus
        .as_ref()
        .map(|code| Campus::from_code(code, None));
    let part_of_term = course
        .part_of_term
        .as_ref()
        .map(|code| PartOfTerm::from_code(code, None));

    let is_async_online = meeting_times.first().is_some_and(|mt| {
        mt.location.as_ref().and_then(|loc| loc.building.as_deref()) == Some("INT")
            && mt.is_time_tba()
    });

    let physical_location = meeting_times
        .iter()
        .filter(|mt| mt.location.as_ref().and_then(|loc| loc.building.as_deref()) != Some("INT"))
        .find_map(|mt| {
            mt.location.as_ref().and_then(|loc| {
                loc.building.as_ref().map(|b| match &loc.room {
                    Some(r) => format!("{b} {r}"),
                    None => b.clone(),
                })
            })
        });
    let has_physical_location = physical_location.is_some();

    let primary_location = physical_location.or_else(|| {
        let is_hybrid = instructional_method
            .as_ref()
            .is_some_and(|m| matches!(m, InstructionalMethod::Hybrid(_)));
        let is_online_method = instructional_method
            .as_ref()
            .is_some_and(|m| matches!(m, InstructionalMethod::Online(_)));
        let is_virtual_campus = campus
            .as_ref()
            .is_some_and(|c| matches!(c, Campus::Internet | Campus::OnlinePrograms));
        if is_hybrid {
            Some("Hybrid".to_string())
        } else if is_online_method || is_virtual_campus {
            Some("Online".to_string())
        } else {
            None
        }
    });

    let enrollment = Enrollment {
        current: course.enrollment,
        max: course.max_enrollment,
        wait_count: course.wait_count,
        wait_capacity: course.wait_capacity,
    };

    let credit_hours = match (
        course.credit_hours,
        course.credit_hour_low,
        course.credit_hour_high,
    ) {
        (Some(fixed), _, _) => Some(CreditHours::Fixed { hours: fixed }),
        (None, Some(low), Some(high)) if low != high => Some(CreditHours::Range { low, high }),
        (None, Some(hours), None) | (None, None, Some(hours)) => Some(CreditHours::Fixed { hours }),
        _ => None,
    };

    let cross_list = course.cross_list.as_ref().and_then(|identifier| {
        course.cross_list_capacity.and_then(|capacity| {
            course.cross_list_count.map(|count| CrossList {
                identifier: identifier.clone(),
                capacity,
                count,
            })
        })
    });

    let section_link = course
        .link_identifier
        .clone()
        .map(|identifier| SectionLink { identifier });

    use crate::banner::models::terms::Term;
    let term_slug = course
        .term_code
        .parse::<Term>()
        .map(|t| t.slug())
        .unwrap_or_else(|_| course.term_code.clone());

    CourseResponse {
        crn: course.crn.clone(),
        subject: course.subject.clone(),
        course_number: course.course_number.clone(),
        title: course.title.clone(),
        term_code: course.term_code.clone(),
        term_slug,
        sequence_number: course.sequence_number.clone(),
        instructional_method,
        instructional_method_code,
        campus,
        enrollment,
        credit_hours,
        cross_list,
        section_link,
        part_of_term,
        is_async_online,
        primary_location,
        has_physical_location,
        primary_instructor_id,
        meeting_times,
        attributes,
        instructors,
    }
}

/// `GET /api/courses/search`
async fn search_courses(
    State(state): State<AppState>,
    axum_extra::extract::Query(params): axum_extra::extract::Query<SearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    use crate::banner::models::terms::Term;

    let term_code =
        Term::resolve_to_code(&params.term).ok_or_else(|| ApiError::invalid_term(&params.term))?;
    let limit = params.limit.clamp(1, 100);
    let offset = params.offset.max(0);

    // Convert typed filter values to raw Banner codes for SQL
    let method_codes: Vec<String> = params
        .instructional_method
        .iter()
        .map(|fv| fv.to_code().into_owned())
        .collect();
    let campus_codes: Vec<String> = params
        .campus
        .iter()
        .map(|fv| fv.to_code().into_owned())
        .collect();
    let pot_codes: Vec<String> = params
        .part_of_term
        .iter()
        .map(|fv| fv.to_code().into_owned())
        .collect();
    let attr_codes: Vec<String> = params
        .attributes
        .iter()
        .map(|fv| fv.to_code().into_owned())
        .collect();

    let (courses, total_count) = data::courses::search_courses(
        &state.db_pool,
        &term_code,
        if params.subject.is_empty() {
            None
        } else {
            Some(&params.subject)
        },
        params.query.as_deref(),
        params.course_number_low,
        params.course_number_high,
        params.open_only,
        if method_codes.is_empty() {
            None
        } else {
            Some(&method_codes)
        },
        if campus_codes.is_empty() {
            None
        } else {
            Some(&campus_codes)
        },
        params.wait_count_max,
        if params.days.is_empty() {
            None
        } else {
            Some(&params.days)
        },
        params.time_start.as_deref(),
        params.time_end.as_deref(),
        if pot_codes.is_empty() {
            None
        } else {
            Some(&pot_codes)
        },
        if attr_codes.is_empty() {
            None
        } else {
            Some(&attr_codes)
        },
        params.credit_hour_min,
        params.credit_hour_max,
        params.instructor.as_deref(),
        limit,
        offset,
        params.sort_by,
        params.sort_dir,
    )
    .await
    .map_err(|e| db_error("Course search", e))?;

    let course_ids: Vec<i32> = courses.iter().map(|c| c.id).collect();
    let mut instructor_map =
        data::courses::get_instructors_for_courses(&state.db_pool, &course_ids)
            .await
            .unwrap_or_default();

    let course_responses: Vec<CourseResponse> = courses
        .iter()
        .map(|course| {
            let instructors = instructor_map.remove(&course.id).unwrap_or_default();
            build_course_response(course, instructors)
        })
        .collect();

    Ok(Json(SearchResponse {
        courses: course_responses,
        total_count: total_count as i32,
    }))
}

/// `GET /api/courses/:term/:crn`
async fn get_course(
    State(state): State<AppState>,
    Path((term, crn)): Path<(String, String)>,
) -> Result<Json<CourseResponse>, ApiError> {
    use crate::banner::models::terms::Term;
    let term_code = Term::resolve_to_code(&term).ok_or_else(|| ApiError::invalid_term(&term))?;
    let course = data::courses::get_course_by_crn(&state.db_pool, &crn, &term_code)
        .await
        .map_err(|e| db_error("Course lookup", e))?
        .ok_or_else(|| ApiError::not_found("Course not found"))?;

    let instructors = data::courses::get_course_instructors(&state.db_pool, course.id)
        .await
        .unwrap_or_default();
    Ok(Json(build_course_response(&course, instructors)))
}

/// `GET /api/courses/:term/:subject/:course_number/sections`
///
/// Returns all sections of the same course (same term, subject, course number).
async fn get_related_sections(
    State(state): State<AppState>,
    Path((term, subject, course_number)): Path<(String, String, String)>,
) -> Result<Json<Vec<CourseResponse>>, ApiError> {
    use crate::banner::models::terms::Term;
    let term_code = Term::resolve_to_code(&term).ok_or_else(|| ApiError::invalid_term(&term))?;
    let courses =
        data::courses::get_related_sections(&state.db_pool, &term_code, &subject, &course_number)
            .await
            .map_err(|e| db_error("Related sections lookup", e))?;

    let course_ids: Vec<i32> = courses.iter().map(|c| c.id).collect();
    let mut instructor_map =
        data::courses::get_instructors_for_courses(&state.db_pool, &course_ids)
            .await
            .unwrap_or_default();

    let responses: Vec<CourseResponse> = courses
        .iter()
        .map(|course| {
            let instructors = instructor_map.remove(&course.id).unwrap_or_default();
            build_course_response(course, instructors)
        })
        .collect();

    Ok(Json(responses))
}

/// `GET /api/reference/:category`
async fn get_reference(
    State(state): State<AppState>,
    Path(category): Path<String>,
) -> Result<Response, ApiError> {
    let cache = state.reference_cache.read().await;
    let entries = cache.entries_for_category(&category);

    if entries.is_empty() {
        drop(cache);
        let rows = data::reference::get_by_category(&category, &state.db_pool)
            .await
            .map_err(|e| db_error(&format!("Reference lookup for {}", category), e))?;

        let rows_mapped: Vec<CodeDescription> = rows
            .into_iter()
            .map(|r| {
                let filter_value = code_to_filter_value(&category, &r.code, Some(&r.description));
                CodeDescription {
                    code: r.code,
                    description: r.description,
                    filter_value,
                }
            })
            .collect();
        return Ok(with_cache_control(
            rows_mapped,
            "private, max-age=600, stale-while-revalidate=60",
        ));
    }

    let entries_mapped: Vec<CodeDescription> = entries
        .into_iter()
        .map(|(code, desc)| {
            let filter_value = code_to_filter_value(&category, code, Some(desc));
            CodeDescription {
                code: code.to_string(),
                description: desc.to_string(),
                filter_value,
            }
        })
        .collect();
    Ok(with_cache_control(
        entries_mapped,
        "private, max-age=600, stale-while-revalidate=60",
    ))
}

/// `GET /api/search-options?term={slug}` (term optional, defaults to latest)
async fn get_search_options(
    State(state): State<AppState>,
    Query(params): Query<SearchOptionsParams>,
) -> Result<Response, ApiError> {
    use crate::banner::models::terms::Term;

    let term_slug = if let Some(ref t) = params.term {
        t.clone()
    } else {
        // Fetch available terms to get the default (latest)
        let term_codes = data::courses::get_available_terms(&state.db_pool)
            .await
            .map_err(|e| db_error("Get terms for default", e))?;

        let first_term: Term = term_codes
            .first()
            .and_then(|code| code.parse().ok())
            .ok_or_else(|| ApiError::new(ApiErrorCode::NoTerms, "No terms available"))?;

        first_term.slug()
    };

    let term_code =
        Term::resolve_to_code(&term_slug).ok_or_else(|| ApiError::invalid_term(&term_slug))?;

    if let Some(cached) = state.search_options_cache.get(&term_code) {
        return Ok(with_cache_control(
            (*cached).clone(),
            "private, max-age=600, stale-while-revalidate=60",
        ));
    }

    if !state.search_options_cache.try_claim(&term_code) {
        // Another request is building this term's options — fall through and build it too.
        // (Acceptable: singleflight is best-effort, not strict.)
    }

    let (term_codes, subject_rows, ranges) = tokio::try_join!(
        data::courses::get_available_terms(&state.db_pool),
        data::courses::get_subjects_by_enrollment(&state.db_pool, &term_code),
        data::courses::get_filter_ranges(&state.db_pool, &term_code),
    )
    .map_err(|e| db_error("Search options", e))?;

    let terms: Vec<TermResponse> = term_codes
        .into_iter()
        .filter_map(|code| {
            let term: Term = code.parse().ok()?;
            Some(TermResponse {
                code,
                slug: term.slug(),
                description: term.description(),
            })
        })
        .collect();

    let subjects: Vec<CodeDescription> = subject_rows
        .into_iter()
        .map(|(code, description, _enrollment)| {
            let filter_value = code.clone();
            CodeDescription {
                code,
                description,
                filter_value,
            }
        })
        .collect();

    let ref_cache = state.reference_cache.read().await;
    let build_ref = |category: &str| -> Vec<CodeDescription> {
        ref_cache
            .entries_for_category(category)
            .into_iter()
            .map(|(code, desc)| {
                let filter_value = code_to_filter_value(category, code, Some(desc));
                CodeDescription {
                    code: code.to_string(),
                    description: desc.to_string(),
                    filter_value,
                }
            })
            .collect()
    };

    let reference = SearchOptionsReference {
        instructional_methods: build_ref("instructional_method"),
        campuses: build_ref("campus"),
        parts_of_term: build_ref("part_of_term"),
        attributes: build_ref("attribute"),
    };

    let response = SearchOptionsResponse {
        terms,
        subjects,
        reference,
        ranges,
    };

    state
        .search_options_cache
        .insert(term_code.clone(), response.clone());
    state.search_options_cache.release(&term_code);

    Ok(with_cache_control(
        response,
        "private, max-age=600, stale-while-revalidate=60",
    ))
}

#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct SuggestParams {
    pub term: String,
    pub q: String,
    #[serde(default = "default_suggest_limit")]
    pub limit: i32,
}

fn default_suggest_limit() -> i32 {
    10
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SuggestResponse {
    pub courses: Vec<CourseSuggestion>,
    pub instructors: Vec<InstructorSuggestion>,
}

/// `GET /api/suggest?term={slug}&q={query}&limit=10`
async fn suggest(
    State(state): State<AppState>,
    Query(params): Query<SuggestParams>,
) -> Result<Response, ApiError> {
    use crate::banner::models::terms::Term;

    let term_code =
        Term::resolve_to_code(&params.term).ok_or_else(|| ApiError::invalid_term(&params.term))?;
    let limit = params.limit.clamp(1, 25);
    let q = params.q.trim();

    if q.chars().count() < 2 {
        return Ok(with_cache_control(
            SuggestResponse {
                courses: vec![],
                instructors: vec![],
            },
            "private, max-age=60",
        ));
    }

    let (courses, instructors) = tokio::try_join!(
        data::courses::suggest_courses(&state.db_pool, &term_code, q, limit),
        data::courses::suggest_instructors(&state.db_pool, &term_code, q, limit),
    )
    .map_err(|e| db_error("Suggest query", e))?;

    Ok(with_cache_control(
        SuggestResponse {
            courses,
            instructors,
        },
        "private, max-age=60",
    ))
}
