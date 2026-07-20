//! Bounded incremental tailing of an active autoloop backend stream.
//!
//! Backend streams can grow to hundreds of megabytes. This tailer therefore
//! reads at most [`MAX_BYTES_PER_POLL`] bytes per poll, retains at most that
//! much unfinished NDJSON internally, returns at most [`MAX_STREAM_LINES`]
//! display items per poll, and truncates each display line to
//! [`MAX_STREAM_LINE_BYTES`].

use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::tool_preview::format_tool_summary;

/// Maximum bytes read from a backend stream during one poll (256 KiB).
pub const MAX_BYTES_PER_POLL: usize = 256 * 1024;
/// Maximum display lines emitted for one iteration.
pub const MAX_STREAM_LINES: usize = 2_000;
/// Maximum UTF-8 bytes retained in one display line.
pub const MAX_STREAM_LINE_BYTES: usize = 4 * 1024;

/// Stable, presentation-independent identity for one tool invocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolCallIdentity(String);

/// One displayable item extracted from a backend's NDJSON stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamLine {
    /// Text emitted by the assistant.
    AgentText(String),
    /// A concise summary of a distinct tool invocation.
    ToolSummary {
        /// Stable backend ID, or a bounded structured-event fingerprint.
        identity: ToolCallIdentity,
        /// Human-readable bounded summary.
        text: String,
    },
    /// Cumulative bytes discarded to keep stream reads bounded.
    Backpressure {
        /// Total bytes discarded by this tailer so far.
        skipped_bytes: u64,
    },
}

#[derive(Debug, Clone, Copy)]
enum StreamFormat {
    Claude,
    Pi,
}

#[derive(Debug)]
struct SelectedStream {
    path: PathBuf,
    format: StreamFormat,
}

/// Incrementally reads one iteration's Claude or Pi stream file.
#[derive(Debug)]
pub struct BackendStreamTailer {
    workspace_root: PathBuf,
    run_dir: PathBuf,
    iteration: u32,
    selected: Option<SelectedStream>,
    position: u64,
    pending: Vec<u8>,
    skipped_bytes: u64,
    seen_tool_ids: HashSet<ToolCallIdentity>,
    tool_id_order: VecDeque<ToolCallIdentity>,
}

