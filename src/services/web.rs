use super::Service;
use crate::state::AppState;
use crate::state::ServiceStatus;
use crate::web::auth::AuthConfig;
use crate::web::create_router;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, trace, warn};

/// Web server service implementation
pub struct WebService {
    port: u16,
    app_state: AppState,
    auth_config: AuthConfig,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl WebService {
    pub fn new(port: u16, app_state: AppState, auth_config: AuthConfig) -> Self {
        Self {
            port,
            app_state,
            auth_config,
            shutdown_tx: None,
        }
    }
    /// Periodically pings the database and updates the "database" service status.
    async fn db_health_check_loop(state: AppState, mut shutdown_rx: broadcast::Receiver<()>) {
        use std::time::Duration;
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let status = match crate::data::health::ping(&state.db_pool).await {
                        Ok(()) => ServiceStatus::Connected,
                        Err(e) => {
                            warn!(error = %e, "DB health check failed");
                            ServiceStatus::Error
                        }
                    };
                    state.service_statuses.set("database", status);
                }
                _ = shutdown_rx.recv() => {
                    break;
                }
            }
        }
    }

    /// Periodically cleans up expired sessions from the database and in-memory cache.
    async fn session_cleanup_loop(state: AppState, mut shutdown_rx: broadcast::Receiver<()>) {
        use std::time::Duration;
        // Run every hour
        let mut interval = tokio::time::interval(Duration::from_secs(3600));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match state.session_cache.cleanup_expired().await {
                        Ok(deleted) => {
                            if deleted > 0 {
                                info!(deleted, "cleaned up expired sessions");
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "session cleanup failed");
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    break;
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl Service for WebService {
    fn name(&self) -> &'static str {
        "web"
    }

    async fn run(&mut self) -> Result<(), anyhow::Error> {
        // Create the main router with Banner API routes
        let app = create_router(self.app_state.clone(), self.auth_config.clone());

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));

        let listener = TcpListener::bind(addr).await?;
        self.app_state
            .service_statuses
            .set("web", ServiceStatus::Active);
        info!(
            service = "web",
            address = %addr,
            link = format!("http://localhost:{}", addr.port()),
            "web server listening"
        );

        // Create internal shutdown channel for axum graceful shutdown
        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // Spawn background DB health check
        let health_state = self.app_state.clone();
        let health_shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            Self::db_health_check_loop(health_state, health_shutdown_rx).await;
        });

        // Spawn session cleanup task
        let cleanup_state = self.app_state.clone();
        let cleanup_shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            Self::session_cleanup_loop(cleanup_state, cleanup_shutdown_rx).await;
        });

        // Use axum's graceful shutdown with the internal shutdown signal.
        // `into_make_service_with_connect_info` makes `ConnectInfo<SocketAddr>`
        // available to handlers (used by the SSR proxy for x-forwarded-for).
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
            trace!(
                service = "web",
                "received shutdown signal, starting graceful shutdown"
            );
        })
        .await?;

        trace!(service = "web", "graceful shutdown completed");
        info!(service = "web", "web server stopped");

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
            trace!(service = "web", "sent shutdown signal to axum");
        } else {
            warn!(
                service = "web",
                "no shutdown channel found, cannot trigger graceful shutdown"
            );
        }
        Ok(())
    }
}
