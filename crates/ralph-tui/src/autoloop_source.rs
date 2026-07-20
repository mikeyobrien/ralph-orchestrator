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
//! ~100ms interval rather than awaiting line-by-line. The same tick also polls
//! the active backend's bounded per-iteration stream, so the content pane
//! advances while an agent is still working and reconciles to authoritative
//! `backend.output` at the iteration boundary.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tokio::sync::watch;
use tracing::debug;

use ralph_adapters::{AutoloopEvent, AutoloopEventTailer, BackendStreamTailer, StreamLine};

use crate::state::TuiState;
use crate::state_mutations::apply_loop_completed;
use ralph_core::sanitize_tui_inline_text;

/// Per-reader translation context: tracks the current iteration's role label
/// from `iteration.banner` (autoloop's role ≈ ralph's hat), plus the last-seen
/// iteration number to detect boundaries.
#[derive(Debug, Default)]
pub struct AutoloopMapCtx {
    /// Display label announced for an iteration by
    /// `iteration.banner.allowedRoles[0]`.
    announced_role: Option<(u32, String)>,
    /// The iteration number of the most recently started iteration. Used to
    /// only reset/create a buffer when the number actually changes.
    current_iteration: Option<u32>,
    /// User-facing names for engine role IDs. Explicit presets leave this
    /// empty and display their role IDs directly.
    role_display_names: HashMap<String, String>,
    /// Workspace root supplied by Ralph when it launches the reader.
    workspace_root: Option<PathBuf>,
    /// Run-scoped directory derived from an event's documented `runId`.
    run_dir: Option<PathBuf>,
    /// Bounded tailer for the currently active iteration's backend stream.
    stream_tailer: Option<BackendStreamTailer>,
    /// Buffer index at which provisional live lines begin.
    live_region_mark: Option<usize>,
}

impl AutoloopMapCtx {
    /// Creates a context with optional user-facing names for engine role IDs.
    pub fn new(role_display_names: HashMap<String, String>) -> Self {
        Self {
            role_display_names,
            ..Self::default()
        }
    }

    /// Uses Ralph's authoritative workspace to locate the active run stream.
    fn with_workspace(mut self, workspace_root: PathBuf) -> Self {
        self.workspace_root = Some(workspace_root);
        self
    }

    /// The role display label to attribute to an iteration.
    fn role_display_for_iteration(&self, iteration: u32) -> String {
        self.announced_role
            .as_ref()
            .filter(|(announced_iteration, _)| *announced_iteration == iteration)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| "working".to_string())
    }

    /// Maps an engine role ID to its configured display name, falling back to
    /// the ID itself for explicit presets.
    fn role_display_name(&self, role_id: &str) -> String {
        self.role_display_names
            .get(role_id)
            .cloned()
            .unwrap_or_else(|| role_id.to_string())
    }
}

