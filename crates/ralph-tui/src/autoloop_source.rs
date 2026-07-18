//! Autoloop event source for the live TUI (#342).
//!
//! Translates autoloop's `--events` NDJSON [`AutoloopEvent`] stream directly
//! onto [`TuiState`] mutators, the same surface [`crate::rpc_source`] drives for
//! the (now-retired) subprocess RPC stream. The TUI runs in-process inside
//! `ralph run`, concurrent with the `autoloop run` subprocess, and live-tails
//! the events file ralph points autoloop at.
//!
//! Unlike the RPC stream (a pipe), the autoloop `--events` file is a *growing
//! file*: [`run_autoloop_event_reader`] polls an [`AutoloopEventTailer`] on a
//! ~100ms interval rather than awaiting line-by-line. The `--events` stream
//! updates at iteration **boundaries**, not per-token, so the content pane
//! advances one iteration at a time (no live token streaming).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tokio::sync::watch;
use tracing::debug;

use ralph_adapters::{AutoloopEvent, AutoloopEventTailer};

use crate::state::TuiState;
use crate::state_mutations::apply_loop_completed;
use ralph_core::sanitize_tui_inline_text;

/// Per-reader translation context: tracks the role label autoloop last reported
/// (autoloop's role ≈ ralph's hat) so the *next* `iteration.start` can label its
/// iteration, plus the last-seen iteration number to detect boundaries.
#[derive(Debug, Default)]
pub struct AutoloopMapCtx {
    /// Display label for the active role, from the most recent
    /// `progress.allowedRoles[0]`. `None` until the first progress event.
    role_label: Option<String>,
    /// The iteration number of the most recently started iteration. Used to
    /// only reset/create a buffer when the number actually changes.
    current_iteration: Option<u32>,
}

impl AutoloopMapCtx {
    /// Creates an empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// The hat/role display label to attribute the next iteration to.
    fn hat_display(&self) -> String {
        self.role_label
            .clone()
            .unwrap_or_else(|| "autoloop".to_string())
    }
}

