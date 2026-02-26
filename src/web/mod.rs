//! Web API module for the banner application.

pub mod admin;
#[cfg(feature = "embed-assets")]
pub mod assets;
pub mod audit;
pub mod auth;
pub mod calendar;
pub mod courses;
pub mod csp_report;
#[cfg(feature = "embed-assets")]
pub mod encoding;
pub mod error;
pub mod instructors;
pub mod middleware;
pub mod proxy;
pub mod routes;
pub mod schedule_cache;
pub mod search_options;
pub mod search_options_cache;
pub mod sitemap;
pub mod sitemap_cache;
pub mod status;
pub mod stream;
pub mod suggest;
pub mod timeline;
pub mod ws;

pub use routes::*;
