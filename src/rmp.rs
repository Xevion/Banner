//! RateMyProfessors GraphQL client for bulk professor data sync
//! and per-instructor review scraping.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, trace};

/// UTSA's school ID on RateMyProfessors (base64 of "School-1516").
const UTSA_SCHOOL_ID: &str = "U2Nob29sLTE1MTY=";

/// Basic auth header value (base64 of "test:test").
const AUTH_HEADER: &str = "Basic dGVzdDp0ZXN0";

/// GraphQL endpoint.
const GRAPHQL_URL: &str = "https://www.ratemyprofessors.com/graphql";

/// Page size for paginated fetches.
const PAGE_SIZE: u32 = 100;

/// A professor record from RateMyProfessors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RmpProfessor {
    pub legacy_id: i32,
    pub graphql_id: String,
    pub first_name: String,
    pub last_name: String,
    pub department: Option<String>,
    pub avg_rating: Option<f32>,
    pub avg_difficulty: Option<f32>,
    pub num_ratings: i32,
    pub would_take_again_pct: Option<f32>,
}

/// Extended professor profile from per-teacher GraphQL node query.
#[derive(Debug, Clone)]
pub struct RmpProfessorDetail {
    pub legacy_id: i32,
    pub ratings_r1: Option<i32>,
    pub ratings_r2: Option<i32>,
    pub ratings_r3: Option<i32>,
    pub ratings_r4: Option<i32>,
    pub ratings_r5: Option<i32>,
    pub course_codes: Vec<RmpCourseCode>,
}

/// A course code entry from a professor's profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RmpCourseCode {
    pub course_name: String,
    pub course_count: i32,
}

/// A single review from RateMyProfessors.
#[derive(Debug, Clone)]
pub struct RmpReview {
    pub comment: Option<String>,
    pub class: Option<String>,
    pub grade: Option<String>,
    pub rating_tags: Vec<String>,
    pub helpful_rating: Option<f32>,
    pub clarity_rating: Option<f32>,
    pub difficulty_rating: Option<f32>,
    pub would_take_again: Option<i16>,
    pub is_for_credit: Option<bool>,
    pub is_for_online_class: Option<bool>,
    pub attendance_mandatory: Option<String>,
    pub flag_status: String,
    pub textbook_use: Option<i32>,
    pub thumbs_up_total: i32,
    pub thumbs_down_total: i32,
    pub posted_at: Option<DateTime<Utc>>,
}

/// Page size for review pagination.
const REVIEW_PAGE_SIZE: u32 = 20;

/// Client for fetching professor data from RateMyProfessors.
pub struct RmpClient {
    http: reqwest::Client,
}

