use clap::Parser;

/// Banner Discord Bot - Course availability monitoring
///
/// This application runs all services:
/// - bot: Discord bot for course monitoring commands
/// - web: HTTP server for web interface and API
/// - scraper: Background service for scraping course data
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Log formatter to use
    #[arg(long, value_enum, default_value_t = default_tracing_format())]
    pub tracing: TracingFormat,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum TracingFormat {
    /// Use pretty formatter (default in debug mode)
    Pretty,
    /// Use JSON formatter (default in release mode)
    Json,
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum ServiceName {
    /// Discord bot for course monitoring commands
    Bot,
    /// HTTP server for web interface and API
    Web,
    /// Background service for scraping course data
    Scraper,
}

impl ServiceName {
    /// Get all available services
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![Self::Bot, Self::Web, Self::Scraper]
    }

    /// Convert to string for service registration
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Bot => "bot",
            Self::Web => "web",
            Self::Scraper => "scraper",
        }
    }
}

#[cfg(debug_assertions)]
const DEFAULT_TRACING_FORMAT: TracingFormat = TracingFormat::Pretty;
#[cfg(not(debug_assertions))]
const DEFAULT_TRACING_FORMAT: TracingFormat = TracingFormat::Json;

const fn default_tracing_format() -> TracingFormat {
    DEFAULT_TRACING_FORMAT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_name_as_str() {
        assert_eq!(ServiceName::Bot.as_str(), "bot");
        assert_eq!(ServiceName::Web.as_str(), "web");
        assert_eq!(ServiceName::Scraper.as_str(), "scraper");
    }

    #[test]
    fn test_service_name_all() {
        let all = ServiceName::all();
        assert_eq!(all.len(), 3);
    }
}
