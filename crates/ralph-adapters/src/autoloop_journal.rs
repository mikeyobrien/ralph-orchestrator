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
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

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
}

impl JournalReplay {
    /// Create an empty replay.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed the next chunk of journal bytes. Every complete (newline-terminated)
    /// line in the buffer is parsed immediately; an incomplete trailing line is
    /// buffered for the next push.
    ///
    /// A line that fails to parse is SKIPPED — the tailer is resilient to an
    /// isolated corrupt line and keeps parsing the rest of the chunk, so valid
    /// records before and after the bad line are all recovered (available via
    /// [`records`](Self::records)). The first parse error encountered in the
    /// chunk is returned so a caller can surface it; subsequent pushes start
    /// clean.
    pub fn push(&mut self, chunk: &str) -> Result<(), JournalError> {
        self.buffer.push_str(chunk);

        let mut first_err: Option<JournalError> = None;
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
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Flush a buffered final line that has no trailing newline. Call this only
    /// when the stream is known to be complete (e.g. process exited). A snapshot
    /// tailer that may still be receiving bytes should NOT call this. Returns an
    /// error if the leftover line is non-empty but unparseable.
    pub fn finish(&mut self) -> Result<(), JournalError> {
        let leftover = std::mem::take(&mut self.buffer);
        if !leftover.trim().is_empty() {
            self.line_no += 1;
            let rec = parse_line(leftover.trim_end_matches(['\r', '\n']), self.line_no)?;
            self.records.push(rec);
        }
        Ok(())
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

    // Terminal record: `loop.complete` for a successful finish, `loop.stop`
    // for a forced termination (max_iterations/backend_failed/stalled/
    // backend_timeout/cost_budget/max_runtime). Both carry a `reason`. Scan
    // from the end so the last terminal record wins.
    let stop_reason = records
        .iter()
        .rev()
        .find(|r| r.topic == "loop.complete" || r.topic == "loop.stop")
        .and_then(|r| r.field("reason"))
        .map(str::to_string);

    RunSummary {
        run_id,
        iterations,
        stop_reason,
    }
}

/// Live, incrementally-updated view of a run, folded from journal records as
/// they arrive. This is the state Ralph surfaces to the TUI / loop registry
/// while an `autoloop` subprocess is running, derived purely by tailing the
/// journal (the only cross-process observability channel autoloop exposes).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LiveRunState {
    /// Run id, taken from the first record observed.
    pub run_id: Option<String>,
    /// The most recent iteration index seen on any record.
    pub current_iteration: Option<u32>,
    /// Count of `iteration.start` records seen so far.
    pub iteration_count: u32,
    /// The topic of the most recent record.
    pub last_topic: Option<String>,
    /// Stop reason, set when a `loop.stop` record is observed.
    pub stop_reason: Option<String>,
    /// True once a `loop.stop` record has been observed.
    pub completed: bool,
}

impl LiveRunState {
    /// Fold a single record into the running state.
    pub fn apply(&mut self, rec: &AutoloopRecord) {
        // `loop.start` is authoritative for the run id; otherwise take the
        // first record seen (matches [`derive_run_summary`]).
        if rec.topic == "loop.start" || self.run_id.is_none() {
            self.run_id = Some(rec.run.clone());
        }
        if let Some(it) = rec.iteration {
            self.current_iteration = Some(it);
        }
        match rec.topic.as_str() {
            "iteration.start" => self.iteration_count += 1,
            // `loop.complete` = successful finish; `loop.stop` = forced
            // termination. Both are terminal and carry a `reason`.
            "loop.complete" | "loop.stop" => {
                self.completed = true;
                if let Some(reason) = rec.field("reason") {
                    self.stop_reason = Some(reason.to_string());
                }
            }
            _ => {}
        }
        self.last_topic = Some(rec.topic.clone());
    }

    /// Fold a batch of records in order.
    pub fn apply_all(&mut self, recs: &[AutoloopRecord]) {
        for rec in recs {
            self.apply(rec);
        }
    }
}

/// Error produced while tailing a journal file on disk.
#[derive(Debug, thiserror::Error)]
pub enum TailError {
    /// Filesystem error reading the journal.
    #[error("i/o error tailing journal: {0}")]
    Io(#[from] std::io::Error),
    /// A journal line failed to parse.
    #[error(transparent)]
    Journal(#[from] JournalError),
}

/// Incremental, file-backed tailer for a live `autoloop` journal.
///
/// Each [`poll`](Self::poll) reads only the bytes appended since the previous
/// poll, decodes the maximal valid UTF-8 prefix (buffering a trailing partial
/// multibyte sequence), and feeds them to an internal [`JournalReplay`] so a
/// half-written final *line* is buffered rather than mis-parsed or dropped. The
/// file position advances monotonically; truncation/rotation (file shorter than
/// the last position) resets the tailer to re-read from the start.
#[derive(Debug)]
pub struct AutoloopJournalTailer {
    path: PathBuf,
    position: u64,
    /// Undecoded trailing bytes (a multibyte char split across a read boundary).
    pending: Vec<u8>,
    replay: JournalReplay,
    state: LiveRunState,
    errors: Vec<JournalError>,
}

impl AutoloopJournalTailer {
    /// Create a tailer for the journal at `path`. The file need not exist yet.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            position: 0,
            pending: Vec::new(),
            replay: JournalReplay::new(),
            state: LiveRunState::default(),
            errors: Vec::new(),
        }
    }

