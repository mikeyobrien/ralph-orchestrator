#![cfg(unix)]
//! A stop aimed at Ralph's pid alone (`ralph loops stop`, a service manager)
//! must reach the engine instead of orphaning it.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const RALPH_YML: &str =
    "core:\n  engine: autoloop\ncli:\n  backend: claude\nevent_loop:\n  max_iterations: 2\n";

/// An engine that runs until SIGTERM, records it, and exits as autoloop does.
const FAKE_ENGINE: &str = r#"#!/bin/sh
case "$1" in --version) echo "autoloop 0.11.0"; exit 0;; esac
trap 'echo "$$" > "$STOP_OUT"; exit 143' TERM
echo "$$" > "$READY_OUT"
while :; do sleep 0.05; done
"#;

fn wait_for(path: &Path, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if path.exists() && !fs::read_to_string(path).unwrap_or_default().is_empty() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn process_alive(pid: i32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None).is_ok()
}

#[test]
fn sigterm_to_ralph_alone_stops_the_engine_and_ralph_waits_for_it() {
    let workspace = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let engine_dir = tempfile::tempdir().unwrap();
    let root = workspace.path();
    fs::write(root.join("ralph.yml"), RALPH_YML).unwrap();
    for args in [
        vec!["init", "--quiet"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "init",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(&args)
                .current_dir(root)
                .status()
                .unwrap()
                .success()
        );
    }
    let engine = engine_dir.path().join("autoloop");
    fs::write(&engine, FAKE_ENGINE).unwrap();
    fs::set_permissions(&engine, fs::Permissions::from_mode(0o755)).unwrap();
    let ready = root.join("engine-ready");
    let stopped = root.join("engine-stopped");

    let mut ralph = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(["run", "--no-tui", "-p", "run until stopped"])
        .current_dir(root)
        .env("HOME", home.path())
        .env("RALPH_ENGINE_DIR", engine_dir.path())
        .env("READY_OUT", &ready)
        .env("STOP_OUT", &stopped)
        .env_remove("RALPH_CONFIG")
        .env_remove("RALPH_WORKSPACE_ROOT")
        .env_remove("RALPH_MERGE_LOOP_ID")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    assert!(
        wait_for(&ready, Duration::from_secs(30)),
        "the engine never started"
    );
    let engine_pid: i32 = fs::read_to_string(&ready).unwrap().trim().parse().unwrap();

    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(ralph.id() as i32),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();

    assert!(
        wait_for(&stopped, Duration::from_secs(10)),
        "the engine never received the stop"
    );
    let start = Instant::now();
    let status = loop {
        if let Some(status) = ralph.try_wait().unwrap() {
            break status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "ralph did not exit after the engine stopped"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let stderr = std::io::read_to_string(ralph.stderr.take().unwrap()).unwrap();
    assert!(
        std::os::unix::process::ExitStatusExt::signal(&status).is_none(),
        "ralph died on the signal instead of waiting for the engine: {status:?}; {stderr}"
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(!process_alive(engine_pid), "the engine was orphaned");
}
