use anyhow::Result;
use banner::banner::{BannerApi, SearchQuery, Term};
use banner::config::Config;
use figment::{Figment, providers::Env};
use futures::future;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<()> {
    // Configure logging
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,banner=trace,reqwest=debug,hyper=info"));
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_target(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting Banner search test");

    dotenvy::dotenv().ok();

    // Load configuration
    let config: Config = Figment::new()
        .merge(Env::raw())
        .extract()
        .expect("Failed to load config");

    info!(
        banner_base_url = config.banner_base_url,
        "Configuration loaded"
    );

    // Create Banner API client
    let banner_api = BannerApi::new_with_config(config.banner_base_url, config.rate_limiting)
        .expect("Failed to create BannerApi");

    // Get current term
    let term = Term::get_current().inner().to_string();
    info!(term = term, "Using current term");

    // Define multiple search queries
    let queries = vec![
        (
            "CS Courses",
            SearchQuery::new().subject("CS").max_results(10),
        ),
        (
            "Math Courses",
            SearchQuery::new().subject("MAT").max_results(10),
        ),
        (
            "3000-level CS",
            SearchQuery::new()
                .subject("CS")
                .course_numbers(3000, 3999)
                .max_results(8),
        ),
        (
            "High Credit Courses",
            SearchQuery::new().credits(4, 6).max_results(8),
        ),
        (
            "Programming Courses",
            SearchQuery::new().keyword("programming").max_results(6),
        ),
    ];

    info!(query_count = queries.len(), "Executing concurrent searches");

    // Execute all searches concurrently
    let search_futures = queries.into_iter().map(|(label, query)| {
        info!(label = %label, "Starting search");
        let banner_api = &banner_api;
        let term = &term;
        async move {
            let result = banner_api
                .search(term, &query, "subjectDescription", false)
                .await;
            (label, result)
        }
    });

    // Wait for all searches to complete
    let search_results = future::join_all(search_futures)
        .await
        .into_iter()
        .filter_map(|(label, result)| match result {
            Ok(search_result) => {
                info!(
                    label = label,
                    success = search_result.success,
                    total_count = search_result.total_count,
                    "Search completed successfully"
                );
                Some((label, search_result))
            }
            Err(e) => {
                error!(label = label, error = ?e, "Search failed");
                None
            }
        })
        .collect::<Vec<_>>();

    // Process and display results
    for (label, search_result) in search_results {
        println!("\n=== {} ===", label);
        if let Some(courses) = &search_result.data {
            if courses.is_empty() {
                println!("  No courses found");
            } else {
                println!("  Found {} courses:", courses.len());
                for course in courses {
                    println!(
                        "    {} {} - {} (CRN: {})",
                        course.subject,
                        course.course_number,
                        course.course_title,
                        course.course_reference_number
                    );
                }
            }
        } else {
            println!("  No courses found");
        }
    }

    info!("Search test completed");
    Ok(())
}
