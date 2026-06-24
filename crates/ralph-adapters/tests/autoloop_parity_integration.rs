//! v3 parity (slice .4): cross-validate the two observability channels of a
//! real `autoloop run` — the "autoloops summary" block on stdout (parsed by
//! [`AutoloopRunner`]) vs. the journal JSONL (tailed by
//! [`AutoloopJournalTailer`] / [`derive_run_summary`]). They must agree on
//! run_id, iteration count, and stop reason; if they diverge, ralph's live
//! state (derived from the journal) would disagree with the authoritative
//! run result, silently breaking the cutover.
//!
//! Opt-in: drives the real autoloop binary with the deterministic mock backend.
//! Skips gracefully when node or the autoloop checkout is absent so a bare
//! clone's CI still passes.

use ralph_adapters::{AutoloopBin, AutoloopJournalTailer, AutoloopRunner, derive_run_summary};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Resolve the autoloop checkout: `AUTOLOOP_ROOT` env override, else search
/// upward from the crate manifest for a sibling `autoloop/` with `bin/autoloop`.
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

#[test]
fn stdout_summary_and_journal_agree_for_a_real_run() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    if which("node").is_none() {
        eprintln!("skip: node not on PATH");
        return;
    }
    let Some(autoloop_root) = find_autoloop_root() else {
        eprintln!("skip: autoloop checkout not found (set AUTOLOOP_ROOT or place a sibling 'autoloop')");
        return;
    };
    let bin = autoloop_root.join("bin/autoloop");
    let preset = autoloop_root.join("packages/presets/presets/autocode");
    let mock = autoloop_root.join("dist/testing/mock-backend.js");
    let fixture = autoloop_root.join("test/fixtures/backend/routed-event-and-promise.json");
    for p in [&bin, &preset, &mock, &fixture] {
        if !p.exists() {
            eprintln!("skip: {} not present", p.display());
            return;
        }
    }

    // Temp git repo as the working dir.
    let tmp = tempfile::tempdir().expect("tempdir");
    let work = tmp.path();
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "test"],
    ] {
        let ok = Command::new("git")
            .args(&args)
            .current_dir(work)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("skip: git {args:?} failed");
            return;
        }
    }
    fs::write(work.join("index.html"), "<p>Hello</p>\n").expect("write index");
    let _ = Command::new("git").args(["add", "."]).current_dir(work).status();
    let _ = Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(work)
        .status();

    // Single-token wrapper around the mock backend (avoids the multi-word -b gotcha).
    let wrapper = work.join("mock-wrapper.sh");
    fs::write(
        &wrapper,
        format!("#!/usr/bin/env bash\nexec node {} \"$@\"\n", mock.display()),
    )
    .expect("write wrapper");
    let mut perms = fs::metadata(&wrapper).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&wrapper, perms).unwrap();

    // Channel 1: the stdout summary parsed by AutoloopRunner.
    let runner = AutoloopRunner::new(&preset, "parity prompt", work)
        .bin(AutoloopBin::Node(bin.clone()))
        .backend(wrapper.to_string_lossy().into_owned())
        .env("MOCK_FIXTURE_PATH", fixture.to_string_lossy().into_owned())
        .max_iterations(3);

    let stdout_summary = runner
        .run()
        .expect("autoloop run should succeed with the mock backend");

    // Channel 2: the journal tailed and derived independently.
    assert!(
        stdout_summary.journal.is_file(),
        "summary journal path should exist: {}",
        stdout_summary.journal.display()
    );
    let mut tailer = AutoloopJournalTailer::new(&stdout_summary.journal);
    let records = tailer.poll().expect("journal should tail cleanly");
    assert!(!records.is_empty(), "journal should contain records");
    let journal_summary = derive_run_summary(&records);

    // The two channels must agree.
    assert_eq!(
        stdout_summary.run_id, journal_summary.run_id,
        "run_id disagreement: stdout vs journal"
    );
    assert_eq!(
        stdout_summary.iterations, journal_summary.iterations,
        "iteration count disagreement: stdout vs journal"
    );
    assert_eq!(
        journal_summary.stop_reason.as_deref(),
        Some(stdout_summary.stop_reason.as_str()),
        "stop_reason disagreement: stdout={:?} journal={:?}",
        stdout_summary.stop_reason,
        journal_summary.stop_reason
    );

    // The live-state view must also reflect a completed run.
    let state = tailer.state();
    assert!(state.completed, "live state should mark the run completed");
    assert_eq!(state.stop_reason.as_deref(), Some(stdout_summary.stop_reason.as_str()));

    eprintln!(
        "parity ok: run_id={} iterations={} stop_reason={}",
        stdout_summary.run_id, stdout_summary.iterations, stdout_summary.stop_reason
    );
}
