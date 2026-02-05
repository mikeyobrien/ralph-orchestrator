//! Integration snapshot tests for TUI state transitions.
//!
//! Uses insta for snapshot testing to catch regressions in state handling.
//! These tests are deterministic and run in CI.

mod common;

use common::{EventSequenceBuilder, TuiTestHarness, create_test_lines, multi_iteration_events};
use insta::{assert_snapshot, assert_yaml_snapshot, with_settings};

// ============================================================================
// Iteration Progression Tests
// ============================================================================

#[test]
fn test_iteration_progression_state() {
    let mut harness = TuiTestHarness::from_fixture("multi_iteration_session.jsonl");

    // Snapshot state at iteration 1
    harness.advance_to_iteration(1);
    assert_yaml_snapshot!("iter1_state", harness.capture_state());

    // Snapshot state at iteration 2
    harness.advance_to_iteration(2);
    assert_yaml_snapshot!("iter2_state", harness.capture_state());

    // Snapshot state at iteration 3
    harness.advance_to_iteration(3);
    assert_yaml_snapshot!("iter3_state", harness.capture_state());
}

#[test]
fn test_completion_state() {
    let mut harness = TuiTestHarness::from_fixture("completion_session.jsonl");

    // Process all events including loop.terminate
    harness.process_all();

    assert_yaml_snapshot!("completion_state", harness.capture_state());
}

// ============================================================================
// Hat Transition Tests
// ============================================================================

#[test]
fn test_hat_transitions() {
    let mut harness = TuiTestHarness::from_fixture("hat_transitions.jsonl");

    harness.process_until_event("task.start");
    assert_yaml_snapshot!("hat_after_task_start", harness.capture_state());

    harness.process_until_event("build.task");
    assert_yaml_snapshot!("hat_after_build_task", harness.capture_state());

    harness.process_until_event("build.blocked");
    assert_yaml_snapshot!("hat_after_build_blocked", harness.capture_state());

    harness.process_until_event("build.done");
    assert_yaml_snapshot!("hat_after_build_done", harness.capture_state());
}

// ============================================================================
// Navigation State Tests
// ============================================================================

#[test]
fn test_navigation_state_preservation() {
    let events = multi_iteration_events(3);
    let mut harness = TuiTestHarness::with_events(events);

    // Process all events to create 3 iterations
    harness.process_all();

    // Add content to iterations for scroll testing
    harness.add_iteration_content(0, create_test_lines(50, "Iter1"));
    harness.add_iteration_content(1, create_test_lines(50, "Iter2"));
    harness.add_iteration_content(2, create_test_lines(50, "Iter3"));

    // Navigate back to iteration 1
    harness.navigate_prev();
    harness.navigate_prev();
    assert_yaml_snapshot!("nav_back_to_iter1", harness.capture_state());

    // Scroll within iteration 1
    harness.scroll_down(20);
    harness.scroll_down(20);
    let scroll_offset_before = harness.current_scroll_offset();

    // Navigate to iteration 2 and back
    harness.navigate_next();
    harness.navigate_prev();

    // Verify scroll is preserved
    let scroll_offset_after = harness.current_scroll_offset();
    assert_eq!(
        scroll_offset_before, scroll_offset_after,
        "Scroll position should be preserved when navigating between iterations"
    );

    assert_yaml_snapshot!("scroll_preserved", harness.capture_state());
}

#[test]
fn test_following_latest_behavior() {
    let mut harness = TuiTestHarness::new();

    // Create iterations and verify following_latest
    {
        let state = harness.state().lock().unwrap();
        assert!(state.following_latest, "Should follow latest by default");
    }

    // Start iterations
    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
        state.start_new_iteration();
        state.start_new_iteration();
    }

    // Should still be following latest
    let snapshot = harness.capture_state();
    assert!(snapshot.following_latest);
    assert_eq!(snapshot.current_view, 2);

    // Navigate back - should disable following
    harness.navigate_prev();
    let snapshot = harness.capture_state();
    assert!(!snapshot.following_latest);
    assert_eq!(snapshot.current_view, 1);

    // Navigate to latest - should re-enable following
    harness.navigate_next();
    let snapshot = harness.capture_state();
    assert!(snapshot.following_latest);
    assert_eq!(snapshot.current_view, 2);

    assert_yaml_snapshot!("following_latest_restored", snapshot);
}

// ============================================================================
// New Iteration Alert Tests
// ============================================================================

