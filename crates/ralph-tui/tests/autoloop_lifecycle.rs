//! Real mock-backed lifecycle coverage for the autoloop TUI event source.
//!
//! The test is opt-in through a sibling checkout or `AUTOLOOP_ROOT`, matching
//! the adapter contract tests. It proves that a real autoloop process drives
//! startup, live event rendering, a blocking ask, and terminal completion.

#![cfg(unix)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ralph_adapters::{AutoloopBin, AutoloopRunner};
use ralph_tui::{TuiState, run_autoloop_event_reader, widgets::footer};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use tokio::sync::watch;

fn find_autoloop_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("AUTOLOOP_ROOT") {
        let path = PathBuf::from(root);
        if path.join("bin/autoloop").is_file() {
            return Some(path);
        }
    }
    for ancestor in Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
        let candidate = ancestor.join("autoloop");
        if candidate.join("bin/autoloop").is_file() {
            return Some(candidate);
        }
    }
    None
}

fn supports_contract(bin: &Path, command: &str, needle: &str) -> bool {
    Command::new("node")
        .arg(bin)
        .args([command, "--help"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout).contains(needle)
                || String::from_utf8_lossy(&output.stderr).contains(needle)
        })
        .unwrap_or(false)
}

fn setup() -> Option<(tempfile::TempDir, AutoloopRunner, PathBuf)> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let root = find_autoloop_root()?;
    let bin = root.join("bin/autoloop");
    let preset = root.join("packages/presets/presets/autocode");
    let mock = root.join("dist/testing/mock-backend.js");
    let fixture = root.join("test/fixtures/backend/human-ask.json");
    if !supports_contract(&bin, "run", "--events") || !supports_contract(&bin, "control", "respond")
    {
        eprintln!("skip: autoloop checkout lacks --events/control respond contracts");
        return None;
    }
    for path in [&preset, &mock, &fixture] {
        if !path.exists() {
            eprintln!("skip: prerequisite missing: {}", path.display());
            return None;
        }
    }

    let work = tempfile::tempdir().expect("temp workspace");
    for args in [
        ["init", "-q"].as_slice(),
        ["config", "user.email", "t@example.com"].as_slice(),
        ["config", "user.name", "t"].as_slice(),
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(work.path())
                .status()
                .unwrap()
                .success()
        );
    }
    fs::write(work.path().join("index.html"), "<p>hi</p>\n").unwrap();
    assert!(
        Command::new("git")
            .args(["add", "."])
            .current_dir(work.path())
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(work.path())
            .status()
            .unwrap()
            .success()
    );

    let wrapper = work.path().join("mock-wrapper.sh");
    fs::write(
        &wrapper,
        format!("#!/usr/bin/env bash\nexec node {} \"$@\"\n", mock.display()),
    )
    .unwrap();
    let mut permissions = fs::metadata(&wrapper).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&wrapper, permissions).unwrap();

    let events_path = work.path().join("events.ndjson");
    let runner = AutoloopRunner::new(preset, "tui lifecycle", work.path())
        .bin(AutoloopBin::Node(bin))
        .backend(wrapper.to_string_lossy().into_owned())
        .env("MOCK_FIXTURE_PATH", fixture.to_string_lossy().into_owned())
        .events_path(&events_path)
        .set_override("event_loop.ask_timeout", "8000")
        .set_override("event_loop.ask_poll_ms", "25")
        .max_iterations(1);
    Some((work, runner, events_path))
}

fn footer_text(state: &TuiState) -> String {
    let area = Rect::new(0, 0, 100, 3);
    let mut buffer = Buffer::empty(area);
    footer::render(state).render(area, &mut buffer);
    buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
}

fn deliver_response(work: &Path, run_id: &str, question_id: &str) {
    use std::fs;

    let control_dir = work.join(".autoloop/runs").join(run_id).join("control");
    fs::create_dir_all(&control_dir).unwrap();
    let request = serde_json::json!({
        "id": "ctl_tui_lifecycle",
        "runId": run_id,
        "requestedAt": "2026-07-16T00:00:00.000Z",
        "verb": "respond",
        "reason": "respond",
        "payload": { "questionId": question_id, "answer": "continue" },
    });
    fs::write(
        control_dir.join("requests.jsonl"),
        format!("{}\n", serde_json::to_string(&request).unwrap()),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_autoloop_events_drive_tui_from_startup_through_completion() {
    let Some((work, runner, events_path)) = setup() else {
        return;
    };

    let state = Arc::new(Mutex::new(TuiState::new()));
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let reader_workspace = work.path().to_path_buf();
    let reader_engine_root = ralph_core::engine_state::engine_state_root(&reader_workspace);
    let reader_events = events_path.clone();
    let reader_state = Arc::clone(&state);
    let reader = tokio::spawn(async move {
        run_autoloop_event_reader(
            reader_events,
            reader_workspace,
            reader_engine_root,
            reader_state,
            cancel_rx,
            HashMap::new(),
        )
        .await;
    });
    let run = tokio::task::spawn_blocking(move || runner.run());

    let pending = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let Ok(content) = std::fs::read_to_string(&events_path) {
                let events = ralph_adapters::parse_events(&content);
                if let Some(ask) = ralph_adapters::first_pending_ask(&events) {
                    let tui_has_ask = state
                        .lock()
                        .unwrap()
                        .pending_ask
                        .as_deref()
                        .is_some_and(|question| !question.is_empty());
                    if tui_has_ask {
                        break ask;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("real autoloop should reach a TUI-visible pending ask");

    {
        let tui = state.lock().unwrap();
        assert!(
            !tui.iterations.is_empty(),
            "startup should create an iteration buffer"
        );
        // A later progress event may already have advanced `last_event`, but the
        // blocking ask must remain visible until it is answered.
        let rendered = footer_text(&tui);
        assert!(
            rendered.contains("HUMAN ASK"),
            "pending ask should render in footer: {rendered:?}"
        );
    }

    deliver_response(work.path(), &pending.run_id, &pending.question_id);
    let summary = tokio::time::timeout(Duration::from_secs(8), run)
        .await
        .expect("autoloop should finish after response")
        .expect("runner task should join")
        .expect("autoloop run should succeed");
    cancel_tx.send(true).unwrap();
    reader.await.unwrap();

    let tui = state.lock().unwrap();
    assert!(
        tui.loop_completed,
        "terminal event should complete the TUI lifecycle"
    );
    assert!(
        tui.pending_ask.is_none(),
        "completion should clear the pending ask"
    );
    assert_eq!(tui.iteration, summary.iterations);
    assert!(matches!(
        tui.last_event.as_deref(),
        Some("loop.finish" | "summary")
    ));
    let rendered = footer_text(&tui);
    assert!(
        rendered.contains("DONE"),
        "completed lifecycle should render DONE: {rendered:?}"
    );
}
