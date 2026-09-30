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
use ralph_proto::json_rpc::{
    GuidanceTarget, RpcCommand, RpcEvent, TerminationReason, emit_event_line, parse_command,
};

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
            assert_eq!(
                hat, "planner",
                "second iteration inherits the progress role"
            );
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
            assert_eq!(
                *total_cost_usd, 0.08,
                "cost comes from loop.finish, not summary"
            );
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

/// Contract test: the `RpcEvent` wire format that `ralph run --rpc` writes to
/// stdout is a documented contract consumed by external callers (IDE
/// integrations, subprocess TUIs). Lock the snake_case `type` tag and the
/// required field names for every variant a consumer may receive, so a
/// serializer change cannot silently break the protocol.
#[test]
fn rpc_event_wire_contract_locks_tags_and_field_names() {
    use ralph_proto::json_rpc::RpcState;

    // loop_started — emitted first by the engine to frame the run.
    let e = RpcEvent::LoopStarted {
        prompt: "p".into(),
        max_iterations: Some(3),
        backend: "autoloop".into(),
        workspace_root: None,
        started_at: 42,
    };
    let v: serde_json::Value = serde_json::to_value(&e).unwrap();
    assert_eq!(v["type"], "loop_started");
    assert_eq!(v["prompt"], "p");
    assert_eq!(v["max_iterations"], 3);
    assert_eq!(v["backend"], "autoloop");
    assert_eq!(v["started_at"], 42);
    assert!(
        !v.as_object().unwrap().contains_key("workspace_root"),
        "workspace_root is skipped when None (skip_serializing_if)"
    );

    // iteration_start
    let e = RpcEvent::IterationStart {
        iteration: 1,
        max_iterations: Some(9),
        hat: "h".into(),
        hat_display: "H".into(),
        backend: "b".into(),
        started_at: 1,
    };
    let v: serde_json::Value = serde_json::to_value(&e).unwrap();
    assert_eq!(v["type"], "iteration_start");
    for key in [
        "iteration",
        "max_iterations",
        "hat",
        "hat_display",
        "backend",
        "started_at",
    ] {
        assert!(v.as_object().unwrap().contains_key(key), "missing {key}");
    }

    // iteration_end — fields consumers use for cost/token accounting.
    let e = RpcEvent::IterationEnd {
        iteration: 1,
        duration_ms: 5,
        cost_usd: 0.1,
        input_tokens: 100,
        output_tokens: 50,
        cache_read_tokens: 10,
        cache_write_tokens: 5,
        context_window: 0,
        context_tokens: 115,
        loop_complete_triggered: false,
    };
    let v: serde_json::Value = serde_json::to_value(&e).unwrap();
    assert_eq!(v["type"], "iteration_end");
    assert_eq!(v["loop_complete_triggered"], false);

    // text_delta / error / hat_changed / task_status_changed / task_counts_updated
    let v: serde_json::Value = serde_json::to_value(&RpcEvent::TextDelta {
        iteration: 1,
        delta: "d".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "text_delta");
    assert_eq!(v["delta"], "d");

    let v: serde_json::Value = serde_json::to_value(&RpcEvent::Error {
        iteration: 0,
        code: "TIMEOUT".into(),
        message: "m".into(),
        recoverable: true,
    })
    .unwrap();
    assert_eq!(v["type"], "error");
    assert_eq!(v["code"], "TIMEOUT");

    let v: serde_json::Value = serde_json::to_value(&RpcEvent::HatChanged {
        iteration: 2,
        from_hat: "a".into(),
        to_hat: "b".into(),
        to_hat_display: "B".into(),
        reason: "r".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "hat_changed");

    let v: serde_json::Value = serde_json::to_value(&RpcEvent::TaskStatusChanged {
        task_id: "t1".into(),
        from_status: "open".into(),
        to_status: "closed".into(),
        title: "T".into(),
    })
    .unwrap();
    assert_eq!(v["type"], "task_status_changed");

    let v: serde_json::Value = serde_json::to_value(&RpcEvent::TaskCountsUpdated {
        total: 3,
        open: 1,
        closed: 2,
        ready: 0,
    })
    .unwrap();
    assert_eq!(v["type"], "task_counts_updated");
    assert_eq!(v["total"], 3);

    // guidance_ack — includes the nested GuidanceTarget enum.
    let v: serde_json::Value = serde_json::to_value(&RpcEvent::GuidanceAck {
        message: "g".into(),
        applies_to: GuidanceTarget::Next,
    })
    .unwrap();
    assert_eq!(v["type"], "guidance_ack");
    assert_eq!(v["applies_to"], "next");

    // response — control-command replies; optionality rules matter here
    // (id/data/error omitted when absent).
    let v: serde_json::Value = serde_json::to_value(&RpcEvent::success_response(
        "get_state",
        Some("c-1".into()),
        None,
    ))
    .unwrap();
    assert_eq!(v["type"], "response");
    assert_eq!(v["command"], "get_state");
    assert_eq!(v["id"], "c-1");
    assert_eq!(v["success"], true);
    assert!(!v.as_object().unwrap().contains_key("data"));
    assert!(!v.as_object().unwrap().contains_key("error"));

    let v: serde_json::Value =
        serde_json::to_value(&RpcEvent::error_response("abort", None, "no loop")).unwrap();
    assert_eq!(v["success"], false);
    assert_eq!(v["error"], "no loop");
    assert!(!v.as_object().unwrap().contains_key("id"));

    // orchestration_event — the generic channel the mapper uses for routing
    // and human-ask; source/target are optional.
    let v: serde_json::Value = serde_json::to_value(&RpcEvent::OrchestrationEvent {
        topic: "human.ask".into(),
        payload: "q?".into(),
        source: Some("q1".into()),
        target: None,
    })
    .unwrap();
    assert_eq!(v["type"], "orchestration_event");
    assert_eq!(v["source"], "q1");
    assert!(!v.as_object().unwrap().contains_key("target"));

    // loop_terminated — the terminal event; TerminationReason serializes
    // snake_case.
    let v: serde_json::Value = serde_json::to_value(&RpcEvent::LoopTerminated {
        reason: TerminationReason::Completed,
        total_iterations: 2,
        duration_ms: 900,
        total_cost_usd: 0.08,
        terminated_at: 2_000,
    })
    .unwrap();
    assert_eq!(v["type"], "loop_terminated");
    assert_eq!(v["reason"], "completed");
    assert_eq!(v["total_iterations"], 2);
    assert_eq!(v["total_cost_usd"], 0.08);

    // get_state response data shape (RpcState) — consumers render state from it.
    let v: serde_json::Value = serde_json::to_value(&RpcState {
        iteration: 1,
        max_iterations: Some(3),
        hat: "h".into(),
        hat_display: "H".into(),
        backend: "b".into(),
        completed: false,
        started_at: 1,
        iteration_started_at: Some(2),
        task_counts: Default::default(),
        active_task: None,
        total_cost_usd: 0.0,
    })
    .unwrap();
    assert_eq!(v["iteration"], 1);
    assert_eq!(v["task_counts"]["total"], 0);

    // And every tag round-trips: parse the serialized line back into an
    // RpcEvent (the consumer's exact decode path).
    let round_trip = serde_json::from_str::<RpcEvent>(&emit_event_line(&e).trim()).unwrap();
    assert_eq!(round_trip, e);
}

/// The RPC contract's `TerminationReason` is deliberately small; the Autoloop
/// mapping collapses the richer autoloop `stopReason` vocabulary onto it. Lock
/// that mapping: every stop reason autoloop reports must produce the
/// documented TerminationReason, so a future change to the collapse is a
/// visible contract change.
#[test]
fn autoloop_stop_reasons_collapse_onto_rpc_termination_reasons() {
    use ralph_adapters::AutoloopEvent;

    let cases: &[(&str, TerminationReason)] = &[
        ("completed", TerminationReason::Completed),
        ("verdict_exit", TerminationReason::Completed),
        ("max_iterations", TerminationReason::MaxIterations),
        ("interrupted", TerminationReason::Interrupted),
        // Everything else collapses to Error (no closer RPC variant).
        ("stalled", TerminationReason::Error),
        ("max_runtime", TerminationReason::Error),
        ("cost_budget", TerminationReason::Error),
        ("backend_failed", TerminationReason::Error),
        ("backend_timeout", TerminationReason::Error),
        ("verdict_takeover", TerminationReason::Error),
        ("some_future_reason", TerminationReason::Error),
    ];

    for (stop_reason, expected) in cases {
        let mut m = AutoloopRpcMapper::new(1_000, "autoloop");
        let json = serde_json::json!({
            "type": "loop.finish",
            "runId": "r1",
            "iterations": 1,
            "stopReason": stop_reason,
        });
        let event: AutoloopEvent = serde_json::from_value(json).unwrap();
        let out = m.map(&event);
        assert_eq!(out.len(), 1, "stopReason {stop_reason:?}");
        match &out[0] {
            RpcEvent::LoopTerminated { reason, .. } => {
                assert_eq!(*reason, *expected, "stopReason {stop_reason:?}");
            }
            other => panic!("expected LoopTerminated, got {other:?}"),
        }
        // The authoritative loop.finish suppresses the finalize() fallback.
        assert!(m.finalize().is_none());
    }

    // A terminal event with no stopReason at all also collapses to Error.
    let mut m = AutoloopRpcMapper::new(1_000, "autoloop");
    let json = serde_json::json!({
        "type": "loop.finish",
        "runId": "r1",
        "iterations": 1,
    });
    let event: AutoloopEvent = serde_json::from_value(json).unwrap();
    match m.map(&event).pop() {
        Some(RpcEvent::LoopTerminated { reason, .. }) => {
            assert_eq!(reason, TerminationReason::Error, "missing stopReason");
        }
        other => panic!("expected LoopTerminated for missing stopReason, got {other:?}"),
    }
}

/// Lifecycle ordering: a full run (2 iterations + HITL round-trip + terminal)
/// must produce a well-ordered event stream that an external consumer can
/// rely on: exactly one terminal event, last; text deltas only within their
/// iteration; ask before answered; and each iteration's events contiguous.
#[test]
fn rpc_lifecycle_event_ordering_is_contracted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autoloop-events.ndjson");

    let mut tailer = AutoloopEventTailer::new(&path);
    let mut mapper = AutoloopRpcMapper::new(1_000, "autoloop");
    let mut events: Vec<RpcEvent> = Vec::new();

    // Iteration 1: start, route, ask, answer, output.
    append(
        &path,
        r#"{"type":"iteration.start","iteration":1,"maxIterations":5,"runId":"r1"}"#,
    );
    append(
        &path,
        r#"{"type":"progress","runId":"r1","iteration":1,"emittedTopic":"build.task","outcome":"continue:routed_event","allowedRoles":["coder"]}"#,
    );
    append(
        &path,
        r#"{"type":"ask.pending","runId":"r1","iteration":1,"questionId":"q1","question":"Ship it?"}"#,
    );
    append(
        &path,
        r#"{"type":"ask.answered","runId":"r1","questionId":"q1","answer":"yes"}"#,
    );
    append(
        &path,
        r#"{"type":"backend.output","runId":"r1","iteration":1,"output":"iteration one work"}"#,
    );
    events.extend(drain(&mut tailer, &mut mapper));

    // Iteration 2: start (with the coder role from iteration 1's progress),
    // output, then the run ends via summary + loop.finish.
    append(
        &path,
        r#"{"type":"iteration.start","iteration":2,"maxIterations":5,"runId":"r1"}"#,
    );
    append(
        &path,
        r#"{"type":"backend.output","runId":"r1","iteration":2,"output":"iteration two work"}"#,
    );
    append(
        &path,
        r#"{"type":"summary","runId":"r1","iterations":2,"stopReason":"completed","costUsd":0.19}"#,
    );
    append(
        &path,
        r#"{"type":"loop.finish","runId":"r1","iterations":2,"stopReason":"completed","costUsd":0.21}"#,
    );
    events.extend(drain(&mut tailer, &mut mapper));
    events.extend(drain(&mut tailer, &mut mapper));
    if let Some(t) = mapper.finalize() {
        events.push(t);
    }

    // 1) Exactly one terminal event, and it is last.
    let terminal_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, RpcEvent::LoopTerminated { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(terminal_positions, vec![events.len() - 1]);

    // 2) Exactly two IterationStarts, with 1 before 2.
    let starts: Vec<u32> = events
        .iter()
        .filter_map(|e| match e {
            RpcEvent::IterationStart { iteration, .. } => Some(*iteration),
            _ => None,
        })
        .collect();
    assert_eq!(starts, vec![1, 2]);

    // 3) TextDeltas are attributed to the iteration that was active when the
    //    backend output arrived — 1 then 2, in order, each after its start.
    let deltas: Vec<(u32, &str)> = events
        .iter()
        .filter_map(|e| match e {
            RpcEvent::TextDelta { iteration, delta } => Some((*iteration, delta.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        deltas,
        vec![(1, "iteration one work"), (2, "iteration two work")]
    );
    let last_start1 = events
        .iter()
        .position(|e| matches!(e, RpcEvent::IterationStart { iteration: 1, .. }))
        .unwrap();
    let first_delta1 = events
        .iter()
        .position(|e| matches!(e, RpcEvent::TextDelta { iteration: 1, .. }))
        .unwrap();
    assert!(
        first_delta1 > last_start1,
        "text delta precedes its iteration start"
    );

    // 4) Human-ask round-trip: human.ask before human.answered, both within
    //    iteration 1's window, carried over the OrchestrationEvent channel.
    let ask_pos = events
        .iter()
        .position(
            |e| matches!(e, RpcEvent::OrchestrationEvent { topic, .. } if topic == "human.ask"),
        )
        .unwrap();
    let answered_pos = events
        .iter()
        .position(|e| matches!(e, RpcEvent::OrchestrationEvent { topic, .. } if topic == "human.answered"))
        .unwrap();
    assert!(
        ask_pos < answered_pos,
        "human.ask must precede human.answered"
    );
    let start2 = events
        .iter()
        .position(|e| matches!(e, RpcEvent::IterationStart { iteration: 2, .. }))
        .unwrap();
    assert!(
        answered_pos < start2,
        "round-trip completes before the next iteration"
    );

    // 5) The terminal carries the authoritative cost from loop.finish.
    match events.last().unwrap() {
        RpcEvent::LoopTerminated {
            reason,
            total_iterations,
            total_cost_usd,
            duration_ms: _,
            terminated_at,
        } => {
            assert_eq!(*reason, TerminationReason::Completed);
            assert_eq!(*total_iterations, 2);
            assert!((*total_cost_usd - 0.21).abs() < f64::EPSILON);
            assert!(*terminated_at >= 1_000);
        }
        other => panic!("expected terminal LoopTerminated, got {other:?}"),
    }

    // 6) The second iteration's IterationStart is attributed to the role the
    //    last progress event named (coder), not the default label.
    match events
        .iter()
        .find(|e| matches!(e, RpcEvent::IterationStart { iteration: 2, .. }))
        .unwrap()
    {
        RpcEvent::IterationStart { hat, .. } => assert_eq!(hat, "coder"),
        other => panic!("expected IterationStart, got {other:?}"),
    }
}

/// The command side of the contract: consumers send `RpcCommand` JSON lines on
/// stdin; ralph replies with correlated `Response` events. Lock the command
/// parse/serialize round-trip and the correlation-id discipline, which is what
/// lets a consumer match a response to the command it sent.
#[test]
fn rpc_command_round_trip_and_response_correlation() {
    let commands: Vec<RpcCommand> = vec![
        RpcCommand::Prompt {
            id: Some("c-1".into()),
            prompt: "do X".into(),
            backend: Some("claude".into()),
            max_iterations: Some(3),
        },
        RpcCommand::Guidance {
            id: Some("c-2".into()),
            message: "focus on tests".into(),
        },
        RpcCommand::Steer {
            id: None,
            message: "steer now".into(),
        },
        RpcCommand::FollowUp {
            id: None,
            message: "next iteration".into(),
        },
        RpcCommand::Abort {
            id: Some("c-3".into()),
            reason: Some("user cancelled".into()),
        },
        RpcCommand::GetState {
            id: Some("c-4".into()),
        },
        RpcCommand::GetIterations {
            id: Some("c-5".into()),
            include_content: true,
        },
        RpcCommand::SetHat {
            id: None,
            hat: "reviewer".into(),
        },
    ];

    for cmd in &commands {
        let line = format!("{}\n", serde_json::to_string(cmd).unwrap());
        let parsed = parse_command(&line).expect("command parses back");
        assert_eq!(&parsed, cmd, "round-trip for {cmd:?}");
        // id() echoes the correlation id exactly as sent.
        assert_eq!(parsed.id(), cmd.id());
    }

    // Response correlation: a success response carries the command's id and
    // type; an error response carries the error text instead of data.
    let ok = RpcEvent::success_response("prompt", Some("c-1".into()), None);
    let err = RpcEvent::error_response("get_state", Some("c-4".into()), "not started");
    let v: serde_json::Value = serde_json::to_value(&ok).unwrap();
    assert_eq!(v["type"], "response");
    assert_eq!(v["command"], "prompt");
    assert_eq!(v["id"], "c-1");
    assert_eq!(v["success"], true);
    let v: serde_json::Value = serde_json::to_value(&err).unwrap();
    assert_eq!(v["command"], "get_state");
    assert_eq!(v["id"], "c-4");
    assert_eq!(v["success"], false);
    assert_eq!(v["error"], "not started");
}
