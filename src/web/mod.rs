//! Web API module for the banner application.

pub mod admin;
#[cfg(feature = "embed-assets")]
pub mod assets;
pub mod audit;
pub mod auth;
pub mod calendar;
#[cfg(feature = "embed-assets")]
pub mod encoding;
pub mod error;
pub mod middleware;
pub mod routes;
pub mod schedule_cache;
pub mod search_options_cache;
pub mod stream;
pub mod timeline;
pub mod ws;

pub use routes::*;
