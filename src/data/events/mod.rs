//! Domain event infrastructure.

mod buffer;
mod types;

pub use buffer::EventBuffer;
pub use types::{AuditLogEvent, DomainEvent};
