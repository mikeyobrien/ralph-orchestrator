//! End-to-end translation test for `ralph run --rpc` on the autoloop engine (#343).
//!
//! Exercises the exact seam the engine drives in `run_autoloop_with_rpc`: the
//! live [`AutoloopEventTailer`] feeding [`AutoloopRpcMapper`], over a `--events`
//! NDJSON file written incrementally (simulating autoloop appending across the
//! reader's poll ticks). Asserts the emitted [`RpcEvent`] sequence and its
//! JSON-lines wire shape, so the autoloop→RPC contract is locked without needing
//! a live autoloop subprocess or backend.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use ralph_adapters::{AutoloopEventTailer, AutoloopRpcMapper};
use ralph_proto::json_rpc::{RpcEvent, TerminationReason, emit_event_line};

fn append(path: &Path, line: &str) {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    f.write_all(line.as_bytes()).unwrap();
    f.write_all(b"\n").unwrap();
}

/// Poll the tailer once and translate every new event, mirroring one reader tick.
fn drain(tailer: &mut AutoloopEventTailer, mapper: &mut AutoloopRpcMapper) -> Vec<RpcEvent> {
    let mut out = Vec::new();
    for event in tailer.poll().expect("poll succeeds") {
        out.extend(mapper.map(&event));
    }
    out
}

#[test]
fn autoloop_events_stream_maps_to_rpc_event_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autoloop-events.ndjson");

    let mut tailer = AutoloopEventTailer::new(&path);
    let mut mapper = AutoloopRpcMapper::new(1_000, "autoloop");
    let mut events: Vec<RpcEvent> = Vec::new();

    // Nothing written yet — a poll before the subprocess appends is empty.
    assert!(drain(&mut tailer, &mut mapper).is_empty());

    // Tick 1: iteration 1 starts, routes to the planner, and produces output.
    append(
        &path,
        r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#,
    );
    append(
        &path,
        r#"{"type":"progress","runId":"r1","iteration":1,"emittedTopic":"tasks.ready","outcome":"continue:routed_event","allowedRoles":["planner"]}"#,
    );
    append(
        &path,
        r#"{"type":"backend.output","runId":"r1","iteration":1,"output":"planned the work"}"#,
    );
    events.extend(drain(&mut tailer, &mut mapper));

    // Tick 2: iteration 2 starts, then the run finishes (summary then loop.finish).
    append(
        &path,
        r#"{"type":"iteration.start","iteration":2,"maxIterations":3,"runId":"r1"}"#,
    );
    append(
        &path,
        r#"{"type":"summary","runId":"r1","iterations":2,"stopReason":"max_iterations"}"#,
    );
    append(
        &path,
        r#"{"type":"loop.finish","iterations":2,"stopReason":"max_iterations","runId":"r1","costUsd":0.08}"#,
    );
    events.extend(drain(&mut tailer, &mut mapper));

    // Final drain + finalize (loop.finish already emitted, so finalize is a no-op).
    events.extend(drain(&mut tailer, &mut mapper));
    if let Some(terminal) = mapper.finalize() {
        events.push(terminal);
    }

    // Expected translated sequence: two IterationStarts (the second labelled with
    // the planner role from the intervening progress), a routing OrchestrationEvent,
    // one TextDelta for the backend output, and exactly one terminal LoopTerminated.
    assert_eq!(events.len(), 5, "sequence: {events:#?}");

    match &events[0] {
        RpcEvent::IterationStart {
            iteration,
            max_iterations,
            hat,
            ..
        } => {
            assert_eq!(*iteration, 1);
            assert_eq!(*max_iterations, Some(3));
            assert_eq!(hat, "autoloop"); // no role known before the first progress
        }
        other => panic!("events[0] expected IterationStart, got {other:?}"),
    }
    match &events[1] {
        RpcEvent::OrchestrationEvent {
            topic,
            payload,
            source,
            ..
        } => {
            assert_eq!(topic, "tasks.ready");
            assert_eq!(payload, "continue:routed_event");
            assert_eq!(source.as_deref(), Some("planner"));
        }
        other => panic!("events[1] expected OrchestrationEvent, got {other:?}"),
    }
    match &events[2] {
        RpcEvent::TextDelta { iteration, delta } => {
            assert_eq!(*iteration, 1);
            assert_eq!(delta, "planned the work");
        }
        other => panic!("events[2] expected TextDelta, got {other:?}"),
    }
    match &events[3] {
        RpcEvent::IterationStart { iteration, hat, .. } => {
            assert_eq!(*iteration, 2);
            assert_eq!(hat, "planner", "second iteration inherits the progress role");
        }
        other => panic!("events[3] expected IterationStart, got {other:?}"),
    }
    match &events[4] {
        RpcEvent::LoopTerminated {
            reason,
            total_iterations,
            total_cost_usd,
            ..
        } => {
            assert_eq!(*reason, TerminationReason::MaxIterations);
            assert_eq!(*total_iterations, 2);
            assert_eq!(*total_cost_usd, 0.08, "cost comes from loop.finish, not summary");
        }
        other => panic!("events[4] expected LoopTerminated, got {other:?}"),
    }

    // Wire shape: each event serializes to a single JSON line tagged by `type`,
    // exactly what `ralph run --rpc` writes to stdout.
    let wire: Vec<String> = events.iter().map(emit_event_line).collect();
    assert!(wire[0].contains(r#""type":"iteration_start""#));
    assert!(wire[1].contains(r#""type":"orchestration_event""#));
    assert!(wire[2].contains(r#""type":"text_delta""#));
    assert!(wire[4].contains(r#""type":"loop_terminated""#));
    for line in &wire {
        assert!(line.ends_with('\n'), "each RPC event is newline-terminated");
        // A well-formed, single-line JSON object per the newline-delimited protocol.
        assert_eq!(line.matches('\n').count(), 1);
        serde_json::from_str::<serde_json::Value>(line.trim()).expect("valid JSON line");
    }
}
