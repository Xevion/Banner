use crate::utils::fmt_duration;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

pub mod bot;
pub mod manager;
pub mod signals;
pub mod web;

#[derive(Debug)]
pub enum ServiceResult {
    GracefulShutdown,
    NormalCompletion,
    Error(anyhow::Error),
}

/// Common trait for all services in the application
#[async_trait::async_trait]
pub trait Service: Send + Sync {
    /// The name of the service for logging
    fn name(&self) -> &'static str;

    /// Run the service's main work loop.
    ///
    /// This method is expected to block indefinitely. A normal (non-error) return is
    /// treated as an unexpected completion by the ServiceManager (`ServiceResult::NormalCompletion`)
    /// and will be logged as a warning.
    ///
    /// Services that spawn tasks internally and then park should use
    /// `std::future::pending::<()>().await` to satisfy this contract.
    async fn run(&mut self) -> Result<(), anyhow::Error>;

    /// Gracefully shutdown the service
    ///
    /// Implementations should initiate shutdown and MAY wait for completion.
    /// Services are expected to respond to this call and begin cleanup promptly.
    /// When managed by ServiceManager, the configured timeout (default 8s) applies to
    /// ALL services combined, not per-service. Services should complete shutdown as
    /// quickly as possible to avoid timeout.
    async fn shutdown(&mut self) -> Result<(), anyhow::Error>;
}

/// Generic service runner that handles the lifecycle
pub async fn run_service(
    mut service: Box<dyn Service>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> ServiceResult {
    let name = service.name();
    info!(service = name, "service started");

    let work = async {
        match service.run().await {
            Ok(()) => {
                warn!(service = name, "service completed unexpectedly");
                ServiceResult::NormalCompletion
            }
            Err(e) => {
                error!(service = name, "service failed: {e}");
                ServiceResult::Error(e)
            }
        }
    };

    tokio::select! {
        result = work => result,
        _ = shutdown_rx.recv() => {
            info!(service = name, "shutting down...");
            let start_time = std::time::Instant::now();

            match service.shutdown().await {
                Ok(()) => {
                    let elapsed = start_time.elapsed();
                    info!(service = name, elapsed = fmt_duration(elapsed), "shutdown completed");
                    ServiceResult::GracefulShutdown
                }
                Err(e) => {
                    let elapsed = start_time.elapsed();
                    error!(service = name, elapsed = fmt_duration(elapsed), error = ?e, "shutdown failed");
                    ServiceResult::Error(e)
                }
            }
        }
    }
}
