#![cfg(unix)]

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use ralph_core::testing::fake_autoloop::build_fake_autoloop;
use serde_json::json;

const ROWS: u16 = 42;
const COLS: u16 = 130;
const RUN_ID: &str = "tui-live-stream";
const LIVE_TEXT: &str = "live fixture text before iteration finish";
const AUTHORITATIVE_TEXT: &str = "authoritative final text from backend output";

const AUTOLOOPS_TOML: &str = r#"
[event_loop]
max_iterations = 1
completion_event = "task.complete"
"#;

const TOPOLOGY_TOML: &str = r#"
name = "tui-live-stream-test"
completion = "task.complete"

[[role]]
id = "planner"
emits = ["task.complete"]
prompt_file = "roles/planner.md"

[handoff]
"loop.start" = ["planner"]
"#;

// Verbatim non-assistant records captured from the real Step 1 Claude stream.
const REAL_CLAUDE_THINKING_1: &str = r#"{"type":"system","subtype":"thinking_tokens","estimated_tokens":50,"estimated_tokens_delta":50,"uuid":"e8214f3e-b206-44ad-abbf-f83e9f298b2d","session_id":"bc2fcab3-578e-4ef3-82fa-f2f39baff624"}"#;
const REAL_CLAUDE_THINKING_2: &str = r#"{"type":"system","subtype":"thinking_tokens","estimated_tokens":200,"estimated_tokens_delta":150,"uuid":"6fc4fae2-5be2-4cc8-b905-d2f7e9887cc4","session_id":"bc2fcab3-578e-4ef3-82fa-f2f39baff624"}"#;

