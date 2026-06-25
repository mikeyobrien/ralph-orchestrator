//! Incremental tailer for autoloop's `--events` NDJSON stream.
//!
//! [`crate::parse_events`] decodes the `--events` file whole; this tails it
//! *live* — each [`poll`](AutoloopEventTailer::poll) returns only the
//! [`AutoloopEvent`]s appended since the last call. It tolerates a torn trailing
//! UTF-8 sequence, a partial final line (no newline yet), and truncation, so it
//! is safe to poll a file an autoloop subprocess is actively appending to. Used
//! by the live TUI (#342) to render a run as it progresses.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::autoloop_events::AutoloopEvent;

/// A file-backed, incremental NDJSON tailer over autoloop's `--events` stream.
#[derive(Debug)]
pub struct AutoloopEventTailer {
    path: PathBuf,
    position: u64,
    /// Trailing bytes that form an incomplete UTF-8 sequence across a read.
    pending_bytes: Vec<u8>,
    /// Decoded text after the last newline — a JSON line not yet terminated.
    pending_line: String,
}

impl AutoloopEventTailer {
    /// Create a tailer for the events file at `path`. The file need not exist yet.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            position: 0,
            pending_bytes: Vec::new(),
            pending_line: String::new(),
        }
    }

    /// The events file being tailed.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn reset(&mut self) {
        self.position = 0;
        self.pending_bytes.clear();
        self.pending_line.clear();
    }

    /// Read newly-appended bytes and return the events decoded since the last
    /// poll. Empty when nothing new is available. A partial trailing line / torn
    /// multibyte char is buffered and surfaces on a later poll once completed;
    /// malformed complete lines are skipped (parity with `parse_events`).
    pub fn poll(&mut self) -> std::io::Result<Vec<AutoloopEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut file = File::open(&self.path)?;
        let len = file.metadata()?.len();
        if len < self.position {
            // Truncated / rotated underneath us — restart from the top.
            self.reset();
        }
        if len == self.position {
            return Ok(Vec::new());
        }

        file.seek(SeekFrom::Start(self.position))?;
        let mut raw = Vec::new();
        let read = file.take(len - self.position).read_to_end(&mut raw)?;
        self.position += read as u64;

        // Prepend bytes left undecoded from a previous poll.
        if !self.pending_bytes.is_empty() {
            let mut combined = std::mem::take(&mut self.pending_bytes);
            combined.extend_from_slice(&raw);
            raw = combined;
        }

        // Decode the maximal valid UTF-8 prefix; stash a torn trailing sequence.
        let text = match std::str::from_utf8(&raw) {
            Ok(s) => s.to_string(),
            Err(e) => {
                let valid = e.valid_up_to();
                self.pending_bytes = raw[valid..].to_vec();
                String::from_utf8(raw[..valid].to_vec())
                    .expect("valid_up_to slice is valid utf-8")
            }
        };

        // Combine with the unterminated line carried from the last poll, then
        // split on newlines. A trailing segment without a newline stays pending.
        let mut buf = std::mem::take(&mut self.pending_line);
        buf.push_str(&text);

        let ends_with_newline = buf.ends_with('\n');
        let mut parts: Vec<&str> = buf.split('\n').collect();
        if ends_with_newline {
            parts.pop(); // trailing empty segment after the final '\n'
        } else {
            // The last segment is an unterminated line — carry it forward.
            self.pending_line = parts.pop().unwrap_or("").to_string();
        }

        let mut events = Vec::new();
        for line in parts {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // A corrupt/partial line is skipped, not fatal (forward-compatible).
            if let Ok(ev) = serde_json::from_str::<AutoloopEvent>(trimmed) {
                events.push(ev);
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn append(path: &Path, s: &str) {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    const ITER: &str = r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#;
    const PROG: &str = r#"{"type":"progress","runId":"r1","iteration":1,"emittedTopic":"tasks.ready","outcome":"continue:routed_event","allowedRoles":["planner"]}"#;
    const FINISH: &str = r#"{"type":"loop.finish","iterations":2,"stopReason":"max_iterations","runId":"r1","costUsd":0.08}"#;

    #[test]
    fn emits_complete_events_incrementally() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.ndjson");
        let mut tailer = AutoloopEventTailer::new(&path);

        // Nothing yet.
        assert!(tailer.poll().unwrap().is_empty());

        append(&path, &format!("{ITER}\n{PROG}\n"));
        let first = tailer.poll().unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].kind, "iteration.start");
        assert_eq!(first[0].max_iterations, Some(3));
        assert_eq!(first[1].allowed_roles.as_deref(), Some(&["planner".to_string()][..]));

        // A second poll with no new bytes yields nothing.
        assert!(tailer.poll().unwrap().is_empty());

        append(&path, &format!("{FINISH}\n"));
        let second = tailer.poll().unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].kind, "loop.finish");
        assert_eq!(second[0].cost_usd, Some(0.08));
    }

    #[test]
    fn buffers_a_partial_final_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.ndjson");
        let mut tailer = AutoloopEventTailer::new(&path);

        // Write a line plus the first half of the next, with no terminating \n.
        let half = &ITER[..20];
        append(&path, &format!("{PROG}\n{half}"));
        let got = tailer.poll().unwrap();
        assert_eq!(got.len(), 1, "only the complete line is emitted");
        assert_eq!(got[0].kind, "progress");

        // Complete the buffered line on the next append.
        append(&path, &format!("{}\n", &ITER[20..]));
        let got = tailer.poll().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].kind, "iteration.start");
    }

    #[test]
    fn skips_malformed_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.ndjson");
        let mut tailer = AutoloopEventTailer::new(&path);

        append(&path, &format!("{ITER}\nnot json at all\n{{partial\n{FINISH}\n"));
        let got = tailer.poll().unwrap();
        assert_eq!(got.len(), 2, "two malformed lines skipped");
        assert_eq!(got[0].kind, "iteration.start");
        assert_eq!(got[1].kind, "loop.finish");
    }

    #[test]
    fn resets_on_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.ndjson");
        let mut tailer = AutoloopEventTailer::new(&path);

        // Three lines, then poll past them.
        append(&path, &format!("{ITER}\n{PROG}\n{FINISH}\n"));
        assert_eq!(tailer.poll().unwrap().len(), 3);

        // Shrink the file (rotation/restart) — len < position triggers a reset
        // and the tailer re-reads from the top.
        std::fs::write(&path, format!("{ITER}\n")).unwrap();
        let got = tailer.poll().unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].kind, "iteration.start");
    }
}