/// Applies one [`AutoloopEvent`] to the TUI state.
///
/// Maps each autoloop event `kind` onto the surviving [`TuiState`] mutators:
///
/// | kind             | effect                                                       |
/// |------------------|--------------------------------------------------------------|
/// | `iteration.banner` | records the current iteration's role label                |
/// | `iteration.start`| new iteration buffer + `iteration`/`max_iterations`          |
/// | `progress`       | `→ <emittedTopic> (<outcome>)` status line; expires asks      |
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

    // Ralph already knows the workspace it launched the engine in. Runtime
    // loop.start events do not consistently include workDir, so only depend on
    // the documented runId to locate the optional provisional stream.
    if ctx.run_dir.is_none()
        && let (Some(workspace_root), Some(run_id)) = (&ctx.workspace_root, &event.run_id)
    {
        ctx.run_dir = Some(workspace_root.join(".autoloop").join("runs").join(run_id));
    }

    match event.kind.as_str() {
        "loop.start" => {
            s.last_event = Some("loop.start".to_string());
            s.last_event_at = Some(now);
        }

        "iteration.banner" => {
            if let (Some(iteration), Some(role_id)) = (
                event.iteration,
                event.allowed_roles.as_ref().and_then(|roles| roles.first()),
            ) {
                let role_label = ctx.role_display_name(role_id);
                ctx.announced_role = Some((iteration, role_label.clone()));

                // The usual event order is iteration.start before
                // iteration.banner. Replace the neutral placeholder without
                // creating a second buffer. Banner-before-start remains
                // supported by retaining the announcement above.
                if ctx.current_iteration == Some(iteration) {
                    s.set_latest_iteration_hat_display(role_label);
                }
            }
            s.last_event = Some("iteration.banner".to_string());
            s.last_event_at = Some(now);
        }

        "iteration.start" => {
            let iteration = event.iteration.unwrap_or(0);
            // Only start a fresh buffer when the iteration number advances —
            // autoloop emits one iteration.start per iteration, but guard against
            // duplicates so we never split one iteration across two buffers.
            let is_new = ctx.current_iteration != Some(iteration);
            if is_new {
                ctx.current_iteration = Some(iteration);
                let role_display = ctx.role_display_for_iteration(iteration);
                s.start_new_iteration_with_metadata(Some(role_display), None);
                s.iteration = iteration;
                if let Some(max) = event.max_iterations {
                    s.max_iterations = Some(max);
                }
                s.iteration_started = Some(now);

                ctx.live_region_mark = s
                    .latest_iteration_lines_handle()
                    .and_then(|handle| handle.lock().ok().map(|lines| lines.len()));
                ctx.stream_tailer = ctx
                    .workspace_root
                    .as_deref()
                    .zip(ctx.run_dir.as_deref())
                    .map(|(workspace_root, run_dir)| {
                        BackendStreamTailer::for_iteration(workspace_root, run_dir, iteration)
                    });
            } else if let Some(max) = event.max_iterations {
                s.max_iterations = Some(max);
            }

            s.last_event = Some("iteration.start".to_string());
            s.last_event_at = Some(now);
        }

        "progress" => {
            if matches!(
                event.outcome.as_deref(),
                Some("ask:timeout" | "ask:answered")
            ) {
                s.pending_ask = None;
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
            let is_current_iteration =
                event.iteration.is_none() || event.iteration == ctx.current_iteration;
            if is_current_iteration {
                if let Some(mark) = ctx.live_region_mark.take()
                    && let Some(handle) = s.latest_iteration_lines_handle()
                    && let Ok(mut lines) = handle.lock()
                {
                    lines.truncate(mark);
                }
                // Once authoritative output arrives, never poll this iteration's
                // stream again or provisional lines could reappear afterward.
                ctx.stream_tailer = None;
            }

            if let Some(output) = &event.output {
                // Split the authoritative iteration result into display lines.
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
            s.pending_ask = Some(sanitize_tui_inline_text(&question));

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

/// Polls and renders provisional output from the active backend stream.
fn poll_backend_stream(state: &Arc<Mutex<TuiState>>, ctx: &mut AutoloopMapCtx) {
    let Some(tailer) = ctx.stream_tailer.as_mut() else {
        return;
    };
    let stream_lines = match tailer.poll() {
        Ok(lines) => lines,
        Err(error) => {
            debug!(%error, "autoloop backend stream poll failed");
            return;
        }
    };
    if stream_lines.is_empty() {
        return;
    }

    let rendered = stream_lines
        .into_iter()
        .map(|line| match line {
            StreamLine::AgentText(text) => Line::from(Span::styled(
                sanitize_tui_inline_text(&text),
                Style::default().add_modifier(Modifier::DIM),
            )),
            StreamLine::ToolSummary { text, .. } => Line::from(Span::styled(
                sanitize_tui_inline_text(&text),
                Style::default().fg(Color::Cyan),
            )),
            StreamLine::Backpressure { skipped_bytes } => Line::from(Span::styled(
                format!("… {skipped_bytes} bytes skipped …"),
                Style::default().add_modifier(Modifier::DIM),
            )),
        })
        .collect();
    if let Ok(mut state) = state.lock() {
        push_lines(&mut state, rendered);
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
pub async fn run_autoloop_event_reader<S>(
    events_path: PathBuf,
    workspace_root: PathBuf,
    state: Arc<Mutex<TuiState>>,
    mut cancel_rx: watch::Receiver<bool>,
    role_display_names: HashMap<String, String, S>,
) where
    S: BuildHasher + Send,
{
    let mut tailer = AutoloopEventTailer::new(events_path);
    let mut ctx = AutoloopMapCtx::new(role_display_names.into_iter().collect())
        .with_workspace(workspace_root);
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
                        poll_backend_stream(&state, &mut ctx);
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
    // Mirror the final event drain for a backend that was killed between ticks.
    // If backend.output landed above, reconciliation already dropped the tailer.
    poll_backend_stream(&state, &mut ctx);

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
    use ratatui::{Terminal, backend::TestBackend};
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

    fn render_header(state: &TuiState) -> String {
        let backend = TestBackend::new(80, 2);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(crate::widgets::header::render(state, 80), frame.area());
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn iteration_start_sets_iteration_and_max() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());

        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":2,"maxIterations":7,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );

        let s = state.lock().unwrap();
        assert_eq!(s.total_iterations(), 1);
        assert_eq!(s.iteration, 2);
        assert_eq!(s.max_iterations, Some(7));
        // Neutral role label until the role announcement arrives.
        assert_eq!(s.iterations[0].hat_display.as_deref(), Some("working"));
        assert_eq!(s.iterations[0].backend, None);
    }

    #[test]
    fn start_before_banner_resolves_live_header_without_engine_suffix() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());

        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        {
            let s = state.lock().unwrap();
            assert_eq!(s.total_iterations(), 1);
            assert_eq!(s.iterations[0].hat_display.as_deref(), Some("working"));
            assert_eq!(s.iterations[0].backend, None);
        }

        apply_autoloop_event(
            &ev(
                r#"{"type":"iteration.banner","iteration":1,"runId":"r1","allowedRoles":["planner"]}"#,
            ),
            &state,
            &mut ctx,
        );

        let mut s = state.lock().unwrap();
        s.following_latest = true;
        assert_eq!(s.total_iterations(), 1);
        assert_eq!(s.iterations[0].hat_display.as_deref(), Some("planner"));
        assert_eq!(s.iterations[0].backend, None);
        let header = render_header(&s);
        assert!(header.contains("planner"), "missing role in: {header}");
        assert!(
            header.contains("[LIVE]"),
            "missing live marker in: {header}"
        );
        assert!(
            !header.contains("@autoloop"),
            "engine suffix leaked in: {header}"
        );
    }

    #[test]
    fn banner_before_start_uses_announced_role_without_duplicate_buffer() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());

        apply_autoloop_event(
            &ev(
                r#"{"type":"iteration.banner","iteration":1,"runId":"r1","allowedRoles":["planner"]}"#,
            ),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );

        let s = state.lock().unwrap();
        assert_eq!(s.total_iterations(), 1);
        assert_eq!(s.iterations[0].hat_display.as_deref(), Some("planner"));
        assert_eq!(s.iterations[0].backend, None);
    }

    #[test]
    fn duplicate_iteration_start_does_not_create_second_buffer() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());
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
    fn banner_labels_each_iteration_with_its_current_role() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());

        // Real autoloop order is iteration.start followed by iteration.banner.
        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(
                r#"{"type":"iteration.banner","iteration":1,"maxIterations":3,"runId":"r1","allowedRoles":["planner"]}"#,
            ),
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

        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":2,"maxIterations":3,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(
                r#"{"type":"iteration.banner","iteration":2,"maxIterations":3,"runId":"r1","allowedRoles":["builder"]}"#,
            ),
            &state,
            &mut ctx,
        );

        let s = state.lock().unwrap();
        assert_eq!(s.iterations[0].hat_display.as_deref(), Some("planner"));
        assert_eq!(s.iterations[1].hat_display.as_deref(), Some("builder"));
        assert!(
            s.iterations
                .iter()
                .all(|iteration| { iteration.hat_display.as_deref() != Some("autoloop") })
        );
    }

    #[test]
    fn explicit_preset_uses_role_ids_for_all_iterations() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());

        for (iteration, role) in [(1, "planner"), (2, "builder"), (3, "finalizer")] {
            apply_autoloop_event(
                &ev(&format!(
                    r#"{{"type":"iteration.start","iteration":{iteration},"maxIterations":3,"runId":"r1"}}"#
                )),
                &state,
                &mut ctx,
            );
            apply_autoloop_event(
                &ev(&format!(
                    r#"{{"type":"iteration.banner","iteration":{iteration},"maxIterations":3,"runId":"r1","allowedRoles":["{role}"]}}"#
                )),
                &state,
                &mut ctx,
            );
        }

        let s = state.lock().unwrap();
        let labels: Vec<_> = s
            .iterations
            .iter()
            .map(|iteration| iteration.hat_display.as_deref())
            .collect();
        assert_eq!(
            labels,
            vec![Some("planner"), Some("builder"), Some("finalizer")]
        );
    }

    #[test]
    fn banner_maps_role_id_to_display_name() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::from([(
            "builder".to_string(),
            "🔨 Builder".to_string(),
        )]));

        apply_autoloop_event(
            &ev(r#"{"type":"iteration.start","iteration":1,"runId":"r1"}"#),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(
                r#"{"type":"iteration.banner","iteration":1,"runId":"r1","allowedRoles":["builder"]}"#,
            ),
            &state,
            &mut ctx,
        );

        let s = state.lock().unwrap();
        assert_eq!(s.iterations[0].hat_display.as_deref(), Some("🔨 Builder"));
    }

    #[test]
    fn backend_output_splits_into_lines() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());
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
        let mut ctx = AutoloopMapCtx::new(HashMap::new());
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
    fn progress_timeout_clears_pending_ask() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());
        apply_autoloop_event(
            &ev(r#"{"type":"ask.pending","question":"Still waiting?"}"#),
            &state,
            &mut ctx,
        );
        apply_autoloop_event(
            &ev(r#"{"type":"progress","outcome":"ask:timeout"}"#),
            &state,
            &mut ctx,
        );

        assert_eq!(state.lock().unwrap().pending_ask, None);
    }

    #[test]
    fn ask_pending_sanitizes_footer_question() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());
        apply_autoloop_event(
            &ev("{\"type\":\"ask.pending\",\"question\":\"line1\\nline2\\u0007\"}"),
            &state,
            &mut ctx,
        );

        let s = state.lock().unwrap();
        assert_eq!(s.pending_ask.as_deref(), Some("line1 line2"));
        let question = s.pending_ask.as_deref().unwrap();
        assert!(!question.contains(['\n', '\r', '\u{0007}']));
    }

    #[test]
    fn loop_finish_completes_and_freezes_elapsed() {
        let state = make_state();
        let mut ctx = AutoloopMapCtx::new(HashMap::new());
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

    async fn wait_for_line(state: &Arc<Mutex<TuiState>>, needle: &str) -> bool {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while tokio::time::Instant::now() < deadline {
            if lines_text(state).iter().any(|line| line.contains(needle)) {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        false
    }

    async fn wait_for_iteration(state: &Arc<Mutex<TuiState>>) -> bool {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while tokio::time::Instant::now() < deadline {
            if state.lock().unwrap().total_iterations() > 0 {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        false
    }

    #[tokio::test]
    async fn reader_uses_ralph_workspace_when_loop_start_omits_work_dir() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let run_dir = workspace.join(".autoloop/runs/live-run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let events_path = dir.path().join("events.ndjson");
        append(
            &events_path,
            &format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "loop.start",
                    "runId": "live-run",
                }),
                serde_json::json!({
                    "type": "iteration.start",
                    "iteration": 1,
                    "maxIterations": 3,
                    "runId": "live-run",
                })
            ),
        );

        let state = make_state();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let reader_state = Arc::clone(&state);
        let reader_workspace = workspace.clone();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                events_path,
                reader_workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
        });

        assert!(
            wait_for_iteration(&state).await,
            "iteration buffer never appeared"
        );
        let private_path = run_dir.join("plan.md");
        append(
            &run_dir.join("pi-stream.1.jsonl"),
            &format!(
                "{}\n{}\n{}\n",
                serde_json::json!({
                    "type": "message_end",
                    "message": {
                        "role": "assistant",
                        "content": [{"type": "text", "text": "live before boundary"}],
                    },
                }),
                serde_json::json!({
                    "type": "tool_execution_start",
                    "toolName": "read",
                    "args": {"path": private_path},
                }),
                serde_json::json!({
                    "type": "tool_execution_start",
                    "toolName": "read",
                    "args": {"path": "crates/ralph-core/src/lib.rs"},
                }),
            ),
        );

        let visible = wait_for_line(&state, "live before boundary").await;
        let private_tool_visible = wait_for_line(&state, "⚙ read: engine:plan.md").await;
        let repository_tool_visible =
            wait_for_line(&state, "⚙ read: crates/ralph-core/src/lib.rs").await;
        {
            let state = state.lock().unwrap();
            let lines = state.iterations.last().unwrap().lines.lock().unwrap();
            let agent = lines
                .iter()
                .find(|line| line.to_string().contains("live before boundary"))
                .unwrap();
            let tool = lines
                .iter()
                .find(|line| line.to_string().contains("⚙ read"))
                .unwrap();
            assert!(agent.spans[0].style.add_modifier.contains(Modifier::DIM));
            assert_eq!(tool.spans[0].style.fg, Some(Color::Cyan));
        }
        cancel_tx.send(true).unwrap();
        handle.await.unwrap();

        let text = lines_text(&state);
        assert!(
            visible && private_tool_visible && repository_tool_visible,
            "stream text and tool summaries should be visible before backend.output: {text:?}"
        );
        assert!(
            text.iter().all(|line| {
                !line.contains(&workspace.display().to_string())
                    && !line.contains(".autoloop/runs/")
            }),
            "visible tool paths must not expose workspace or private run prefixes: {text:?}"
        );
    }

    #[tokio::test]
    async fn backend_output_replaces_the_provisional_live_region() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let run_dir = workspace.join(".autoloop/runs/reconcile-run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let events_path = dir.path().join("events.ndjson");
        append(
            &events_path,
            &format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "loop.start",
                    "runId": "reconcile-run",
                    "workDir": workspace,
                }),
                serde_json::json!({
                    "type": "iteration.start",
                    "iteration": 1,
                    "runId": "reconcile-run",
                })
            ),
        );
        append(
            &run_dir.join("pi-stream.1.jsonl"),
            concat!(
                r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"provisional live text"}]}}"#,
                "\n",
            ),
        );

        let state = make_state();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let reader_state = Arc::clone(&state);
        let reader_events = events_path.clone();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                reader_events,
                workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
        });
        assert!(wait_for_line(&state, "provisional live text").await);

        append(
            &events_path,
            concat!(
                r#"{"type":"backend.output","iteration":1,"runId":"reconcile-run","output":"authoritative final text"}"#,
                "\n",
                r#"{"type":"loop.finish","runId":"reconcile-run","iterations":1,"stopReason":"completed"}"#,
                "\n",
            ),
        );
        assert!(wait_for_line(&state, "authoritative final text").await);
        cancel_tx.send(true).unwrap();
        handle.await.unwrap();

        let text = lines_text(&state);
        assert_eq!(
            text.iter()
                .filter(|line| line.contains("authoritative final text"))
                .count(),
            1,
            "authoritative output must appear exactly once: {text:?}"
        );
        assert!(
            text.iter()
                .all(|line| !line.contains("provisional live text")),
            "provisional live output must be removed: {text:?}"
        );
    }

    #[tokio::test]
    async fn huge_live_stream_keeps_the_tui_buffer_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let run_dir = workspace.join(".autoloop/runs/bounded-run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let events_path = dir.path().join("events.ndjson");
        append(
            &events_path,
            &format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "loop.start",
                    "runId": "bounded-run",
                    "workDir": workspace,
                }),
                serde_json::json!({
                    "type": "iteration.start",
                    "iteration": 1,
                    "runId": "bounded-run",
                })
            ),
        );

        let mut stream = String::new();
        for index in 0..5_000 {
            stream.push_str(
                &serde_json::json!({
                    "type": "message_end",
                    "message": {
                        "role": "assistant",
                        "content": [{
                            "type": "text",
                            "text": format!("bounded line {index:04}"),
                        }],
                    },
                })
                .to_string(),
            );
            stream.push('\n');
        }
        stream.push_str(
            concat!(
                r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"newest bounded marker"}]}}"#,
                "\n",
            ),
        );
        std::fs::write(run_dir.join("pi-stream.1.jsonl"), stream).unwrap();

        let state = make_state();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let reader_state = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                events_path,
                workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
        });
        assert!(wait_for_line(&state, "newest bounded marker").await);

        let line_count = lines_text(&state).len();
        assert!(
            line_count <= ralph_adapters::backend_stream_tailer::MAX_STREAM_LINES,
            "live buffer exceeded stream cap: {line_count}"
        );

        cancel_tx.send(true).unwrap();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn missing_backend_stream_preserves_boundary_only_output() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let events_path = dir.path().join("events.ndjson");
        append(
            &events_path,
            &format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "loop.start",
                    "runId": "command-run",
                    "workDir": workspace,
                }),
                serde_json::json!({
                    "type": "iteration.start",
                    "iteration": 1,
                    "runId": "command-run",
                })
            ),
        );

        let state = make_state();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let reader_state = Arc::clone(&state);
        let reader_events = events_path.clone();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                reader_events,
                workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
        });
        assert!(wait_for_iteration(&state).await);
        assert!(lines_text(&state).is_empty());

        append(
            &events_path,
            concat!(
                r#"{"type":"backend.output","iteration":1,"runId":"command-run","output":"command boundary output"}"#,
                "\n",
                r#"{"type":"loop.finish","runId":"command-run","iterations":1,"stopReason":"completed"}"#,
                "\n",
            ),
        );
        assert!(wait_for_line(&state, "command boundary output").await);
        cancel_tx.send(true).unwrap();
        handle.await.unwrap();

        let text = lines_text(&state);
        assert_eq!(
            text.iter()
                .filter(|line| line.contains("command boundary output"))
                .count(),
            1,
            "boundary output should retain existing behavior: {text:?}"
        );
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
        let workspace = dir.path().to_path_buf();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                reader_path,
                workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
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
        let workspace = dir.path().to_path_buf();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                reader_path,
                workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
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
