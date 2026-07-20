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

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::BuildHasher;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tokio::sync::watch;
use tracing::debug;

use ralph_adapters::{
    AutoloopEvent, AutoloopEventTailer, BackendStreamTailer, StreamLine,
    backend_stream_tailer::{MAX_STREAM_LINE_BYTES, MAX_STREAM_LINES, ToolCallIdentity},
};

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
    /// Bounded content that existed before this iteration's live region.
    iteration_prefix: VecDeque<Line<'static>>,
    /// Newest bounded assistant/tool presentation for the active iteration.
    live_items: VecDeque<LiveItem>,
    /// Tool identities retained independently from rendered text.
    seen_live_tool_ids: HashSet<ToolCallIdentity>,
    /// Bounded insertion order for tool identity retention.
    live_tool_id_order: VecDeque<ToolCallIdentity>,
    /// Latest cumulative dropped-byte count, rendered as one replaceable status.
    backpressure_bytes: Option<u64>,
    /// Newest bounded lifecycle/status lines emitted after iteration start.
    lifecycle_lines: VecDeque<Line<'static>>,
    /// Newest distinct tool summaries retained after authoritative reconciliation.
    completed_tool_lines: VecDeque<Line<'static>>,
    /// Bounded authoritative output lines, retained as a newest tail.
    authoritative_lines: VecDeque<Line<'static>>,
    /// Prevents replayed `backend.output` events from appending final text twice.
    authoritative_output_applied: bool,
}

