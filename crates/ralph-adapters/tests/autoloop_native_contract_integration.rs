//! v3 "native usage" end-to-end: ralph drives the real `autoloop` CLI through
//! the new contracts — the structured `--events` stream (#30) and the blocking
//! human-ask / `control respond` HITL flow (#32) — proving ralph consumes the
//! full cutover surface, not just the legacy journal.
//!
//! Opt-in: drives the real autoloop binary with the deterministic mock backend.
//! Skips when node or the autoloop checkout is absent.

use ralph_adapters::{
    AutoloopBin, AutoloopRunner, events_run_result, first_pending_ask, parse_events,
};
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_autoloop_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("AUTOLOOP_ROOT") {
        let p = PathBuf::from(root);
        if p.join("bin/autoloop").is_file() {
            return Some(p);
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

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let candidate = dir.join(program);
        candidate.is_file().then_some(candidate)
    })
}

/// Probe whether the resolved autoloop build implements the `--events` NDJSON
/// stream (contract #30). Conforming builds list the flag in `run --help`;
/// builds that predate the contract silently ignore it and advertise nothing.
/// Invoked as `node <bin>` to match how the tests drive the binary.
fn autoloop_supports_events(bin: &Path) -> bool {
    Command::new("node")
        .arg(bin)
        .args(["run", "--help"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains("--events"))
        .unwrap_or(false)
}

/// Probe whether the resolved autoloop build implements the human-ask /
/// `control respond` HITL round-trip (contract #32). Conforming builds expose a
/// `respond` verb in `control --help`; builds that predate it reject the
/// `human.ask` event and never surface `ask.pending`, so the round-trip can't be
/// exercised. Invoked as `node <bin>` to match how the tests drive the binary.
fn autoloop_supports_control_respond(bin: &Path) -> bool {
    Command::new("node")
        .arg(bin)
        .args(["control", "--help"])
        .output()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout).contains("respond")
                || String::from_utf8_lossy(&out.stderr).contains("respond")
        })
        .unwrap_or(false)
}

struct Env {
    work: tempfile::TempDir,
    bin: PathBuf,
    preset: PathBuf,
    wrapper: PathBuf,
    fixture: PathBuf,
}

/// Build a temp git project + a single-token mock-backend wrapper. Returns None
/// (caller skips) when prerequisites are missing.
fn setup(fixture_name: &str) -> Option<Env> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    if which("node").is_none() {
        eprintln!("skip: node not on PATH");
        return None;
    }
    let root = find_autoloop_root()?;
    let bin = root.join("bin/autoloop");
    let preset = root.join("packages/presets/presets/autocode");
    let mock = root.join("dist/testing/mock-backend.js");
    let fixture = root.join("test/fixtures/backend").join(fixture_name);
    for p in [&bin, &preset, &mock, &fixture] {
        if !p.exists() {
            eprintln!("skip: {} not present", p.display());
            return None;
        }
    }

    // The `--events` NDJSON stream (contract #30) is an opt-in autoloop
    // capability. An older/non-conforming build silently ignores the flag and
    // writes no file, which would make these tests hard-fail on a stale sibling
    // checkout. Both tests exist solely to exercise that stream, so skip (rather
    // than fail) when the resolved build doesn't advertise `--events` — matching
    // the skip-on-missing-prereq pattern above. The gate reopens automatically
    // once a conforming autoloop is present.
    if !autoloop_supports_events(&bin) {
        eprintln!(
            "skip: autoloop at {} does not implement the --events contract (#30)",
            bin.display()
        );
        return None;
    }

    let work = tempfile::tempdir().expect("tempdir");
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "t@example.com"],
        vec!["config", "user.name", "t"],
    ] {
        let ok = Command::new("git")
            .args(&args)
            .current_dir(work.path())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("skip: git {args:?} failed");
            return None;
        }
    }
    fs::write(work.path().join("index.html"), "<p>hi</p>\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(work.path())
        .status();
    let _ = Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(work.path())
        .status();

    let wrapper = work.path().join("mock-wrapper.sh");
    fs::write(
        &wrapper,
        format!("#!/usr/bin/env bash\nexec node {} \"$@\"\n", mock.display()),
    )
    .unwrap();
    let mut perms = fs::metadata(&wrapper).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&wrapper, perms).unwrap();

    Some(Env {
        work,
        bin,
        preset,
        wrapper,
        fixture,
    })
}

