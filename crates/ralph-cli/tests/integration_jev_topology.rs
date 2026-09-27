//! Jev topology routing (Step 11, 2c.3): the refuse-to-start validation, the
//! generated pre_emit seam, and the hook's own decisions.

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use ralph_core::testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop};

struct Workspace {
    dir: tempfile::TempDir,
    home: tempfile::TempDir,
    fake: FakeAutoloop,
}

impl Workspace {
    fn new(ralph_yml: &str, fixture: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("ralph.yml"), ralph_yml).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success()
        );
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/autoloop")
            .join(fixture);
        let fake = build_fake_autoloop(&dir.path().join("fake-autoloop"), &fixture).unwrap();
        Self { dir, home, fake }
    }

    fn ralph(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.fake.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited));
        let mut command = Command::new(env!("CARGO_BIN_EXE_ralph"));
        command
            .args(args)
            .current_dir(self.dir.path())
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID")
            .env_remove("RALPH_ENGINE_DIR")
            .env_remove("TYPESAFE_API_KEY")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().unwrap()
    }

    fn run(&self) -> Output {
        self.ralph(
            &[
                "--color",
                "never",
                "--config",
                "ralph.yml",
                "run",
                "--no-tui",
                "--skip-preflight",
                "-p",
                "route me",
            ],
            &[],
        )
    }

    fn file(&self, relative: &str) -> String {
        fs::read_to_string(self.dir.path().join(relative)).unwrap()
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

const TOPOLOGY_ON: &str = "cli:\n  backend: claude\ncore:\n  routing:\n    topology:\n      jev:\n        enabled: true\n        start: planner\n";
const HATS: &str = "hats:\n  planner:\n    name: Planner\n    description: Plans the work\n    instructions: Plan it.\n  builder:\n    name: Builder\n    description: Writes the code\n    instructions: Build it.\n";

#[test]
fn a_hat_that_still_routes_refuses_to_start_and_is_named() {
    let workspace = Workspace::new(
        &format!("{TOPOLOGY_ON}{HATS}    triggers: [\"build.start\"]\n"),
        "engine_hooks.jsonl",
    );
    let output = workspace.run();
    let text = text(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("hat `builder` declares `triggers`, but core.routing.topology.jev makes Jev the routing authority"),
        "{text}"
    );
    assert!(
        !workspace.dir.path().join(".ralph/autoloop-preset").exists(),
        "the run must refuse before generating a preset"
    );
}

#[test]
fn topology_mode_generates_the_pre_emit_seam_and_route_only_handoffs() {
    let workspace = Workspace::new(&format!("{TOPOLOGY_ON}{HATS}"), "jev_new_engine.jsonl");
    let output = workspace.run();
    assert!(output.status.success(), "{}", text(&output));

    let autoloops = workspace.file(".ralph/autoloop-preset/autoloops.toml");
    let hook = autoloops
        .lines()
        .find(|line| line.starts_with("hook = [{ phase = \"pre_emit\""))
        .unwrap_or_else(|| panic!("no pre_emit hook: {autoloops}"));
    assert!(hook.contains(" gate jev-route --snapshot "), "{hook}");
    assert!(
        hook.contains("on_error = \"block\", mutate = \"event\""),
        "{hook}"
    );
    let topology = workspace.file(".ralph/autoloop-preset/topology.toml");
    assert!(
        topology.contains("\"loop.start\" = [\"planner\"]"),
        "{topology}"
    );
    assert!(
        topology.contains("\"route.builder\" = [\"builder\"]"),
        "{topology}"
    );
    let snapshot = workspace.file(".ralph/autoloop/jev-route.json");
    assert!(snapshot.contains("Writes the code"), "{snapshot}");
}

#[test]
fn with_topology_mode_off_triggers_still_route() {
    let workspace = Workspace::new(
        "cli:\n  backend: claude\nhats:\n  builder:\n    name: Builder\n    description: d\n    triggers: [\"build.start\"]\n    publishes: [\"build.done\"]\n    instructions: i\n  reviewer:\n    name: Reviewer\n    description: d\n    triggers: [\"build.done\"]\n    publishes: [\"task.complete\"]\n    instructions: i\n",
        "engine_hooks.jsonl",
    );
    let output = workspace.run();
    assert!(output.status.success(), "{}", text(&output));
    let topology = workspace.file(".ralph/autoloop-preset/topology.toml");
    assert!(
        topology.contains("\"build.done\" = [\"reviewer\"]"),
        "{topology}"
    );
    assert!(
        topology.contains("\"build.start\" = [\"builder\"]"),
        "{topology}"
    );
    let autoloops = workspace.file(".ralph/autoloop-preset/autoloops.toml");
    assert!(!autoloops.contains("pre_emit"), "{autoloops}");
}

#[test]
fn the_route_hook_blocks_self_routing_and_unjudged_steps_and_passes_completion() {
    let workspace = Workspace::new("cli:\n  backend: claude\n", "engine_hooks.jsonl");
    let state = workspace.dir.path().join(".ralph/autoloop");
    fs::create_dir_all(&state).unwrap();
    let snapshot = serde_json::json!({
        "settings": {"model": "jev-1.13.0", "min_confidence": 0.8, "timeout_ms": 1000},
        "roles": [{"id": "builder", "description": "Writes the code"}],
        "completion_event": "task.complete",
        "journal_file": state.join("journal.jsonl"),
        "record_file": state.join("jev-route.jsonl"),
    });
    let path = state.join("jev-route.json");
    fs::write(&path, snapshot.to_string()).unwrap();
    let hook = |topic: &str| {
        workspace.ralph(
            &["gate", "jev-route", "--snapshot", path.to_str().unwrap()],
            &[
                ("AUTOLOOP_EMIT_TOPIC", topic),
                ("AUTOLOOP_EMIT_PAYLOAD", "step summary"),
            ],
        )
    };

    let direct = hook("route.builder");
    assert_eq!(direct.status.code(), Some(1));
    assert!(
        text(&direct).contains("picks a role directly"),
        "{}",
        text(&direct)
    );

    let unjudged = hook("step.done");
    assert_eq!(unjudged.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&unjudged.stdout).trim(),
        "jev route blocked: TYPESAFE_API_KEY is not set; no fallback routing"
    );
    let record = fs::read_to_string(state.join("jev-route.jsonl")).unwrap();
    assert!(record.contains("\"routed\":false"), "{record}");

    let done = hook("task.complete");
    assert!(done.status.success(), "{}", text(&done));
    assert!(
        done.stdout.is_empty(),
        "completion passes through unchanged"
    );
}
