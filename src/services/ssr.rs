//! Supervises the SvelteKit SSR server as a child process.
//!
//! In production the Rust binary is PID 1 and owns the Node process that renders
//! pages. Nothing is spawned when `ssr_command` is unset, which is the case in
//! development where Vite serves SSR on the same port.

use super::Service;
use anyhow::{Context, Result, anyhow};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::broadcast;
use tracing::{info, trace, warn};

/// How long the SSR process gets to exit after SIGTERM before it is killed.
const TERM_GRACE: Duration = Duration::from_secs(5);

pub struct SsrService {
    command: String,
    port: u16,
    backend_url: String,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl SsrService {
    pub fn new(command: String, port: u16, backend_url: String) -> Self {
        Self {
            command,
            port,
            backend_url,
            shutdown_tx: None,
        }
    }

    fn spawn(&self) -> Result<Child> {
        let mut parts = self.command.split_whitespace();
        let program = parts
            .next()
            .ok_or_else(|| anyhow!("SSR_COMMAND is set but empty"))?;
        let args: Vec<&str> = parts.collect();

        // Bind all interfaces rather than loopback: the proxy target resolves
        // through localhost, which may pick either IPv4 or IPv6.
        Command::new(program)
            .args(&args)
            .env("PORT", self.port.to_string())
            .env("HOST", "0.0.0.0")
            .env("BACKEND_URL", &self.backend_url)
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("failed to spawn SSR process: {}", self.command))
    }

    /// Ask the child to exit, escalating to SIGKILL if it ignores SIGTERM.
    ///
    /// adapter-node drains in-flight renders on SIGTERM, so the polite signal is
    /// worth the extra round trip.
    async fn terminate(child: &mut Child) {
        let Some(pid) = child.id() else {
            return;
        };

        // SAFETY: `pid` came from a live child we own, and SIGTERM carries no
        // pointer arguments.
        let sent = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        if sent != 0 {
            warn!(pid, "failed to signal SSR process, killing it");
            let _ = child.kill().await;
            return;
        }

        match tokio::time::timeout(TERM_GRACE, child.wait()).await {
            Ok(_) => trace!(pid, "SSR process exited after SIGTERM"),
            Err(_) => {
                warn!(pid, "SSR process ignored SIGTERM, killing it");
                let _ = child.kill().await;
            }
        }
    }
}

#[async_trait::async_trait]
impl Service for SsrService {
    fn name(&self) -> &'static str {
        "ssr"
    }

    async fn run(&mut self) -> Result<(), anyhow::Error> {
        let mut child = self.spawn()?;
        info!(
            service = "ssr",
            port = self.port,
            command = %self.command,
            "SSR process started"
        );

        let (shutdown_tx, mut shutdown_rx) = broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        tokio::select! {
            status = child.wait() => {
                // The SSR process is not restarted in place; exiting lets the
                // platform replace the whole container.
                let code = status.ok().and_then(|s| s.code());
                Err(anyhow!("SSR process exited unexpectedly (code {code:?})"))
            }
            _ = shutdown_rx.recv() => {
                Self::terminate(&mut child).await;
                info!(service = "ssr", "SSR process stopped");
                Ok(())
            }
        }
    }

    async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        } else {
            warn!(
                service = "ssr",
                "no shutdown channel, SSR may be killed abruptly"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert2::check;

    fn service(command: &str) -> SsrService {
        SsrService::new(
            command.to_string(),
            3001,
            "http://localhost:8080".to_string(),
        )
    }

    #[tokio::test]
    async fn blank_command_is_rejected() {
        check!(service("   ").spawn().is_err());
    }

    #[tokio::test]
    async fn run_reports_an_unexpected_exit() {
        // `true` exits immediately, standing in for an SSR process that dies.
        let mut svc = service("true");
        let result = svc.run().await;
        check!(result.is_err());
        check!(
            result
                .unwrap_err()
                .to_string()
                .contains("exited unexpectedly")
        );
    }

    #[tokio::test]
    async fn terminate_stops_a_running_process() {
        let mut child = service("sleep 30").spawn().expect("spawn should succeed");
        check!(child.id().is_some());

        SsrService::terminate(&mut child).await;

        let waited = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        check!(waited.is_ok(), "child should have been reaped");
    }
}
