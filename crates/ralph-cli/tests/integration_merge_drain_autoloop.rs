#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use ralph_core::merge_queue::{MergeEvent, MergeEventType, MergeQueue, MergeState};
use tempfile::TempDir;

const LOOP_ID: &str = "merge-lifecycle-test";

struct Harness {
    workspace: TempDir,
    home: TempDir,
    bin_dir: PathBuf,
    spawn_log: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");

        fs::write(
            workspace.path().join("ralph.yml"),
            "core:\n  engine: autoloop\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n",
        )
        .expect("write ralph.yml");
        fs::write(workspace.path().join("README.md"), "merge lifecycle test\n")
            .expect("write README");
        run_git(workspace.path(), &["init", "--quiet"]);
        run_git(workspace.path(), &["add", "README.md", "ralph.yml"]);
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

        let bin_dir = workspace.path().join("bin");
        fs::create_dir(&bin_dir).expect("create fake bin dir");
        write_executable(
            &bin_dir.join("autoloop"),
            r#"#!/bin/sh
set -eu
cat <<'SUMMARY'
autoloops summary
===================
run_id: run-merge-lifecycle
iterations: 1
stop_reason: completed
journal: /tmp/autoloop-merge-lifecycle-journal.jsonl
memory: /tmp/autoloop-merge-lifecycle-memory.jsonl
SUMMARY
"#,
        );

        let spawn_log = workspace.path().join("ralph-spawns.log");
        write_executable(
            &bin_dir.join("ralph"),
            r#"#!/bin/sh
set -eu
: "${REAL_RALPH:?REAL_RALPH must be set}"
: "${SPAWN_LOG:?SPAWN_LOG must be set}"
printf 'RALPH_MERGE_LOOP_ID=%s %s\n' "${RALPH_MERGE_LOOP_ID:-}" "$*" >> "$SPAWN_LOG"
exec "$REAL_RALPH" "$@"
"#,
        );

        // The merge child runs preflight without --skip-preflight. Its bundled
        // merge config defaults to the Claude backend, so make that executable
        // discoverable without invoking a real agent.
        write_executable(&bin_dir.join("claude"), "#!/bin/sh\nexit 0\n");

        Self {
            workspace,
            home,
            bin_dir,
            spawn_log,
        }
    }

    fn queue(&self) -> MergeQueue {
        MergeQueue::new(self.workspace.path())
    }

    fn process_queue(&self) -> Output {
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.bin_dir.clone()];
        paths.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(paths).expect("construct PATH");

        Command::new(env!("CARGO_BIN_EXE_ralph"))
            .args(["--color", "never", "loops", "process"])
            .current_dir(self.workspace.path())
            .env("PATH", path)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("REAL_RALPH", env!("CARGO_BIN_EXE_ralph"))
            .env("SPAWN_LOG", &self.spawn_log)
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID")
            .output()
            .expect("execute ralph loops process")
    }

    fn wait_until_merged(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let entry = self
                .queue()
                .get_entry(LOOP_ID)
                .expect("read merge queue")
                .expect("seeded queue entry");
            if entry.state == MergeState::Merged {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "merge did not complete within 30s; final state: {:?}",
                entry.state
            );
            thread::sleep(Duration::from_millis(200));
        }
    }

    fn events_for_loop(&self) -> Vec<MergeEvent> {
        fs::read_to_string(self.workspace.path().join(MergeQueue::QUEUE_FILE))
            .expect("read merge event log")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<MergeEvent>(line).expect("parse merge event"))
            .filter(|event| event.loop_id == LOOP_ID)
            .collect()
    }

    fn spawn_lines(&self) -> Vec<String> {
        fs::read_to_string(&self.spawn_log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make executable");
}

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "ralph failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn process_queue_observes_full_lifecycle_and_does_not_remerge() {
    let harness = Harness::new();
    harness
        .queue()
        .enqueue(LOOP_ID, "test prompt")
        .expect("enqueue merge");

    let first = harness.process_queue();
    assert_success(&first);
    harness.wait_until_merged();

    let events = harness.events_for_loop();
    assert_eq!(events.len(), 3, "unexpected merge events: {events:?}");
    assert!(matches!(events[0].event, MergeEventType::Queued { .. }));
    let merging_pid = match events[1].event {
        MergeEventType::Merging { pid } => pid,
        ref event => panic!("expected merging event, got {event:?}"),
    };
    assert!(merging_pid > 0, "merging event must record the child pid");
    assert!(matches!(events[2].event, MergeEventType::Merged { .. }));

    let first_spawn_lines = harness.spawn_lines();
    assert_eq!(
        first_spawn_lines.len(),
        1,
        "expected exactly one merge child spawn: {first_spawn_lines:?}"
    );
    assert!(
        first_spawn_lines[0].starts_with(&format!("RALPH_MERGE_LOOP_ID={LOOP_ID} ")),
        "merge loop id was not forwarded: {first_spawn_lines:?}"
    );

    let second = harness.process_queue();
    assert_success(&second);
    thread::sleep(Duration::from_secs(2));

    assert_eq!(
        harness.spawn_lines(),
        first_spawn_lines,
        "a terminal merge entry must not be spawned again"
    );
    assert_eq!(
        harness.events_for_loop(),
        events,
        "a second drain must not append lifecycle events"
    );
    assert_eq!(
        harness
            .queue()
            .get_entry(LOOP_ID)
            .expect("read queue")
            .expect("merge entry")
            .state,
        MergeState::Merged
    );
}
