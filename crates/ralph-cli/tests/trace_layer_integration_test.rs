use ralph_core::diagnostics::DiagnosticTraceLayer;
use std::fs;
use tempfile::TempDir;
use tracing_subscriber::prelude::*;

#[test]
fn test_trace_layer_can_be_added_to_subscriber() {
    // Setup: Create temp directory for diagnostics
    let temp_dir = TempDir::new().unwrap();
    let session_dir = temp_dir.path().join("test-session");
    fs::create_dir_all(&session_dir).unwrap();

    // Create the trace layer
    let layer = DiagnosticTraceLayer::new(&session_dir).expect("Should create trace layer");

    // RED: Try to add it to a subscriber
    // This tests that the layer can be composed with other layers
    let _subscriber = tracing_subscriber::registry().with(layer);

    // Verify trace.jsonl was created
    let trace_file = session_dir.join("trace.jsonl");
    assert!(
        trace_file.exists(),
        "trace.jsonl should be created when layer is initialized"
    );
}

#[test]
fn test_trace_layer_captures_log_events() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let session_dir = temp_dir.path().join("test-session");
    fs::create_dir_all(&session_dir).unwrap();

    // Create and install the layer
    let layer = DiagnosticTraceLayer::new(&session_dir).expect("Should create trace layer");

    let subscriber = tracing_subscriber::registry().with(layer);

    // RED: Set as global subscriber and emit a log
    tracing::subscriber::set_global_default(subscriber).expect("Should set subscriber");

    tracing::info!("test message from integration");

    // Verify it was captured in trace.jsonl
    let trace_file = session_dir.join("trace.jsonl");
    let content = fs::read_to_string(&trace_file).expect("Should read trace.jsonl");

    assert!(
        content.contains("test message from integration"),
        "Trace layer should capture log messages"
    );
}
