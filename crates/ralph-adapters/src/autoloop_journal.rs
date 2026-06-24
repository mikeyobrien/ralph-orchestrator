//! Deterministic replay/derivation substrate for autoloop journals.
//!
//! Autoloop writes an append-only `journal.jsonl` under `.autoloop/`. Each line
//! is one JSON object describing a loop lifecycle event. Ralph tails this
//! journal live to derive run state (run id, iteration count, stop reason).
//! This module provides the *parsing and derivation* half of that contract so
//! it can be validated against a recorded fixture WITHOUT a live autoloop.
//!
//! ## Record shapes
//!
//! The journal mixes two on-disk shapes, both tolerated by [`AutoloopRecord`]:
//!
//! - **FieldsEvent** — lifecycle records carry a `fields` string map, e.g.
//!   `loop.start`, `iteration.start`, `backend.start`, `event.invalid`,
//!   `backend.finish`, `iteration.finish`, `loop.stop`.
//! - **PayloadEvent** — agent-emitted topics (e.g. `tasks.ready`) carry a
//!   `payload` string and a `source` instead of `fields`.
//!
//! `iteration` is stored as a JSON *string* (e.g. `"3"`) when present and is
//! absent on `loop.start` / `loop.stop`. Both are handled.
//!
//! ## Tailing contract
//!
//! [`replay_journal`] reads line-by-line and only yields *complete* lines: a
//! partial final line (no trailing newline yet, mid-write) is buffered and not
//! emitted until completed. [`JournalReplay`] makes this incremental so a
//! reader splitting the byte stream mid-line still recovers the exact same
//! event set as reading the whole file at once.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// Errors produced while parsing an autoloop journal.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    /// A line was syntactically valid JSON but not a JSON object, or was missing
    /// the required `topic`/`run` fields.
    #[error("journal line {line} is not a valid record: {reason}")]
    InvalidRecord { line: usize, reason: String },

    /// A line failed JSON deserialization.
    #[error("journal line {line} is not valid json: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
}

/// A single decoded journal record, normalized across the FieldsEvent and
/// PayloadEvent on-disk shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoloopRecord {
    /// The run id (`run` key) this record belongs to.
    pub run: String,
    /// The iteration index, when present. Absent for `loop.start`/`loop.stop`.
    pub iteration: Option<u32>,
    /// The event topic, e.g. `loop.start`, `tasks.ready`, `loop.stop`.
    pub topic: String,
    /// String-valued fields. For FieldsEvent records this is the `fields` map.
    /// For PayloadEvent records, `payload` and `source` are folded in here so
    /// downstream code has a single uniform map to read from.
    pub fields: BTreeMap<String, String>,
}