#[derive(Debug)]
enum LiveItem {
    Assistant(Line<'static>),
    Tool(Line<'static>),
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

                ctx.iteration_prefix.clear();
                ctx.live_region_mark = s.latest_iteration_lines_handle().and_then(|handle| {
                    handle.lock().ok().map(|lines| {
                        ctx.iteration_prefix.extend(lines.iter().cloned());
                        lines.len()
                    })
                });
                while ctx.iteration_prefix.len() > MAX_STREAM_LINES {
                    ctx.iteration_prefix.pop_front();
                }
                ctx.live_items.clear();
                ctx.seen_live_tool_ids.clear();
                ctx.live_tool_id_order.clear();
                ctx.backpressure_bytes = None;
                ctx.lifecycle_lines.clear();
                ctx.completed_tool_lines.clear();
                ctx.authoritative_lines.clear();
                ctx.authoritative_output_applied = false;
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
            push_iteration_line(
                &mut s,
                ctx,
                Line::from(vec![Span::styled(
                    bounded_inline_text(&text),
                    Style::default().fg(Color::Cyan),
                )]),
            );

            s.last_event = Some("progress".to_string());
            s.last_event_at = Some(now);
        }

        "backend.output" => {
            let is_current_iteration =
                event.iteration.is_none() || event.iteration == ctx.current_iteration;
            if is_current_iteration && !ctx.authoritative_output_applied {
                // Take the tailer before rebuilding the region so no later tick
                // can resurrect provisional output from this stream.
                ctx.stream_tailer = None;
                ctx.authoritative_output_applied = true;
                reconcile_authoritative_output(&mut s, ctx, event.output.as_deref());
            }

            s.last_event = Some("backend.output".to_string());
            s.last_event_at = Some(now);
        }

        "ask.pending" => {
            let question = event.question.clone().unwrap_or_default();
            push_iteration_line(
                &mut s,
                ctx,
                Line::from(vec![
                    Span::styled(
                        "\u{26A0} HUMAN ASK: ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(bounded_inline_text(&question)),
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
                push_iteration_line(
                    &mut s,
                    ctx,
                    Line::from(vec![
                        Span::styled("\u{25A0} run finished: ", Style::default().fg(Color::Blue)),
                        Span::raw(bounded_inline_text(stop_reason)),
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

const COMPLETED_TOOL_LINE_BUDGET: usize = 256;
const LIFECYCLE_LINE_BUDGET: usize = 64;

/// Records an autoloop lifecycle line without allowing it to escape the shared
/// iteration bound. Once an iteration has started, rendering owns the whole
/// bounded region so later events cannot evict the protected tool slice.
fn push_iteration_line(state: &mut TuiState, ctx: &mut AutoloopMapCtx, line: Line<'static>) {
    if ctx.live_region_mark.is_some() {
        push_bounded(&mut ctx.lifecycle_lines, line, LIFECYCLE_LINE_BUDGET);
        if ctx.authoritative_output_applied {
            render_completed_region(state, ctx);
        } else {
            render_live_region(state, ctx);
        }
        return;
    }

    if let Some(handle) = state.latest_iteration_lines_handle()
        && let Ok(mut lines) = handle.lock()
    {
        if lines.len() == MAX_STREAM_LINES {
            lines.remove(0);
        }
        lines.push(line);
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

    for stream_line in stream_lines {
        match stream_line {
            StreamLine::AgentText(text) => {
                ctx.live_items
                    .push_back(LiveItem::Assistant(styled_stream_line(
                        &text,
                        Style::default().add_modifier(Modifier::DIM),
                    )))
            }
            StreamLine::ToolSummary { identity, text } => {
                if remember_live_tool(ctx, identity) {
                    let line = styled_stream_line(&text, Style::default().fg(Color::Cyan));
                    push_bounded(
                        &mut ctx.completed_tool_lines,
                        line.clone(),
                        COMPLETED_TOOL_LINE_BUDGET,
                    );
                    ctx.live_items.push_back(LiveItem::Tool(line));
                }
            }
            StreamLine::Backpressure { skipped_bytes } => {
                ctx.backpressure_bytes = Some(skipped_bytes);
            }
        }
    }
    trim_live_items(ctx);
    if let Ok(mut state) = state.lock() {
        render_live_region(&mut state, ctx);
    }
}

fn remember_live_tool(ctx: &mut AutoloopMapCtx, identity: ToolCallIdentity) -> bool {
    if !ctx.seen_live_tool_ids.insert(identity.clone()) {
        return false;
    }
    if ctx.live_tool_id_order.len() == MAX_STREAM_LINES
        && let Some(expired) = ctx.live_tool_id_order.pop_front()
    {
        ctx.seen_live_tool_ids.remove(&expired);
    }
    ctx.live_tool_id_order.push_back(identity);
    true
}

fn trim_live_items(ctx: &mut AutoloopMapCtx) {
    while ctx.live_items.len() > MAX_STREAM_LINES {
        ctx.live_items.pop_front();
    }
}

fn render_live_region(state: &mut TuiState, ctx: &AutoloopMapCtx) {
    let Some(mark) = ctx.live_region_mark else {
        return;
    };
    let Some(handle) = state.latest_iteration_lines_handle() else {
        return;
    };
    let Ok(mut lines) = handle.lock() else {
        return;
    };
    let prefix: Vec<_> = ctx.iteration_prefix.iter().cloned().collect();
    debug_assert_eq!(mark, ctx.iteration_prefix.len());
    let status_lines = usize::from(ctx.backpressure_bytes.is_some());
    let lifecycle_count = ctx
        .lifecycle_lines
        .len()
        .min(LIFECYCLE_LINE_BUDGET)
        .min(MAX_STREAM_LINES.saturating_sub(status_lines));
    let live_count = ctx.live_items.len().min(
        MAX_STREAM_LINES
            .saturating_sub(status_lines)
            .saturating_sub(lifecycle_count),
    );
    let prefix_count = prefix
        .len()
        .min(MAX_STREAM_LINES.saturating_sub(status_lines + lifecycle_count + live_count));

    lines.clear();
    lines.extend(
        prefix
            .into_iter()
            .rev()
            .take(prefix_count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev(),
    );
    if let Some(skipped_bytes) = ctx.backpressure_bytes {
        lines.push(backpressure_line(skipped_bytes));
    }
    lines.extend(
        ctx.live_items
            .iter()
            .skip(ctx.live_items.len() - live_count)
            .map(|item| match item {
                LiveItem::Assistant(line) | LiveItem::Tool(line) => line.clone(),
            }),
    );
    lines.extend(
        ctx.lifecycle_lines
            .iter()
            .skip(ctx.lifecycle_lines.len() - lifecycle_count)
            .cloned(),
    );
    debug_assert!(lines.len() <= MAX_STREAM_LINES);
}

fn reconcile_authoritative_output(
    state: &mut TuiState,
    ctx: &mut AutoloopMapCtx,
    output: Option<&str>,
) {
    if ctx.live_region_mark.is_none() {
        return;
    }

    ctx.authoritative_lines.clear();
    if let Some(output) = output {
        for text in output.split('\n') {
            push_bounded(
                &mut ctx.authoritative_lines,
                Line::raw(bounded_inline_text(text)),
                MAX_STREAM_LINES,
            );
        }
    }
    ctx.live_items.clear();
    render_completed_region(state, ctx);
}

/// Rebuilds completed history with explicit slices. Tool history and lifecycle
/// status are reserved before authoritative output consumes the remaining
/// capacity; authoritative output itself is always the newest bounded tail.
fn render_completed_region(state: &mut TuiState, ctx: &AutoloopMapCtx) {
    let Some(mark) = ctx.live_region_mark else {
        return;
    };
    let Some(handle) = state.latest_iteration_lines_handle() else {
        return;
    };
    let Ok(mut lines) = handle.lock() else {
        return;
    };
    let prefix: Vec<_> = ctx.iteration_prefix.iter().cloned().collect();
    debug_assert_eq!(mark, ctx.iteration_prefix.len());
    let status_count = usize::from(ctx.backpressure_bytes.is_some());
    let mut available = MAX_STREAM_LINES.saturating_sub(status_count);

    let lifecycle_count = ctx
        .lifecycle_lines
        .len()
        .min(LIFECYCLE_LINE_BUDGET)
        .min(available);
    available -= lifecycle_count;

    let authoritative_min = usize::from(!ctx.authoritative_lines.is_empty());
    let tool_count = ctx
        .completed_tool_lines
        .len()
        .min(COMPLETED_TOOL_LINE_BUDGET)
        .min(available.saturating_sub(authoritative_min));
    available -= tool_count;

    let authoritative_count = ctx.authoritative_lines.len().min(available);
    available -= authoritative_count;
    let prefix_count = prefix.len().min(available);

    lines.clear();
    lines.extend(
        prefix
            .into_iter()
            .rev()
            .take(prefix_count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev(),
    );
    if let Some(skipped_bytes) = ctx.backpressure_bytes {
        lines.push(backpressure_line(skipped_bytes));
    }
    lines.extend(
        ctx.completed_tool_lines
            .iter()
            .skip(ctx.completed_tool_lines.len() - tool_count)
            .cloned(),
    );
    lines.extend(
        ctx.authoritative_lines
            .iter()
            .skip(ctx.authoritative_lines.len() - authoritative_count)
            .cloned(),
    );
    lines.extend(
        ctx.lifecycle_lines
            .iter()
            .skip(ctx.lifecycle_lines.len() - lifecycle_count)
            .cloned(),
    );
    debug_assert!(lines.len() <= MAX_STREAM_LINES);
}

fn push_bounded(lines: &mut VecDeque<Line<'static>>, line: Line<'static>, capacity: usize) {
    if capacity == 0 {
        return;
    }
    if lines.len() == capacity {
        lines.pop_front();
    }
    lines.push_back(line);
}

fn styled_stream_line(text: &str, style: Style) -> Line<'static> {
    Line::from(Span::styled(bounded_inline_text(text), style))
}

fn backpressure_line(skipped_bytes: u64) -> Line<'static> {
    styled_stream_line(
        &format!("… {skipped_bytes} bytes skipped …"),
        Style::default().add_modifier(Modifier::DIM),
    )
}

fn bounded_inline_text(text: &str) -> String {
    let sanitized = sanitize_tui_inline_text(text);
    if sanitized.len() <= MAX_STREAM_LINE_BYTES {
        return sanitized;
    }
    let mut end = MAX_STREAM_LINE_BYTES.saturating_sub('…'.len_utf8());
    while !sanitized.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    let mut bounded = sanitized[..end].to_string();
    bounded.push('…');
    bounded
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
        push_iteration_line(
            &mut s,
            &mut ctx,
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

    fn iteration_lines_text(state: &Arc<Mutex<TuiState>>, index: usize) -> Vec<String> {
        let s = state.lock().unwrap();
        let lines = s.iterations[index].lines.lock().unwrap();
        lines.iter().map(|line| line.to_string()).collect()
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
        assert!(
            text.iter().all(|line| !line.contains("bytes skipped")),
            "normal incremental stream must not show backpressure: {text:?}"
        );
    }

    #[tokio::test]
    async fn backend_output_preserves_distinct_tools_across_history_and_export() {
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
        let stream_path = run_dir.join("pi-stream.1.jsonl");
        append(
            &stream_path,
            concat!(
                r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"provisional live text"}]}}"#,
                "\n",
                r#"{"type":"tool_execution_start","toolCallId":"call-a","toolName":"read","args":{"path":"Cargo.toml"}}"#,
                "\n",
            ),
        );

        let state = make_state();
        state.lock().unwrap().set_export_workspace_root(&workspace);
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let reader_state = Arc::clone(&state);
        let reader_events = events_path.clone();
        let reader_workspace = workspace.clone();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                reader_events,
                reader_workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
        });
        assert!(wait_for_line(&state, "provisional live text").await);
        assert!(wait_for_line(&state, "⚙ read: Cargo.toml").await);

        // Pi streams may replay cumulative records. The replayed stable ID is
        // suppressed while a genuinely distinct same-rendering ID survives.
        append(
            &stream_path,
            concat!(
                r#"{"type":"tool_execution_start","toolCallId":"call-a","toolName":"read","args":{"path":"Cargo.toml"}}"#,
                "\n",
                r#"{"type":"tool_execution_start","toolCallId":"call-b","toolName":"read","args":{"path":"Cargo.toml"}}"#,
                "\n",
            ),
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let count = lines_text(&state)
                .iter()
                .filter(|line| line.contains("⚙ read: Cargo.toml"))
                .count();
            if count == 2 {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "second tool never appeared"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        append(
            &events_path,
            concat!(
                r#"{"type":"backend.output","iteration":1,"runId":"reconcile-run","output":"authoritative final text"}"#,
                "\n",
                r#"{"type":"backend.output","iteration":1,"runId":"reconcile-run","output":"authoritative final text"}"#,
                "\n",
            ),
        );
        assert!(wait_for_line(&state, "authoritative final text").await);

        // Content appended after the boundary must never resume this tailer.
        append(
            &stream_path,
            concat!(
                r#"{"type":"tool_execution_start","toolCallId":"call-c","toolName":"read","args":{"path":"must-not-resume.rs"}}"#,
                "\n",
            ),
        );
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        append(
            &events_path,
            concat!(
                r#"{"type":"iteration.start","iteration":2,"runId":"reconcile-run"}"#,
                "\n",
                r#"{"type":"loop.finish","runId":"reconcile-run","iterations":2,"stopReason":"completed"}"#,
                "\n",
            ),
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while state.lock().unwrap().total_iterations() != 2 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "second iteration never appeared"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        cancel_tx.send(true).unwrap();
        handle.await.unwrap();

        let completed = iteration_lines_text(&state, 0);
        assert_eq!(
            completed
                .iter()
                .filter(|line| line.contains("⚙ read: Cargo.toml"))
                .count(),
            2,
            "stable replay must be removed but distinct IDs retained: {completed:?}"
        );
        assert_eq!(
            completed
                .iter()
                .filter(|line| line.contains("authoritative final text"))
                .count(),
            1,
            "authoritative output must appear exactly once: {completed:?}"
        );
        assert!(
            completed
                .iter()
                .all(|line| !line.contains("provisional live text"))
        );
        assert!(
            completed
                .iter()
                .all(|line| !line.contains("must-not-resume.rs"))
        );

        {
            let mut state = state.lock().unwrap();
            state.navigate_prev();
            assert_eq!(state.current_view, 0);
            state.navigate_next();
            state.navigate_prev();
            assert!(state.export_current_iteration_to_disk());
        }
        assert_eq!(iteration_lines_text(&state, 0), completed);
        let export_dir = crate::export::export_dir(&workspace);
        let export_path = std::fs::read_dir(export_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let exported = std::fs::read_to_string(export_path).unwrap();
        assert_eq!(exported.matches("⚙ read: Cargo.toml").count(), 2);
        assert_eq!(exported.matches("authoritative final text").count(), 1);
        assert!(!exported.contains("provisional live text"));
        assert!(!exported.contains("must-not-resume.rs"));
    }

    #[tokio::test]
    async fn saturated_reconciliation_preserves_tools_final_tail_lifecycle_and_export() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let run_dir = workspace.join(".autoloop/runs/pressure-run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let events_path = dir.path().join("events.ndjson");
        append(
            &events_path,
            &format!(
                "{}\n{}\n",
                serde_json::json!({"type":"loop.start","runId":"pressure-run"}),
                serde_json::json!({"type":"iteration.start","iteration":1,"runId":"pressure-run"}),
            ),
        );
        let stream_path = run_dir.join("pi-stream.1.jsonl");
        append(
            &stream_path,
            concat!(
                r#"{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"provisional must disappear"}]}}"#,
                "\n",
                r#"{"type":"tool_execution_start","toolCallId":"pressure-a","toolName":"read","args":{"path":"same.rs"}}"#,
                "\n",
                r#"{"type":"tool_execution_start","toolCallId":"pressure-b","toolName":"read","args":{"path":"same.rs"}}"#,
                "\n",
                r#"{"type":"tool_execution_start","toolCallId":"pressure-c","toolName":"write","args":{"path":"newest-tool.rs"}}"#,
                "\n",
            ),
        );

        let state = make_state();
        state.lock().unwrap().set_export_workspace_root(&workspace);
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let reader_state = Arc::clone(&state);
        let reader_events = events_path.clone();
        let reader_workspace = workspace.clone();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                reader_events,
                reader_workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
        });
        assert!(wait_for_line(&state, "newest-tool.rs").await);

        let authoritative = (0..(MAX_STREAM_LINES + 200))
            .map(|index| format!("authoritative pressure line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        append(
            &events_path,
            &format!(
                "{}\n",
                serde_json::json!({
                    "type":"backend.output",
                    "iteration":1,
                    "runId":"pressure-run",
                    "output":authoritative,
                })
            ),
        );
        assert!(wait_for_line(&state, "authoritative pressure line 2199").await);
        for index in 0..100 {
            append(
                &events_path,
                &format!(
                    "{}\n",
                    serde_json::json!({
                        "type":"progress",
                        "iteration":1,
                        "runId":"pressure-run",
                        "emittedTopic":format!("pressure.lifecycle.{index}"),
                    })
                ),
            );
        }
        append(
            &events_path,
            &format!(
                "{}\n",
                serde_json::json!({
                    "type":"loop.finish",
                    "iterations":1,
                    "runId":"pressure-run",
                    "stopReason":"pressure-completed",
                })
            ),
        );
        assert!(wait_for_line(&state, "run finished: pressure-completed").await);

        // Add a second iteration solely to exercise historical navigation.
        append(
            &events_path,
            &format!(
                "{}\n{}\n",
                serde_json::json!({"type":"iteration.start","iteration":2,"runId":"pressure-run"}),
                serde_json::json!({"type":"backend.output","iteration":2,"runId":"pressure-run","output":"second iteration"}),
            ),
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while state.lock().unwrap().total_iterations() != 2 {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        cancel_tx.send(true).unwrap();
        handle.await.unwrap();

        let completed = iteration_lines_text(&state, 0);
        assert!(completed.len() <= MAX_STREAM_LINES);
        assert_eq!(
            completed
                .iter()
                .filter(|line| line.contains("⚙ read: same.rs"))
                .count(),
            2
        );
        assert_eq!(
            completed
                .iter()
                .filter(|line| line.contains("newest-tool.rs"))
                .count(),
            1
        );
        assert_eq!(
            completed
                .iter()
                .filter(|line| line.contains("authoritative pressure line 2199"))
                .count(),
            1
        );
        assert!(
            !completed
                .iter()
                .any(|line| line.contains("provisional must disappear"))
        );
        assert!(
            completed
                .iter()
                .any(|line| line.contains("pressure.lifecycle.99"))
        );
        assert!(
            completed
                .iter()
                .any(|line| line.contains("run finished: pressure-completed"))
        );

        {
            let mut state = state.lock().unwrap();
            state.navigate_prev();
            assert_eq!(state.current_view, 0);
            assert!(state.export_current_iteration_to_disk());
            state.navigate_next();
            state.navigate_prev();
        }
        assert_eq!(iteration_lines_text(&state, 0), completed);
        let export_path = std::fs::read_dir(crate::export::export_dir(&workspace))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let exported = std::fs::read_to_string(export_path).unwrap();
        let exported_content_count = exported
            .lines()
            .find_map(|line| line.strip_prefix("Lines: "))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(exported_content_count <= MAX_STREAM_LINES);
        assert_eq!(exported_content_count, completed.len());
        assert_eq!(exported.matches("⚙ read: same.rs").count(), 2);
        assert_eq!(exported.matches("newest-tool.rs").count(), 1);
        assert_eq!(
            exported.matches("authoritative pressure line 2199").count(),
            1
        );
        assert!(exported.contains("run finished: pressure-completed"));
        assert!(!exported.contains("provisional must disappear"));
    }

    #[tokio::test]
    async fn saturated_cancellation_retains_eof_warning_within_bound() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let run_dir = workspace.join(".autoloop/runs/eof-pressure-run");
        std::fs::create_dir_all(&run_dir).unwrap();
        let events_path = dir.path().join("events.ndjson");
        append(
            &events_path,
            &format!(
                "{}\n{}\n",
                serde_json::json!({"type":"loop.start","runId":"eof-pressure-run"}),
                serde_json::json!({"type":"iteration.start","iteration":1,"runId":"eof-pressure-run"}),
            ),
        );
        append(
            &run_dir.join("pi-stream.1.jsonl"),
            concat!(
                r#"{"type":"tool_execution_start","toolCallId":"eof-a","toolName":"read","args":{"path":"eof-tool-a.rs"}}"#,
                "\n",
                r#"{"type":"tool_execution_start","toolCallId":"eof-b","toolName":"read","args":{"path":"eof-tool-b.rs"}}"#,
                "\n",
            ),
        );

        let state = make_state();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let reader_state = Arc::clone(&state);
        let reader_events = events_path.clone();
        let reader_workspace = workspace.clone();
        let handle = tokio::spawn(async move {
            run_autoloop_event_reader(
                reader_events,
                reader_workspace,
                reader_state,
                cancel_rx,
                HashMap::new(),
            )
            .await;
        });
        assert!(wait_for_line(&state, "eof-tool-b.rs").await);
        let authoritative = (0..(MAX_STREAM_LINES + 50))
            .map(|index| format!("eof authoritative {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        append(
            &events_path,
            &format!(
                "{}\n",
                serde_json::json!({
                    "type":"backend.output",
                    "iteration":1,
                    "runId":"eof-pressure-run",
                    "output":authoritative,
                })
            ),
        );
        assert!(wait_for_line(&state, "eof authoritative 2049").await);
        cancel_tx.send(true).unwrap();
        handle.await.unwrap();

        let text = lines_text(&state);
        assert!(text.len() <= MAX_STREAM_LINES);
        assert!(text.iter().any(|line| line.contains("eof-tool-a.rs")));
        assert!(text.iter().any(|line| line.contains("eof-tool-b.rs")));
        assert!(
            text.iter()
                .any(|line| line.contains("eof authoritative 2049"))
        );
        assert!(
            text.iter()
                .any(|line| line.contains("run ended before reporting a result"))
        );
    }

    #[tokio::test]
    async fn repeated_oversized_growth_coalesces_status_and_retains_newest_lines() {
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

        let stream_path = run_dir.join("pi-stream.1.jsonl");
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
        assert!(wait_for_iteration(&state).await);

        // One oversized unterminated record is entirely discarded while the
        // tailer's pending memory remains bounded.
        let unterminated =
            MAX_STREAM_LINE_BYTES + ralph_adapters::backend_stream_tailer::MAX_BYTES_PER_POLL;
        append(&stream_path, &"x".repeat(unterminated));
        assert!(wait_for_line(&state, &format!("… {unterminated} bytes skipped …")).await);

        let polls = 6_u64;
        let oversized = ralph_adapters::backend_stream_tailer::MAX_BYTES_PER_POLL;
        for index in 0..polls {
            let record = serde_json::json!({
                "type": "message_end",
                "message": {
                    "role": "assistant",
                    "content": [{"type": "text", "text": format!("newest marker {index}")}],
                },
            });
            let tool = serde_json::json!({
                "type": "tool_execution_start",
                "toolCallId": format!("newest-tool-{index}"),
                "toolName": "read",
                "args": {"path": format!("newest-tool-{index}.rs")},
            });
            append(
                &stream_path,
                &format!("\n{}\n{}\n{}\n", "x".repeat(oversized), record, tool),
            );
            assert!(wait_for_line(&state, &format!("newest marker {index}")).await);
            assert!(wait_for_line(&state, &format!("newest-tool-{index}.rs")).await);
        }

        let expected_skipped = unterminated as u64 + polls * (oversized as u64 + 2);
        assert!(
            wait_for_line(&state, &format!("… {expected_skipped} bytes skipped …")).await,
            "cumulative status did not reach expected total"
        );
        let text = lines_text(&state);
        assert_eq!(
            text.iter()
                .filter(|line| line.contains("bytes skipped"))
                .count(),
            1,
            "backpressure updates must replace one visible status: {text:?}"
        );
        assert!(text.iter().any(|line| line.contains("newest marker 5")));
        assert!(text.iter().any(|line| line.contains("newest-tool-5.rs")));
        assert!(text.len() <= MAX_STREAM_LINES, "live buffer exceeded cap");
        assert!(
            text.iter().all(|line| line.len() <= MAX_STREAM_LINE_BYTES),
            "rendered line exceeded byte cap"
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
