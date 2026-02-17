//! Fixed-capacity ring buffer for domain events.

use std::collections::VecDeque;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::watch;

use super::DomainEvent;

/// Fixed-capacity ring buffer with cursor-based multi-reader access.
pub struct EventBuffer {
    events: RwLock<VecDeque<DomainEvent>>,
    capacity: usize,
    base_offset: AtomicU64,
    head: watch::Sender<u64>,
}

impl EventBuffer {
    /// Create a new EventBuffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let (head, _) = watch::channel(0);
        Self {
            events: RwLock::new(VecDeque::with_capacity(capacity)),
            capacity,
            base_offset: AtomicU64::new(0),
            head,
        }
    }

    /// Publish an event to the buffer.
    pub fn publish(&self, event: DomainEvent) {
        let mut events = self.events.write().expect("lock poisoned");
        if events.len() >= self.capacity {
            events.pop_front();
            self.base_offset.fetch_add(1, Ordering::Release);
        }
        events.push_back(event);
        let new_head = self.base_offset.load(Ordering::Acquire) + events.len() as u64;
        let _ = self.head.send(new_head);
    }

    /// Subscribe to the buffer, returning current head position and a watch receiver.
    pub fn subscribe(&self) -> (u64, watch::Receiver<u64>) {
        let head = *self.head.borrow();
        (head, self.head.subscribe())
    }

    /// Read an event at the given cursor position.
    /// Returns None if cursor is behind base_offset (consumer lagged) or ahead of head.
    pub fn read(&self, cursor: u64) -> Option<DomainEvent> {
        let events = self.events.read().expect("lock poisoned");
        let base = self.base_offset.load(Ordering::Acquire);
        if cursor < base {
            return None; // Consumer fell behind
        }
        let index = (cursor - base) as usize;
        events.get(index).cloned()
    }

    /// Get the current base offset (logical index of first event in buffer).
    pub fn base_offset(&self) -> u64 {
        self.base_offset.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::ws::ScrapeJobEvent;

    fn make_scrape_event(id: i32) -> DomainEvent {
        DomainEvent::ScrapeJob(ScrapeJobEvent::Completed { id, subject: None })
    }

    #[test]
    fn publish_and_read_single_event() {
        let buffer = EventBuffer::new(10);
        let (cursor, _) = buffer.subscribe();

        buffer.publish(make_scrape_event(1));

        let event = buffer.read(cursor);
        assert!(event.is_some());
    }

    #[test]
    fn cursor_advances_correctly() {
        let buffer = EventBuffer::new(10);
        let (mut cursor, _) = buffer.subscribe();

        buffer.publish(make_scrape_event(1));
        buffer.publish(make_scrape_event(2));
        buffer.publish(make_scrape_event(3));

        assert!(buffer.read(cursor).is_some());
        cursor += 1;
        assert!(buffer.read(cursor).is_some());
        cursor += 1;
        assert!(buffer.read(cursor).is_some());
        cursor += 1;
        assert!(buffer.read(cursor).is_none()); // Past head
    }

    #[test]
    fn oldest_events_pruned_at_capacity() {
        let buffer = EventBuffer::new(3);
        let (initial_cursor, _) = buffer.subscribe();

        // Fill buffer
        buffer.publish(make_scrape_event(1));
        buffer.publish(make_scrape_event(2));
        buffer.publish(make_scrape_event(3));

        // Overflow - oldest gets pruned
        buffer.publish(make_scrape_event(4));

        // Initial cursor should now be behind base_offset
        assert!(buffer.read(initial_cursor).is_none());
        assert_eq!(buffer.base_offset(), 1);
    }

    #[test]
    fn cursor_behind_base_returns_none() {
        let buffer = EventBuffer::new(2);

        buffer.publish(make_scrape_event(1));
        buffer.publish(make_scrape_event(2));
        buffer.publish(make_scrape_event(3)); // Prunes event 1

        // Cursor 0 is now behind base_offset (1)
        assert!(buffer.read(0).is_none());
        assert!(buffer.read(1).is_some()); // Event 2
        assert!(buffer.read(2).is_some()); // Event 3
    }

    #[tokio::test]
    async fn subscribe_notifies_on_publish() {
        let buffer = EventBuffer::new(10);
        let (_, mut watch_rx) = buffer.subscribe();

        buffer.publish(make_scrape_event(1));

        // Should have changed
        assert!(watch_rx.has_changed().unwrap());
        watch_rx.mark_changed(); // Reset
        assert_eq!(*watch_rx.borrow(), 1);
    }
}
