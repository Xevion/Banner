pub mod adaptive;
pub mod jobs;
pub mod scheduler;
pub mod worker;

use crate::banner::BannerApi;
use crate::data::DbContext;
use crate::data::events::EventBuffer;
use crate::services::Service;
use crate::state::ReferenceCache;
use crate::state::{ServiceStatus, ServiceStatusRegistry};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use self::scheduler::Scheduler;
use self::worker::Worker;

/// The main service that will be managed by the application's `ServiceManager`.
///
/// It holds the shared resources (database pool, API client) and manages the
/// lifecycle of the Scheduler and Worker tasks.
pub struct ScraperService {
    db_pool: PgPool,
    banner_api: Arc<BannerApi>,
    reference_cache: Arc<RwLock<ReferenceCache>>,
    service_statuses: ServiceStatusRegistry,
    events: Arc<EventBuffer>,
    scheduler_handle: Option<JoinHandle<()>>,
    worker_handles: Vec<JoinHandle<()>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl ScraperService {
    /// Creates a new `ScraperService`.
    pub fn new(
        db_pool: PgPool,
        banner_api: Arc<BannerApi>,
        reference_cache: Arc<RwLock<ReferenceCache>>,
        service_statuses: ServiceStatusRegistry,
        events: Arc<EventBuffer>,
    ) -> Self {
        Self {
            db_pool,
            banner_api,
            reference_cache,
            service_statuses,
            events,
            scheduler_handle: None,
            worker_handles: Vec::new(),
            shutdown_tx: None,
        }
    }

    /// Starts the scheduler and a pool of workers.
    ///
    /// Force-unlocks any jobs left locked by a previous unclean shutdown before
    /// spawning workers, so those jobs re-enter the queue immediately.
    pub async fn start(&mut self) {
        let db = DbContext::new(self.db_pool.clone(), self.events.clone());

        // Recover jobs left locked by a previous crash/unclean shutdown
        match db.scrape_jobs().force_unlock_all().await {
            Ok(0) => {}
            Ok(count) => warn!(count, "Force-unlocked stale jobs from previous run"),
            Err(e) => warn!(error = ?e, "Failed to force-unlock stale jobs"),
        }

        info!("ScraperService starting");

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let scheduler = Scheduler::new(
            db.clone(),
            self.banner_api.clone(),
            self.reference_cache.clone(),
        );
        let shutdown_rx = shutdown_tx.subscribe();
        let scheduler_handle = tokio::spawn(async move {
            scheduler.run(shutdown_rx).await;
        });
        self.scheduler_handle = Some(scheduler_handle);
        info!("Scheduler task spawned");

        let worker_count = 4; // This could be configurable
        for i in 0..worker_count {
            let worker_db = DbContext::new(self.db_pool.clone(), self.events.clone());
            let worker = Worker::new(i, worker_db, self.banner_api.clone());
            let shutdown_rx = shutdown_tx.subscribe();
            let worker_handle = tokio::spawn(async move {
                worker.run(shutdown_rx).await;
            });
            self.worker_handles.push(worker_handle);
        }
        info!(
            worker_count = self.worker_handles.len(),
            "Spawned worker tasks"
        );
        self.service_statuses.set("scraper", ServiceStatus::Active);
    }
}

#[async_trait::async_trait]
impl Service for ScraperService {
    fn name(&self) -> &'static str {
        "scraper"
    }

    async fn run(&mut self) -> Result<(), anyhow::Error> {
        self.start().await;
        std::future::pending::<()>().await;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        self.service_statuses
            .set("scraper", ServiceStatus::Disabled);
        info!("Shutting down scraper service");

        // Send shutdown signal to all tasks
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        } else {
            warn!("No shutdown channel found for scraper service");
            return Err(anyhow::anyhow!("No shutdown channel available"));
        }

        // Collect all handles
        let mut all_handles = Vec::new();
        if let Some(handle) = self.scheduler_handle.take() {
            all_handles.push(handle);
        }
        all_handles.append(&mut self.worker_handles);

        // Wait for all tasks to complete (no internal timeout - let ServiceManager handle it)
        let results = futures::future::join_all(all_handles).await;
        let failed = results.iter().filter(|r| r.is_err()).count();
        if failed > 0 {
            warn!(
                failed_count = failed,
                "Some scraper tasks panicked during shutdown"
            );
            return Err(anyhow::anyhow!("{} task(s) panicked", failed));
        }

        info!("All scraper tasks shutdown gracefully");
        Ok(())
    }
}
