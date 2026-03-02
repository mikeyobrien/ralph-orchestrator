//! RPC event source for reading JSON-RPC events from a subprocess.
//!
//! This module provides an async event reader that:
//! - Reads JSON lines from a subprocess's stdout
//! - Parses each line as an `RpcEvent`
//! - Translates events into `TuiState` mutations
//!
//! This replaces the in-process `EventBus` observer when running in subprocess mode.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tracing::{debug, warn};

use ralph_proto::json_rpc::RpcEvent;

use crate::state::{TaskCounts, TuiState};
use crate::state_mutations::{
    append_error_line, apply_loop_completed, apply_task_active, apply_task_close,
};
use crate::text_renderer::{text_to_lines, truncate};

/// Runs the RPC event reader, processing events from the given async reader.
///
/// This function reads JSON lines from the subprocess stdout, parses them as
/// `RpcEvent`, and applies the corresponding mutations to the TUI state.
///
/// # Arguments
/// * `reader` - Any async reader (typically `tokio::process::ChildStdout`)
/// * `state` - Shared TUI state to mutate
/// * `cancel_rx` - Watch channel to signal cancellation
///
/// The function exits when:
/// - EOF is reached (subprocess exited)
/// - An unrecoverable error occurs
/// - The cancel signal is received
pub async fn run_rpc_event_reader<R>(
    reader: R,
    state: Arc<Mutex<TuiState>>,
    mut cancel_rx: tokio::sync::watch::Receiver<bool>,
) where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut received_any_event = false;
    // Accumulates raw text deltas so markdown is rendered from the full
    // buffer rather than per-chunk (mirrors TuiStreamHandler behaviour).
    let mut text_accumulator = TextAccumulator::new();

    loop {
        tokio::select! {
            biased;

            // Check for cancellation
            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    debug!("RPC event reader cancelled");
                    break;
                }
            }

            // Read next line
            result = lines.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<RpcEvent>(line) {
                            Ok(event) => {
                                received_any_event = true;
                                apply_rpc_event(&event, &state, &mut text_accumulator);
                            }
                            Err(e) => {
                                debug!(error = %e, line = %line, "Failed to parse RPC event");
                            }
                        }
                    }
                    Ok(None) => {
                        // EOF - subprocess exited
                        debug!("RPC event reader reached EOF");
                        if let Ok(mut s) = state.lock() {
                            if !received_any_event {
                                // Subprocess died before sending any events (e.g., worktree
                                // creation failure, config error, lock conflict).
                                warn!("Subprocess exited without sending any RPC events");
                                s.subprocess_error = Some(
                                    "Subprocess exited before starting. Check .ralph/diagnostics/logs/ for details.".to_string()
                                );
                                // Create an error iteration so the content pane shows the message
                                s.start_new_iteration();
                                if let Some(handle) = s.latest_iteration_lines_handle()
                                    && let Ok(mut lines) = handle.lock()
                                {
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            "\u{26A0} ",
                                            ratatui::style::Style::default()
                                                .fg(ratatui::style::Color::Red)
                                                .add_modifier(ratatui::style::Modifier::BOLD),
                                        ),
                                        Span::raw("Subprocess exited before starting the orchestration loop."),
                                    ]));
                                    lines.push(Line::raw(""));
                                    lines.push(Line::raw("Possible causes:"));
                                    lines.push(Line::raw("  - Loop lock held by another process (stale .ralph/loop.lock)"));
                                    lines.push(Line::raw("  - Worktree creation failed (branch name collision)"));
                                    lines.push(Line::raw("  - Configuration error in hat/config files"));
                                    lines.push(Line::raw(""));
                                    lines.push(Line::raw("Check logs: .ralph/diagnostics/logs/"));
                                }
                            }
                            s.loop_completed = true;
                            s.finish_latest_iteration();
                        }
                        break;
                    }
                    Err(e) => {
                        warn!(error = %e, "Error reading from subprocess stdout");
                        break;
                    }
                }
            }
        }
    }
}

/// Accumulates text deltas and non-text blocks in chronological order,
/// mirroring the `TuiStreamHandler` content-block approach so that
/// markdown is rendered from the full accumulated text rather than
/// per-chunk.
struct TextAccumulator {
    /// Chronological content blocks for the current iteration.
    blocks: Vec<ContentBlock>,
    /// Current unfrozen text buffer.
    current_text: String,
}

