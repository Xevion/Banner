//! Stream WebSocket handler.

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast::error::RecvError;
use tracing::debug;

use crate::data::events::{AuditLogEvent, DomainEvent};
use crate::state::AppState;
use crate::web::admin::scraper::{
    compute_stats, compute_subjects, compute_timeseries, default_bucket_for_period,
};
use crate::web::auth::extractors::AdminUser;
use crate::web::stream::computed::{ComputedCacheKey, ComputedUpdate};
use crate::web::stream::protocol::{
    STREAM_PROTOCOL_VERSION, StreamClientMessage, StreamDelta, StreamError, StreamErrorCode,
    StreamKind, StreamServerMessage, StreamSnapshot,
};
use crate::web::stream::streams::{audit_log, scrape_jobs};
use crate::web::stream::subscriptions::{Subscription, SubscriptionRegistry, build_subscription};
use crate::web::ws::ScrapeJobEvent;

/// Outcome of processing a single client WebSocket message.
enum ClientMessageResult {
    /// Message processed successfully; continue the loop.
    Continue,
    /// A protocol-level error was sent to the client; continue the loop.
    ErrorSent,
    /// A WebSocket send failed; the connection is dead.
    Disconnected,
}

impl ClientMessageResult {
    /// Convert the result of a `send_error` call (true = sent, false = send failed).
    fn from_error_send(sent: bool) -> Self {
        if sent {
            Self::ErrorSent
        } else {
            Self::Disconnected
        }
    }
}

/// Convert a subscription to its corresponding computed cache key, if applicable.
fn subscription_to_cache_key(sub: &Subscription) -> Option<ComputedCacheKey> {
    match sub {
        Subscription::ScraperStats { filter } => Some(ComputedCacheKey::Stats {
            period: filter.period.clone(),
            term: filter.term.clone(),
        }),
        Subscription::ScraperTimeseries { filter } => Some(ComputedCacheKey::Timeseries {
            period: filter.period.clone(),
            bucket: filter
                .bucket
                .clone()
                .unwrap_or_else(|| default_bucket_for_period(&filter.period).to_string()),
            term: filter.term.clone(),
        }),
        Subscription::ScraperSubjects => Some(ComputedCacheKey::Subjects),
        _ => None,
    }
}

/// WebSocket endpoint for real-time streams.
pub async fn stream_ws(
    ws: WebSocketUpgrade,
    AdminUser(_user): AdminUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_stream_ws(socket, state))
}

async fn send_message(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    message: &StreamServerMessage,
) -> bool {
    let Ok(json) = serde_json::to_string(message) else {
        return true;
    };
    sink.send(Message::Text(json.into())).await.is_ok()
}

async fn send_error(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    request_id: Option<String>,
    code: StreamErrorCode,
    message: &str,
) -> bool {
    let msg = StreamServerMessage::Error {
        request_id,
        code,
        message: message.to_string(),
    };
    send_message(sink, &msg).await
}

