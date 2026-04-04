//! Notification dispatcher: watches the event buffer for course changes and
//! sends Discord DMs to users who have active course watches.

use crate::data::events::{AuditLogEvent, DomainEvent, EventBuffer};
use crate::data::watches::{self, TriggeredWatch};
use serenity::all::{Color, CreateEmbed, CreateMessage, UserId};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::Service;

pub struct NotificationService {
    pool: PgPool,
    events: Arc<EventBuffer>,
    http: Arc<serenity::http::Http>,
    base_url: Option<String>,
}

impl NotificationService {
    pub fn new(
        pool: PgPool,
        events: Arc<EventBuffer>,
        http: Arc<serenity::http::Http>,
        base_url: Option<String>,
    ) -> Self {
        Self {
            pool,
            events,
            http,
            base_url,
        }
    }

    async fn process_audit_event(&self, event: &AuditLogEvent) {
        // Classify changed course IDs by change type
        let mut enrollment_ids: Vec<i32> = Vec::new();
        let mut waitlist_ids: Vec<i32> = Vec::new();
        let mut any_change_ids: Vec<i32> = Vec::new();

        for entry in &event.entries {
            if entry.field_changed == "initial" {
                continue;
            }
            any_change_ids.push(entry.course_id);
            match entry.field_changed.as_str() {
                "enrollment" | "max_enrollment" => enrollment_ids.push(entry.course_id),
                "wait_count" | "wait_capacity" => waitlist_ids.push(entry.course_id),
                _ => {}
            }
        }

        if any_change_ids.is_empty() {
            return;
        }

        // Deduplicate each set
        enrollment_ids.sort_unstable();
        enrollment_ids.dedup();
        waitlist_ids.sort_unstable();
        waitlist_ids.dedup();
        any_change_ids.sort_unstable();
        any_change_ids.dedup();

        let triggered = match watches::find_triggered_watches(
            &self.pool,
            &enrollment_ids,
            &waitlist_ids,
            &any_change_ids,
        )
        .await
        {
            Ok(w) => w,
            Err(e) => {
                warn!(error = ?e, "failed to query triggered watches");
                return;
            }
        };

        if triggered.is_empty() {
            return;
        }

        debug!(
            count = triggered.len(),
            "dispatching course watch notifications"
        );

        for watch in &triggered {
            match self.send_notification(watch).await {
                Ok(()) => {
                    if let Err(e) = watches::mark_notified(&self.pool, watch.watch_id).await {
                        warn!(watch_id = watch.watch_id, error = ?e, "failed to mark watch notified");
                    }
                }
                Err(e) => {
                    warn!(
                        watch_id = watch.watch_id,
                        discord_user_id = watch.discord_user_id,
                        error = ?e,
                        "failed to send watch notification"
                    );
                }
            }
        }
    }

    async fn send_notification(&self, watch: &TriggeredWatch) -> anyhow::Result<()> {
        let user_id = UserId::new(watch.discord_user_id as u64);
        let dm = user_id.create_dm_channel(&self.http).await?;

        let course_link = self
            .base_url
            .as_deref()
            .map(|base| format!("{}/courses/{}/{}", base, watch.term_code, watch.crn));

        let embed = build_embed(watch, course_link.as_deref());
        dm.send_message(&self.http, CreateMessage::new().embed(embed))
            .await?;
        Ok(())
    }
}

fn build_embed(watch: &TriggeredWatch, course_url: Option<&str>) -> CreateEmbed {
    let course_label = format!(
        "{} {} - {} (CRN {})",
        watch.subject, watch.course_number, watch.title, watch.crn
    );

    let (description, color) = match watch.watch_type.as_str() {
        "seats_available" => {
            let seats = watch.max_enrollment - watch.enrollment;
            (
                format!("A seat has opened up! {} seat(s) available.", seats),
                Color::from_rgb(0, 200, 100),
            )
        }
        "waitlist_open" => {
            let slots = watch.wait_capacity - watch.wait_count;
            (
                format!("A waitlist spot is available! {} slot(s) open.", slots),
                Color::from_rgb(0, 150, 200),
            )
        }
        _ => (
            "This course has been updated.".to_string(),
            Color::from_rgb(150, 150, 150),
        ),
    };

    let enrollment_field = format!("{}/{}", watch.enrollment, watch.max_enrollment);
    let waitlist_field = format!("{}/{}", watch.wait_count, watch.wait_capacity);

    let mut embed = CreateEmbed::new()
        .title(course_label)
        .description(description)
        .color(color)
        .field("Term", &watch.term_code, true)
        .field("Enrollment", enrollment_field, true)
        .field("Waitlist", waitlist_field, true);

    if let Some(url) = course_url {
        embed = embed.url(url);
    }

    embed
}

#[async_trait::async_trait]
impl Service for NotificationService {
    fn name(&self) -> &'static str {
        "notifications"
    }

    async fn run(&mut self) -> Result<(), anyhow::Error> {
        info!("notification dispatcher started");

        let (mut cursor, mut watch_rx) = self.events.subscribe();

        loop {
            watch_rx
                .changed()
                .await
                .map_err(|_| anyhow::anyhow!("event buffer watch channel closed"))?;

            // If the consumer fell behind and events were pruned, skip ahead to avoid a gap.
            let base = self.events.base_offset();
            if cursor < base {
                warn!(
                    skipped = base - cursor,
                    "notification dispatcher fell behind event buffer, skipping pruned events"
                );
                cursor = base;
            }

            loop {
                match self.events.read(cursor) {
                    Some(DomainEvent::AuditLog(event)) => {
                        cursor += 1;
                        self.process_audit_event(&event).await;
                    }
                    Some(_) => {
                        cursor += 1;
                    }
                    None => break,
                }
            }
        }
    }

    async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