impl Default for RmpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RmpClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Fetch all professors for UTSA via paginated GraphQL queries.
    pub async fn fetch_all_professors(&self) -> Result<Vec<RmpProfessor>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let after_clause = match &cursor {
                Some(c) => format!(r#", after: "{}""#, c),
                None => String::new(),
            };

            let query = format!(
                r#"query {{
  newSearch {{
    teachers(query: {{ text: "", schoolID: "{school_id}" }}, first: {page_size}{after}) {{
      edges {{
        cursor
        node {{
          id
          legacyId
          firstName
          lastName
          department
          avgRating
          avgDifficulty
          numRatings
          wouldTakeAgainPercent
        }}
      }}
      pageInfo {{
        hasNextPage
        endCursor
      }}
    }}
  }}
}}"#,
                school_id = UTSA_SCHOOL_ID,
                page_size = PAGE_SIZE,
                after = after_clause,
            );

            let body = serde_json::json!({ "query": query });

            let resp = self
                .http
                .post(GRAPHQL_URL)
                .header("Authorization", AUTH_HEADER)
                .json(&body)
                .send()
                .await?;

            let status = resp.status();
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("RMP GraphQL request failed ({status}): {text}");
            }

            let json: serde_json::Value = resp.json().await?;

            let teachers = &json["data"]["newSearch"]["teachers"];
            let edges = teachers["edges"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Missing edges in RMP response"))?;

            for edge in edges {
                let node = &edge["node"];
                let wta = node["wouldTakeAgainPercent"]
                    .as_f64()
                    .map(|v| v as f32)
                    .filter(|&v| v >= 0.0);

                all.push(RmpProfessor {
                    legacy_id: node["legacyId"]
                        .as_i64()
                        .ok_or_else(|| anyhow::anyhow!("Missing legacyId"))?
                        as i32,
                    graphql_id: node["id"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing id"))?
                        .to_string(),
                    first_name: node["firstName"].as_str().unwrap_or_default().to_string(),
                    last_name: node["lastName"].as_str().unwrap_or_default().to_string(),
                    department: node["department"].as_str().map(|s| s.to_string()),
                    avg_rating: node["avgRating"].as_f64().map(|v| v as f32),
                    avg_difficulty: node["avgDifficulty"].as_f64().map(|v| v as f32),
                    num_ratings: node["numRatings"].as_i64().unwrap_or(0) as i32,
                    would_take_again_pct: wta,
                });
            }

            let page_info = &teachers["pageInfo"];
            let has_next = page_info["hasNextPage"].as_bool().unwrap_or(false);

            if !has_next {
                break;
            }

            cursor = page_info["endCursor"].as_str().map(|s| s.to_string());

            tracing::trace!(fetched = all.len(), "RMP pagination: fetching next page");
        }

        info!(total = all.len(), "Fetched all RMP professors");
        Ok(all)
    }

    /// Send a GraphQL request with variables and return the parsed JSON.
    async fn graphql_request(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let resp = self
            .http
            .post(GRAPHQL_URL)
            .header("Authorization", AUTH_HEADER)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("RMP GraphQL request failed ({status}): {text}");
        }

        Ok(resp.json().await?)
    }

    /// Fetch extended profile data for a single professor.
    pub async fn fetch_professor_detail(&self, graphql_id: &str) -> Result<RmpProfessorDetail> {
        let query = r#"
            query TeacherRatingsPageQuery($id: ID!) {
              node(id: $id) {
                ... on Teacher {
                  id legacyId
                  ratingsDistribution { r1 r2 r3 r4 r5 }
                  courseCodes { courseName courseCount }
                }
              }
            }
        "#;

        let json = self
            .graphql_request(query, serde_json::json!({ "id": graphql_id }))
            .await?;

        let node = &json["data"]["node"];
        let dist = &node["ratingsDistribution"];
        let course_codes_arr = node["courseCodes"].as_array().cloned().unwrap_or_default();

        let course_codes: Vec<RmpCourseCode> = course_codes_arr
            .into_iter()
            .filter_map(|cc| {
                Some(RmpCourseCode {
                    course_name: cc["courseName"].as_str()?.to_string(),
                    course_count: cc["courseCount"].as_i64()? as i32,
                })
            })
            .collect();

        Ok(RmpProfessorDetail {
            legacy_id: node["legacyId"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("Missing legacyId in professor detail"))?
                as i32,
            ratings_r1: dist["r1"].as_i64().map(|v| v as i32),
            ratings_r2: dist["r2"].as_i64().map(|v| v as i32),
            ratings_r3: dist["r3"].as_i64().map(|v| v as i32),
            ratings_r4: dist["r4"].as_i64().map(|v| v as i32),
            ratings_r5: dist["r5"].as_i64().map(|v| v as i32),
            course_codes,
        })
    }

    /// Fetch all reviews for a professor, paginating through all pages.
    pub async fn fetch_professor_reviews(&self, graphql_id: &str) -> Result<Vec<RmpReview>> {
        let query = r#"
            query RatingsListQuery($id: ID!, $count: Int!, $cursor: String) {
              node(id: $id) {
                ... on Teacher {
                  ratings(first: $count, after: $cursor) {
                    edges {
                      cursor
                      node {
                        comment date class grade helpfulRating clarityRating
                        difficultyRating wouldTakeAgain isForCredit isForOnlineClass
                        attendanceMandatory ratingTags flagStatus textbookUse
                        thumbsUpTotal thumbsDownTotal
                      }
                    }
                    pageInfo { hasNextPage endCursor }
                  }
                }
              }
            }
        "#;

        let mut all = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let variables = serde_json::json!({
                "id": graphql_id,
                "count": REVIEW_PAGE_SIZE,
                "cursor": cursor,
            });

            let json = self.graphql_request(query, variables).await?;

            let ratings = &json["data"]["node"]["ratings"];
            let edges = ratings["edges"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Missing ratings edges in RMP response"))?;

            for edge in edges {
                let node = &edge["node"];
                let tags: Vec<String> = node["ratingTags"]
                    .as_str()
                    .map(|s| {
                        s.split("--")
                            .filter(|t| !t.is_empty())
                            .map(|t| t.to_string())
                            .collect()
                    })
                    .unwrap_or_default();

                let posted_at = node["date"]
                    .as_str()
                    .and_then(|s| s.parse::<DateTime<Utc>>().ok());

                let wta = node["wouldTakeAgain"].as_i64().map(|v| v as i16);

                all.push(RmpReview {
                    comment: node["comment"].as_str().map(|s| s.to_string()),
                    class: node["class"].as_str().map(|s| s.to_string()),
                    grade: node["grade"].as_str().map(|s| s.to_string()),
                    rating_tags: tags,
                    helpful_rating: node["helpfulRating"].as_f64().map(|v| v as f32),
                    clarity_rating: node["clarityRating"].as_f64().map(|v| v as f32),
                    difficulty_rating: node["difficultyRating"].as_f64().map(|v| v as f32),
                    would_take_again: wta,
                    is_for_credit: node["isForCredit"].as_bool(),
                    is_for_online_class: node["isForOnlineClass"].as_bool(),
                    attendance_mandatory: node["attendanceMandatory"]
                        .as_str()
                        .map(|s| s.to_string()),
                    flag_status: node["flagStatus"].as_str().unwrap_or("visible").to_string(),
                    textbook_use: node["textbookUse"].as_i64().map(|v| v as i32),
                    thumbs_up_total: node["thumbsUpTotal"].as_i64().unwrap_or(0) as i32,
                    thumbs_down_total: node["thumbsDownTotal"].as_i64().unwrap_or(0) as i32,
                    posted_at,
                });
            }

            let page_info = &ratings["pageInfo"];
            let has_next = page_info["hasNextPage"].as_bool().unwrap_or(false);

            if !has_next {
                break;
            }

            cursor = page_info["endCursor"].as_str().map(|s| s.to_string());
            trace!(
                fetched = all.len(),
                "RMP reviews pagination: fetching next page"
            );
        }

        Ok(all)
    }

    /// Fetch both extended profile and all reviews for a professor.
    pub async fn fetch_professor_with_reviews(
        &self,
        graphql_id: &str,
    ) -> Result<(RmpProfessorDetail, Vec<RmpReview>)> {
        let detail = self.fetch_professor_detail(graphql_id).await?;
        let reviews = self.fetch_professor_reviews(graphql_id).await?;
        Ok((detail, reviews))
    }
}