#[test]
fn test_new_iteration_alert() {
    let mut harness = TuiTestHarness::new();

    // Create 2 iterations
    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
        state.start_new_iteration();
    }

    // Navigate back to iteration 1
    harness.navigate_prev();

    // Now add a new iteration while viewing history
    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
    }

    // Should have alert since viewing old iteration
    let snapshot = harness.capture_state();
    assert!(snapshot.has_alert, "Should have new iteration alert");
    assert_yaml_snapshot!("new_iteration_alert", snapshot);

    // Navigate to latest - alert should clear
    harness.navigate_next();
    harness.navigate_next();
    let snapshot = harness.capture_state();
    assert!(!snapshot.has_alert, "Alert should clear when at latest");
    assert_yaml_snapshot!("alert_cleared", snapshot);
}

// ============================================================================
// Search State Tests
// ============================================================================

#[test]
fn test_search_state() {
    let mut harness = TuiTestHarness::new();

    // Create an iteration with searchable content
    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
    }

    // Add content with multiple occurrences of "error"
    harness.add_iteration_content(
        0,
        vec![
            ratatui::text::Line::raw("Line 1: No issues here"),
            ratatui::text::Line::raw("Line 2: Found an error in the code"),
            ratatui::text::Line::raw("Line 3: Another error message"),
            ratatui::text::Line::raw("Line 4: All good"),
            ratatui::text::Line::raw("Line 5: Error at the start"),
        ],
    );

    // Initiate search
    harness.search("error");
    assert_yaml_snapshot!("search_active", harness.capture_state());

    // Navigate to next match
    harness.search_next();
    assert_yaml_snapshot!("search_next", harness.capture_state());

    // Navigate to previous match (should wrap to last)
    harness.search_prev();
    assert_yaml_snapshot!("search_prev", harness.capture_state());

    // Clear search
    harness.clear_search();
    assert_yaml_snapshot!("search_cleared", harness.capture_state());
}

#[test]
fn test_search_case_insensitive() {
    let mut harness = TuiTestHarness::new();

    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
    }

    harness.add_iteration_content(
        0,
        vec![
            ratatui::text::Line::raw("ERROR in uppercase"),
            ratatui::text::Line::raw("error in lowercase"),
            ratatui::text::Line::raw("Error in mixed case"),
        ],
    );

    // Search should find all variants
    harness.search("error");
    let snapshot = harness.capture_state();
    assert_eq!(
        snapshot.search_matches, 3,
        "Should find all case variants of 'error'"
    );
    assert_yaml_snapshot!("search_case_insensitive", snapshot);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_state() {
    let harness = TuiTestHarness::new();
    assert_yaml_snapshot!("empty_state", harness.capture_state());
}

#[test]
fn test_single_iteration() {
    let events = EventSequenceBuilder::new()
        .task_start()
        .build_task("Only task")
        .build_done()
        .loop_terminate()
        .build();

    let mut harness = TuiTestHarness::with_events(events);
    harness.process_all();

    assert_yaml_snapshot!("single_iteration", harness.capture_state());
}

#[test]
fn test_navigation_bounds() {
    let mut harness = TuiTestHarness::new();

    // Create single iteration
    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
    }

    // Try to navigate past bounds
    harness.navigate_prev(); // Should do nothing at index 0
    let snapshot = harness.capture_state();
    assert_eq!(snapshot.current_view, 0, "Should stay at 0");

    harness.navigate_next(); // Should do nothing at last index
    let snapshot = harness.capture_state();
    assert_eq!(snapshot.current_view, 0, "Should stay at last index");

    assert_yaml_snapshot!("navigation_bounds", snapshot);
}

#[test]
fn test_scroll_bounds() {
    let mut harness = TuiTestHarness::new();

    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
    }

    // Add limited content (5 lines, viewport of 10)
    harness.add_iteration_content(0, create_test_lines(5, "Line"));

    // Try to scroll down past content
    harness.scroll_down(10);
    let offset = harness.current_scroll_offset();
    assert_eq!(offset, 0, "Should not scroll when content fits viewport");

    // Add more content
    harness.add_iteration_content(0, create_test_lines(20, "More"));

    // Now scroll should work
    harness.scroll_down(10);
    let offset = harness.current_scroll_offset();
    assert!(offset > 0, "Should scroll when content exceeds viewport");

    // Try to scroll up past top
    harness.scroll_up();
    harness.scroll_up();
    harness.scroll_up();
    harness.scroll_up();
    let offset = harness.current_scroll_offset();
    assert_eq!(offset, 0, "Should stop at top");
}

// ============================================================================
// Rendered Output Tests
// ============================================================================

#[test]
fn test_header_rendering() {
    let mut harness =
        TuiTestHarness::from_fixture("multi_iteration_session.jsonl").with_terminal_size(80, 24);
    harness.advance_to_iteration(2);

    // Snapshot with time redaction (MM:SS pattern)
    with_settings!({
        filters => vec![
            (r"\d{2}:\d{2}", "[TIME]"),
        ]
    }, {
        assert_snapshot!("header_iter2", harness.render_header());
    });
}