impl BackendStreamTailer {
    /// Create a tailer for an iteration. Its stream file need not exist yet.
    ///
    /// `workspace_root` and `run_dir` provide the presentation boundary for
    /// tool paths: repository files are workspace-relative and private run
    /// state is shown with an `engine:` prefix.
    pub fn for_iteration(workspace_root: &Path, run_dir: &Path, iteration: u32) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
            run_dir: run_dir.to_path_buf(),
            iteration,
            selected: None,
            position: 0,
            pending: Vec::new(),
            skipped_bytes: 0,
            seen_tool_ids: HashSet::new(),
            tool_id_order: VecDeque::new(),
        }
    }

    /// Return displayable items appended since the previous poll.
    ///
    /// Backpressure is cumulative metadata. A caller should replace its prior
    /// status when a newer `Backpressure` item arrives rather than append it.
    pub fn poll(&mut self) -> io::Result<Vec<StreamLine>> {
        if self.selected.is_none() {
            self.selected = self.probe_stream();
        }
        let Some(selected) = &self.selected else {
            return Ok(Vec::new());
        };
        let format = selected.format;

        let mut file = match File::open(&selected.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let len = file.metadata()?.len();
        if len < self.position {
            self.position = 0;
            self.pending.clear();
        }
        if len == self.position {
            return Ok(Vec::new());
        }

        let skipped_before = self.skipped_bytes;
        let unread = len - self.position;
        let mut must_align = false;
        if unread > MAX_BYTES_PER_POLL as u64 {
            let start = len - MAX_BYTES_PER_POLL as u64;
            self.record_skipped(start.saturating_sub(self.position));
            self.position = start;
            self.record_skipped(self.pending.len() as u64);
            self.pending.clear();
            must_align = true;
        }

        file.seek(SeekFrom::Start(self.position))?;
        let mut bytes = Vec::with_capacity((len - self.position) as usize);
        let read = file
            .take(MAX_BYTES_PER_POLL as u64)
            .read_to_end(&mut bytes)?;
        self.position += read as u64;

        if must_align {
            if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
                self.record_skipped((newline + 1) as u64);
                bytes.drain(..=newline);
            } else {
                self.record_skipped(bytes.len() as u64);
                return Ok(self.backpressure_update(skipped_before));
            }
        }

        self.pending.extend_from_slice(&bytes);
        if self.pending.len() > MAX_BYTES_PER_POLL {
            let excess = self.pending.len() - MAX_BYTES_PER_POLL;
            self.pending.drain(..excess);
            self.record_skipped(excess as u64);
            if let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
                self.pending.drain(..=newline);
                self.record_skipped((newline + 1) as u64);
            } else {
                let discarded = self.pending.len();
                self.pending.clear();
                self.record_skipped(discarded as u64);
            }
        }

        let complete_len = self
            .pending
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let complete = self.pending.drain(..complete_len).collect::<Vec<_>>();

        let status_changed = self.skipped_bytes != skipped_before;
        let useful_limit = MAX_STREAM_LINES.saturating_sub(usize::from(status_changed));
        let mut newest = VecDeque::with_capacity(useful_limit);
        for raw_line in complete.split(|byte| *byte == b'\n') {
            let Ok(line) = std::str::from_utf8(raw_line) else {
                continue;
            };
            for parsed in parse_stream_line(format, line, &self.workspace_root, &self.run_dir) {
                if let StreamLine::ToolSummary { identity, .. } = &parsed {
                    if !self.remember_tool(identity.clone()) {
                        continue;
                    }
                }
                if useful_limit == 0 {
                    continue;
                }
                if newest.len() == useful_limit {
                    newest.pop_front();
                }
                newest.push_back(parsed);
            }
        }

        let mut lines = self.backpressure_update(skipped_before);
        lines.extend(newest);
        debug_assert!(lines.len() <= MAX_STREAM_LINES);
        Ok(lines)
    }

    fn probe_stream(&self) -> Option<SelectedStream> {
        let claude = self
            .run_dir
            .join(format!("claude-stream.{}.jsonl", self.iteration));
        if claude.exists() {
            return Some(SelectedStream {
                path: claude,
                format: StreamFormat::Claude,
            });
        }

        let pi = self
            .run_dir
            .join(format!("pi-stream.{}.jsonl", self.iteration));
        pi.exists().then_some(SelectedStream {
            path: pi,
            format: StreamFormat::Pi,
        })
    }

    fn record_skipped(&mut self, bytes: u64) {
        self.skipped_bytes = self.skipped_bytes.saturating_add(bytes);
    }

    fn backpressure_update(&self, previous: u64) -> Vec<StreamLine> {
        if self.skipped_bytes == previous {
            Vec::new()
        } else {
            vec![StreamLine::Backpressure {
                skipped_bytes: self.skipped_bytes,
            }]
        }
    }

    fn remember_tool(&mut self, identity: ToolCallIdentity) -> bool {
        if self.seen_tool_ids.contains(&identity) {
            return false;
        }
        if self.tool_id_order.len() == MAX_STREAM_LINES {
            if let Some(expired) = self.tool_id_order.pop_front() {
                self.seen_tool_ids.remove(&expired);
            }
        }
        self.seen_tool_ids.insert(identity.clone());
        self.tool_id_order.push_back(identity);
        true
    }
}

fn parse_stream_line(
    format: StreamFormat,
    line: &str,
    workspace_root: &Path,
    run_dir: &Path,
) -> Vec<StreamLine> {
    let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
        return Vec::new();
    };
    match format {
        StreamFormat::Claude => parse_claude_event(&event, workspace_root, run_dir),
        StreamFormat::Pi => parse_pi_event(&event, workspace_root, run_dir),
    }
}

fn parse_claude_event(event: &Value, workspace_root: &Path, run_dir: &Path) -> Vec<StreamLine> {
    if event.get("type").and_then(Value::as_str) != Some("assistant") {
        return Vec::new();
    }
    content_lines(event.pointer("/message/content"), workspace_root, run_dir)
}

fn parse_pi_event(event: &Value, workspace_root: &Path, run_dir: &Path) -> Vec<StreamLine> {
    match event.get("type").and_then(Value::as_str) {
        Some("message_end")
            if event.pointer("/message/role").and_then(Value::as_str) == Some("assistant") =>
        {
            content_lines(event.pointer("/message/content"), workspace_root, run_dir)
        }
        Some("tool_execution_start") => {
            let Some(name) = event.get("toolName").and_then(Value::as_str) else {
                return Vec::new();
            };
            vec![tool_line(
                name,
                event.get("args").unwrap_or(&Value::Null),
                event.get("toolCallId").and_then(Value::as_str),
                event,
                workspace_root,
                run_dir,
            )]
        }
        _ => Vec::new(),
    }
}