    /// The journal path being tailed.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The live state folded from all records observed so far.
    pub fn state(&self) -> &LiveRunState {
        &self.state
    }

    /// All records parsed so far.
    pub fn records(&self) -> &[AutoloopRecord] {
        self.replay.records()
    }

    /// Parse errors encountered while tailing. Corrupt lines are skipped rather
    /// than fatal, and recorded here; empty for a well-formed journal.
    pub fn errors(&self) -> &[JournalError] {
        &self.errors
    }

    fn reset(&mut self) {
        self.position = 0;
        self.pending.clear();
        self.replay = JournalReplay::new();
        self.state = LiveRunState::default();
        self.errors.clear();
    }

    /// Read newly-appended journal bytes and return the records parsed since the
    /// last poll. Returns an empty vec when nothing new is available. A partial
    /// final line (or a torn multibyte char) is buffered internally and surfaces
    /// on a later poll once completed.
    pub fn poll(&mut self) -> Result<Vec<AutoloopRecord>, TailError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut file = File::open(&self.path)?;
        let len = file.metadata()?.len();
        if len < self.position {
            // File truncated or rotated underneath us — restart from the top.
            self.reset();
        }
        if len == self.position {
            return Ok(Vec::new());
        }

        file.seek(SeekFrom::Start(self.position))?;
        let mut raw = Vec::new();
        let read = file.take(len - self.position).read_to_end(&mut raw)?;
        self.position += read as u64;

        // Prepend any bytes left undecoded from the previous poll.
        if !self.pending.is_empty() {
            let mut combined = std::mem::take(&mut self.pending);
            combined.extend_from_slice(&raw);
            raw = combined;
        }

        // Decode the maximal valid UTF-8 prefix; keep a trailing partial
        // multibyte sequence for the next poll.
        let text = match std::str::from_utf8(&raw) {
            Ok(s) => s.to_string(),
            Err(e) => {
                let valid = e.valid_up_to();
                let text = String::from_utf8(raw[..valid].to_vec())
                    .expect("valid_up_to slice is valid utf-8");
                self.pending = raw[valid..].to_vec();
                text
            }
        };

        let before = self.replay.records().len();
        // A corrupt line is skipped, not fatal: record the error so a caller can
        // surface it via `errors()` while still receiving the valid records.
        if let Err(e) = self.replay.push(&text) {
            self.errors.push(e);
        }
        let new: Vec<AutoloopRecord> = self.replay.records()[before..].to_vec();
        self.state.apply_all(&new);
        Ok(new)
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

    fn append(path: &Path, s: &str) {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    #[test]
    fn tailer_emits_complete_records_incrementally() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        let mut tailer = AutoloopJournalTailer::new(&path);

        // Polling a not-yet-existent journal yields nothing, not an error.
        assert!(tailer.poll().unwrap().is_empty());

        append(&path, "{\"run\":\"r\",\"topic\":\"loop.start\",\"fields\":{}}\n");
        let recs = tailer.poll().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].topic, "loop.start");

        // Append a complete line plus the FIRST half of another (no newline).
        append(
            &path,
            "{\"run\":\"r\",\"iteration\":\"1\",\"topic\":\"iteration.start\",\"fields\":{}}\n{\"run\":\"r\",\"iteration\":\"1\",\"topic\":\"ba",
        );
        let recs = tailer.poll().unwrap();
        assert_eq!(recs.len(), 1, "only the complete line should surface");
        assert_eq!(recs[0].topic, "iteration.start");

