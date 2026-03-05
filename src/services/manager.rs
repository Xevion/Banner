use crate::utils::fmt_duration;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::{info, trace, warn};

use crate::services::{Service, ServiceResult, run_service};

/// Manages multiple services and their lifecycle
pub struct ServiceManager {
    registered_services: HashMap<String, Box<dyn Service>>,
    service_handles: HashMap<String, tokio::task::AbortHandle>,
    completion_rx: Option<mpsc::UnboundedReceiver<(String, ServiceResult)>>,
    completion_tx: mpsc::UnboundedSender<(String, ServiceResult)>,
    shutdown_tx: broadcast::Sender<()>,
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceManager {
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let (completion_tx, completion_rx) = mpsc::unbounded_channel();

        Self {
            registered_services: HashMap::new(),
            service_handles: HashMap::new(),
            completion_rx: Some(completion_rx),
            completion_tx,
            shutdown_tx,
        }
    }

    /// Register a service to be managed (not yet spawned)
    pub fn register_service(&mut self, name: &str, service: Box<dyn Service>) {
        self.registered_services.insert(name.to_string(), service);
    }

    /// Check if there are any registered services
    pub fn has_services(&self) -> bool {
        !self.registered_services.is_empty()
    }

    /// Spawn all registered services
    pub fn spawn_all(&mut self) {
        let service_count = self.registered_services.len();
        let service_names: Vec<_> = self.registered_services.keys().cloned().collect();

        for (name, service) in self.registered_services.drain() {
            let shutdown_rx = self.shutdown_tx.subscribe();
            let completion_tx = self.completion_tx.clone();
            let name_clone = name.clone();

            // Spawn service task
            let handle = tokio::spawn(async move {
                let result = run_service(service, shutdown_rx).await;
                // Send completion notification
                let _ = completion_tx.send((name_clone, result));
            });

            // Store abort handle for shutdown control
            self.service_handles
                .insert(name.clone(), handle.abort_handle());
        }

        info!(
            service_count,
            services = ?service_names,
            "spawned {} services",
            service_count
        );
    }

    /// Run all services until one completes or fails
    /// Returns the first service that completes and its result
    pub async fn run(&mut self) -> (String, ServiceResult) {
        if self.service_handles.is_empty() {
            return (
                "none".to_string(),
                ServiceResult::Error(anyhow::anyhow!("No services to run")),
            );
        }

        info!(
            "service manager running {} services",
            self.service_handles.len()
        );

        // Wait for any service to complete via the channel
        let completion_rx = self
            .completion_rx
            .as_mut()
            .expect("completion_rx should be available");

        completion_rx
            .recv()
            .await
            .map(|(name, result)| {
                self.service_handles.remove(&name);
                (name, result)
            })
            .unwrap_or_else(|| {
                (
                    "channel_closed".to_string(),
                    ServiceResult::Error(anyhow::anyhow!("Completion channel closed")),
                )
            })
    }

    /// Shutdown all services gracefully with a timeout.
    ///
    /// All services receive the shutdown signal simultaneously and shut down in parallel.
    /// Each service gets the full timeout duration (they don't share/consume from a budget).
    /// If any service fails to shutdown within the timeout, it will be aborted.
    ///
    /// Returns the elapsed time if all succeed, or a list of failed service names.
    pub async fn shutdown(&mut self, timeout: Duration) -> Result<Duration, Vec<String>> {
        let service_count = self.service_handles.len();
        let service_names: Vec<_> = self.service_handles.keys().cloned().collect();

        info!(
            service_count,
            services = ?service_names,
            timeout = fmt_duration(timeout),
            "shutting down {} services in parallel with {:?} timeout each",
            service_count,
            timeout
        );

        if service_count == 0 {
            return Ok(Duration::ZERO);
        }

        // Send shutdown signal to all services simultaneously
        let _ = self.shutdown_tx.send(());

        let start_time = std::time::Instant::now();

        // Collect results from all services with timeout
        let completion_rx = self
            .completion_rx
            .as_mut()
            .expect("completion_rx should be available");

        // Collect all completion results with a single timeout
        let collect_future = async {
            let mut collected: Vec<Option<(String, ServiceResult)>> = Vec::new();
            for _ in 0..service_count {
                if let Some(result) = completion_rx.recv().await {
                    collected.push(Some(result));
                } else {
                    collected.push(None);
                }
            }
            collected
        };

        let results = match tokio::time::timeout(timeout, collect_future).await {
            Ok(results) => results,
            Err(_) => {
                // Timeout exceeded - abort all remaining services
                warn!(
                    timeout = fmt_duration(timeout),
                    "shutdown timeout exceeded - aborting all remaining services"
                );

                let failed: Vec<String> = self.service_handles.keys().cloned().collect();
                for handle in self.service_handles.values() {
                    handle.abort();
                }
                self.service_handles.clear();

                return Err(failed);
            }
        };

        // Process results and identify failures
        let mut failed_services = Vec::new();
        for (name, service_result) in results.into_iter().flatten() {
            self.service_handles.remove(&name);

            if matches!(service_result, ServiceResult::GracefulShutdown) {
                trace!(service = name, "service shutdown completed");
            } else {
                warn!(
                    service = name,
                    result = ?service_result,
                    "service shutdown with non-graceful result"
                );
                failed_services.push(name);
            }
        }

        let elapsed = start_time.elapsed();

        if failed_services.is_empty() {
            info!(
                service_count,
                elapsed = fmt_duration(elapsed),
                "all services shutdown successfully: {}",
                service_names.join(", ")
            );
            Ok(elapsed)
        } else {
            warn!(
                failed_count = failed_services.len(),
                failed_services = ?failed_services,
                elapsed = fmt_duration(elapsed),
                "{} service(s) failed to shutdown gracefully: {}",
                failed_services.len(),
                failed_services.join(", ")
            );
            Err(failed_services)
        }
    }
}