enum ContentBlock {
    Text(String),
    NonText(Line<'static>),
}

impl TextAccumulator {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current_text: String::new(),
        }
    }

    /// Append a text delta and re-render all lines into the buffer.
    fn push_text(&mut self, delta: &str, lines_handle: &Arc<Mutex<Vec<Line<'static>>>>) {
        self.current_text.push_str(delta);
        self.rebuild_lines(lines_handle);
    }

    /// Freeze current text, add a non-text line, and re-render.
    fn push_non_text(
        &mut self,
        line: Line<'static>,
        lines_handle: &Arc<Mutex<Vec<Line<'static>>>>,
    ) {
        if !self.current_text.is_empty() {
            self.blocks
                .push(ContentBlock::Text(std::mem::take(&mut self.current_text)));
        }
        self.blocks.push(ContentBlock::NonText(line));
        self.rebuild_lines(lines_handle);
    }

    /// Reset for a new iteration.
    fn reset(&mut self) {
        self.blocks.clear();
        self.current_text.clear();
    }

    /// Re-render all accumulated content into the shared lines buffer.
    fn rebuild_lines(&self, lines_handle: &Arc<Mutex<Vec<Line<'static>>>>) {
        let mut all_lines = Vec::new();
        for block in &self.blocks {
            match block {
                ContentBlock::Text(t) => all_lines.extend(text_to_lines(t)),
                ContentBlock::NonText(l) => all_lines.push(l.clone()),
            }
        }
        if !self.current_text.is_empty() {
            all_lines.extend(text_to_lines(&self.current_text));
        }
        if let Ok(mut buf) = lines_handle.lock() {
            *buf = all_lines;
        }
    }
}