/// Applies one [`AutoloopEvent`] to the TUI state.
///
/// Maps each autoloop event `kind` onto the surviving [`TuiState`] mutators:
///
/// | kind             | effect                                                       |
/// |------------------|--------------------------------------------------------------|
/// | `iteration.start`| new iteration buffer + `iteration`/`max_iterations`          |
/// | `progress`       | `→ <emittedTopic> (<outcome>)` status line; updates role      |
/// | `backend.output` | splits agent output into lines in the current iteration      |
/// | `ask.pending`    | `⚠ HUMAN ASK` line + footer `pending_ask` (display only)      |
/// | `loop.finish`/`summary` | final iteration count + cost; freezes via `apply_loop_completed` |
/// | (other)          | liveness only                                                |
pub fn apply_autoloop_event(
    event: &AutoloopEvent,
    state: &Arc<Mutex<TuiState>>,
    ctx: &mut AutoloopMapCtx,
) {
    let Ok(mut s) = state.lock() else {
        return;
    };
    let now = Instant::now();

    match event.kind.as_str() {
        "iteration.start" => {
            let iteration = event.iteration.unwrap_or(0);
            // Only start a fresh buffer when the iteration number advances —
            // autoloop emits one iteration.start per iteration, but guard against
            // duplicates so we never split one iteration across two buffers.
            let is_new = ctx.current_iteration != Some(iteration);
            if is_new {
                ctx.current_iteration = Some(iteration);
                let hat_display = ctx.hat_display();
                s.start_new_iteration_with_metadata(
                    Some(hat_display),
                    Some("autoloop".to_string()),
                );
                s.iteration = iteration;
                if let Some(max) = event.max_iterations {
                    s.max_iterations = Some(max);
                }
                s.iteration_started = Some(now);
            } else if let Some(max) = event.max_iterations {
                s.max_iterations = Some(max);
            }

            s.last_event = Some("iteration.start".to_string());
            s.last_event_at = Some(now);
        }

        "progress" => {
            // Update the role label for the NEXT iteration's header.
            if let Some(roles) = &event.allowed_roles
                && let Some(first) = roles.first()
            {
                ctx.role_label = Some(first.clone());
            }

            let topic = event.emitted_topic.as_deref().unwrap_or("(none)");
            let outcome = event.outcome.as_deref().unwrap_or("");
            let text = if outcome.is_empty() {
                format!("\u{2192} {topic}")
            } else {
                format!("\u{2192} {topic} ({outcome})")
            };
            push_line(
                &mut s,
                Line::from(vec![Span::styled(
                    sanitize_tui_inline_text(&text),
                    Style::default().fg(Color::Cyan),
                )]),
            );

            s.last_event = Some("progress".to_string());
            s.last_event_at = Some(now);
        }

        "backend.output" => {
            if let Some(output) = &event.output {
                // The one real per-iteration agent content the coarse --events
                // stream carries. Split on newlines into individual Lines.
                let lines: Vec<Line<'static>> = output
                    .split('\n')
                    .map(|l| Line::raw(sanitize_tui_inline_text(l)))
                    .collect();
                push_lines(&mut s, lines);
            }

            s.last_event = Some("backend.output".to_string());
            s.last_event_at = Some(now);
        }

        "ask.pending" => {
            let question = event.question.clone().unwrap_or_default();
            push_line(
                &mut s,
                Line::from(vec![
                    Span::styled(
                        "\u{26A0} HUMAN ASK: ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(sanitize_tui_inline_text(&question)),
                ]),
            );
            s.pending_ask = Some(question);

            s.last_event = Some("ask.pending".to_string());
            s.last_event_at = Some(now);
        }

        "ask.answered" => {
            // The blocking ask resolved — clear the footer affordance.
            s.pending_ask = None;
            s.last_event = Some("ask.answered".to_string());
            s.last_event_at = Some(now);
        }

        "loop.finish" | "summary" => {
            if let Some(iterations) = event.iterations {
                s.iteration = iterations;
            }
            if let Some(cost) = event.cost_usd {
                s.final_cost_usd = Some(cost);
            }
            // Surface the stop reason as a raw string — autoloop's vocabulary
            // differs from ralph's TerminationReason enum, so do NOT map it.
            if let Some(stop_reason) = &event.stop_reason {
                push_line(
                    &mut s,
                    Line::from(vec![
                        Span::styled("\u{25A0} run finished: ", Style::default().fg(Color::Blue)),
                        Span::raw(sanitize_tui_inline_text(stop_reason)),
                    ]),
                );
            }
            // Clear any dangling ask once the run is over.
            s.pending_ask = None;
            apply_loop_completed(&mut s);

            s.last_event = Some(event.kind.clone());
            s.last_event_at = Some(now);
        }

        other => {
            // Unknown kind: liveness only.
            s.last_event = Some(other.to_string());
            s.last_event_at = Some(now);
        }
    }
}

/// Pushes a single line into the latest iteration buffer, if one exists.
fn push_line(state: &mut TuiState, line: Line<'static>) {
    if let Some(handle) = state.latest_iteration_lines_handle()
        && let Ok(mut lines) = handle.lock()
    {
        lines.push(line);
    }
}

/// Pushes multiple lines into the latest iteration buffer, if one exists.
fn push_lines(state: &mut TuiState, new_lines: Vec<Line<'static>>) {
    if let Some(handle) = state.latest_iteration_lines_handle()
        && let Ok(mut lines) = handle.lock()
    {
        lines.extend(new_lines);
    }
}

/// Live-tails the autoloop `--events` file and applies each event to the TUI
/// state, until cancellation.
///
/// Structured like [`crate::rpc_source::run_rpc_event_reader`] but reads a
/// growing FILE (via [`AutoloopEventTailer`]) rather than a pipe: a
/// `tokio::time::interval(100ms)` tick drives each poll+apply.
///
/// On cancel, performs exactly ONE final `poll()` and applies the remaining
/// events — this captures the terminal `loop.finish`, which autoloop writes
/// synchronously just before exiting (the parent signals cancel the instant the
/// subprocess `JoinHandle` completes, so without this final drain the terminal
/// event would be lost).
///
/// If the stream drains to the terminal cancel without ever seeing a
/// `loop.finish` / `summary`, an error line is appended to the latest iteration
/// (mirrors the EOF-without-terminal-event handling in `rpc_source`).
pub async fn run_autoloop_event_reader(
    events_path: PathBuf,
    state: Arc<Mutex<TuiState>>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    let mut tailer = AutoloopEventTailer::new(events_path);
    let mut ctx = AutoloopMapCtx::new();
    let mut saw_terminal = false;
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(100));

    loop {
        tokio::select! {
            biased;

            _ = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    debug!("autoloop event reader cancelled");
                    break;
                }
            }

            _ = ticker.tick() => {
                match tailer.poll() {
                    Ok(events) => {
                        for event in &events {
                            if is_terminal(event) {
                                saw_terminal = true;
                            }
                            apply_autoloop_event(event, &state, &mut ctx);
                        }
                    }
                    Err(e) => {
                        debug!(error = %e, "autoloop event reader poll failed");
                    }
                }
            }
        }
    }

    // CRITICAL (critique gap #5): one final drain after cancel. autoloop writes
    // the terminal `loop.finish` synchronously before exit, and the parent
    // signals cancel as soon as the subprocess JoinHandle completes — so the
    // last event(s) may only land on disk after the select loop has broken.
    match tailer.poll() {
        Ok(events) => {
            for event in &events {
                if is_terminal(event) {
                    saw_terminal = true;
                }
                apply_autoloop_event(event, &state, &mut ctx);
            }
        }
        Err(e) => {
            debug!(error = %e, "autoloop event reader final drain failed");
        }
    }

    // If the run ended without ever reporting a terminal result, surface that in
    // the content pane rather than leaving the view ambiguously "running".
    if !saw_terminal && let Ok(mut s) = state.lock() {
        if s.iterations.is_empty() {
            s.start_new_iteration();
        }
        push_line(
            &mut s,
            Line::from(vec![
                Span::styled(
                    "\u{26A0} ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw("run ended before reporting a result"),
            ]),
        );
        apply_loop_completed(&mut s);
    }
}