fn run_git(cwd: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("execute git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn wait_for_file(path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

fn screen_contents(parser: &Arc<Mutex<vt100::Parser>>) -> String {
    parser
        .lock()
        .expect("lock terminal parser")
        .screen()
        .contents()
}

fn wait_for_screen(
    parser: &Arc<Mutex<vt100::Parser>>,
    needle: &str,
    timeout: Duration,
) -> Option<String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let screen = screen_contents(parser);
        if screen.contains(needle) {
            return Some(screen);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

fn write_fixture(workspace: &Path, fixture: &Path) {
    let initial_events = [
        json!({
            "type": "loop.start",
            "runId": RUN_ID,
            "workDir": workspace,
            "maxIterations": 1,
        })
        .to_string(),
        json!({
            "type": "iteration.start",
            "runId": RUN_ID,
            "iteration": 1,
            "maxIterations": 1,
        })
        .to_string(),
        json!({
            "type": "iteration.banner",
            "runId": RUN_ID,
            "iteration": 1,
            "maxIterations": 1,
            "allowedRoles": ["planner"],
        })
        .to_string(),
    ];
    let assistant = json!({
        "type": "assistant",
        "message": {
            "content": [
                {"type": "text", "text": LIVE_TEXT},
                {"type": "tool_use", "name": "Read", "input": {"file_path": "DESIGN.md"}},
            ]
        }
    })
    .to_string();
    let finish_events = [
        json!({
            "type": "backend.output",
            "runId": RUN_ID,
            "iteration": 1,
            "output": AUTHORITATIVE_TEXT,
        })
        .to_string(),
        json!({
            "type": "iteration.finish",
            "runId": RUN_ID,
            "iteration": 1,
        })
        .to_string(),
        json!({
            "type": "loop.finish",
            "runId": RUN_ID,
            "iterations": 1,
            "stopReason": "completed",
            "costUsd": 0.01,
        })
        .to_string(),
    ];
    let stream_path = format!(".autoloop/runs/{RUN_ID}/claude-stream.1.jsonl");

    // Separate stream steps append complete real-format records one at a time,
    // matching the incremental file growth Ralph observes from real autoloop.
    let invocation = json!({
        "steps": [
            {"events": initial_events},
            {"stream": {"path": stream_path, "lines": [REAL_CLAUDE_THINKING_1]}},
            {"stream": {"path": stream_path, "lines": [REAL_CLAUDE_THINKING_2]}},
            {"stream": {"path": stream_path, "lines": [assistant]}},
            {"barrier": {"ready_env": "LIVE_READY", "release_env": "LIVE_RELEASE"}},
            {"events": finish_events},
            {"barrier": {"ready_env": "FINISH_READY", "release_env": "FINISH_RELEASE"}},
            {"summary": {
                "run_id": RUN_ID,
                "iterations": 1,
                "stop_reason": "completed",
                "cost_usd": 0.01,
                "journal": "/tmp/autoloop-tui-live-journal.jsonl",
                "memory": "/tmp/autoloop-tui-live-memory.jsonl"
            }}
        ]
    });
    fs::write(fixture, format!("{invocation}\n")).expect("write fake-autoloop fixture");
}

fn release_and_kill(child: &mut Box<dyn portable_pty::Child + Send + Sync>, releases: &[&Path]) {
    for release in releases {
        let _ = fs::write(release, "release\n");
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn tui_shows_incremental_backend_stream_before_iteration_finish_and_reconciles_once() {
    let workspace = tempfile::tempdir().expect("workspace temp dir");
    let home = tempfile::tempdir().expect("home temp dir");
    let preset = workspace.path().join("preset");
    fs::create_dir_all(preset.join("roles")).expect("create preset roles dir");
    fs::write(preset.join("autoloops.toml"), AUTOLOOPS_TOML).expect("write autoloops.toml");
    fs::write(preset.join("topology.toml"), TOPOLOGY_TOML).expect("write topology.toml");
    fs::write(preset.join("roles/planner.md"), "Complete the test.").expect("write role prompt");
    fs::write(
        workspace.path().join("ralph.yml"),
        "core:\n  engine: autoloop\n  autoloop_preset: preset\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n",
    )
    .expect("write ralph.yml");
    fs::write(workspace.path().join("README.md"), "tui live-stream test\n").expect("write README");
    run_git(workspace.path(), &["init", "--quiet"]);
    run_git(
        workspace.path(),
        &["add", "README.md", "ralph.yml", "preset"],
    );
    run_git(
        workspace.path(),
        &[
            "-c",
            "user.name=Ralph Test",
            "-c",
            "user.email=ralph@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );

    let fixture = workspace.path().join("tui-live-stream.jsonl");
    write_fixture(workspace.path(), &fixture);
    let fake_autoloop = build_fake_autoloop(&workspace.path().join("fake-autoloop"), &fixture)
        .expect("build fixture-driven fake autoloop");

    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![fake_autoloop.bin_dir().to_path_buf()];
    paths.extend(std::env::split_paths(&inherited_path));
    let path = std::env::join_paths(paths).expect("construct PATH");
    let live_ready = workspace.path().join("live-ready");
    let live_release = workspace.path().join("live-release");
    let finish_ready = workspace.path().join("finish-ready");
    let finish_release = workspace.path().join("finish-release");

    let pty = native_pty_system()
        .openpty(PtySize {
            rows: ROWS,
            cols: COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_ralph"));
    command.args([
        "--config",
        "ralph.yml",
        "run",
        "--skip-preflight",
        "--max-iterations",
        "1",
        "-p",
        "exercise live TUI streaming",
    ]);
    command.cwd(workspace.path());
    command.env("TERM", "xterm-256color");
    command.env("PATH", path);
    command.env("HOME", home.path());
    command.env("USERPROFILE", home.path());
    command.env("LIVE_READY", &live_ready);
    command.env("LIVE_RELEASE", &live_release);
    command.env("FINISH_READY", &finish_ready);
    command.env("FINISH_RELEASE", &finish_release);
    command.env_remove("RALPH_CONFIG");
    command.env_remove("RALPH_WORKSPACE_ROOT");
    command.env_remove("RALPH_MERGE_LOOP_ID");

    let mut reader = pty.master.try_clone_reader().expect("clone pty reader");
    let mut child = pty
        .slave
        .spawn_command(command)
        .expect("spawn ralph in pty");
    drop(pty.slave);
    drop(pty.master);

    let parser = Arc::new(Mutex::new(vt100::Parser::new(ROWS, COLS, 0)));
    let parser_for_reader = Arc::clone(&parser);
    let transcript = Arc::new(Mutex::new(Vec::new()));
    let transcript_for_reader = Arc::clone(&transcript);
    let reader_thread = thread::spawn(move || {
        let mut bytes = [0_u8; 8192];
        loop {
            match reader.read(&mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    parser_for_reader
                        .lock()
                        .expect("lock terminal parser")
                        .process(&bytes[..read]);
                    transcript_for_reader
                        .lock()
                        .expect("lock transcript")
                        .extend_from_slice(&bytes[..read]);
                }
            }
        }
    });

    if !wait_for_file(&live_ready, Duration::from_secs(10)) {
        release_and_kill(&mut child, &[&live_release, &finish_release]);
        panic!("fake autoloop never reached the live-stream barrier");
    }
    let Some(live_screen) = wait_for_screen(&parser, LIVE_TEXT, Duration::from_secs(5)) else {
        release_and_kill(&mut child, &[&live_release, &finish_release]);
        panic!(
            "live agent text was not visible before iteration finish; screen:\n{}",
            screen_contents(&parser)
        );
    };
    assert!(
        live_screen.contains("[LIVE]"),
        "live content appeared outside an active iteration:\n{live_screen}"
    );
    assert!(
        !live_screen.contains(AUTHORITATIVE_TEXT),
        "authoritative boundary output appeared before barrier release:\n{live_screen}"
    );

    fs::write(&live_release, "release\n").expect("release live-stream barrier");
    if !wait_for_file(&finish_ready, Duration::from_secs(10)) {
        release_and_kill(&mut child, &[&finish_release]);
        panic!("fake autoloop never reached the post-finish barrier");
    }
    let Some(finished_screen) =
        wait_for_screen(&parser, AUTHORITATIVE_TEXT, Duration::from_secs(5))
    else {
        release_and_kill(&mut child, &[&finish_release]);
        panic!(
            "authoritative output was not visible after iteration finish; screen:\n{}",
            screen_contents(&parser)
        );
    };
    assert_eq!(
        finished_screen.matches(AUTHORITATIVE_TEXT).count(),
        1,
        "authoritative output must appear exactly once:\n{finished_screen}"
    );
    assert!(
        !finished_screen.contains(LIVE_TEXT),
        "provisional live text survived reconciliation:\n{finished_screen}"
    );

    fs::write(&finish_release, "release\n").expect("release post-finish barrier");
    let status = child.wait().expect("wait for ralph");
    reader_thread.join().expect("join pty reader");
    assert!(
        status.success(),
        "ralph failed with {status:?}; transcript:\n{}",
        String::from_utf8_lossy(&transcript.lock().expect("lock transcript"))
    );
}
