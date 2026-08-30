pub mod formatter;

use crate::cli::TracingFormat;
use crate::config::Config;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt::format::JsonFields};

/// Build the filter used when `RUST_LOG` is unset.
///
/// `access` carries one event per served request, on its own target so that silencing routine
/// traffic (`access=off`) does not also silence application INFO.
fn default_filter(base_level: &str) -> String {
    format!(
        "warn,banner={base_level},banner::banner::middleware=warn,banner::banner::session=warn,access=info"
    )
}

/// Configure and initialize logging for the application.
pub fn setup_logging(config: &Config, tracing_format: TracingFormat) {
    // Configure logging based on config.
    // Module paths use `banner::banner::` because the crate (`banner`) contains
    // a `banner` submodule for the Banner API client.
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_filter(&config.log_level)));

    let use_pretty = match tracing_format {
        TracingFormat::Pretty => true,
        TracingFormat::Json => false,
    };

    if use_pretty {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .event_format(formatter::CustomPrettyFormatter)
                    .fmt_fields(formatter::compact_fields()),
            )
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .event_format(formatter::CustomJsonFormatter)
                    .fmt_fields(JsonFields::new()),
            )
            .init();
    }
}

#[cfg(test)]
mod tests {
    use super::default_filter;
    use assert2::check;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;

    /// Collects formatted events so a test can assert what the filter actually let through.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl Capture {
        fn contents(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl std::io::Write for Capture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Run `f` with a subscriber built from the default filter, returning what it emitted.
    fn capture_with_default_filter(base_level: &str, f: impl FnOnce()) -> String {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(default_filter(
                base_level,
            )))
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_ansi(false)
                    .with_writer(move || writer.clone()),
            );

        tracing::subscriber::with_default(subscriber, f);
        capture.contents()
    }

    #[test]
    fn test_default_filter_emits_access_events_at_info() {
        let output = capture_with_default_filter("info", || {
            tracing::info!(target: "access", status = 200, "Response");
        });

        check!(output.contains("Response"));
        check!(output.contains("access"));
    }

    /// Access logging must survive the non-verbose application level production runs at.
    #[test]
    fn test_access_events_survive_warn_application_level() {
        let output = capture_with_default_filter("warn", || {
            tracing::info!(target: "access", status = 200, "Response");
        });

        check!(output.contains("Response"));
    }

    #[test]
    fn test_access_target_is_independently_mutable() {
        let output = capture_with_default_filter("info", || {
            tracing::debug!(target: "access", status = 200, "Response");
        });

        check!(!output.contains("Response"));
    }
}