/// True for the terminal events that carry a machine-readable run result.
fn is_terminal(event: &AutoloopEvent) -> bool {
    event.kind == "loop.finish" || event.kind == "summary"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;

    fn make_state() -> Arc<Mutex<TuiState>> {
        Arc::new(Mutex::new(TuiState::new()))
    }

    fn ev(json: &str) -> AutoloopEvent {
        serde_json::from_str(json).expect("valid AutoloopEvent json")
    }

    fn lines_text(state: &Arc<Mutex<TuiState>>) -> Vec<String> {
        let s = state.lock().unwrap();
        let Some(buf) = s.iterations.last() else {
            return Vec::new();
        };
        let lines = buf.lines.lock().unwrap();
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn iteration_start_sets_iteration_and_max() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new();

        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":2,"maxIterations":7,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );

        let s = state.lock().unwrap();
        assert_eq!(s.total_iterations(), 1);
        assert_eq!(s.iteration, 2);
        assert_eq!(s.max_iterations, Some(7));
        // Default role label before any progress event.
        assert_eq!(s.iterations[0].hat_display.as_deref(), Some("autoloop"));
        assert_eq!(s.iterations[0].backend.as_deref(), Some("autoloop"));
    }

    #[test]
    fn duplicate_iteration_start_does_not_create_second_buffer() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new();
        let line = r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#;
        apply_autoloop_event(&ev(line), &state, &mut ctx);
        apply_autoloop_event(&ev(line), &state, &mut ctx);
        let s = state.lock().unwrap();
        assert_eq!(
            s.total_iterations(),
            1,
            "same iteration number reuses buffer"
        );
    }

    #[test]
    fn progress_pushes_routing_line_and_updates_role() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new();

        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(
                r#"{"type":"progress","runId":"r1","iteration":1,"emittedTopic":"tasks.ready","outcome":"continue:routed_event","allowedRoles":["planner"]}"#,
            ),
            &state,
            &mut ctx,
        );

        let text = lines_text(&state);
        assert!(
            text.iter()
                .any(|l| l.contains("\u{2192} tasks.ready (continue:routed_event)")),
            "expected routing line, got: {text:?}"
        );

        // The role label is now available for the NEXT iteration's header.
        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":2,"maxIterations":3,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        let s = state.lock().unwrap();
        assert_eq!(
            s.iterations.last().unwrap().hat_display.as_deref(),
            Some("planner"),
            "second iteration should be labelled with the role from progress"
        );
    }

    #[test]
    fn backend_output_splits_into_lines() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new();
        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":1,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(r#"{"type":"backend.output","output":"first line\nsecond line\nthird"}"#),
            &state,
            &mut ctx,
        );

        let text = lines_text(&state);
        assert!(text.iter().any(|l| l.contains("first line")));
        assert!(text.iter().any(|l| l.contains("second line")));
        assert!(text.iter().any(|l| l.contains("third")));
    }

    #[test]
    fn ask_pending_sets_footer_and_line() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new();
        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":1,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(
                r#"{"type":"ask.pending","runId":"r1","iteration":1,"questionId":"q1","question":"Proceed with delete?"}"#,
            ),
            &state,
            &mut ctx,
        );

        {
            let s = state.lock().unwrap();
            assert_eq!(s.pending_ask.as_deref(), Some("Proceed with delete?"));
        }
        let text = lines_text(&state);
        assert!(
            text.iter()
                .any(|l| l.contains("HUMAN ASK") && l.contains("Proceed with delete?")),
            "expected human-ask line, got: {text:?}"
        );
    }

    #[test]
    fn loop_finish_completes_and_freezes_elapsed() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new();
        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(
                r#"{"type":"loop.finish","iterations":2,"stopReason":"max_iterations","runId":"r1","costUsd":0.08}"#,
            ),
            &state,
            &mut ctx,
        );

        let s = state.lock().unwrap();
        assert!(s.loop_completed);
        assert_eq!(s.iteration, 2);
        assert_eq!(s.final_cost_usd, Some(0.08));
        assert!(
            s.final_loop_elapsed.is_some(),
            "elapsed should be frozen on completion"
        );
    }

    fn append(path: &Path, s: &str) {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    #[tokio::test]
    async fn final_drain_after_cancel_applies_loop_finish() {
        // The terminal loop.finish is written AFTER cancel is signalled — the
        // reader's single final poll must still capture it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.ndjson");
        append(
            &path,
            "{\"type\":\"iteration.start\",\"iteration\":1,\"maxIterations\":3,\"runId\":\"r1\"}\n",
        );

        let state = make_state();
        let (cancel_tx, cancel_rx) = watch::channel(false);

        let reader_state = Arc::clone(&state);
        let reader_path = path.clone();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(reader_path, reader_state, cancel_rx).await;
        });

        // Give the reader a couple of ticks to consume the first event.
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;

        // Write the terminal event, THEN signal cancel (mirrors autoloop writing
        // loop.finish synchronously right before the subprocess exits).
        append(
            &path,
            "{\"type\":\"loop.finish\",\"iterations\":2,\"stopReason\":\"completed\",\"runId\":\"r1\",\"costUsd\":0.05}\n",
        );
        cancel_tx.send(true).unwrap();

        handle.await.unwrap();

        let s = state.lock().unwrap();
        assert!(s.loop_completed, "final drain should apply loop.finish");
        assert_eq!(s.iteration, 2);
        assert_eq!(s.final_cost_usd, Some(0.05));
    }

    #[tokio::test]
    async fn eof_without_terminal_event_appends_error_line() {
        // The stream has an iteration but no loop.finish before cancel.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.ndjson");
        append(
            &path,
            "{\"type\":\"iteration.start\",\"iteration\":1,\"maxIterations\":3,\"runId\":\"r1\"}\n",
        );

        let state = make_state();
        let (cancel_tx, cancel_rx) = watch::channel(false);

        let reader_state = Arc::clone(&state);
        let reader_path = path.clone();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(reader_path, reader_state, cancel_rx).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        cancel_tx.send(true).unwrap();
        handle.await.unwrap();

        let text = lines_text(&state);
        assert!(
            text.iter()
                .any(|l| l.contains("run ended before reporting a result")),
            "expected synthesized error line, got: {text:?}"
        );
        let s = state.lock().unwrap();
        assert!(s.loop_completed);
    }
}