fn runner(env: &Env) -> AutoloopRunner {
    AutoloopRunner::new(&env.preset, "native contract", env.work.path())
        .bin(AutoloopBin::Node(env.bin.clone()))
        .backend(env.wrapper.to_string_lossy().into_owned())
        .env(
            "MOCK_FIXTURE_PATH",
            env.fixture.to_string_lossy().into_owned(),
        )
}

#[test]
fn ralph_owned_state_overrides_native_preset_paths() {
    let Some(env) = setup("routed-event-and-promise.json") else {
        return;
    };
    let owned_root = env.work.path().join(".ralph/autoloop");
    let agent_root = env.work.path().join(".ralph/agent");
    std::fs::create_dir_all(&agent_root).unwrap();
    let ralph_tasks = agent_root.join("tasks.jsonl");
    let ralph_memory = agent_root.join("memories.md");
    std::fs::write(&ralph_tasks, "ralph-task-sentinel\n").unwrap();
    std::fs::write(&ralph_memory, "ralph-memory-sentinel\n").unwrap();

    // The stock native preset explicitly declares core.state_dir plus journal
    // and memory files under .autoloop. Top-level buildLoopContext consumes CLI
    // config overrides; environment exports keep nested tools on the same root.
    std::fs::create_dir_all(&owned_root).unwrap();
    let events_path = owned_root.join("events.ndjson");
    let summary = runner(&env)
        .env(
            "AUTOLOOP_STATE_DIR",
            owned_root.to_string_lossy().into_owned(),
        )
        .env(
            "AUTOLOOP_JOURNAL_FILE",
            owned_root
                .join("journal.jsonl")
                .to_string_lossy()
                .into_owned(),
        )
        .env(
            "AUTOLOOP_MEMORY_FILE",
            owned_root
                .join("memory.jsonl")
                .to_string_lossy()
                .into_owned(),
        )
        .env(
            "AUTOLOOP_TASKS_FILE",
            owned_root
                .join("tasks.jsonl")
                .to_string_lossy()
                .into_owned(),
        )
        .set_override("core.state_dir", &owned_root.to_string_lossy())
        .set_override(
            "core.journal_file",
            &owned_root.join("journal.jsonl").to_string_lossy(),
        )
        .set_override(
            "core.memory_file",
            &owned_root.join("memory.jsonl").to_string_lossy(),
        )
        .set_override(
            "core.tasks_file",
            &owned_root.join("tasks.jsonl").to_string_lossy(),
        )
        .events_path(&events_path)
        .max_iterations(3)
        .run()
        .expect("autoloop run with Ralph-owned state should succeed");

    assert_eq!(summary.journal, owned_root.join("journal.jsonl"));
    assert_eq!(summary.memory, owned_root.join("memory.jsonl"));
    assert!(summary.journal.is_file());
    assert!(events_path.is_file());
    assert!(
        owned_root.join("runs").join(&summary.run_id).is_dir(),
        "run-scoped state must resolve beneath the Ralph-owned root"
    );
    assert!(
        !env.work.path().join(".autoloop").exists(),
        "explicit native preset escaped to a top-level .autoloop"
    );
    assert_eq!(
        std::fs::read_to_string(ralph_tasks).unwrap(),
        "ralph-task-sentinel\n"
    );
    assert_eq!(
        std::fs::read_to_string(ralph_memory).unwrap(),
        "ralph-memory-sentinel\n"
    );
}

