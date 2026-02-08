//! File watcher for events.jsonl.
//!
//! Watches for changes to `.agent/events.jsonl` and reads new events incrementally.
//! Uses notify crate for efficient file system monitoring without polling.
//! Broadcasts events to multiple subscribers via tokio broadcast channel.

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use ralph_core::{Event, EventReader, ParseResult};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Thread-safe handle for subscribing to events.
///
/// This is safe to store in AppState and share across threads.
#[derive(Clone)]
pub struct SessionBroadcast {
    event_tx: broadcast::Sender<Event>,
}

impl SessionBroadcast {
    /// Subscribe to receive events from this session.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    /// Creates a new SessionBroadcast from a broadcast sender.
    pub fn new(event_tx: broadcast::Sender<Event>) -> Self {
        Self { event_tx }
    }
}

/// Watches events.jsonl and yields new events.
///
/// Supports multiple subscribers via tokio broadcast channel.
/// Each subscriber receives all events from the point they subscribe.
pub struct EventWatcher {
    #[allow(dead_code)] // Used by path() for SSE endpoint in task-09
    path: PathBuf,
    reader: EventReader,
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
    /// Broadcast sender for multi-subscriber event distribution.
    /// Channel capacity is 100 messages; slow subscribers will lag.
    event_tx: broadcast::Sender<Event>,
}

impl EventWatcher {
    /// Broadcast channel capacity - 100 messages before lagging.
    const CHANNEL_CAPACITY: usize = 100;

    /// Creates a new EventWatcher for the given events.jsonl path.
    ///
    /// Initializes file watcher, reader at position 0, and broadcast channel.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, notify::Error> {
        let path = path.into();
        let reader = EventReader::new(&path);

        let (tx, rx) = mpsc::channel();

