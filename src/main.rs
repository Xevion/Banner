use crate::app::App;
use crate::cli::{Args, ServiceName};
use crate::logging::setup_logging;
use clap::Parser;
use std::process::ExitCode;
use tracing::info;

mod app;
mod banner;
mod bluebook;
mod bot;
mod calendar;
mod cli;
mod config;
mod data;
mod fmt;
mod logging;
mod rmp;
mod scraper;
mod services;
mod state;
mod utils;
mod web;

#[tokio::main]
async fn main() -> ExitCode {
    dotenvy::dotenv().ok();

    // Parse CLI arguments
    let args = Args::parse();

    // Always run all services
    let enabled_services = ServiceName::all();

    // Load config and setup logging before App::new() so startup logs are never silently dropped
    let early_config = {
        use figment::providers::Env;
        use figment::value::UncasedStr;
        figment::Figment::new()
            .merge(Env::raw().map(|k| {
                if k == UncasedStr::new("RAILWAY_DEPLOYMENT_DRAINING_SECONDS") {
                    "SHUTDOWN_TIMEOUT".into()
                } else {
                    k.into()
                }
            }))
            .extract::<crate::config::Config>()
            .expect("Failed to load config for logging setup")
    };
    setup_logging(&early_config, args.tracing);

    // Create and initialize the application
    let mut app = App::new().await.expect("Failed to initialize application");

    info!(
        enabled_services = ?enabled_services,
        "services configuration loaded"
    );

    // Log application startup context
    info!(
        version = env!("CARGO_PKG_VERSION"),
        environment = if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        },
        "starting banner"
    );

    // Setup services (web, scraper)
    app.setup_services(&enabled_services)
        .expect("Failed to setup services");

    // Setup bot service if enabled
    if enabled_services.contains(&ServiceName::Bot) {
        app.setup_bot_service()
            .expect("Failed to setup bot service");
    }

    // Start all services and run the application
    app.start_services();
    app.run().await
}
