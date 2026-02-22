pub mod formatter;

use crate::cli::TracingFormat;
use crate::config::Config;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt::format::JsonFields};

/// Configure and initialize logging for the application.
pub fn setup_logging(config: &Config, tracing_format: TracingFormat) {
    // Configure logging based on config.
    // Module paths use `banner::banner::` because the crate (`banner`) contains
    // a `banner` submodule for the Banner API client.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let base_level = &config.log_level;
        EnvFilter::new(format!(
            "warn,banner={base_level},banner::banner::middleware=warn,banner::banner::session=warn"
        ))
    });

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
