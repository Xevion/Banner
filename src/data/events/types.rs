//! Domain event types.

use crate::web::audit::AuditLogEntry;
use crate::web::ws::ScrapeJobEvent;

/// Unified enum for all domain events.
#[derive(Debug, Clone)]
pub enum DomainEvent {
    ScrapeJob(ScrapeJobEvent),
    AuditLog(AuditLogEvent),
}

/// Audit log event containing one or more entries.
#[derive(Debug, Clone)]
pub struct AuditLogEvent {
    pub entries: Vec<AuditLogEntry>,
}
