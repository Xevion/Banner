use crate::services::ServiceResult;
use crate::services::manager::ServiceManager;
use crate::utils::fmt_duration;
use std::process::ExitCode;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info, warn};

/// Handle application shutdown signals and graceful shutdown
pub async fn handle_shutdown_signals(
    mut service_manager: ServiceManager,
    shutdown_timeout: Duration,
) -> ExitCode {
    // Set up signal handling for both SIGINT (Ctrl+C) and SIGTERM
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("received ctrl+c, gracefully shutting down...");
    };

    #[cfg(unix)]
    let sigterm = async {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm_stream =
            signal(SignalKind::terminate()).expect("Failed to install SIGTERM signal handler");
        sigterm_stream.recv().await;
        info!("received SIGTERM, gracefully shutting down...");
    };

    #[cfg(not(unix))]
    let sigterm = async {
        // On non-Unix systems, create a future that never completes
        // This ensures the select! macro works correctly
        std::future::pending::<()>().await;
    };

    // Main application loop - wait for services or signals
    let mut exit_code = ExitCode::SUCCESS;

    tokio::select! {
        (service_name, result) = service_manager.run() => {
            // A service completed unexpectedly
            match result {
                ServiceResult::GracefulShutdown => {
                    info!(service = service_name, "service completed gracefully");
                }
                ServiceResult::NormalCompletion => {
                    warn!(service = service_name, "service completed unexpectedly");
                    exit_code = ExitCode::FAILURE;
                }
                ServiceResult::Error(e) => {
                    error!(service = service_name, error = ?e, "service failed");
                    exit_code = ExitCode::FAILURE;
                }
            }

            // Shutdown remaining services
            exit_code = handle_graceful_shutdown(service_manager, shutdown_timeout, exit_code).await;
        }
        _ = ctrl_c => {
            // User requested shutdown via Ctrl+C
            info!("user requested shutdown via ctrl+c");
            exit_code = handle_graceful_shutdown(service_manager, shutdown_timeout, ExitCode::SUCCESS).await;
        }
        _ = sigterm => {
            // System requested shutdown via SIGTERM
            info!("system requested shutdown via SIGTERM");
            exit_code = handle_graceful_shutdown(service_manager, shutdown_timeout, ExitCode::SUCCESS).await;
        }
    }

    info!(exit_code = ?exit_code, "application shutdown complete");
    exit_code
}

/// Handle graceful shutdown of remaining services
async fn handle_graceful_shutdown(
    mut service_manager: ServiceManager,
    shutdown_timeout: Duration,
    current_exit_code: ExitCode,
) -> ExitCode {
    match service_manager.shutdown(shutdown_timeout).await {
        Ok(elapsed) => {
            info!(
                remaining = fmt_duration(shutdown_timeout - elapsed),
                "graceful shutdown complete"
            );
            current_exit_code
        }
        Err(pending_services) => {
            warn!(
                pending_count = pending_services.len(),
                pending_services = ?pending_services,
                "graceful shutdown elapsed - {} service(s) did not complete",
                pending_services.len()
            );

            // Non-zero exit code, default to FAILURE if not set
            if current_exit_code == ExitCode::SUCCESS {
                ExitCode::FAILURE
            } else {
                current_exit_code
            }
        }
    }
}
