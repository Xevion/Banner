use crate::banner::BannerApi;
use crate::cli::ServiceName;
use crate::config::Config;
use crate::scraper::ScraperService;
use crate::scraper::scheduler::KV_TERM_SYNC;
use crate::services::bot::BotService;
use crate::services::manager::ServiceManager;
use crate::services::web::WebService;
use crate::state::AppState;
use crate::utils::fmt_duration;
use crate::web::auth::AuthConfig;
use anyhow::Context;
use chrono::Utc;
use figment::value::UncasedStr;
use figment::{Figment, providers::Env};
use sqlx::ConnectOptions;
use sqlx::postgres::PgPoolOptions;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tracing::{error, info, warn};

/// Main application struct containing all necessary components
pub struct App {
    config: Config,
    db_pool: sqlx::PgPool,
    banner_api: Arc<BannerApi>,
    app_state: AppState,
    service_manager: ServiceManager,
}

impl App {
    /// Create a new App instance with all necessary components initialized
    pub async fn new() -> Result<Self, anyhow::Error> {
        // Load configuration
        let config: Config = Figment::new()
            .merge(Env::raw().map(|k| {
                if k == UncasedStr::new("RAILWAY_DEPLOYMENT_DRAINING_SECONDS") {
                    "SHUTDOWN_TIMEOUT".into()
                } else {
                    k.into()
                }
            }))
            .extract()
            .context("Failed to load config")?;

        // Check if the database URL is via private networking
        let is_private = config.database_url.contains("railway.internal");
        let slow_threshold = Duration::from_millis(if is_private { 200 } else { 500 });

        // Create database connection pool
        let connect_options = sqlx::postgres::PgConnectOptions::from_str(&config.database_url)
            .context("Failed to parse database URL")?
            .log_statements(tracing::log::LevelFilter::Debug)
            .log_slow_statements(tracing::log::LevelFilter::Warn, Duration::from_secs(1));

        let db_pool = PgPoolOptions::new()
            .min_connections(0)
            .max_connections(4)
            .acquire_slow_threshold(slow_threshold)
            .acquire_timeout(Duration::from_secs(4))
            .idle_timeout(Duration::from_secs(60 * 2))
            .max_lifetime(Duration::from_secs(60 * 30))
            .connect_with(connect_options)
            .await
            .context("Failed to create database pool")?;

        info!(
            is_private = is_private,
            min_connections = 0,
            max_connections = 4,
            acquire_timeout = "4s",
            idle_timeout = "2m",
            max_lifetime = "30m",
            acquire_slow_threshold = fmt_duration(slow_threshold),
            "database pool established"
        );

        // Run database migrations
        info!("Running database migrations...");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .context("Failed to run database migrations")?;
        info!("Database migrations completed successfully");

        // Create BannerApi early so we can use it for term sync
        let banner_api = BannerApi::new_with_config(
            config.banner_base_url.clone(),
            config.rate_limiting.clone(),
        )
        .context("Failed to create BannerApi")?;
        let banner_api_arc = Arc::new(banner_api);

        // Sync terms from Banner API (non-fatal if fails).
        // Persist the timestamp so the scheduler doesn't repeat this on its first cycle.
        match Self::sync_terms_on_startup(&db_pool, &banner_api_arc).await {
            Ok(result) => {
                info!(
                    inserted = result.inserted,
                    updated = result.updated,
                    "Term sync completed"
                );
                if let Err(e) =
                    crate::data::kv::set_timestamp(&db_pool, KV_TERM_SYNC, Utc::now()).await
                {
                    warn!(error = ?e, "Failed to persist term sync timestamp");
                }
            }
            Err(e) => {
                // Non-fatal: app can start without terms, scheduler will retry
                warn!(error = ?e, "Failed to sync terms on startup (non-fatal)");
            }
        }

        // Backfill structured name columns for existing instructors
        if let Err(e) = crate::data::names::backfill_instructor_names(&db_pool).await {
            warn!(error = ?e, "Failed to backfill instructor names (non-fatal)");
        }

        // Backfill URL slugs for instructors that don't have one
        match crate::data::instructors::backfill_instructor_slugs(&db_pool).await {
            Ok(0) => {}
            Ok(n) => info!(count = n, "Backfilled instructor slugs"),
            Err(e) => warn!(error = ?e, "Failed to backfill instructor slugs (non-fatal)"),
        }

        // Compute instructor scores from RMP + BlueBook data
        match crate::data::scoring::recompute_all_scores(&db_pool).await {
            Ok(0) => info!("Computed instructor scores (none found - no RMP or BlueBook data)"),
            Ok(n) => info!(count = n, "Computed instructor scores"),
            Err(e) => {
                error!(error = ?e, "Failed to compute instructor scores");
                return Err(e.context("Failed to compute instructor scores on startup"));
            }
        }

        // Create shared BlueBook sync notify and force flag for manual trigger from admin endpoints
        let bluebook_sync_notify = Arc::new(tokio::sync::Notify::new());
        let bluebook_force_flag = Arc::new(AtomicBool::new(false));

        // Create AppState (BannerApi already created above for term sync)
        let app_state = AppState::new(
            banner_api_arc.clone(),
            db_pool.clone(),
            config.ssr_downstream.clone(),
            bluebook_sync_notify,
            bluebook_force_flag.clone(),
            config.public_origin.clone(),
        );

        // Load reference data cache from DB (may be empty on first run)
        if let Err(e) = app_state.load_reference_cache().await {
            info!(error = ?e, "Could not load reference cache on startup (may be empty)");
        }

        // Spawn background reference cache refresh every 30 minutes
        app_state.spawn_reference_cache_refresh(std::time::Duration::from_secs(30 * 60));

        // Load schedule cache for timeline enrollment queries
        if let Err(e) = app_state.schedule_cache.load().await {
            info!(error = ?e, "Could not load schedule cache on startup (may be empty)");
        }

        // Seed the initial admin user if configured
        if let Some(admin_id) = config.admin_discord_id {
            let user = crate::data::users::ensure_seed_admin(&db_pool, admin_id as i64)
                .await
                .context("Failed to seed admin user")?;
            info!(discord_id = admin_id, username = %user.discord_username, "Seed admin ensured");

            #[cfg(debug_assertions)]
            {
                app_state
                    .session_cache
                    .inject_dev_session("dev-admin", user);
                info!("Dev auth bypass active -- use: Cookie: session=dev-admin");
            }
        }

        Ok(App {
            config,
            db_pool,
            banner_api: banner_api_arc,
            app_state,
            service_manager: ServiceManager::new(),
        })
    }

