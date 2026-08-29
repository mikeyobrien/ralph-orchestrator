#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use ralph_core::testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop};
use ralph_proto::json_rpc::RpcEvent;
use tempfile::TempDir;

const RALPH_YML: &str = r#"
core:
  engine: autoloop
cli:
  backend: claude
event_loop:
  max_iterations: 2
  completion_promise: LOOP_COMPLETE
hats:
  planner:
    name: "Planning Lead"
    description: "Plans the work"
    triggers: ["loop.start"]
    publishes: ["build.task"]
    instructions: "Plan the work."
  builder:
    name: "Build Crew"
    description: "Builds the work"
    triggers: ["build.task"]
    publishes: ["task.complete"]
    instructions: "Build the work."
features:
  auto_merge: false
  preflight:
    enabled: true
    skip: [config, hooks, backend, telegram, git, paths, tools]
"#;

struct Harness {
    workspace: TempDir,
    home: TempDir,
    fake_autoloop: FakeAutoloop,
    path: OsString,
}

impl Harness {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");
        fs::write(workspace.path().join("ralph.yml"), RALPH_YML).expect("write ralph.yml");
        fs::write(workspace.path().join("README.md"), "rpc test\n").expect("write README");
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

        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/autoloop/headless_voice.jsonl");
        let fake_autoloop = build_fake_autoloop(&workspace.path().join("fake-autoloop"), &fixture)
            .expect("build fixture-driven fake autoloop");
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![fake_autoloop.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(paths).expect("construct PATH");

        Self {
            workspace,
            home,
            fake_autoloop,
            path,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ralph"));
        command
            .current_dir(self.workspace.path())
            .env("PATH", &self.path)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("ARGV_OUT", self.fake_autoloop.argv_out())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID");
        command
    }
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

fn rendered(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn run_rpc_emits_rpc_events_from_autoloop_events_and_persists_run_id() {
    let harness = Harness::new();
    let output = harness
        .command()
        .args([
            "--color",
            "never",
            "--config",
            "ralph.yml",
            "run",
            "--rpc",
            "--skip-preflight",
            "--max-iterations",
            "2",
            "-p",
            "exercise rpc",
        ])
        .output()
        .expect("run ralph --rpc");
    let text = rendered(&output);
    assert!(output.status.success(), "rpc run failed: {text}");

    let events: Vec<RpcEvent> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("stdout line is not an RpcEvent ({error}): {line}"))
        })
        .collect();

    assert!(
        events
            .iter()
            .any(|event| matches!(event, RpcEvent::LoopStarted { .. })),
        "LoopStarted missing: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            RpcEvent::IterationStart {
                hat_display,
                ..
            } if hat_display == "Planning Lead"
        )),
        "planner IterationStart missing: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            RpcEvent::OrchestrationEvent { topic, .. } if topic == "build.task"
        )),
        "build.task OrchestrationEvent missing: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, RpcEvent::LoopTerminated { .. })),
        "LoopTerminated missing: {events:?}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("Iteration 1/2 started"),
        "headless banner leaked onto the RPC stream: {text}"
    );

    let argv = harness
        .fake_autoloop
        .recorded_argv()
        .expect("recorded autoloop argv");
    assert!(
        argv.iter().any(|arg| arg == "--events"),
        "RPC mode must still pass Autoloop --events: {argv:?}"
    );
    assert_eq!(
        fs::read_to_string(
            harness
                .workspace
                .path()
                .join(".ralph/autoloop/current-run-id")
        )
        .expect("read persisted run_id")
        .trim(),
        "run-headless-voice"
    );
}
