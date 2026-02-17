//! RateMyProfessors GraphQL client for bulk professor data sync.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

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
}