/// Applies an RPC event to the TUI state.
fn apply_rpc_event(event: &RpcEvent, state: &Arc<Mutex<TuiState>>, acc: &mut TextAccumulator) {
    let Ok(mut s) = state.lock() else {
        return;
    };

    match event {
        RpcEvent::LoopStarted {
            max_iterations,
            backend,
            ..
        } => {
            s.loop_started = Some(Instant::now());
            s.max_iterations = *max_iterations;
            s.pending_backend = Some(backend.clone());
        }

        RpcEvent::IterationStart {
            iteration,
            max_iterations,
            hat_display,
            backend,
            ..
        } => {
            s.max_iterations = *max_iterations;
            s.pending_backend = Some(backend.clone());

            // Reset accumulator for the new iteration
            acc.reset();

            // Start a new iteration buffer with metadata
            // (this also resets the rpc text accumulation buffer)
            s.start_new_iteration_with_metadata(Some(hat_display.clone()), Some(backend.clone()));

            // Update iteration counter
            s.iteration = *iteration;
            s.iteration_started = Some(Instant::now());

            // Update last event tracking
            s.last_event = Some("iteration_start".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::IterationEnd {
            loop_complete_triggered,
            ..
        } => {
            // Freeze any accumulated text before ending the iteration

            s.prev_iteration = s.iteration;
            s.finish_latest_iteration();

            // Freeze loop elapsed if loop is completing
            if *loop_complete_triggered {
                s.final_loop_elapsed = s.loop_started.map(|start| start.elapsed());
            }

            s.last_event = Some("iteration_end".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::TextDelta { delta, .. } => {
            if let Some(handle) = s.latest_iteration_lines_handle() {
                acc.push_text(delta, &handle);
            }

            s.last_event = Some("text_delta".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::ToolCallStart {
            tool_name, input, ..
        } => {
            // ACP titles can be descriptive ("Reading /path/to/file") or bare ("read").
            // If the title already contains context (has a space), use it as-is.
            // Otherwise try to extract a summary from the input JSON.
            let mut spans = vec![Span::styled(
                format!("\u{2699} [{}]", tool_name),
                Style::default().fg(Color::Blue),
            )];

            if !tool_name.contains(' ')
                && let Some(summary) = format_tool_summary(tool_name, input)
            {
                spans.push(Span::styled(
                    format!(" {}", summary),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let line = Line::from(spans);

            if let Some(handle) = s.latest_iteration_lines_handle() {
                acc.push_non_text(line, &handle);
            }

            s.last_event = Some("tool_call_start".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::ToolCallEnd {
            output, is_error, ..
        } => {
            let display = format_tool_result(output);
            if display.is_empty() {
                s.last_event = Some("tool_call_end".to_string());
                s.last_event_at = Some(Instant::now());
                return;
            }

            let (prefix, color) = if *is_error {
                ("\u{2717} ", Color::Red)
            } else {
                ("\u{2713} ", Color::DarkGray)
            };

            let truncated = truncate(&display, 200);
            let line = Line::from(Span::styled(
                format!(" {}{}", prefix, truncated),
                Style::default().fg(color),
            ));

            if let Some(handle) = s.latest_iteration_lines_handle() {
                acc.push_non_text(line, &handle);
            }

            s.last_event = Some("tool_call_end".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::Error { code, message, .. } => {
            append_error_line(&mut s, code, message);

            s.last_event = Some("error".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::HatChanged {
            to_hat,
            to_hat_display,
            ..
        } => {
            use ralph_proto::HatId;
            s.pending_hat = Some((HatId::new(to_hat), to_hat_display.clone()));

            s.last_event = Some("hat_changed".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::TaskStatusChanged {
            task_id,
            to_status,
            title,
            ..
        } => {
            match to_status.as_str() {
                "running" | "in_progress" => {
                    apply_task_active(&mut s, task_id, title, to_status);
                }
                "closed" | "done" | "completed" => {
                    apply_task_close(&mut s, task_id);
                }
                _ => {}
            }

            s.last_event = Some("task_status_changed".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::TaskCountsUpdated {
            total,
            open,
            closed,
            ready,
        } => {
            s.set_task_counts(TaskCounts::new(*total, *open, *closed, *ready));

            s.last_event = Some("task_counts_updated".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::GuidanceAck { .. } => {
            // Just update liveness
            s.last_event = Some("guidance_ack".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::LoopTerminated {
            total_iterations, ..
        } => {
            s.iteration = *total_iterations;
            apply_loop_completed(&mut s);

            s.last_event = Some("loop_terminated".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::Response { .. } => {
            // Responses are typically handled by the caller of get_state/etc.
            // Just update liveness
            s.last_event = Some("response".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::OrchestrationEvent { topic, .. } => {
            // Generic orchestration events - just update liveness
            s.last_event = Some(topic.clone());
            s.last_event_at = Some(Instant::now());
        }
    }
}

/// Extracts the most relevant field from tool input for display.
fn format_tool_summary(name: &str, input: &Value) -> Option<String> {
    // Try the primary key for the tool name first, then common fallbacks.
    // ACP tools use lowercase names (read, write, shell, ls, glob, grep)
    // while Claude uses PascalCase (Read, Write, Bash, Glob, Grep).
    match name {
        "Read" | "Edit" | "Write" | "read" | "write" => input
            .get("path")
            .or_else(|| input.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "Bash" | "shell" => {
            let cmd = input.get("command")?.as_str()?;
            Some(truncate(cmd, 60))
        }
        "Grep" | "grep" => input.get("pattern")?.as_str().map(|s| s.to_string()),
        "Glob" | "glob" | "ls" => input
            .get("pattern")
            .or_else(|| input.get("path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "Task" => input.get("description")?.as_str().map(|s| s.to_string()),
        "WebFetch" | "web_fetch" => input.get("url")?.as_str().map(|s| s.to_string()),
        "WebSearch" | "web_search" => input.get("query")?.as_str().map(|s| s.to_string()),
        "LSP" => {
            let op = input.get("operation")?.as_str()?;
            let file = input.get("filePath")?.as_str()?;
            Some(format!("{} @ {}", op, file))
        }
        "NotebookEdit" => input.get("notebook_path")?.as_str().map(|s| s.to_string()),
        "TodoWrite" => Some("updating todo list".to_string()),
        _ => {
            // Generic fallback: try common keys
            input
                .get("path")
                .or_else(|| input.get("file_path"))
                .or_else(|| input.get("command"))
                .or_else(|| input.get("pattern"))
                .or_else(|| input.get("url"))
                .or_else(|| input.get("query"))
                .and_then(|v| v.as_str())
                .map(|s| truncate(s, 60))
        }
    }
}

/// Extracts human-readable content from ACP tool result JSON envelopes.
///
/// ACP tool results arrive as `{"items":[{"Text":"..."} | {"Json":{...}}]}`.
fn format_tool_result(output: &str) -> String {
    let Ok(val) = serde_json::from_str::<Value>(output) else {
        return output.to_string();
    };
    let Some(items) = val.get("items").and_then(|v| v.as_array()) else {
        return output.to_string();
    };
    let Some(item) = items.first() else {
        return String::new();
    };

    if let Some(text) = item.get("Text").and_then(|v| v.as_str()) {
        return text.to_string();
    }

    if let Some(json) = item.get("Json") {
        // Shell: {exit_status, stdout, stderr}
        if let Some(stdout) = json.get("stdout").and_then(|v| v.as_str()) {
            let stderr = json.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
            let exit = json
                .get("exit_status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let failed = !exit.contains("status: 0");
            return if failed && !stderr.is_empty() {
                stderr.to_string()
            } else if !stdout.is_empty() {
                stdout.to_string()
            } else {
                stderr.to_string()
            };
        }
        // Glob/ls: {filePaths, totalFiles}
        if let Some(paths) = json.get("filePaths").and_then(|v| v.as_array()) {
            let total = json
                .get("totalFiles")
                .and_then(|v| v.as_u64())
                .unwrap_or(paths.len() as u64);
            let names: Vec<&str> = paths
                .iter()
                .filter_map(|p| p.as_str())
                .map(|p| p.rsplit('/').next().unwrap_or(p))
                .collect();
            return format!("{} files: {}", total, names.join(", "));
        }
        // Grep: {numFiles, numMatches, results}
        if let Some(results) = json.get("results").and_then(|v| v.as_array()) {
            let num_matches = json.get("numMatches").and_then(|v| v.as_u64()).unwrap_or(0);
            let first_match = results.first().and_then(|r| {
                let file = r.get("file").and_then(|v| v.as_str()).unwrap_or("");
                let basename = file.rsplit('/').next().unwrap_or(file);
                let matches = r.get("matches").and_then(|v| v.as_array())?;
                let first = matches.first().and_then(|m| m.as_str())?;
                Some(format!("{}: {}", basename, first.trim()))
            });
            return match first_match {
                Some(m) => format!("{} matches: {}", num_matches, m),
                None => format!("{} matches", num_matches),
            };
        }

        return json.to_string();
    }

    output.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_proto::json_rpc::{RpcEvent, TerminationReason};
    use serde_json::json;

    fn make_state() -> Arc<Mutex<TuiState>> {
        Arc::new(Mutex::new(TuiState::new()))
    }

    fn make_acc() -> TextAccumulator {
        TextAccumulator::new()
    }

    #[test]
    fn test_loop_started_sets_timer() {
        let state = make_state();
        let mut acc = make_acc();
        {
            let mut s = state.lock().unwrap();
            s.loop_started = None;
        }

        let event = RpcEvent::LoopStarted {
            prompt: "test".to_string(),
            max_iterations: Some(10),
            backend: "claude".to_string(),
            started_at: 0,
        };
        apply_rpc_event(&event, &state, &mut acc);

        let s = state.lock().unwrap();
        assert!(s.loop_started.is_some());
        assert_eq!(s.max_iterations, Some(10));
    }

    #[test]
    fn test_iteration_start_creates_buffer() {
        let state = make_state();
        let mut acc = make_acc();

        let event = RpcEvent::IterationStart {
            iteration: 1,
            max_iterations: Some(10),
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            started_at: 0,
        };
        apply_rpc_event(&event, &state, &mut acc);

        let s = state.lock().unwrap();
        assert_eq!(s.total_iterations(), 1);
        assert_eq!(s.iteration, 1);
    }

    #[test]
    fn test_text_delta_appends_content() {
        let state = make_state();
        let mut acc = make_acc();

        // Start an iteration first
        let start_event = RpcEvent::IterationStart {
            iteration: 1,
            max_iterations: None,
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            started_at: 0,
        };
        apply_rpc_event(&start_event, &state, &mut acc);

        // Now add text
        let text_event = RpcEvent::TextDelta {
            iteration: 1,
            delta: "Hello world".to_string(),
        };
        apply_rpc_event(&text_event, &state, &mut acc);

        let s = state.lock().unwrap();
        let lines = s.iterations[0].lines.lock().unwrap();
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_tool_call_start_adds_header() {
        let state = make_state();
        let mut acc = make_acc();

        // Start an iteration first
        let start_event = RpcEvent::IterationStart {
            iteration: 1,
            max_iterations: None,
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            started_at: 0,
        };
        apply_rpc_event(&start_event, &state, &mut acc);

        let tool_event = RpcEvent::ToolCallStart {
            iteration: 1,
            tool_name: "Bash".to_string(),
            tool_call_id: "tool_1".to_string(),
            input: json!({"command": "ls -la"}),
        };
        apply_rpc_event(&tool_event, &state, &mut acc);

        let s = state.lock().unwrap();
        let lines = s.iterations[0].lines.lock().unwrap();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_loop_terminated_marks_complete() {
        let state = make_state();
        let mut acc = make_acc();

        let event = RpcEvent::LoopTerminated {
            reason: TerminationReason::Completed,
            total_iterations: 5,
            duration_ms: 10000,
            total_cost_usd: 0.25,
            terminated_at: 0,
        };
        apply_rpc_event(&event, &state, &mut acc);

        let s = state.lock().unwrap();
        assert!(s.loop_completed);
        assert_eq!(s.iteration, 5);
    }

    #[test]
    fn test_task_counts_updated() {
        let state = make_state();
        let mut acc = make_acc();

        let event = RpcEvent::TaskCountsUpdated {
            total: 10,
            open: 3,
            closed: 7,
            ready: 2,
        };
        apply_rpc_event(&event, &state, &mut acc);

        let s = state.lock().unwrap();
        assert_eq!(s.task_counts.total, 10);
        assert_eq!(s.task_counts.open, 3);
        assert_eq!(s.task_counts.closed, 7);
        assert_eq!(s.task_counts.ready, 2);
    }

    #[test]
    fn test_small_text_deltas_form_flowing_paragraph_not_one_per_line() {
        // Simulates Pi's text_delta streaming pattern: many small deltas (5-50
        // chars each) without newlines. The TUI should render these as flowing
        // paragraph text, NOT one line per delta.
        let state = make_state();
        let mut acc = make_acc();

        // Start an iteration
        apply_rpc_event(
            &RpcEvent::IterationStart {
                iteration: 1,
                max_iterations: None,
                hat: "scoper".to_string(),
                hat_display: "🔎 Scoper".to_string(),
                backend: "pi".to_string(),
                started_at: 0,
            },
            &state,
            &mut acc,
        );

        // Send 8 small text deltas (typical Pi streaming pattern)
        let deltas = vec![
            "Rust",
            " is a systems",
            " programming language",
            " that runs",
            " blazingly fast,",
            " prevents segfaults,",
            " and guarantees",
            " thread safety.",
        ];
        for delta in deltas {
            apply_rpc_event(
                &RpcEvent::TextDelta {
                    iteration: 1,
                    delta: delta.to_string(),
                },
                &state,
                &mut acc,
            );
        }

        let s = state.lock().unwrap();
        let lines = s.iterations[0].lines.lock().unwrap();

        // With 8 small deltas forming ~120 chars of text (no newlines), the
        // result should be a flowing paragraph (1-3 lines when wrapped at
        // typical terminal width), NOT 8 separate lines.
        assert!(
            lines.len() <= 3,
            "Small text deltas without newlines should form a flowing paragraph, \
             not one line per delta. Expected <= 3 lines but got {} lines: {:?}",
            lines.len(),
            lines.iter().map(|l| l.to_string()).collect::<Vec<_>>()
        );

        // Verify the full text is present and contiguous
        let full_text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            full_text.contains("Rust is a systems programming language"),
            "Text should flow as a paragraph. Got: {:?}",
            full_text
        );
    }

    #[test]
    fn test_text_deltas_frozen_by_tool_call_preserve_order() {
        // When text deltas are followed by a tool call, the accumulated text
        // should be frozen and appear before the tool call line.
        let state = make_state();
        let mut acc = make_acc();

        apply_rpc_event(
            &RpcEvent::IterationStart {
                iteration: 1,
                max_iterations: None,
                hat: "builder".to_string(),
                hat_display: "🔨 Builder".to_string(),
                backend: "pi".to_string(),
                started_at: 0,
            },
            &state,
            &mut acc,
        );

        // Send streaming text, then a tool call, then more text
        for delta in ["I'll ", "review ", "the code."] {
            apply_rpc_event(
                &RpcEvent::TextDelta {
                    iteration: 1,
                    delta: delta.to_string(),
                },
                &state,
                &mut acc,
            );
        }

        apply_rpc_event(
            &RpcEvent::ToolCallStart {
                iteration: 1,
                tool_name: "Read".to_string(),
                tool_call_id: "t1".to_string(),
                input: json!({"file_path": "src/main.rs"}),
            },
            &state,
            &mut acc,
        );

        for delta in ["Now ", "checking."] {
            apply_rpc_event(
                &RpcEvent::TextDelta {
                    iteration: 1,
                    delta: delta.to_string(),
                },
                &state,
                &mut acc,
            );
        }

        let s = state.lock().unwrap();
        let lines = s.iterations[0].lines.lock().unwrap();
        let line_strs: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

        // Find positions: text1 should be before tool, text2 after tool
        let text1_idx = line_strs.iter().position(|l| l.contains("review the code"));
        let tool_idx = line_strs.iter().position(|l| l.contains("Read"));
        let text2_idx = line_strs.iter().position(|l| l.contains("checking"));

        assert!(
            text1_idx.is_some(),
            "text1 should be present: {:?}",
            line_strs
        );
        assert!(
            tool_idx.is_some(),
            "tool should be present: {:?}",
            line_strs
        );
        assert!(
            text2_idx.is_some(),
            "text2 should be present: {:?}",
            line_strs
        );

        assert!(
            text1_idx.unwrap() < tool_idx.unwrap(),
            "text1 should come before tool: {:?}",
            line_strs
        );
        assert!(
            tool_idx.unwrap() < text2_idx.unwrap(),
            "tool should come before text2: {:?}",
            line_strs
        );
    }

    #[test]
    fn test_format_tool_summary() {
        // Primary key: "path" (Claude Code convention)
        assert_eq!(
            format_tool_summary("Read", &json!({"path": "/foo/bar.rs"})),
            Some("/foo/bar.rs".to_string())
        );
        // Fallback key: "file_path"
        assert_eq!(
            format_tool_summary("Edit", &json!({"file_path": "/foo/bar.rs"})),
            Some("/foo/bar.rs".to_string())
        );
        assert_eq!(
            format_tool_summary("Bash", &json!({"command": "ls"})),
            Some("ls".to_string())
        );
        assert_eq!(format_tool_summary("Unknown", &json!({})), None);

        // ACP tool names (lowercase)
        assert_eq!(
            format_tool_summary("read", &json!({"path": "/foo/bar.rs"})),
            Some("/foo/bar.rs".to_string())
        );
        assert_eq!(
            format_tool_summary("shell", &json!({"command": "cargo test"})),
            Some("cargo test".to_string())
        );
        assert_eq!(
            format_tool_summary("ls", &json!({"path": "/src"})),
            Some("/src".to_string())
        );
        assert_eq!(
            format_tool_summary("grep", &json!({"pattern": "TODO"})),
            Some("TODO".to_string())
        );
    }

    #[test]
    fn test_truncate() {
        use crate::text_renderer::truncate;
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello...");
    }

    // =========================================================================
    // Subprocess Death Detection Tests
    // =========================================================================

    #[tokio::test]
    async fn test_eof_without_events_sets_subprocess_error() {
        // Given a reader that immediately returns EOF (simulating subprocess death)
        let empty_input: &[u8] = b"";
        let state = make_state();
        let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

        // When the event reader runs to completion
        run_rpc_event_reader(empty_input, state.clone(), cancel_rx).await;

        // Then subprocess_error should be set (subprocess died before sending events)
        let s = state.lock().unwrap();
        assert!(
            s.subprocess_error.is_some(),
            "should set subprocess_error on EOF without events"
        );
        assert!(s.loop_completed, "should mark loop as completed");
    }

    #[tokio::test]
    async fn test_eof_without_events_creates_error_iteration() {
        // Given a reader that immediately returns EOF
        let empty_input: &[u8] = b"";
        let state = make_state();
        let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

        // When the event reader runs to completion
        run_rpc_event_reader(empty_input, state.clone(), cancel_rx).await;

        // Then an error iteration should be created so the content pane has something to show
        let s = state.lock().unwrap();
        assert_eq!(s.total_iterations(), 1, "should create one error iteration");
        let lines = s.iterations[0].lines.lock().unwrap();
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect();
        assert!(
            text.contains("Subprocess exited"),
            "error iteration should contain error message, got: {}",
            text
        );
    }

    #[tokio::test]
    async fn test_eof_after_loop_started_does_not_set_subprocess_error() {
        // Given a reader with a valid LoopStarted event then EOF
        let event = RpcEvent::LoopStarted {
            prompt: "test".to_string(),
            max_iterations: Some(10),
            backend: "claude".to_string(),
            started_at: 0,
        };
        let line = format!("{}\n", serde_json::to_string(&event).unwrap());
        let state = make_state();
        let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

        // When the event reader processes the event and then hits EOF
        run_rpc_event_reader(line.as_bytes(), state.clone(), cancel_rx).await;

        // Then subprocess_error should NOT be set (subprocess ran normally)
        let s = state.lock().unwrap();
        assert!(
            s.subprocess_error.is_none(),
            "should NOT set subprocess_error when events were received"
        );
    }
}
