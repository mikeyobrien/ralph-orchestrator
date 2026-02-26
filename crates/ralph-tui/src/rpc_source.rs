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
                                apply_rpc_event(&event, &state);
                            }
                            Err(e) => {
                                debug!(error = %e, line = %line, "Failed to parse RPC event");
                            }
                        }
                    }
                    Ok(None) => {
                        // EOF - subprocess exited
                        debug!("RPC event reader reached EOF");
                        // Mark loop as completed
                        if let Ok(mut s) = state.lock() {
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

/// Applies an RPC event to the TUI state.
fn apply_rpc_event(event: &RpcEvent, state: &Arc<Mutex<TuiState>>) {
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

            // Start a new iteration buffer with metadata
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
            // Parse markdown text and append to latest iteration buffer
            let lines = text_to_lines(delta);
            if let Some(handle) = s.latest_iteration_lines_handle()
                && let Ok(mut buffer_lines) = handle.lock()
            {
                buffer_lines.extend(lines);
            }

            s.last_event = Some("text_delta".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::ToolCallStart {
            tool_name, input, ..
        } => {
            // Format tool call header
            let mut spans = vec![Span::styled(
                format!("\u{2699} [{}]", tool_name),
                Style::default().fg(Color::Blue),
            )];

            // Add summary if available
            if let Some(summary) = format_tool_summary(tool_name, input) {
                spans.push(Span::styled(
                    format!(" {}", summary),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let line = Line::from(spans);

            if let Some(handle) = s.latest_iteration_lines_handle()
                && let Ok(mut buffer_lines) = handle.lock()
            {
                buffer_lines.push(line);
            }

            s.last_event = Some("tool_call_start".to_string());
            s.last_event_at = Some(Instant::now());
        }

        RpcEvent::ToolCallEnd {
            output, is_error, ..
        } => {
            let (prefix, color) = if *is_error {
                ("\u{2717} ", Color::Red)
            } else {
                ("\u{2713} ", Color::DarkGray)
            };

            let truncated = truncate(output, 200);
            let line = Line::from(Span::styled(
                format!(" {}{}", prefix, truncated),
                Style::default().fg(color),
            ));

            if let Some(handle) = s.latest_iteration_lines_handle()
                && let Ok(mut buffer_lines) = handle.lock()
            {
                buffer_lines.push(line);
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
    match name {
        "Read" | "Edit" | "Write" => input
            .get("path")
            .or_else(|| input.get("file_path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "Bash" => {
            let cmd = input.get("command")?.as_str()?;
            Some(truncate(cmd, 60))
        }
        "Grep" => input.get("pattern")?.as_str().map(|s| s.to_string()),
        "Glob" => input.get("pattern")?.as_str().map(|s| s.to_string()),
        "Task" => input.get("description")?.as_str().map(|s| s.to_string()),
        "WebFetch" => input.get("url")?.as_str().map(|s| s.to_string()),
        "WebSearch" => input.get("query")?.as_str().map(|s| s.to_string()),
        "LSP" => {
            let op = input.get("operation")?.as_str()?;
            let file = input.get("filePath")?.as_str()?;
            Some(format!("{} @ {}", op, file))
        }
        "NotebookEdit" => input.get("notebook_path")?.as_str().map(|s| s.to_string()),
        "TodoWrite" => Some("updating todo list".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_proto::json_rpc::{RpcEvent, TerminationReason};
    use serde_json::json;

    fn make_state() -> Arc<Mutex<TuiState>> {
        Arc::new(Mutex::new(TuiState::new()))
    }

    #[test]
    fn test_loop_started_sets_timer() {
        let state = make_state();
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
        apply_rpc_event(&event, &state);

        let s = state.lock().unwrap();
        assert!(s.loop_started.is_some());
        assert_eq!(s.max_iterations, Some(10));
    }

    #[test]
    fn test_iteration_start_creates_buffer() {
        let state = make_state();

        let event = RpcEvent::IterationStart {
            iteration: 1,
            max_iterations: Some(10),
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            started_at: 0,
        };
        apply_rpc_event(&event, &state);

        let s = state.lock().unwrap();
        assert_eq!(s.total_iterations(), 1);
        assert_eq!(s.iteration, 1);
    }

    #[test]
    fn test_text_delta_appends_content() {
        let state = make_state();

        // Start an iteration first
        let start_event = RpcEvent::IterationStart {
            iteration: 1,
            max_iterations: None,
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            started_at: 0,
        };
        apply_rpc_event(&start_event, &state);

        // Now add text
        let text_event = RpcEvent::TextDelta {
            iteration: 1,
            delta: "Hello world".to_string(),
        };
        apply_rpc_event(&text_event, &state);

        let s = state.lock().unwrap();
        let lines = s.iterations[0].lines.lock().unwrap();
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_tool_call_start_adds_header() {
        let state = make_state();

        // Start an iteration first
        let start_event = RpcEvent::IterationStart {
            iteration: 1,
            max_iterations: None,
            hat: "builder".to_string(),
            hat_display: "🔨Builder".to_string(),
            backend: "claude".to_string(),
            started_at: 0,
        };
        apply_rpc_event(&start_event, &state);

        let tool_event = RpcEvent::ToolCallStart {
            iteration: 1,
            tool_name: "Bash".to_string(),
            tool_call_id: "tool_1".to_string(),
            input: json!({"command": "ls -la"}),
        };
        apply_rpc_event(&tool_event, &state);

        let s = state.lock().unwrap();
        let lines = s.iterations[0].lines.lock().unwrap();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_loop_terminated_marks_complete() {
        let state = make_state();

        let event = RpcEvent::LoopTerminated {
            reason: TerminationReason::Completed,
            total_iterations: 5,
            duration_ms: 10000,
            total_cost_usd: 0.25,
            terminated_at: 0,
        };
        apply_rpc_event(&event, &state);

        let s = state.lock().unwrap();
        assert!(s.loop_completed);
        assert_eq!(s.iteration, 5);
    }

    #[test]
    fn test_task_counts_updated() {
        let state = make_state();

        let event = RpcEvent::TaskCountsUpdated {
            total: 10,
            open: 3,
            closed: 7,
            ready: 2,
        };
        apply_rpc_event(&event, &state);

        let s = state.lock().unwrap();
        assert_eq!(s.task_counts.total, 10);
        assert_eq!(s.task_counts.open, 3);
        assert_eq!(s.task_counts.closed, 7);
        assert_eq!(s.task_counts.ready, 2);
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
    }

    #[test]
    fn test_truncate() {
        use crate::text_renderer::truncate;
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello...");
    }
}
