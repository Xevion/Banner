//! Real-time stream WebSocket module.

pub mod computed;
pub mod filters;
pub mod protocol;
pub mod streams;
pub mod subscriptions;

mod handler;
mod sink;

pub use handler::stream_ws;