fn content_lines(
    content: Option<&Value>,
    workspace_root: &Path,
    run_dir: &Path,
) -> Vec<StreamLine> {
    let Some(blocks) = content.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    push_text_lines(&mut lines, text);
                }
            }
            Some("tool_use" | "toolCall") => {
                let Some(name) = block.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let input = block
                    .get("input")
                    .or_else(|| block.get("arguments"))
                    .or_else(|| block.get("args"))
                    .unwrap_or(&Value::Null);
                let stable_id = block
                    .get("id")
                    .or_else(|| block.get("toolCallId"))
                    .and_then(Value::as_str);
                lines.push(tool_line(
                    name,
                    input,
                    stable_id,
                    block,
                    workspace_root,
                    run_dir,
                ));
            }
            _ => {}
        }
    }
    lines
}

fn push_text_lines(lines: &mut Vec<StreamLine>, text: &str) {
    lines.extend(
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| StreamLine::AgentText(truncate_utf8(line, MAX_STREAM_LINE_BYTES))),
    );
}

fn tool_line(
    name: &str,
    input: &Value,
    stable_id: Option<&str>,
    structured_event: &Value,
    workspace_root: &Path,
    run_dir: &Path,
) -> StreamLine {
    let presented_input = present_tool_paths(input, workspace_root, run_dir);
    let detail = format_tool_summary(name, &presented_input).map(|value| one_line(&value));
    let summary = match detail {
        Some(detail) if !detail.is_empty() => format!("⚙ {name}: {detail}"),
        _ => format!("⚙ {name}"),
    };
    StreamLine::ToolSummary {
        identity: tool_identity(stable_id, structured_event),
        text: truncate_utf8(&summary, MAX_STREAM_LINE_BYTES),
    }
}

fn tool_identity(stable_id: Option<&str>, structured_event: &Value) -> ToolCallIdentity {
    if let Some(id) = stable_id {
        return ToolCallIdentity(format!("id:{id}"));
    }

    let mut hasher = DefaultHasher::new();
    serde_json::to_vec(structured_event)
        .unwrap_or_default()
        .hash(&mut hasher);
    ToolCallIdentity(format!("event:{:016x}", hasher.finish()))
}

fn present_tool_paths(input: &Value, workspace_root: &Path, run_dir: &Path) -> Value {
    let Value::Object(fields) = input else {
        return input.clone();
    };

    let mut presented = fields.clone();
    for key in ["path", "file_path", "filePath", "notebook_path"] {
        let Some(path) = fields.get(key).and_then(Value::as_str) else {
            continue;
        };
        presented.insert(
            key.to_string(),
            Value::String(format_tool_path(path, workspace_root, run_dir)),
        );
    }
    Value::Object(presented)
}