async fn handle_stream_ws(socket: WebSocket, state: AppState) {
    debug!("stream WebSocket connected");

    let (mut sink, mut stream) = socket.split();
    let ready = StreamServerMessage::Ready {
        protocol_version: STREAM_PROTOCOL_VERSION,
    };
    if !send_message(&mut sink, &ready).await {
        return;
    }

    let mut registry = SubscriptionRegistry::new();

    let (mut cursor, mut head_watch) = state.events.subscribe();
    let mut computed_rx = state.computed_streams.subscribe();

    loop {
        tokio::select! {
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if matches!(
                            handle_client_message(&mut sink, &state, &mut registry, &text).await,
                            ClientMessageResult::Disconnected
                        ) {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            result = head_watch.changed() => {
                if result.is_err() {
                    break;
                }
                // Check for lag - cursor fell behind the buffer's oldest event
                if cursor < state.events.base_offset() {
                    if !resync_all(&mut sink, &state, &mut registry).await {
                        break;
                    }
                    // Start from oldest available event to avoid missing any
                    cursor = state.events.base_offset();
                    continue;
                }
                // Process all events from cursor to head
                let mut send_failed = false;
                while let Some(event) = state.events.read(cursor) {
                    if !dispatch_event(&mut sink, &state, &mut registry, event).await {
                        send_failed = true;
                        break;
                    }
                    cursor += 1;
                }
                if send_failed {
                    break;
                }
            }
            update = computed_rx.recv() => {
                match update {
                    Ok(update) => {
                        if !dispatch_computed_update(&mut sink, &registry, update).await {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        debug!(skipped = n, "Computed updates lagged, resyncing");
                        if !resync_computed(&mut sink, &state, &registry).await {
                            break;
                        }
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }

    debug!("stream WebSocket disconnected");
}

async fn handle_client_message(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    registry: &mut SubscriptionRegistry,
    text: &str,
) -> ClientMessageResult {
    let parsed = match serde_json::from_str::<StreamClientMessage>(text) {
        Ok(msg) => msg,
        Err(_) => {
            let sent = send_error(
                sink,
                None,
                StreamErrorCode::InvalidMessage,
                "Invalid message",
            )
            .await;
            return ClientMessageResult::from_error_send(sent);
        }
    };

    match parsed {
        StreamClientMessage::Subscribe {
            request_id,
            stream,
            filter,
        } => {
            let sub_id = registry.allocate_id();

            let subscription = match build_subscription(stream, filter) {
                Ok(sub) => sub,
                Err(StreamError { code, message }) => {
                    let sent = send_error(sink, Some(request_id), code, &message).await;
                    return ClientMessageResult::from_error_send(sent);
                }
            };

            // Register computed stream interest before inserting
            let cache_key = subscription_to_cache_key(&subscription);
            registry.insert(sub_id.clone(), subscription);
            if let Some(key) = cache_key {
                state.computed_streams.register(key);
            }

            let subscribed = StreamServerMessage::Subscribed {
                request_id,
                subscription_id: sub_id.clone(),
                stream,
            };
            if !send_message(sink, &subscribed).await {
                return ClientMessageResult::Disconnected;
            }

            if !send_snapshot(sink, state, registry, &sub_id).await {
                return ClientMessageResult::Disconnected;
            }
        }
        StreamClientMessage::Modify {
            request_id,
            subscription_id,
            filter,
        } => {
            let Some(subscription) = registry.get(&subscription_id) else {
                let sent = send_error(
                    sink,
                    Some(request_id),
                    StreamErrorCode::UnknownSubscription,
                    "Unknown subscription",
                )
                .await;
                return ClientMessageResult::from_error_send(sent);
            };

            let stream = subscription.kind();
            let old_cache_key = subscription_to_cache_key(subscription);

            let updated = match build_subscription(stream, filter) {
                Ok(sub) => sub,
                Err(StreamError { code, message }) => {
                    let sent = send_error(sink, Some(request_id), code, &message).await;
                    return ClientMessageResult::from_error_send(sent);
                }
            };

            let new_cache_key = subscription_to_cache_key(&updated);

            // Update computed stream registration if cache key changed
            if old_cache_key != new_cache_key {
                if let Some(key) = old_cache_key {
                    state.computed_streams.deregister(key);
                }
                if let Some(key) = new_cache_key {
                    state.computed_streams.register(key);
                }
            }

            // Now get mutable reference and update
            let subscription = registry.get_mut(&subscription_id).unwrap();
            *subscription = updated;
            let modified = StreamServerMessage::Modified {
                request_id,
                subscription_id: subscription_id.clone(),
            };
            if !send_message(sink, &modified).await {
                return ClientMessageResult::Disconnected;
            }

            if !send_snapshot(sink, state, registry, &subscription_id).await {
                return ClientMessageResult::Disconnected;
            }
        }
        StreamClientMessage::Unsubscribe {
            request_id,
            subscription_id,
        } => {
            // Deregister computed stream interest before removing
            if let Some(sub) = registry.get(&subscription_id)
                && let Some(key) = subscription_to_cache_key(sub)
            {
                state.computed_streams.deregister(key);
            }
            registry.remove(&subscription_id);
            let msg = StreamServerMessage::Unsubscribed {
                request_id,
                subscription_id,
            };
            if !send_message(sink, &msg).await {
                return ClientMessageResult::Disconnected;
            }
        }
        StreamClientMessage::Ping {
            request_id,
            timestamp,
        } => {
            let pong = StreamServerMessage::Pong {
                request_id,
                timestamp,
            };
            if !send_message(sink, &pong).await {
                return ClientMessageResult::Disconnected;
            }
        }
    }

    ClientMessageResult::Continue
}

async fn send_snapshot(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    registry: &mut SubscriptionRegistry,
    subscription_id: &str,
) -> bool {
    let Some(subscription) = registry.get_mut(subscription_id) else {
        return true;
    };

    match subscription {
        Subscription::ScrapeJobs { filter, known_ids } => {
            let snapshot = match scrape_jobs::build_snapshot(&state.db_pool, filter).await {
                Ok(jobs) => {
                    *known_ids = jobs.iter().map(|job| job.id).collect();
                    StreamSnapshot::ScrapeJobs { jobs }
                }
                Err(_) => {
                    return send_error(
                        sink,
                        None,
                        StreamErrorCode::InternalError,
                        "Failed to load scrape jobs snapshot",
                    )
                    .await;
                }
            };

            send_message(
                sink,
                &StreamServerMessage::Snapshot {
                    subscription_id: subscription_id.to_string(),
                    snapshot,
                },
            )
            .await
        }
        Subscription::AuditLog { filter } => {
            let snapshot = match audit_log::build_snapshot(&state.db_pool, filter).await {
                Ok(entries) => StreamSnapshot::AuditLog { entries },
                Err(_) => {
                    return send_error(
                        sink,
                        None,
                        StreamErrorCode::InternalError,
                        "Failed to load audit log snapshot",
                    )
                    .await;
                }
            };

            send_message(
                sink,
                &StreamServerMessage::Snapshot {
                    subscription_id: subscription_id.to_string(),
                    snapshot,
                },
            )
            .await
        }
        Subscription::ScraperStats { filter } => {
            match compute_stats(&state.db_pool, &filter.period, filter.term.as_deref()).await {
                Ok(stats) => {
                    send_message(
                        sink,
                        &StreamServerMessage::Snapshot {
                            subscription_id: subscription_id.to_string(),
                            snapshot: StreamSnapshot::ScraperStats { stats },
                        },
                    )
                    .await
                }
                Err(_) => {
                    send_error(
                        sink,
                        None,
                        StreamErrorCode::InternalError,
                        "Failed to load stats",
                    )
                    .await
                }
            }
        }
        Subscription::ScraperTimeseries { filter } => {
            let bucket = filter
                .bucket
                .clone()
                .unwrap_or_else(|| default_bucket_for_period(&filter.period).to_string());
            match compute_timeseries(
                &state.db_pool,
                &filter.period,
                Some(bucket.as_str()),
                filter.term.as_deref(),
            )
            .await
            {
                Ok((points, period, bucket)) => {
                    send_message(
                        sink,
                        &StreamServerMessage::Snapshot {
                            subscription_id: subscription_id.to_string(),
                            snapshot: StreamSnapshot::ScraperTimeseries {
                                points,
                                period,
                                bucket,
                            },
                        },
                    )
                    .await
                }
                Err(_) => {
                    send_error(
                        sink,
                        None,
                        StreamErrorCode::InternalError,
                        "Failed to load timeseries",
                    )
                    .await
                }
            }
        }
        Subscription::ScraperSubjects => {
            let ref_cache = state.reference_cache.read().await;
            match compute_subjects(&state.db_pool, &state.events, &ref_cache).await {
                Ok(subjects) => {
                    send_message(
                        sink,
                        &StreamServerMessage::Snapshot {
                            subscription_id: subscription_id.to_string(),
                            snapshot: StreamSnapshot::ScraperSubjects { subjects },
                        },
                    )
                    .await
                }
                Err(_) => {
                    send_error(
                        sink,
                        None,
                        StreamErrorCode::InternalError,
                        "Failed to load subjects",
                    )
                    .await
                }
            }
        }
    }
}

async fn dispatch_event(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    registry: &mut SubscriptionRegistry,
    event: DomainEvent,
) -> bool {
    match event {
        DomainEvent::ScrapeJob(scrape_event) => {
            dispatch_scrape_job_event(sink, state, registry, scrape_event).await
        }
        DomainEvent::AuditLog(audit_event) => {
            dispatch_audit_log_event(sink, registry, audit_event).await
        }
    }
}

async fn resync_all(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    registry: &mut SubscriptionRegistry,
) -> bool {
    resync_scrape_jobs(sink, state, registry).await && resync_audit_log(sink, state, registry).await
}

async fn dispatch_scrape_job_event(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    registry: &mut SubscriptionRegistry,
    event: ScrapeJobEvent,
) -> bool {
    let mut job_details: Option<crate::web::ws::ScrapeJobDto> = None;

    for (subscription_id, subscription) in registry.iter_mut() {
        let Subscription::ScrapeJobs { filter, known_ids } = subscription else {
            continue;
        };

        let matches =
            scrape_jobs::event_matches(&state.db_pool, filter, known_ids, &event, &mut job_details)
                .await;

        if matches {
            let delta = StreamServerMessage::Delta {
                subscription_id: subscription_id.clone(),
                delta: StreamDelta::ScrapeJobs {
                    event: event.clone(),
                },
            };
            if !send_message(sink, &delta).await {
                return false;
            }
        }
    }

    true
}

async fn dispatch_audit_log_event(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    registry: &mut SubscriptionRegistry,
    event: AuditLogEvent,
) -> bool {
    for (subscription_id, subscription) in registry.iter_mut() {
        let Subscription::AuditLog { filter } = subscription else {
            continue;
        };

        let entries = audit_log::filter_entries(filter, &event.entries);
        if entries.is_empty() {
            continue;
        }

        let delta = StreamServerMessage::Delta {
            subscription_id: subscription_id.clone(),
            delta: StreamDelta::AuditLog { entries },
        };
        if !send_message(sink, &delta).await {
            return false;
        }
    }

    true
}

async fn resync_scrape_jobs(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    registry: &mut SubscriptionRegistry,
) -> bool {
    let ids = registry.ids_for_kind(StreamKind::ScrapeJobs);
    for subscription_id in ids {
        if !send_snapshot(sink, state, registry, &subscription_id).await {
            return false;
        }
    }
    true
}

async fn resync_audit_log(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    registry: &mut SubscriptionRegistry,
) -> bool {
    let ids = registry.ids_for_kind(StreamKind::AuditLog);
    for subscription_id in ids {
        if !send_snapshot(sink, state, registry, &subscription_id).await {
            return false;
        }
    }
    true
}

async fn dispatch_computed_update(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    registry: &SubscriptionRegistry,
    update: ComputedUpdate,
) -> bool {
    for (subscription_id, subscription) in registry.iter() {
        let matches = match (&update.key, subscription) {
            (ComputedCacheKey::Stats { period, term }, Subscription::ScraperStats { filter }) => {
                &filter.period == period && &filter.term == term
            }
            (
                ComputedCacheKey::Timeseries {
                    period,
                    bucket,
                    term,
                },
                Subscription::ScraperTimeseries { filter },
            ) => {
                let filter_bucket = filter
                    .bucket
                    .clone()
                    .unwrap_or_else(|| default_bucket_for_period(&filter.period).to_string());
                &filter.period == period && &filter_bucket == bucket && &filter.term == term
            }
            (ComputedCacheKey::Subjects, Subscription::ScraperSubjects) => true,
            _ => false,
        };

        if matches && let Some(ref delta) = update.delta {
            let msg = StreamServerMessage::Delta {
                subscription_id: subscription_id.clone(),
                delta: delta.clone(),
            };
            if !send_message(sink, &msg).await {
                return false;
            }
        }
    }
    true
}

/// Send computed snapshots for all computed subscriptions.
async fn send_computed_snapshot(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    subscription: &Subscription,
    subscription_id: &str,
) -> bool {
    match subscription {
        Subscription::ScraperStats { filter } => {
            match compute_stats(&state.db_pool, &filter.period, filter.term.as_deref()).await {
                Ok(stats) => {
                    send_message(
                        sink,
                        &StreamServerMessage::Snapshot {
                            subscription_id: subscription_id.to_string(),
                            snapshot: StreamSnapshot::ScraperStats { stats },
                        },
                    )
                    .await
                }
                Err(_) => {
                    send_error(
                        sink,
                        None,
                        StreamErrorCode::InternalError,
                        "Failed to load stats",
                    )
                    .await
                }
            }
        }
        Subscription::ScraperTimeseries { filter } => {
            let bucket = filter
                .bucket
                .clone()
                .unwrap_or_else(|| default_bucket_for_period(&filter.period).to_string());
            match compute_timeseries(
                &state.db_pool,
                &filter.period,
                Some(bucket.as_str()),
                filter.term.as_deref(),
            )
            .await
            {
                Ok((points, period, bucket)) => {
                    send_message(
                        sink,
                        &StreamServerMessage::Snapshot {
                            subscription_id: subscription_id.to_string(),
                            snapshot: StreamSnapshot::ScraperTimeseries {
                                points,
                                period,
                                bucket,
                            },
                        },
                    )
                    .await
                }
                Err(_) => {
                    send_error(
                        sink,
                        None,
                        StreamErrorCode::InternalError,
                        "Failed to load timeseries",
                    )
                    .await
                }
            }
        }
        Subscription::ScraperSubjects => {
            let ref_cache = state.reference_cache.read().await;
            match compute_subjects(&state.db_pool, &state.events, &ref_cache).await {
                Ok(subjects) => {
                    send_message(
                        sink,
                        &StreamServerMessage::Snapshot {
                            subscription_id: subscription_id.to_string(),
                            snapshot: StreamSnapshot::ScraperSubjects { subjects },
                        },
                    )
                    .await
                }
                Err(_) => {
                    send_error(
                        sink,
                        None,
                        StreamErrorCode::InternalError,
                        "Failed to load subjects",
                    )
                    .await
                }
            }
        }
        _ => true, // Non-computed subscriptions don't need resync here
    }
}

async fn resync_computed(
    sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
    registry: &SubscriptionRegistry,
) -> bool {
    for (subscription_id, subscription) in registry.iter() {
        if subscription.is_computed()
            && !send_computed_snapshot(sink, state, subscription, subscription_id).await
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::admin::scraper::validate_bucket;
    use crate::web::stream::filters::ScraperTimeseriesFilter;
    use crate::web::stream::subscriptions::Subscription;

    #[test]
    fn subscription_to_cache_key_default_bucket_is_valid() {
        let periods = ["1h", "6h", "24h", "7d", "30d"];
        for period in periods {
            let sub = Subscription::ScraperTimeseries {
                filter: ScraperTimeseriesFilter {
                    period: period.to_string(),
                    bucket: None,
                    term: None,
                },
            };
            let key = subscription_to_cache_key(&sub)
                .expect("ScraperTimeseries should produce a cache key");
            if let ComputedCacheKey::Timeseries { bucket, .. } = &key {
                assert!(
                    validate_bucket(bucket).is_some(),
                    "Default bucket '{bucket}' for period '{period}' is not valid"
                );
            } else {
                panic!("Expected Timeseries cache key");
            }
        }
    }
}
