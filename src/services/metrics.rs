use super::Service;
use crate::telemetry;
use axum::{Router, http::StatusCode, routing::get};
use sqlx::PgPool;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, trace, warn};

/// Under a typical 15s scrape interval, so a scrape never reads a stale gauge.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);

/// Serves Prometheus exposition on its own listener, which the chart keeps out of the Service.
pub struct MetricsService {
    port: u16,
    db_pool: PgPool,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl MetricsService {
    pub fn new(port: u16, db_pool: PgPool) -> Self {
        Self {
            port,
            db_pool,
            shutdown_tx: None,
        }
    }

    /// Refreshes the pool gauges. `render` drains histograms itself, so no upkeep tick is needed.
    async fn sample_loop(db_pool: PgPool, mut shutdown_rx: broadcast::Receiver<()>) {
        let mut ticker = tokio::time::interval(SAMPLE_INTERVAL);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    telemetry::sample_pool(&db_pool);
                    telemetry::process::sample();
                }
                _ = shutdown_rx.recv() => { break; }
            }
        }
    }
}

/// Renders the exposition off the runtime workers.
///
/// Cost is linear in series count (~2us each), so on a busy registry this would otherwise stall a
/// worker for the whole render. A join failure returns 503 rather than an empty body, which would
/// read as "the app stopped emitting" instead of "the scrape failed".
async fn render_metrics() -> Result<String, StatusCode> {
    tokio::task::spawn_blocking(|| telemetry::recorder().render())
        .await
        .map_err(|e| {
            warn!(error = %e, "metrics render task failed");
            StatusCode::SERVICE_UNAVAILABLE
        })
}

#[async_trait::async_trait]
impl Service for MetricsService {
    fn name(&self) -> &'static str {
        "metrics"
    }

    async fn run(&mut self) -> Result<(), anyhow::Error> {
        let app = Router::new().route("/metrics", get(render_metrics));

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = TcpListener::bind(addr).await?;
        info!(
            service = "metrics",
            address = %addr,
            link = format!("http://localhost:{}/metrics", addr.port()),
            "metrics server listening"
        );

        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let sample_pool = self.db_pool.clone();
        let sample_shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            Self::sample_loop(sample_pool, sample_shutdown_rx).await;
        });

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.recv().await;
                trace!(service = "metrics", "received shutdown signal");
            })
            .await?;

        info!(service = "metrics", "metrics server stopped");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        } else {
            warn!(
                service = "metrics",
                "no shutdown channel found, cannot trigger graceful shutdown"
            );
        }
        Ok(())
    }
}