        // Create broadcast channel for multi-subscriber distribution
        let (event_tx, _) = broadcast::channel(Self::CHANNEL_CAPACITY);

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        )?;

        // Watch the file (or parent dir if file doesn't exist yet)
        let watch_path = if path.exists() {
            path.clone()
        } else {
            path.parent().unwrap_or(Path::new(".")).to_path_buf()
        };
        watcher.watch(&watch_path, RecursiveMode::NonRecursive)?;

        info!("Watching for events at {:?}", path);

        Ok(Self {
            path,
            reader,
            _watcher: watcher,
            rx,
            event_tx,
        })
    }

    /// Subscribe to receive events from this watcher.
    ///
    /// Returns a broadcast receiver that will receive all events
    /// from the point of subscription. If the receiver falls behind
    /// by more than 100 messages, older messages will be dropped.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    /// Waits for file changes and reads new events.
    ///
    /// Blocks until the file is modified, then reads and returns new events.
    /// Also broadcasts valid events to all subscribers.
    /// Returns None if the watcher is stopped, times out, or encounters an error.
    pub fn wait_for_events(&mut self, timeout: Duration) -> Option<ParseResult> {
        use std::time::Instant;
        let deadline = Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None; // Timeout
            }

            match self.rx.recv_timeout(remaining) {
                Ok(Ok(event)) => {
                    // Process any modify event (macOS sends various subtypes)
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_)
                    ) {
                        debug!("File modified, reading new events");
                        match self.reader.read_new_events() {
                            Ok(result) if !result.events.is_empty() || !result.malformed.is_empty() => {
                                // Broadcast valid events to subscribers
                                self.broadcast_events(&result.events);
                                return Some(result);
                            }
                            Ok(_) => {
                                // No new content yet, keep waiting
                                continue;
                            }
                            Err(e) => {
                                warn!("Failed to read events: {}", e);
                                return None;
                            }
                        }
                    }
                    // Ignore other event types and keep waiting
                }
                Ok(Err(e)) => {
                    warn!("Watcher error: {:?}", e);
                    return None;
                }
                Err(_) => return None, // Timeout
            }
        }
    }

    /// Broadcasts events to all subscribers.
    ///
    /// Silently handles send errors (no subscribers or lagged receivers).
    fn broadcast_events(&self, events: &[Event]) {
        for event in events {
            // send() returns Err if no receivers, which is fine
            let _ = self.event_tx.send(event.clone());
        }
    }

    /// Reads all current events without waiting.
    ///
    /// Used for initial load when starting the watcher.
    pub fn read_current_events(&mut self) -> std::io::Result<ParseResult> {
        self.reader.read_new_events()
    }

    /// Returns the current file position.
    pub fn position(&self) -> u64 {
        self.reader.position()
    }

    /// Returns the path being watched.
    #[allow(dead_code)] // Will be used by SSE endpoint in task-09
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns a thread-safe broadcast handle for this watcher.
    ///
    /// The returned handle can be safely shared across threads and stored
    /// in shared state like Actix's `web::Data`.
    pub fn broadcast_handle(&self) -> SessionBroadcast {
        SessionBroadcast::new(self.event_tx.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::thread;
    use tempfile::TempDir;

    fn create_events_file(dir: &Path) -> PathBuf {
        let events_path = dir.join("events.jsonl");
        File::create(&events_path).unwrap();
        events_path
    }

    fn append_event(path: &Path, topic: &str) {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(
            file,
            r#"{{"topic":"{}","ts":"2024-01-01T00:00:00Z"}}"#,
            topic
        )
        .unwrap();
        file.flush().unwrap();
    }

    #[test]
    fn test_detects_new_events() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        let mut watcher = EventWatcher::new(&events_path).unwrap();

        // Append event in background after short delay
        let path_clone = events_path.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            append_event(&path_clone, "test.event");
        });

        // Wait for the event
        let result = watcher.wait_for_events(Duration::from_secs(2));

        assert!(result.is_some(), "Should detect new event");
        let parse_result = result.unwrap();
        assert_eq!(parse_result.events.len(), 1);
        assert_eq!(parse_result.events[0].topic, "test.event");
    }

    #[test]
    fn test_tracks_read_position() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        // Write initial events
        append_event(&events_path, "first");
        append_event(&events_path, "second");

        let mut watcher = EventWatcher::new(&events_path).unwrap();

        // Read initial events
        let result = watcher.read_current_events().unwrap();
        assert_eq!(result.events.len(), 2);
        let initial_pos = watcher.position();
        assert!(initial_pos > 0);

        // Append more events in background
        let path_clone = events_path.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            append_event(&path_clone, "third");
        });

        // Wait for new event
        let result = watcher.wait_for_events(Duration::from_secs(2));

        assert!(result.is_some());
        let parse_result = result.unwrap();
        // Should only have the NEW event, not re-read old ones
        assert_eq!(parse_result.events.len(), 1);
        assert_eq!(parse_result.events[0].topic, "third");

        // Position should have advanced
        assert!(watcher.position() > initial_pos);
    }

    #[test]
    fn test_handles_malformed_events() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        let mut watcher = EventWatcher::new(&events_path).unwrap();

        // Write events including malformed in background
        let path_clone = events_path.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            // Good event
            append_event(&path_clone, "good");
            // Malformed event
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&path_clone)
                .unwrap();
            writeln!(file, "{{invalid json}}").unwrap();
            file.flush().unwrap();
            // Another good event
            append_event(&path_clone, "also_good");
        });

        // Wait for events
        let result = watcher.wait_for_events(Duration::from_secs(2));

        assert!(result.is_some());
        let parse_result = result.unwrap();

        // Good events should be parsed
        assert_eq!(parse_result.events.len(), 2);
        assert_eq!(parse_result.events[0].topic, "good");
        assert_eq!(parse_result.events[1].topic, "also_good");

        // Malformed line should be captured
        assert_eq!(parse_result.malformed.len(), 1);
    }

    #[test]
    fn test_uses_event_type() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        // Write event with payload
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .unwrap();
        writeln!(
            file,
            r#"{{"topic":"design.drafted","payload":"Ready for review","ts":"2024-01-01T00:00:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut watcher = EventWatcher::new(&events_path).unwrap();
        let result = watcher.read_current_events().unwrap();

        assert_eq!(result.events.len(), 1);
        let event = &result.events[0];
        assert_eq!(event.topic, "design.drafted");
        assert_eq!(event.payload, Some("Ready for review".to_string()));
        assert_eq!(event.ts, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_timeout_returns_none() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        let mut watcher = EventWatcher::new(&events_path).unwrap();

        // Short timeout with no events
        let result = watcher.wait_for_events(Duration::from_millis(50));
        assert!(result.is_none());
    }

    #[test]
    fn test_broadcasts_to_subscribers() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        let mut watcher = EventWatcher::new(&events_path).unwrap();

        // Create two subscribers
        let mut rx1 = watcher.subscribe();
        let mut rx2 = watcher.subscribe();

        // Append event in background after short delay
        let path_clone = events_path.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            append_event(&path_clone, "broadcast.test");
        });

        // Wait for the event (this also broadcasts)
        let result = watcher.wait_for_events(Duration::from_secs(2));
        assert!(result.is_some());

        // Both subscribers should receive the event
        let event1 = rx1.try_recv().expect("rx1 should receive event");
        let event2 = rx2.try_recv().expect("rx2 should receive event");

        assert_eq!(event1.topic, "broadcast.test");
        assert_eq!(event2.topic, "broadcast.test");
    }

    #[test]
    fn test_broadcast_handles_malformed_gracefully() {
        let temp = TempDir::new().unwrap();
        let events_path = create_events_file(temp.path());

        let mut watcher = EventWatcher::new(&events_path).unwrap();
        let mut rx = watcher.subscribe();

        // Write events including malformed in background
        let path_clone = events_path.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            // Good event
            append_event(&path_clone, "good.event");
            // Malformed event (should not be broadcast)
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&path_clone)
                .unwrap();
            writeln!(file, "{{invalid json}}").unwrap();
            file.flush().unwrap();
        });

        // Wait for events
        let result = watcher.wait_for_events(Duration::from_secs(2));
        assert!(result.is_some());

        // Only valid event should be broadcast
        let event = rx.try_recv().expect("should receive valid event");
        assert_eq!(event.topic, "good.event");

        // No more events (malformed was not broadcast)
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_multiple_sessions_isolated() {
        let temp1 = TempDir::new().unwrap();
        let temp2 = TempDir::new().unwrap();
        let events_path1 = create_events_file(temp1.path());
        let events_path2 = create_events_file(temp2.path());

        let mut watcher1 = EventWatcher::new(&events_path1).unwrap();
        let watcher2 = EventWatcher::new(&events_path2).unwrap();

        let mut rx1 = watcher1.subscribe();
        let mut rx2 = watcher2.subscribe();

        // Write event only to session 1
        let path_clone = events_path1.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            append_event(&path_clone, "session1.event");
        });

        // Watcher 1 should detect and broadcast
        let result = watcher1.wait_for_events(Duration::from_secs(2));
        assert!(result.is_some());

        // rx1 should receive the event
        let event = rx1.try_recv().expect("rx1 should receive event");
        assert_eq!(event.topic, "session1.event");

        // rx2 should NOT receive anything (different session)
        assert!(rx2.try_recv().is_err(), "rx2 should not receive session1 events");
    }
}
