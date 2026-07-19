//! Bounded incremental tailing of an active autoloop backend stream.
//!
//! Backend streams can grow to hundreds of megabytes. This tailer therefore
//! reads at most [`MAX_BYTES_PER_POLL`] bytes per poll, retains at most that
//! much unfinished NDJSON internally, emits at most [`MAX_STREAM_LINES`] per
//! iteration, and truncates each emitted line to [`MAX_STREAM_LINE_BYTES`].

use std::collections::VecDeque;
use std::fs::File;
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

/// One displayable item extracted from a backend's NDJSON stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamLine {
    /// Text emitted by the assistant.
    AgentText(String),
    /// A concise summary of a tool invocation.
    ToolSummary(String),
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
    run_dir: PathBuf,
    iteration: u32,
    selected: Option<SelectedStream>,
    position: u64,
    pending: Vec<u8>,
    emitted_lines: usize,
}

impl BackendStreamTailer {
    /// Create a tailer for an iteration. Its stream file need not exist yet.
    pub fn for_iteration(run_dir: &Path, iteration: u32) -> Self {
        Self {
            run_dir: run_dir.to_path_buf(),
            iteration,
            selected: None,
            position: 0,
            pending: Vec::new(),
            emitted_lines: 0,
        }
    }

    /// Return displayable items appended since the previous poll.
    pub fn poll(&mut self) -> io::Result<Vec<StreamLine>> {
        if self.selected.is_none() {
            self.selected = self.probe_stream();
        }
        let Some(selected) = &self.selected else {
            return Ok(Vec::new());
        };

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

        let unread = len - self.position;
        let mut skipped = 0_u64;
        if unread > MAX_BYTES_PER_POLL as u64 {
            let start = len - MAX_BYTES_PER_POLL as u64;
            skipped = start.saturating_sub(self.position);
            self.position = start;
            self.pending.clear();
        }

        file.seek(SeekFrom::Start(self.position))?;
        let mut bytes = Vec::with_capacity((len - self.position) as usize);
        let read = file
            .take(MAX_BYTES_PER_POLL as u64)
            .read_to_end(&mut bytes)?;
        self.position += read as u64;

        if skipped > 0 {
            if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
                bytes.drain(..=newline);
            } else {
                return Ok(self.note_skipped(skipped));
            }
        }

        self.pending.extend_from_slice(&bytes);
        if self.pending.len() > MAX_BYTES_PER_POLL {
            self.pending.clear();
            return Ok(self.note_skipped(skipped.max(MAX_BYTES_PER_POLL as u64)));
        }

        let complete_len = self
            .pending
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let complete = self.pending.drain(..complete_len).collect::<Vec<_>>();

        let mut lines = self.note_skipped(skipped);
        let remaining = MAX_STREAM_LINES.saturating_sub(self.emitted_lines + lines.len());
        let mut newest = VecDeque::with_capacity(remaining);
        for raw_line in complete.split(|byte| *byte == b'\n') {
            let Ok(line) = std::str::from_utf8(raw_line) else {
                continue;
            };
            for parsed in parse_stream_line(selected.format, line) {
                if remaining == 0 {
                    continue;
                }
                if newest.len() == remaining {
                    newest.pop_front();
                }
                newest.push_back(parsed);
            }
        }
        lines.extend(newest);
        self.emitted_lines += lines.len();
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

    fn note_skipped(&self, bytes: u64) -> Vec<StreamLine> {
        if bytes == 0 || self.emitted_lines >= MAX_STREAM_LINES {
            Vec::new()
        } else {
            vec![StreamLine::AgentText(format!("… {bytes} bytes skipped …"))]
        }
    }
}

fn parse_stream_line(format: StreamFormat, line: &str) -> Vec<StreamLine> {
    let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
        return Vec::new();
    };
    match format {
        StreamFormat::Claude => parse_claude_event(&event),
        StreamFormat::Pi => parse_pi_event(&event),
    }
}

fn parse_claude_event(event: &Value) -> Vec<StreamLine> {
    if event.get("type").and_then(Value::as_str) != Some("assistant") {
        return Vec::new();
    }
    content_lines(event.pointer("/message/content"))
}

fn parse_pi_event(event: &Value) -> Vec<StreamLine> {
    match event.get("type").and_then(Value::as_str) {
        Some("message_end")
            if event.pointer("/message/role").and_then(Value::as_str) == Some("assistant") =>
        {
            content_lines(event.pointer("/message/content"))
        }
        Some("tool_execution_start") => {
            let Some(name) = event.get("toolName").and_then(Value::as_str) else {
                return Vec::new();
            };
            vec![tool_line(name, event.get("args").unwrap_or(&Value::Null))]
        }
        _ => Vec::new(),
    }
}

fn content_lines(content: Option<&Value>) -> Vec<StreamLine> {
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
                lines.push(tool_line(name, input));
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

fn tool_line(name: &str, input: &Value) -> StreamLine {
    let detail = format_tool_summary(name, input).map(|value| one_line(&value));
    let summary = match detail {
        Some(detail) if !detail.is_empty() => format!("⚙ {name}: {detail}"),
        _ => format!("⚙ {name}"),
    };
    StreamLine::ToolSummary(truncate_utf8(&summary, MAX_STREAM_LINE_BYTES))
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

    #[test]
    fn claude_stream_emits_only_new_assistant_text_and_tool_calls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude-stream.3.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), 3);

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
                StreamLine::ToolSummary("⚙ read: src/main.rs".into()),
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
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), 8);

        append(
            &path,
            concat!(
                r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"checking now"},{"type":"toolCall","name":"grep","arguments":{"pattern":"needle"}}]}}"#,
                "\n",
                r#"{"type":"tool_execution_start","toolName":"bash","args":{"command":"cargo test\n--workspace"}}"#,
                "\n",
                r#"{"type":"message_end","message":{"role":"user","content":[{"type":"text","text":"ignore me"}]}}"#,
                "\n",
            ),
        );

        assert_eq!(
            tailer.poll().unwrap(),
            vec![
                StreamLine::AgentText("checking now".into()),
                StreamLine::ToolSummary("⚙ grep: needle".into()),
                StreamLine::ToolSummary("⚙ bash: cargo test --workspace".into()),
            ]
        );
    }

    #[test]
    fn missing_stream_is_empty_and_a_later_file_is_discovered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pi-stream.2.jsonl");
        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), 2);

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

        let mut tailer = BackendStreamTailer::for_iteration(dir.path(), 1);
        let lines = tailer.poll().unwrap();

        assert!(
            matches!(lines.first(), Some(StreamLine::AgentText(line)) if line.contains("bytes skipped"))
        );
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
                    StreamLine::AgentText(text) | StreamLine::ToolSummary(text) => text.len(),
                })
                .sum::<usize>()
                <= MAX_STREAM_LINES * MAX_STREAM_LINE_BYTES
        );
        assert_eq!(tailer.position, fixture.len() as u64);
    }
}