        // Complete the half-written line; it must now surface, not be dropped.
        append(&path, "ckend.start\",\"fields\":{}}\n");
        let recs = tailer.poll().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].topic, "backend.start");

        // Nothing new -> empty.
        assert!(tailer.poll().unwrap().is_empty());
    }

    #[test]
    fn tailer_tracks_live_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        append(&path, sample());

        let mut tailer = AutoloopJournalTailer::new(&path);
        tailer.poll().unwrap();
        let st = tailer.state();
        assert_eq!(st.run_id.as_deref(), Some("r1"));
        assert_eq!(st.iteration_count, 2);
        assert_eq!(st.current_iteration, Some(2));
        assert!(st.completed);
        assert_eq!(st.stop_reason.as_deref(), Some("max_iterations"));
        assert_eq!(st.last_topic.as_deref(), Some("loop.stop"));
    }

    #[test]
    fn tailer_resets_on_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        append(&path, sample());
        let mut tailer = AutoloopJournalTailer::new(&path);
        tailer.poll().unwrap();
        assert_eq!(tailer.state().run_id.as_deref(), Some("r1"));

        // Truncate and write a fresh run (e.g. a new run reusing the path).
        std::fs::write(&path, "").unwrap();
        append(
            &path,
            concat!(
                r#"{"run":"r9","topic":"loop.start","fields":{}}"#,
                "\n",
                r#"{"run":"r9","topic":"loop.stop","fields":{"reason":"completed"}}"#,
                "\n",
            ),
        );
        let recs = tailer.poll().unwrap();
        assert_eq!(recs.len(), 2, "tailer should re-read from the top after truncation");
        assert_eq!(tailer.state().run_id.as_deref(), Some("r9"));
        assert_eq!(tailer.state().stop_reason.as_deref(), Some("completed"));
        assert!(tailer.state().completed);
    }

    #[test]
    fn tailer_handles_multibyte_split_across_reads() {
        // A non-ASCII payload whose UTF-8 bytes are split across two appends must
        // still decode correctly (the partial multibyte tail is buffered).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        let mut tailer = AutoloopJournalTailer::new(&path);

        let line = "{\"run\":\"r\",\"iteration\":\"1\",\"topic\":\"tasks.ready\",\"payload\":\"café\",\"source\":\"agent\"}\n";
        let bytes = line.as_bytes();
        // Find a split index that lands inside the 'é' (2-byte) sequence.
        let e_pos = line.find('é').unwrap();
        let split = e_pos + 1; // mid-multibyte
        assert!(!line.is_char_boundary(split));
        append(&path, std::str::from_utf8(&bytes[..e_pos]).unwrap());
        // Write the raw partial multibyte by bytes.
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
            f.write_all(&bytes[e_pos..split]).unwrap();
        }
        // No complete UTF-8 line yet.
        assert!(tailer.poll().unwrap().is_empty());
        // Write the remainder.
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
            f.write_all(&bytes[split..]).unwrap();
        }
        let recs = tailer.poll().unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].field("payload"), Some("café"));
    }

    #[test]
    fn loop_complete_is_terminal_on_the_happy_path() {
        // A SUCCESSFUL autoloop run journals `loop.complete` (with reason),
        // not `loop.stop`. Both derivation paths must treat it as terminal.
        let content = concat!(
            r#"{"run":"r","topic":"loop.start","fields":{}}"#,
            "\n",
            r#"{"run":"r","iteration":"1","topic":"iteration.start","fields":{}}"#,
            "\n",
            r#"{"run":"r","iteration":"1","topic":"loop.complete","fields":{"reason":"completed"}}"#,
            "\n",
        );

        // derive_run_summary
        let recs = replay_journal(content).unwrap();
        let summary = derive_run_summary(&recs);
        assert_eq!(summary.stop_reason.as_deref(), Some("completed"));
        assert_eq!(summary.iterations, 1);

        // LiveRunState via the file tailer
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        append(&path, content);
        let mut tailer = AutoloopJournalTailer::new(&path);
        tailer.poll().unwrap();
        assert!(tailer.state().completed, "loop.complete must mark completion");
        assert_eq!(tailer.state().stop_reason.as_deref(), Some("completed"));
    }

    #[test]
    fn tailer_skips_corrupt_line_and_continues() {
        // An isolated corrupt line is skipped (recorded in errors()), not fatal;
        // valid records before AND after it are still surfaced.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        append(
            &path,
            concat!(
                r#"{"run":"r","topic":"loop.start","fields":{}}"#,
                "\n",
                "{not valid json}\n",
                r#"{"run":"r","topic":"loop.complete","fields":{"reason":"completed"}}"#,
                "\n",
            ),
        );
        let mut tailer = AutoloopJournalTailer::new(&path);
        let recs = tailer.poll().unwrap();
        assert_eq!(recs.len(), 2, "valid records around the corrupt line survive");
        assert_eq!(recs[0].topic, "loop.start");
        assert_eq!(recs[1].topic, "loop.complete");
        assert_eq!(tailer.errors().len(), 1, "the corrupt line is recorded as an error");
        assert!(tailer.state().completed);
        assert_eq!(tailer.state().stop_reason.as_deref(), Some("completed"));
    }
}