/// Formats a tool path for display without exposing workspace or private run
/// directory prefixes. Paths outside the workspace remain unchanged.
fn format_tool_path(path: &str, workspace_root: &Path, run_dir: &Path) -> String {
    let path = Path::new(path);
    if let Ok(relative) = path.strip_prefix(run_dir) {
        return format!("engine:{}", relative.display());
    }
    if let Ok(relative) = path.strip_prefix(workspace_root) {
        return relative.display().to_string();
    }
    path.display().to_string()
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes.saturating_sub('…'.len_utf8());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn append(path: &Path, text: &str) {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    fn tool(id: &str, text: impl Into<String>) -> StreamLine {
        StreamLine::ToolSummary {
            identity: ToolCallIdentity(format!("id:{id}")),
            text: text.into(),
        }
    }

    #[test]
    fn claude_stream_emits_only_new_assistant_text_and_tool_calls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude-stream.3.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 3);

        append(
            &path,
            concat!(
                r#"{"type":"system","session_id":"s"}"#,
                "\n",
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"first line"},{"type":"tool_use","id":"t1","name":"read","input":{"path":"src/main.rs"}}]}}"#,
                "\n",
            ),
        );

        assert_eq!(
            tailer.poll().unwrap(),
            vec![
                StreamLine::AgentText("first line".into()),
                tool("t1", "⚙ read: src/main.rs"),
            ]
        );
        assert!(tailer.poll().unwrap().is_empty());

        append(
            &path,
            concat!(
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"second line"}]}}"#,
                "\n",
            ),
        );
        assert_eq!(
            tailer.poll().unwrap(),
            vec![StreamLine::AgentText("second line".into())]
        );
    }

    #[test]
    fn pi_stream_emits_assistant_message_end_text_and_tool_calls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi-stream.8.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 8);

        append(
            &path,
            concat!(
                r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"checking now"},{"type":"toolCall","toolCallId":"p1","name":"grep","arguments":{"pattern":"needle"}}]}}"#,
                "\n",
                r#"{"type":"tool_execution_start","toolCallId":"p2","toolName":"bash","args":{"command":"cargo test\n--workspace"}}"#,
                "\n",
                r#"{"type":"message_end","message":{"role":"user","content":[{"type":"text","text":"ignore me"}]}}"#,
                "\n",
            ),
        );

        assert_eq!(
            tailer.poll().unwrap(),
            vec![
                StreamLine::AgentText("checking now".into()),
                tool("p1", "⚙ grep: needle"),
                tool("p2", "⚙ bash: cargo test --workspace"),
            ]
        );
    }

    #[test]
    fn tool_paths_hide_workspace_and_private_run_prefixes() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let run_dir = workspace.join(".autoloop/runs/example");
        std::fs::create_dir_all(&run_dir).unwrap();
        let external = dir.path().join("external/input.txt");
        let stream_path = run_dir.join("pi-stream.4.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(&workspace, &run_dir, 4);

        for (index, path) in [
            run_dir.join("plan.md"),
            workspace.join("crates/ralph-tui/src/lib.rs"),
            external.clone(),
        ]
        .into_iter()
        .enumerate()
        {
            append(
                &stream_path,
                &format!(
                    "{}\n",
                    serde_json::json!({
                        "type": "tool_execution_start",
                        "toolCallId": format!("path-{index}"),
                        "toolName": "read",
                        "args": { "path": path },
                    })
                ),
            );
        }

        assert_eq!(
            tailer.poll().unwrap(),
            vec![
                tool("path-0", "⚙ read: engine:plan.md"),
                tool("path-1", "⚙ read: crates/ralph-tui/src/lib.rs"),
                tool("path-2", format!("⚙ read: {}", external.display())),
            ]
        );
    }

    #[test]
    fn missing_stream_is_empty_and_a_later_file_is_discovered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi-stream.2.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 2);

        assert!(tailer.poll().unwrap().is_empty());
        assert!(tailer.poll().unwrap().is_empty());

        append(
            &path,
            concat!(
                r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"arrived"}]}}"#,
                "\n",
            ),
        );
        assert_eq!(
            tailer.poll().unwrap(),
            vec![StreamLine::AgentText("arrived".into())]
        );
    }

    #[test]
    fn repeated_oversized_polls_report_one_truthful_cumulative_status() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi-stream.5.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 5);
        let ordinary = concat!(
            r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"ordinary"}]}}"#,
            "\n",
        );
        let mut expected_skipped = 0_u64;

        for poll_index in 0..8 {
            let mut growth = ordinary.repeat(MAX_BYTES_PER_POLL / ordinary.len() + 20);
            growth.push_str(&format!(
                "{}\n",
                serde_json::json!({
                    "type": "message_end",
                    "message": {
                        "role": "assistant",
                        "content": [{"type": "text", "text": format!("newest-{poll_index}")}]
                    }
                })
            ));
            let skipped_prefix = growth.len() - MAX_BYTES_PER_POLL;
            let alignment = growth.as_bytes()[skipped_prefix..]
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap()
                + 1;
            expected_skipped += (skipped_prefix + alignment) as u64;
            append(&path, &growth);

            let lines = tailer.poll().unwrap();
            assert_eq!(
                lines
                    .iter()
                    .filter(|line| matches!(line, StreamLine::Backpressure { .. }))
                    .count(),
                1
            );
            assert_eq!(
                lines.first(),
                Some(&StreamLine::Backpressure {
                    skipped_bytes: expected_skipped,
                })
            );
            assert!(lines.iter().any(
                |line| matches!(line, StreamLine::AgentText(text) if text == &format!("newest-{poll_index}"))
            ));
            assert!(lines.len() <= MAX_STREAM_LINES);
        }
    }

    #[test]
    fn oversized_unterminated_record_is_fully_counted_and_not_retained() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi-stream.6.jsonl");
        let bytes = vec![b'x'; MAX_BYTES_PER_POLL + 12_345];
        std::fs::write(&path, &bytes).unwrap();
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 6);

        assert_eq!(
            tailer.poll().unwrap(),
            vec![StreamLine::Backpressure {
                skipped_bytes: bytes.len() as u64,
            }]
        );
        assert!(tailer.pending.is_empty());
        assert_eq!(tailer.position, bytes.len() as u64);
        assert!(tailer.poll().unwrap().is_empty());
    }

    #[test]
    fn repeated_tool_records_are_deduplicated_by_id_not_rendered_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi-stream.7.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 7);
        let record = |id: &str| {
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "tool_execution_start",
                    "toolCallId": id,
                    "toolName": "read",
                    "args": {"path": "same.rs"},
                })
            )
        };

        append(
            &path,
            &(record("call-a") + &record("call-a") + &record("call-b")),
        );
        assert_eq!(
            tailer.poll().unwrap(),
            vec![
                tool("call-a", "⚙ read: same.rs"),
                tool("call-b", "⚙ read: same.rs"),
            ]
        );

        append(&path, &(record("call-a") + &record("call-b")));
        assert!(tailer.poll().unwrap().is_empty());
        assert_eq!(tailer.seen_tool_ids.len(), 2);
    }

    #[test]
    fn identical_idless_structured_tool_events_use_bounded_fallback_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi-stream.9.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 9);
        let record = concat!(
            r#"{"type":"tool_execution_start","toolName":"grep","args":{"pattern":"needle"}}"#,
            "\n",
        );
        append(&path, &(record.to_owned() + record));

        let lines = tailer.poll().unwrap();
        assert_eq!(lines.len(), 1);
        assert!(matches!(
            &lines[0],
            StreamLine::ToolSummary { identity, text }
                if identity.0.starts_with("event:") && text == "⚙ grep: needle"
        ));
        assert_eq!(tailer.seen_tool_ids.len(), 1);
    }

    #[test]
    fn poll_output_pending_identity_and_line_sizes_remain_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi-stream.10.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 10);
        let mut fixture = String::new();
        for index in 0..(MAX_STREAM_LINES + 100) {
            fixture.push_str(
                &serde_json::json!({
                    "type": "tool_execution_start",
                    "toolCallId": format!("bounded-{index}"),
                    "toolName": "read",
                    "args": {"path": "x"},
                })
                .to_string(),
            );
            fixture.push('\n');
        }
        assert!(fixture.len() < MAX_BYTES_PER_POLL);
        append(&path, &fixture);

        let lines = tailer.poll().unwrap();
        assert_eq!(lines.len(), MAX_STREAM_LINES);
        assert!(tailer.pending.len() <= MAX_BYTES_PER_POLL);
        assert_eq!(tailer.seen_tool_ids.len(), MAX_STREAM_LINES);
        assert_eq!(tailer.tool_id_order.len(), MAX_STREAM_LINES);
        assert!(lines.iter().all(|line| match line {
            StreamLine::AgentText(text) | StreamLine::ToolSummary { text, .. } => {
                text.len() <= MAX_STREAM_LINE_BYTES
            }
            StreamLine::Backpressure { .. } => true,
        }));

        append(
            &path,
            &format!(
                "{}\n",
                serde_json::json!({
                    "type": "message_end",
                    "message": {
                        "role": "assistant",
                        "content": [{"type": "text", "text": "z".repeat(MAX_STREAM_LINE_BYTES * 2)}]
                    }
                })
            ),
        );
        let long_line = tailer.poll().unwrap();
        assert!(matches!(
            &long_line[..],
            [StreamLine::AgentText(text)] if text.len() <= MAX_STREAM_LINE_BYTES
        ));
    }

    #[test]
    fn huge_existing_stream_reads_only_tail_window_and_caps_retained_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude-stream.1.jsonl");
        let mut fixture = Vec::with_capacity(5 * 1024 * 1024);
        let ordinary = concat!(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"ordinary output line"}]}}"#,
            "\n",
        );
        while fixture.len() < 5 * 1024 * 1024 {
            fixture.extend_from_slice(ordinary.as_bytes());
        }
        fixture.extend_from_slice(
            concat!(
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"final marker"}]}}"#,
                "\n",
            )
            .as_bytes(),
        );
        std::fs::write(&path, &fixture).unwrap();

        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), dir.path(), 1);
        let lines = tailer.poll().unwrap();

        assert!(matches!(
            lines.first(),
            Some(StreamLine::Backpressure { skipped_bytes }) if *skipped_bytes > 0
        ));
        assert!(
            lines
                .iter()
                .any(|line| line == &StreamLine::AgentText("final marker".into()))
        );
        assert!(lines.len() <= MAX_STREAM_LINES);
        assert!(tailer.pending.len() <= MAX_BYTES_PER_POLL);
        assert!(
            lines
                .iter()
                .map(|line| match line {
                    StreamLine::AgentText(text) | StreamLine::ToolSummary { text, .. } =>
                        text.len(),
                    StreamLine::Backpressure { .. } => 0,
                })
                .sum::<usize>()
                <= MAX_STREAM_LINES * MAX_STREAM_LINE_BYTES
        );
        assert_eq!(tailer.position, fixture.len() as u64);
    }
}
