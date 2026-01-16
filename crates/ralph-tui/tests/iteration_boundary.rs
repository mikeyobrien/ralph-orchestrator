//! Integration tests for iteration boundary handling.

use ralph_proto::Event;
use ralph_tui::TerminalWidget;
use std::sync::{Arc, Mutex};

/// Helper to create a TuiState and simulate events.
fn simulate_events(
    events: Vec<Event>,
) -> (
    Arc<Mutex<ralph_tui::state::TuiState>>,
    Arc<Mutex<TerminalWidget>>,
) {
    let state = Arc::new(Mutex::new(ralph_tui::state::TuiState::new()));
    let widget = Arc::new(Mutex::new(TerminalWidget::new()));

    for event in events {
        state.lock().unwrap().update(&event);
    }

    (state, widget)
}

#[test]
fn iteration_changes_on_build_done() {
    let (state, _widget) = simulate_events(vec![
        Event::new("task.start", "Start"),
        Event::new("build.task", "Task 1"),
    ]);

    let initial_iteration = state.lock().unwrap().iteration;

    // Simulate build.done event
    state
        .lock()
        .unwrap()
        .update(&Event::new("build.done", "Done"));

    let new_iteration = state.lock().unwrap().iteration;
    assert_eq!(new_iteration, initial_iteration + 1);
}

#[test]
fn iteration_changed_detects_transition() {
    let (state, _widget) = simulate_events(vec![Event::new("task.start", "Start")]);

    // Initially no change
    assert!(!state.lock().unwrap().iteration_changed());

    // After build.done, change detected
    state
        .lock()
        .unwrap()
        .update(&Event::new("build.done", "Done"));
    assert!(state.lock().unwrap().iteration_changed());
}

#[test]
fn terminal_widget_clear_resets_parser() {
    let mut widget = TerminalWidget::new();

    // Add some content
    widget.process(b"Hello, world!\n");
    widget.process(b"Line 2\n");
    widget.process(b"Line 3\n");

    // Clear should reset
    widget.clear();

    // After clear, total lines should be minimal (just screen size)
    let total = widget.total_lines();
    assert!(
        total <= 24,
        "Expected minimal lines after clear, got {}",
        total
    );
}

#[test]
fn header_shows_updated_iteration() {
    let (state, _widget) = simulate_events(vec![Event::new("task.start", "Start")]);

    let initial = state.lock().unwrap().iteration;
    assert_eq!(initial, 0);

    state
        .lock()
        .unwrap()
        .update(&Event::new("build.done", "Done"));
    let after_first = state.lock().unwrap().iteration;
    assert_eq!(after_first, 1);

    state
        .lock()
        .unwrap()
        .update(&Event::new("build.done", "Done"));
    let after_second = state.lock().unwrap().iteration;
    assert_eq!(after_second, 2);
}

#[test]
fn multiple_iterations_tracked_correctly() {
    let (state, _widget) = simulate_events(vec![Event::new("task.start", "Start")]);

    for i in 0..5 {
        let before = state.lock().unwrap().iteration;
        assert_eq!(before, i);

        state
            .lock()
            .unwrap()
            .update(&Event::new("build.done", "Done"));

        let after = state.lock().unwrap().iteration;
        assert_eq!(after, i + 1);
    }
}