#[test]
fn test_footer_rendering() {
    let mut harness =
        TuiTestHarness::from_fixture("multi_iteration_session.jsonl").with_terminal_size(80, 24);
    harness.advance_to_iteration(2);

    assert_snapshot!("footer_iter2", harness.render_footer());
}

#[test]
fn test_footer_with_search() {
    let mut harness = TuiTestHarness::new().with_terminal_size(80, 24);

    // Create an iteration with searchable content
    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
    }

    harness.add_iteration_content(
        0,
        vec![
            ratatui::text::Line::raw("Line with error"),
            ratatui::text::Line::raw("Another error here"),
        ],
    );

    // Activate search
    harness.search("error");

    assert_snapshot!("footer_with_search", harness.render_footer());
}

#[test]
fn test_footer_with_alert() {
    let mut harness = TuiTestHarness::new().with_terminal_size(80, 24);

    // Create 2 iterations
    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
        state.start_new_iteration();
    }

    // Navigate back to iteration 1
    harness.navigate_prev();

    // Start new iteration while viewing history
    {
        let mut state = harness.state().lock().unwrap();
        state.start_new_iteration();
    }

    // Footer should show alert
    assert_snapshot!("footer_with_alert", harness.render_footer());
}

// ============================================================================
// Width-Responsive Layout Tests
// ============================================================================

#[test]
fn test_full_layout_80_columns() {
    let mut harness =
        TuiTestHarness::from_fixture("multi_iteration_session.jsonl").with_terminal_size(80, 10);
    harness.advance_to_iteration(1);

    // Add content to have something to render
    harness.add_iteration_content(0, create_test_lines(5, "Content"));

    with_settings!({
        filters => vec![
            (r"\d{2}:\d{2}", "[TIME]"),
        ]
    }, {
        assert_snapshot!("full_layout_80col", harness.render_full());
    });
}

#[test]
fn test_full_layout_60_columns() {
    let mut harness =
        TuiTestHarness::from_fixture("multi_iteration_session.jsonl").with_terminal_size(60, 10);
    harness.advance_to_iteration(1);

    harness.add_iteration_content(0, create_test_lines(5, "Content"));

    with_settings!({
        filters => vec![
            (r"\d{2}:\d{2}", "[TIME]"),
        ]
    }, {
        assert_snapshot!("full_layout_60col", harness.render_full());
    });
}

#[test]
fn test_full_layout_40_columns() {
    let mut harness =
        TuiTestHarness::from_fixture("multi_iteration_session.jsonl").with_terminal_size(40, 10);
    harness.advance_to_iteration(1);

    harness.add_iteration_content(0, create_test_lines(5, "Content"));

    with_settings!({
        filters => vec![
            (r"\d{2}:\d{2}", "[TIME]"),
        ]
    }, {
        assert_snapshot!("full_layout_40col", harness.render_full());
    });
}

#[test]
fn test_header_width_breakpoints() {
    // Test header at different widths to verify progressive disclosure

    // Full width (80+) - should show help hint
    let harness_80 = TuiTestHarness::new().with_terminal_size(80, 1);
    {
        let mut state = harness_80.state().lock().unwrap();
        state.start_new_iteration();
    }
    let header_80 = harness_80.render_header();
    assert!(
        header_80.contains("help"),
        "80col header should show help hint"
    );

    // Narrow (50) - should compress mode to symbols
    let harness_50 = TuiTestHarness::new().with_terminal_size(50, 1);
    {
        let mut state = harness_50.state().lock().unwrap();
        state.start_new_iteration();
    }
    let header_50 = harness_50.render_header();
    assert!(
        !header_50.contains("[LIVE]"),
        "50col header should compress mode"
    );
    assert!(
        header_50.contains("▶") || header_50.contains("◀"),
        "50col header should use symbol mode"
    );
}

// ============================================================================
// Long Output / Scroll Tests
// ============================================================================

#[test]
fn test_long_output_scroll_rendering() {
    let mut harness =
        TuiTestHarness::from_fixture("long_output_scroll.jsonl").with_terminal_size(80, 10);
    harness.process_all();

    // Add 50 lines of content
    harness.add_iteration_content(0, create_test_lines(50, "HTTP Status"));

    // Snapshot at top
    with_settings!({
        filters => vec![
            (r"\d{2}:\d{2}", "[TIME]"),
        ]
    }, {
        assert_snapshot!("long_output_top", harness.render_full());
    });

    // Scroll down
    harness.scroll_down(8); // viewport is 8 lines (10 - 2 for header/footer)

    with_settings!({
        filters => vec![
            (r"\d{2}:\d{2}", "[TIME]"),
        ]
    }, {
        assert_snapshot!("long_output_scrolled", harness.render_full());
    });
}
