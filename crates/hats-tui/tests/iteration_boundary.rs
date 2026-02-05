//! Integration tests for iteration boundary handling.

use hats_proto::Event;
use std::sync::{Arc, Mutex};

/// Helper to create a TuiState and simulate events.
fn simulate_events(events: Vec<Event>) -> Arc<Mutex<hats_tui::state::TuiState>> {
    let state = Arc::new(Mutex::new(hats_tui::state::TuiState::new()));

    for event in events {
        state.lock().unwrap().update(&event);
    }

    state
}

#[test]
fn iteration_changes_on_build_done() {
    let state = simulate_events(vec![
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
    let state = simulate_events(vec![Event::new("task.start", "Start")]);

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
fn header_shows_updated_iteration() {
    let state = simulate_events(vec![Event::new("task.start", "Start")]);

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
    let state = simulate_events(vec![Event::new("task.start", "Start")]);

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
