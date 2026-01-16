//! Outputs header and footer widgets to files for TUI validation.
//!
//! Run with: cargo run -p ralph-tui --example validate_widgets

use ralph_proto::{Event, HatId};
use ralph_tui::TuiState;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use std::fs;
use std::time::Duration;

fn render_to_string(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut lines = Vec::new();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            line.push_str(cell.symbol());
        }
        lines.push(line.trim_end().to_string());
    }
    lines.join("\n")
}

fn main() {
    let output_dir = std::env::current_dir().unwrap().join("tui-validation");
    fs::create_dir_all(&output_dir).unwrap();

    // Create a fully populated state for validation
    let mut state = TuiState::new();
    let event = Event::new("task.start", "");
    state.update(&event);

    state.iteration = 2;
    state.max_iterations = Some(10);
    state.loop_started = Some(
        std::time::Instant::now()
            .checked_sub(Duration::from_secs(272))
            .unwrap(),
    );
    state.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));
    state.loop_mode = ralph_tui::LoopMode::Auto;
    state.last_event = Some("build.task".to_string());
    state.last_event_at = Some(std::time::Instant::now()); // Active

    // Render header
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = ralph_tui::header::render(&state);
            f.render_widget(widget, f.area());
        })
        .unwrap();
    let header_output = render_to_string(&terminal);
    fs::write(output_dir.join("header.txt"), &header_output).unwrap();
    println!("Header output written to tui-validation/header.txt");
    println!("{}", header_output);
    println!();

    // Render header with scroll mode
    state.in_scroll_mode = true;
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = ralph_tui::header::render(&state);
            f.render_widget(widget, f.area());
        })
        .unwrap();
    let header_scroll_output = render_to_string(&terminal);
    fs::write(output_dir.join("header_scroll.txt"), &header_scroll_output).unwrap();
    println!("Header (scroll mode) output written to tui-validation/header_scroll.txt");
    println!("{}", header_scroll_output);
    println!();
    state.in_scroll_mode = false;

    // Render header with paused mode
    state.loop_mode = ralph_tui::LoopMode::Paused;
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = ralph_tui::header::render(&state);
            f.render_widget(widget, f.area());
        })
        .unwrap();
    let header_paused_output = render_to_string(&terminal);
    fs::write(output_dir.join("header_paused.txt"), &header_paused_output).unwrap();
    println!("Header (paused mode) output written to tui-validation/header_paused.txt");
    println!("{}", header_paused_output);
    println!();
    state.loop_mode = ralph_tui::LoopMode::Auto;

    // Render header with idle countdown
    state.idle_timeout_remaining = Some(Duration::from_secs(25));
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = ralph_tui::header::render(&state);
            f.render_widget(widget, f.area());
        })
        .unwrap();
    let header_idle_output = render_to_string(&terminal);
    fs::write(output_dir.join("header_idle.txt"), &header_idle_output).unwrap();
    println!("Header (idle countdown) output written to tui-validation/header_idle.txt");
    println!("{}", header_idle_output);
    println!();
    state.idle_timeout_remaining = None;

    // Render footer (default)
    let scroll_manager = ralph_tui::scroll::ScrollManager::new();
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = ralph_tui::footer::render(&state, &scroll_manager);
            f.render_widget(widget, f.area());
        })
        .unwrap();
    let footer_output = render_to_string(&terminal);
    fs::write(output_dir.join("footer_active.txt"), &footer_output).unwrap();
    println!("Footer (active) output written to tui-validation/footer_active.txt");
    println!("{}", footer_output);
    println!();

    // Render footer (idle state)
    state.last_event_at = Some(
        std::time::Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap(),
    );
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = ralph_tui::footer::render(&state, &scroll_manager);
            f.render_widget(widget, f.area());
        })
        .unwrap();
    let footer_idle_output = render_to_string(&terminal);
    fs::write(output_dir.join("footer_idle.txt"), &footer_idle_output).unwrap();
    println!("Footer (idle) output written to tui-validation/footer_idle.txt");
    println!("{}", footer_idle_output);
    println!();

    // Render footer (done state)
    state.pending_hat = None;
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let widget = ralph_tui::footer::render(&state, &scroll_manager);
            f.render_widget(widget, f.area());
        })
        .unwrap();
    let footer_done_output = render_to_string(&terminal);
    fs::write(output_dir.join("footer_done.txt"), &footer_done_output).unwrap();
    println!("Footer (done) output written to tui-validation/footer_done.txt");
    println!("{}", footer_done_output);
    println!();

    // Render full layout simulation
    state.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));
    state.last_event_at = Some(std::time::Instant::now());
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(f.area());

            f.render_widget(ralph_tui::header::render(&state), chunks[0]);
            // Middle content area (just empty for this test)
            f.render_widget(
                ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::ALL)
                    .title(" Terminal Output "),
                chunks[1],
            );
            f.render_widget(
                ralph_tui::footer::render(&state, &scroll_manager),
                chunks[2],
            );
        })
        .unwrap();
    let full_output = render_to_string(&terminal);
    fs::write(output_dir.join("full_layout.txt"), &full_output).unwrap();
    println!("Full layout output written to tui-validation/full_layout.txt");
    println!("{}", full_output);

    println!("\n=== All validation outputs written to tui-validation/ ===");
}
