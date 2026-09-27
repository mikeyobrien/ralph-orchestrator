//! Renders real TUI frames from a recorded autoloop run.
//!
//! The fixture in `fixtures/autoloop_code_assist/` is a sanitized capture of a
//! live `ralph run -H builtin:code-assist` on autoloop 0.11.0 with the claude
//! backend: the `--events` stream, the per-iteration claude streams, and the
//! journal's `backend.start` records. The test stages them the way the engine
//! writes them, drives the production reader, and snapshots frames drawn by the
//! same `render_frame` the live app uses.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ralph_adapters::AutoloopJournalTailer;
use ralph_core::engine_state::{engine_journal_path, engine_state_root};
use ralph_tui::{TuiState, render_frame, run_autoloop_event_reader_with_journal};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::Value;
use tokio::sync::watch;

const RUN_ID: &str = "allied-shard";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/autoloop_code_assist")
        .join(name)
}

fn append(path: &Path, text: &str) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

fn all_lines(state: &Arc<Mutex<TuiState>>) -> Vec<Vec<String>> {
    let state = state.lock().unwrap();
    state
        .iterations
        .iter()
        .map(|buffer| {
            buffer
                .lines_handle()
                .lock()
                .unwrap()
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .collect()
}

async fn wait_until(
    state: &Arc<Mutex<TuiState>>,
    what: &str,
    done: impl Fn(&[Vec<String>]) -> bool,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if done(&all_lines(state)) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {what}: {:#?}", all_lines(state));
}

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

/// The last line of assistant text in an iteration's stream: once it renders,
/// the whole stream has been read.
fn last_stream_text(iteration: u32) -> String {
    let text =
        std::fs::read_to_string(fixture(&format!("claude-stream.{iteration}.jsonl"))).unwrap();
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|record| record["type"] == "assistant")
        .flat_map(|record| {
            record["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|block| block["text"].as_str().map(str::to_string))
        .flat_map(|text| {
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .last()
        .expect("each iteration's stream has assistant text")
}

struct Replay {
    _dir: tempfile::TempDir,
    events_path: PathBuf,
    state: Arc<Mutex<TuiState>>,
    cancel: watch::Sender<bool>,
    reader: tokio::task::JoinHandle<()>,
}

async fn replay_code_assist_run() -> Replay {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("work");
    let engine_root = engine_state_root(&workspace);
    let run_dir = ralph_core::engine_run_dir(&engine_root, RUN_ID).unwrap();
    std::fs::create_dir_all(&run_dir).unwrap();
    let events_path = engine_root.join("events.ndjson");
    let journal_path = engine_journal_path(&engine_root);

    let state = Arc::new(Mutex::new(TuiState::new()));
    let role_names: HashMap<String, String> = [
        ("planner", "📋 Planner"),
        ("builder", "⚙️ Builder"),
        ("critic", "🧪 Fresh-Eyes Critic"),
        ("finalizer", "🏁 Finalizer"),
    ]
    .into_iter()
    .map(|(id, name)| (id.to_string(), name.to_string()))
    .collect();
    let journal = AutoloopJournalTailer::from_end(journal_path.clone());
    let (cancel, cancel_rx) = watch::channel(false);
    let reader = tokio::spawn({
        let state = Arc::clone(&state);
        let events_path = events_path.clone();
        async move {
            run_autoloop_event_reader_with_journal(
                events_path,
                workspace,
                engine_root,
                journal,
                state,
                cancel_rx,
                role_names,
            )
            .await;
        }
    });

    let journal_records: Vec<String> = std::fs::read_to_string(fixture("journal.jsonl"))
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    let events = std::fs::read_to_string(fixture("events.ndjson")).unwrap();
    for line in events.lines() {
        let event: Value = serde_json::from_str(line).unwrap();
        let kind = event["type"].as_str().unwrap_or_default();
        let iteration = event["iteration"].as_u64().unwrap_or(0) as u32;
        if kind == "iteration.start" {
            // The engine journals backend.start as the iteration begins.
            if let Some(record) = journal_records
                .iter()
                .find(|record| record.contains(&format!("\"iteration\":\"{iteration}\"")))
            {
                append(&journal_path, &format!("{record}\n"));
            }
        }
        if kind == "iteration.footer" {
            // The agent streams its work before the engine closes the iteration.
            let current = state.lock().unwrap().iterations.len();
            let stream =
                std::fs::read_to_string(fixture(&format!("claude-stream.{iteration}.jsonl")))
                    .unwrap();
            append(
                &run_dir.join(format!("claude-stream.{iteration}.jsonl")),
                &stream,
            );
            let needle = last_stream_text(iteration);
            wait_until(
                &state,
                &format!("iteration {iteration} stream"),
                |iterations| {
                    iterations.len() == current
                        && iterations
                            .last()
                            .is_some_and(|lines| lines.iter().any(|line| line.contains(&needle)))
                },
            )
            .await;
        }
        append(&events_path, &format!("{line}\n"));
        if kind == "iteration.start" {
            wait_until(&state, &format!("iteration {iteration}"), |iterations| {
                iterations.len() == iteration as usize
            })
            .await;
        }
    }
    wait_until(&state, "the run to finish", |iterations| {
        iterations
            .last()
            .is_some_and(|lines| lines.iter().any(|line| line.contains("run finished")))
    })
    .await;

    Replay {
        _dir: dir,
        events_path,
        state,
        cancel,
        reader,
    }
}

impl Replay {
    async fn stop(self) {
        let _ = self.cancel.send(true);
        self.reader.await.unwrap();
    }
}

#[tokio::test]
async fn recorded_code_assist_run_renders_history_tools_roles_and_harness() {
    let replay = replay_code_assist_run().await;

    // Every iteration keeps its tool calls after reconciliation, and each
    // iteration's header names its role and the harness that ran it.
    {
        let mut state = replay.state.lock().unwrap();
        let total = state.total_iterations();
        // Four agent iterations, then the engine's closing review iteration,
        // which runs no agent.
        assert_eq!(total, 5);
        let mut headers = Vec::new();
        for view in 0..total {
            state.current_view = view;
            state.following_latest = view + 1 == total;
            let buffer = state.current_iteration().unwrap();
            let lines: Vec<String> = buffer
                .lines_handle()
                .lock()
                .unwrap()
                .iter()
                .map(ToString::to_string)
                .collect();
            assert!(
                view == 4 || lines.iter().any(|line| line.starts_with("⚙ Bash")),
                "iteration {} lost its tool calls: {lines:#?}",
                view + 1
            );
            headers.push(frame(&state, 100, 6).lines().next().unwrap().to_string());
        }
        insta::assert_snapshot!("code_assist_headers_per_iteration", headers.join("\n"));

        // Prior-iteration history, reviewed from the finished run.
        state.current_view = 0;
        state.following_latest = false;
        if let Some(buffer) = state.current_iteration_mut() {
            buffer.scroll_offset = 0;
            buffer.following_bottom = false;
        }
        insta::assert_snapshot!("code_assist_iteration_1_review", frame(&state, 100, 30));

        // The live view of the last iteration at a narrow width.
        state.current_view = total - 1;
        state.following_latest = true;
        insta::assert_snapshot!("code_assist_final_narrow", frame(&state, 40, 16));
    }

    // Autoloop concurrency has no per-branch view; nothing claims one.
    {
        let state = replay.state.lock().unwrap();
        for width in [40, 80, 120] {
            let rendered = frame(&state, width, 20);
            assert!(!rendered.contains("WAVE"), "{rendered}");
        }
    }

    // A malformed engine line is skipped and the header says progress may be
    // incomplete.
    append(&replay.events_path, "{not json\n");
    append(
        &replay.events_path,
        &format!(
            "{}\n",
            serde_json::json!({"type":"progress","runId":RUN_ID,"iteration":5,"emittedTopic":"after.drop"})
        ),
    );
    wait_until(&replay.state, "the dropped-line warning", |iterations| {
        iterations
            .last()
            .is_some_and(|lines| lines.iter().any(|line| line.contains("after.drop")))
    })
    .await;
    {
        let state = replay.state.lock().unwrap();
        let header = frame(&state, 100, 6).lines().next().unwrap().to_string();
        assert!(
            state.attention.is_some(),
            "no attention after a dropped line"
        );
        insta::assert_snapshot!("code_assist_dropped_line_header", header);
        let narrow = frame(&state, 40, 6);
        assert!(
            narrow.contains('\u{26A0}'),
            "narrow header lost the warning: {narrow}"
        );
    }

    replay.stop().await;
}