    /// Setup and register services based on enabled service list
    pub fn setup_services(&mut self, services: &[ServiceName]) -> Result<(), anyhow::Error> {
        // Register enabled services with the manager
        if services.contains(&ServiceName::Web) {
            let auth_config = AuthConfig {
                client_id: self.config.discord_client_id.clone(),
                client_secret: self.config.discord_client_secret.clone(),
                redirect_base: self.config.discord_redirect_uri.clone(),
            };
            let web_service = Box::new(WebService::new(
                self.config.port,
                self.app_state.clone(),
                auth_config,
            ));
            self.service_manager
                .register_service(ServiceName::Web.as_str(), web_service);
        }

        if services.contains(&ServiceName::Scraper) {
            let scraper_service = Box::new(ScraperService::new(
                self.db_pool.clone(),
                self.banner_api.clone(),
                self.app_state.reference_cache.clone(),
                self.app_state.service_statuses.clone(),
                self.app_state.events.clone(),
                self.app_state.bluebook_sync_notify.clone(),
                self.app_state.bluebook_force_flag.clone(),
            ));
            self.service_manager
                .register_service(ServiceName::Scraper.as_str(), scraper_service);
        }

        // Check if any services are enabled
        if !self.service_manager.has_services() && !services.contains(&ServiceName::Bot) {
            error!("No services enabled. Cannot start application.");
            return Err(anyhow::anyhow!("No services enabled"));
        }

        Ok(())
    }

    /// Setup bot service if enabled
    pub async fn setup_bot_service(&mut self) -> Result<(), anyhow::Error> {
        use std::sync::Arc;
        use tokio::sync::{Mutex, broadcast};

        // Create shutdown channel for status update task
        let (status_shutdown_tx, status_shutdown_rx) = broadcast::channel(1);
        let status_task_handle = Arc::new(Mutex::new(None));

        let client = BotService::create_client(
            &self.config,
            self.app_state.clone(),
            status_task_handle.clone(),
            status_shutdown_rx,
        )
        .await
        .context("Failed to create Discord client")?;

        let bot_service = Box::new(BotService::new(
            client,
            status_task_handle,
            status_shutdown_tx,
            self.app_state.service_statuses.clone(),
        ));

        self.service_manager
            .register_service(ServiceName::Bot.as_str(), bot_service);
        Ok(())
    }

    /// Start all registered services
    pub fn start_services(&mut self) {
        self.service_manager.spawn_all();
    }

    /// Run the application and handle shutdown signals
    pub async fn run(self) -> ExitCode {
        use crate::services::signals::handle_shutdown_signals;
        handle_shutdown_signals(self.service_manager, self.config.shutdown_timeout).await
    }

    /// Sync terms from Banner API on startup.
    ///
    /// This is non-fatal - if it fails, the scheduler will retry periodically.
    async fn sync_terms_on_startup(
        db_pool: &sqlx::PgPool,
        banner_api: &Arc<BannerApi>,
    ) -> Result<crate::data::terms::SyncResult, anyhow::Error> {
        let banner_terms = banner_api
            .get_terms("", 1, 500)
            .await
            .context("Failed to fetch terms from Banner API")?;

        crate::data::terms::sync_terms_from_banner(db_pool, banner_terms)
            .await
            .context("Failed to sync terms to database")
    }
}
