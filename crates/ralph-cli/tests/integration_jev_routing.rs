//! Jev routing is never dropped silently on any translation path (Step 9,
//! Phase 2c.1): the generated preset carries `core.routing.jev`, an engine
//! that ignores `[routing.jev]` is refused, and paths that cannot carry the
//! block refuse and name the fix.

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use ralph_core::testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop};

const ROUTES: &str =
    r#"[{"id":"fix","description":"Fix a bug","instructions":"Reproduce, then fix."}]"#;

fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?}");
}

struct Workspace {
    dir: tempfile::TempDir,
    home: tempfile::TempDir,
    fake: FakeAutoloop,
}

impl Workspace {
    fn new(ralph_yml: &str, fixture: &str) -> Self {
        let dir = tempfile::tempdir().expect("workspace");
        let home = tempfile::tempdir().expect("home");
        let root = dir.path();
        fs::write(root.join("ralph.yml"), ralph_yml).unwrap();
        fs::write(root.join("routes.json"), ROUTES).unwrap();
        git(root, &["init", "--quiet"]);
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/autoloop")
            .join(fixture);
        let fake = build_fake_autoloop(&root.join("fake-autoloop"), &fixture).expect("fake");
        Self { dir, home, fake }
    }

    fn write_preset(&self, routing: bool) {
        let preset = self.dir.path().join("preset");
        fs::create_dir_all(preset.join("roles")).unwrap();
        let mut autoloops = String::from("event_loop.max_iterations = 1\n");
        if routing {
            autoloops.push_str("\n[routing.jev]\nenabled = true\nroutes_file = \"routes.json\"\n");
        }
        fs::write(preset.join("autoloops.toml"), autoloops).unwrap();
        fs::write(
            preset.join("topology.toml"),
            "name = \"p\"\ncompletion = \"task.complete\"\n[[role]]\nid = \"worker\"\nemits = [\"task.complete\"]\nprompt_file = \"roles/worker.md\"\n[handoff]\n\"loop.start\" = [\"worker\"]\n",
        )
        .unwrap();
        fs::write(preset.join("roles/worker.md"), "Work.").unwrap();
        fs::write(preset.join("routes.json"), ROUTES).unwrap();
    }

    fn run(&self, extra: &[&str]) -> Output {
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.fake.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited));
        let mut args = vec!["--color", "never", "--config", "ralph.yml", "run"];
        args.extend_from_slice(extra);
        args.extend(["--no-tui", "--skip-preflight", "-p", "route me"]);
        Command::new(env!("CARGO_BIN_EXE_ralph"))
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
            .stderr(Stdio::piped())
            .output()
            .expect("run ralph")
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

const GENERATED_ROUTING: &str = "cli:\n  backend: claude\ncore:\n  routing:\n    jev:\n      enabled: true\n      routes_file: routes.json\n      min_confidence: 0.9\n";

#[test]
fn generated_preset_carries_core_routing_jev_on_a_capable_engine() {
    let workspace = Workspace::new(GENERATED_ROUTING, "jev_new_engine.jsonl");
    let output = workspace.run(&[]);
    assert!(output.status.success(), "{}", text(&output));

    let autoloops = fs::read_to_string(
        workspace
            .dir
            .path()
            .join(".ralph/autoloop-preset/autoloops.toml"),
    )
    .unwrap();
    let routes = workspace.dir.path().join("routes.json");
    for line in [
        "routing.jev.enabled = true".to_string(),
        format!("routing.jev.routes_file = \"{}\"", routes.display()),
        "routing.jev.min_confidence = 0.9".to_string(),
    ] {
        assert!(autoloops.contains(&line), "missing {line}: {autoloops}");
    }
}

#[test]
fn an_engine_that_ignores_routing_is_refused() {
    let workspace = Workspace::new(GENERATED_ROUTING, "jev_old_engine.jsonl");
    let output = workspace.run(&[]);
    let text = text(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("autoloop 0.11.0 ignores [routing.jev]") && text.contains(">= 0.12.0"),
        "{text}"
    );
}

#[test]
fn an_explicit_preset_with_routing_is_also_gated_on_the_engine() {
    let workspace = Workspace::new(
        "cli:\n  backend: claude\ncore:\n  autoloop_preset: preset\n",
        "jev_old_engine.jsonl",
    );
    workspace.write_preset(true);
    let output = workspace.run(&[]);
    let text = text(&output);
    assert!(!output.status.success(), "{text}");
    assert!(text.contains("ignores [routing.jev]"), "{text}");
}

#[test]
fn core_routing_with_an_explicit_preset_refuses_and_names_the_fix() {
    let workspace = Workspace::new(
        &format!("{GENERATED_ROUTING}  autoloop_preset: preset\n"),
        "jev_new_engine.jsonl",
    );
    workspace.write_preset(false);
    let output = workspace.run(&[]);
    let text = text(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("core.routing.jev is set, but core.autoloop_preset (preset)"),
        "{text}"
    );
}

#[test]
fn a_hats_overlay_preset_with_routing_refuses_and_names_the_path() {
    let workspace = Workspace::new("cli:\n  backend: claude\n", "jev_new_engine.jsonl");
    workspace.write_preset(true);
    let output = workspace.run(&["-H", "preset"]);
    let text = text(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("enables [routing.jev], which a hats overlay (-H) cannot carry"),
        "{text}"
    );
}
