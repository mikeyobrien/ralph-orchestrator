//! Writes real TUI frames to `tui-validation/` for visual inspection.
//!
//! The state is built from the recorded autoloop run in
//! `tests/fixtures/autoloop_code_assist/` through the production event mapping,
//! and every frame is drawn by the same `render_frame` the live app uses.
//! (The per-iteration tool calls come from the backend streams, which the
//! `autoloop_frames` test replays; this example maps the `--events` stream.)
//!
//! Run with: cargo run -p ralph-tui --example validate_widgets

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use ralph_adapters::parse_events;
use ralph_tui::autoloop_source::{AutoloopMapCtx, apply_autoloop_event};
use ralph_tui::{TuiState, render_frame};
use ratatui::{Terminal, backend::TestBackend};

fn frame(state: &TuiState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| render_frame(f, state)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/autoloop_code_assist/events.ndjson");
    let events = parse_events(&fs::read_to_string(&fixture).unwrap());
    let role_names: HashMap<String, String> = [
        ("planner", "📋 Planner"),
        ("builder", "⚙️ Builder"),
        ("critic", "🧪 Fresh-Eyes Critic"),
        ("finalizer", "🏁 Finalizer"),
    ]
    .into_iter()
    .map(|(id, name)| (id.to_string(), name.to_string()))
    .collect();

    let state = Arc::new(Mutex::new(TuiState::new()));
    // As `ralph run` sets it for the engine's event stream.
    state.lock().unwrap().autoloop_source = true;
    let mut ctx = AutoloopMapCtx::new(role_names);
    let output_dir = std::env::current_dir().unwrap().join("tui-validation");
    fs::create_dir_all(&output_dir).unwrap();
    let write = |name: &str, text: String| {
        fs::write(output_dir.join(name), &text).unwrap();
        println!("== tui-validation/{name}\n{text}\n");
    };

    // Mid-run: the builder's iteration, live.
    let builder_done = events
        .iter()
        .position(|event| event.kind == "progress" && event.iteration == Some(2))
        .expect("fixture has the builder's progress");
    for event in &events[..=builder_done] {
        apply_autoloop_event(event, &state, &mut ctx);
    }
    {
        let state = state.lock().unwrap();
        write("live_builder_100x24.txt", frame(&state, 100, 24));
        write("live_builder_40x12.txt", frame(&state, 40, 12));
    }

    // Finished: reviewing the first iteration, and the final screen.
    for event in &events[builder_done + 1..] {
        apply_autoloop_event(event, &state, &mut ctx);
    }
    let mut state = state.lock().unwrap();
    let last = state.total_iterations() - 1;
    state.current_view = 0;
    state.following_latest = false;
    write("review_iteration_1_100x24.txt", frame(&state, 100, 24));
    state.current_view = last;
    state.following_latest = true;
    write("finished_80x12.txt", frame(&state, 80, 12));
    state.show_help = true;
    write("help_80x24.txt", frame(&state, 80, 24));
}