#[test]
fn ralph_consumes_the_structured_events_stream() {
    let Some(env) = setup("routed-event-and-promise.json") else {
        return;
    };
    let events_path = env.work.path().join("events.ndjson");

    let summary = runner(&env)
        .events_path(&events_path)
        .max_iterations(3)
        .run()
        .expect("autoloop run should succeed");

    assert!(events_path.is_file(), "events file should be written");
    let events = parse_events(&std::fs::read_to_string(&events_path).unwrap());
    assert!(!events.is_empty());

    // The resolved `progress` event (routing + outcome) is on the stream — it
    // is NOT in the journal, so this proves native consumption of #30.
    assert!(
        events
            .iter()
            .any(|e| e.kind == "progress" && e.outcome.is_some()),
        "stream should carry resolved progress events"
    );

    // The machine-readable run result agrees with the stdout summary.
    let result = events_run_result(&events).expect("terminal result on the stream");
    assert_eq!(result.run_id, summary.run_id);
    assert_eq!(result.iterations, summary.iterations);
    assert_eq!(result.stop_reason, summary.stop_reason);

    eprintln!(
        "events-stream ok: run_id={} iterations={} stop_reason={}",
        result.run_id, result.iterations, result.stop_reason
    );
}

#[test]
fn ralph_drives_the_hitl_round_trip_via_control_respond() {
    let Some(env) = setup("human-ask.json") else {
        return;
    };
    // The HITL round-trip needs the `control respond` verb (contract #32) on top
    // of the `--events` stream gated in setup(). A build without it rejects
    // `human.ask` and never emits `ask.pending`, so skip rather than hard-fail —
    // the gate reopens once a conforming autoloop is present.
    if !autoloop_supports_control_respond(&env.bin) {
        eprintln!(
            "skip: autoloop at {} does not implement the `control respond` HITL contract (#32)",
            env.bin.display()
        );
        return;
    }
    let events_path = env.work.path().join("hitl-events.ndjson");
    let work = env.work.path().to_path_buf();
    let answer = "go with approach B";

    // Responder thread: watch the events stream for ask.pending, then deliver a
    // `respond` control request to unblock the run — exactly what ralph's RObot
    // layer does (Telegram reply -> autoloop control respond).
    let responder_events = events_path.clone();
    let responder_work = work.clone();
    let responder = std::thread::spawn(move || {
        for _ in 0..200 {
            if let Ok(content) = std::fs::read_to_string(&responder_events)
                && let Some(ask) = first_pending_ask(&parse_events(&content))
            {
                deliver_respond(&responder_work, &ask.run_id, &ask.question_id, answer);
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        false
    });

    let summary = runner(&env)
        .events_path(&events_path)
        .set_override("event_loop.ask_timeout", "8000")
        .set_override("event_loop.ask_poll_ms", "25")
        .max_iterations(1)
        .run()
        .expect("autoloop run should succeed");

    let delivered = responder.join().unwrap();
    assert!(
        delivered,
        "responder should have observed ask.pending and replied"
    );

    // The run completed normally (the ask was answered, not timed out).
    assert!(!summary.stop_reason.is_empty());

    // The answer reached the loop: journal records ask.answered + the answer,
    // injected as guidance for the next iteration.
    let journal = std::fs::read_to_string(&summary.journal).unwrap();
    assert!(journal.contains(r#""topic": "ask.pending""#));
    assert!(
        journal.contains(r#""topic": "ask.answered""#),
        "the human response should have been delivered and recorded"
    );
    assert!(
        journal.contains(answer),
        "the answer text should be in the journal"
    );
    assert!(
        !journal.contains(r#""topic": "ask.timeout""#),
        "should not have timed out"
    );

    eprintln!("hitl round-trip ok: stop_reason={}", summary.stop_reason);
}

/// Write a `respond` control request to the run's control queue (the same file
/// `autoloop control respond` writes to), keyed by the question id.
fn deliver_respond(work: &Path, run_id: &str, question_id: &str, answer: &str) {
    use std::fs;
    // Run-scoped isolation (autoloop default): <work>/.autoloop/runs/<run>/control
    let control_dir = work
        .join(".autoloop")
        .join("runs")
        .join(run_id)
        .join("control");
    fs::create_dir_all(&control_dir).unwrap();
    let request = serde_json::json!({
        "id": "ctl_ralph_test",
        "runId": run_id,
        "requestedAt": "2026-06-24T00:00:00.000Z",
        "verb": "respond",
        "reason": "respond",
        "payload": { "questionId": question_id, "answer": answer },
    });
    let line = format!("{}\n", serde_json::to_string(&request).unwrap());
    let path = control_dir.join("requests.jsonl");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    fs::write(&path, format!("{existing}{line}")).unwrap();
}