impl AutoloopRecord {
    /// Convenience accessor for a field value.
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

/// Raw deserialization target tolerant of both record shapes.
#[derive(Debug, Deserialize)]
struct RawRecord {
    run: Option<String>,
    topic: Option<String>,
    /// Stored as a JSON string in real journals (e.g. `"3"`); accept numbers too.
    #[serde(default)]
    iteration: Option<Value>,
    /// FieldsEvent shape.
    #[serde(default)]
    fields: Option<BTreeMap<String, Value>>,
    /// PayloadEvent shape.
    #[serde(default)]
    payload: Option<Value>,
    #[serde(default)]
    source: Option<Value>,
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn parse_iteration(v: &Value) -> Option<u32> {
    match v {
        Value::String(s) => s.trim().parse::<u32>().ok(),
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

/// Parse a single JSON object line into an [`AutoloopRecord`].
fn parse_line(line: &str, line_no: usize) -> Result<AutoloopRecord, JournalError> {
    let raw: RawRecord =
        serde_json::from_str(line).map_err(|source| JournalError::Json { line: line_no, source })?;

    let topic = raw.topic.ok_or_else(|| JournalError::InvalidRecord {
        line: line_no,
        reason: "missing `topic`".to_string(),
    })?;
    let run = raw.run.ok_or_else(|| JournalError::InvalidRecord {
        line: line_no,
        reason: "missing `run`".to_string(),
    })?;

    let iteration = raw.iteration.as_ref().and_then(parse_iteration);

    // Normalize both shapes into a single string map.
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    if let Some(map) = raw.fields {
        for (k, v) in map {
            fields.insert(k, value_to_string(&v));
        }
    }
    if let Some(payload) = raw.payload.as_ref() {
        fields.insert("payload".to_string(), value_to_string(payload));
    }
    if let Some(source) = raw.source.as_ref() {
        fields.insert("source".to_string(), value_to_string(source));
    }

    Ok(AutoloopRecord {
        run,
        iteration,
        topic,
        fields,
    })
}

/// Replay a whole journal string into the complete set of records.
///
/// A trailing partial line (no terminating newline) is treated as not-yet-written
/// and is silently skipped, mirroring how a live tailer must avoid consuming a
/// half-flushed final line. Fully-formed lines (including the last one when it
/// ends with `\n`) are parsed. Blank lines are ignored.
pub fn replay_journal(content: &str) -> Result<Vec<AutoloopRecord>, JournalError> {
    let mut replay = JournalReplay::new();
    replay.push(content)?;
    // Do NOT finish(): replay_journal models a snapshot read where the final
    // line is only trusted once newline-terminated.
    Ok(replay.into_records())
}

/// Incremental journal replay that tolerates being fed the byte stream in
/// arbitrary chunks, including splits in the middle of a line.
///
/// Only newline-terminated lines are parsed and appended; any trailing bytes are
/// buffered until the rest of the line arrives. This is the core of the
/// journal-tailing contract: a partial final line is never dropped or
/// half-parsed.
#[derive(Debug, Default)]
pub struct JournalReplay {
    buffer: String,
    records: Vec<AutoloopRecord>,
    line_no: usize,
    error: Option<JournalError>,
}

impl JournalReplay {
    /// Create an empty replay.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed the next chunk of journal bytes. Complete lines are parsed
    /// immediately; an incomplete trailing line is buffered for the next push.
    pub fn push(&mut self, chunk: &str) -> Result<(), JournalError> {
        if self.error.is_some() {
            // Once errored, stay errored deterministically.
            return Err(self.take_error());
        }
        self.buffer.push_str(chunk);

        // Drain every complete (newline-terminated) line out of the buffer.
        while let Some(idx) = self.buffer.find('\n') {
            // Split off the line including the newline, keep the remainder.
            let remainder = self.buffer.split_off(idx + 1);
            let mut line = std::mem::replace(&mut self.buffer, remainder);
            // line currently ends with '\n'; trim line terminators.
            while line.ends_with('\n') || line.ends_with('\r') {
                line.pop();
            }
            self.line_no += 1;
            if line.trim().is_empty() {
                continue;
            }
            match parse_line(&line, self.line_no) {
                Ok(rec) => self.records.push(rec),
                Err(e) => {
                    self.error = Some(e);
                    return Err(self.take_error());
                }
            }
        }
        Ok(())
    }

    /// Flush a buffered final line that has no trailing newline. Call this only
    /// when the stream is known to be complete (e.g. process exited). A snapshot
    /// tailer that may still be receiving bytes should NOT call this.
    pub fn finish(&mut self) -> Result<(), JournalError> {
        if self.error.is_some() {
            return Err(self.take_error());
        }
        let leftover = std::mem::take(&mut self.buffer);
        if !leftover.trim().is_empty() {
            self.line_no += 1;
            match parse_line(leftover.trim_end_matches(['\r', '\n']), self.line_no) {
                Ok(rec) => self.records.push(rec),
                Err(e) => {
                    self.error = Some(e);
                    return Err(self.take_error());
                }
            }
        }
        Ok(())
    }

    fn take_error(&mut self) -> JournalError {
        self.error.take().expect("error present")
    }

    /// Records parsed so far.
    pub fn records(&self) -> &[AutoloopRecord] {
        &self.records
    }

    /// Consume the replay and return all parsed records.
    pub fn into_records(self) -> Vec<AutoloopRecord> {
        self.records
    }
}

/// Summary derived from a journal, mirroring what Ralph computes live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    /// Run id, taken from the `loop.start` record (falls back to any record's run).
    pub run_id: String,
    /// Number of distinct iterations observed (count of `iteration.start`,
    /// falling back to the max `iteration` index seen).
    pub iterations: u32,
    /// Stop reason from the `loop.stop` record's `reason` field, when present.
    pub stop_reason: Option<String>,
}

/// Derive a [`RunSummary`] from a slice of decoded records.
///
/// - `run_id` comes from the `loop.start` record (or the first record's `run`).
/// - `iterations` counts `iteration.start` records; if none are present it falls
///   back to the maximum `iteration` index observed on any record.
/// - `stop_reason` is the `reason` field of the `loop.stop` record.
pub fn derive_run_summary(records: &[AutoloopRecord]) -> RunSummary {
    let run_id = records
        .iter()
        .find(|r| r.topic == "loop.start")
        .or_else(|| records.first())
        .map(|r| r.run.clone())
        .unwrap_or_default();

    let iteration_starts = records
        .iter()
        .filter(|r| r.topic == "iteration.start")
        .count() as u32;

    let iterations = if iteration_starts > 0 {
        iteration_starts
    } else {
        records.iter().filter_map(|r| r.iteration).max().unwrap_or(0)
    };

    let stop_reason = records
        .iter()
        .find(|r| r.topic == "loop.stop")
        .and_then(|r| r.field("reason"))
        .map(str::to_string);

    RunSummary {
        run_id,
        iterations,
        stop_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        // Compact hand-authored journal matching the contract event shapes.
        concat!(
            r#"{"run":"r1","topic":"loop.start","fields":{"max_iterations":"2","objective":"o","reason":"x"}}"#,
            "\n",
            r#"{"run":"r1","iteration":"1","topic":"iteration.start","fields":{"recent_event":"loop.start"}}"#,
            "\n",
            r#"{"run":"r1","iteration":"1","topic":"backend.start","fields":{"backend_kind":"command"}}"#,
            "\n",
            r#"{"run":"r1","iteration":"1","topic":"tasks.ready","payload":"ready","source":"agent"}"#,
            "\n",
            r#"{"run":"r1","iteration":"1","topic":"backend.finish","fields":{"exit_code":"0"}}"#,
            "\n",
            r#"{"run":"r1","iteration":"1","topic":"iteration.finish","fields":{"exit_code":"0"}}"#,
            "\n",
            r#"{"run":"r1","iteration":"2","topic":"iteration.start","fields":{"recent_event":"tasks.ready"}}"#,
            "\n",
            r#"{"run":"r1","iteration":"2","topic":"event.invalid","fields":{"emitted":"tasks.ready"}}"#,
            "\n",
            r#"{"run":"r1","topic":"loop.stop","fields":{"reason":"max_iterations","max_iterations":"2"}}"#,
            "\n",
        )
    }

    #[test]
    fn parses_both_record_shapes() {
        let recs = replay_journal(sample()).unwrap();
        assert_eq!(recs.len(), 9);

        // FieldsEvent without iteration.
        assert_eq!(recs[0].topic, "loop.start");
        assert_eq!(recs[0].iteration, None);
        assert_eq!(recs[0].field("objective"), Some("o"));

        // FieldsEvent with iteration (string -> u32).
        assert_eq!(recs[1].topic, "iteration.start");
        assert_eq!(recs[1].iteration, Some(1));

        // PayloadEvent folds payload/source into fields.
        let payload_rec = &recs[3];
        assert_eq!(payload_rec.topic, "tasks.ready");
        assert_eq!(payload_rec.iteration, Some(1));
        assert_eq!(payload_rec.field("payload"), Some("ready"));
        assert_eq!(payload_rec.field("source"), Some("agent"));
    }

    #[test]
    fn derive_summary_from_sample() {
        let recs = replay_journal(sample()).unwrap();
        let summary = derive_run_summary(&recs);
        assert_eq!(summary.run_id, "r1");
        assert_eq!(summary.iterations, 2);
        assert_eq!(summary.stop_reason.as_deref(), Some("max_iterations"));
    }

    #[test]
    fn iteration_fallback_to_max_index_when_no_iteration_start() {
        // No iteration.start records; iterations should fall back to max index.
        let content = concat!(
            r#"{"run":"r2","topic":"loop.start","fields":{}}"#,
            "\n",
            r#"{"run":"r2","iteration":"5","topic":"backend.start","fields":{}}"#,
            "\n",
            r#"{"run":"r2","topic":"loop.stop","fields":{"reason":"completed"}}"#,
            "\n",
        );
        let recs = replay_journal(content).unwrap();
        let summary = derive_run_summary(&recs);
        assert_eq!(summary.iterations, 5);
        assert_eq!(summary.stop_reason.as_deref(), Some("completed"));
    }

    #[test]
    fn partial_final_line_is_not_dropped_when_split_mid_line() {
        let full = sample();
        // Split the byte stream at every interior position; each split must
        // recover the same record set as a whole-file read.
        let whole = replay_journal(full).unwrap();

        let bytes = full.as_bytes();
        for split in 1..bytes.len() {
            // Ensure we split on a char boundary (sample is ASCII, but be safe).
            if !full.is_char_boundary(split) {
                continue;
            }
            let (a, b) = full.split_at(split);
            let mut replay = JournalReplay::new();
            replay.push(a).unwrap();
            replay.push(b).unwrap();
            assert_eq!(
                replay.into_records(),
                whole,
                "split at {split} diverged from whole-file replay"
            );
        }
    }

    #[test]
    fn snapshot_read_skips_unterminated_final_line() {
        // A final line with no trailing newline is treated as half-written.
        let content = concat!(
            r#"{"run":"r3","topic":"loop.start","fields":{}}"#,
            "\n",
            r#"{"run":"r3","topic":"loop.st"#, // truncated, no newline
        );
        let recs = replay_journal(content).unwrap();
        assert_eq!(recs.len(), 1, "truncated trailing line must be skipped");

        // finish() must NOT rescue an invalid truncated line as a record.
        let mut replay = JournalReplay::new();
        replay.push(content).unwrap();
        assert!(replay.finish().is_err());
    }
}
