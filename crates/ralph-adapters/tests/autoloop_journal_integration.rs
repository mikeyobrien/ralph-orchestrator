//! Deterministic replay tests against a committed autoloop journal fixture.
//!
//! The fixture `tests/fixtures/autoloop/max_iterations.journal.jsonl` was
//! produced by running the real autoloop mock backend (autocode preset,
//! `event_loop.max_iterations=3`, routed-event-and-promise fixture) and copying
//! the resulting `.autoloop/journal.jsonl`. Machine-local temp paths embedded in
//! the prompt projection were rewritten to a stable placeholder; every event
//! shape is otherwise byte-for-byte as autoloop wrote it.
//!
//! These tests need no node/autoloop at runtime: they read the committed fixture
//! and exercise the pure parser/derivation in `ralph_adapters::autoloop_journal`.

use ralph_adapters::{JournalReplay, derive_run_summary, replay_journal};

const FIXTURE: &str = include_str!("fixtures/autoloop/max_iterations.journal.jsonl");

#[test]
fn fixture_topic_sequence_matches_contract() {
    let recs = replay_journal(FIXTURE).expect("fixture parses");
    let topics: Vec<&str> = recs.iter().map(|r| r.topic.as_str()).collect();

    let expected = [
        "loop.start",
        "iteration.start",
        "backend.start",
        "tasks.ready",
        "backend.finish",
        "iteration.finish",
        "iteration.start",
        "backend.start",
        "event.invalid",
        "backend.finish",
        "iteration.finish",
        "iteration.start",
        "backend.start",
        "event.invalid",
        "backend.finish",
        "iteration.finish",
        "loop.stop",
    ];

    assert_eq!(
        topics, expected,
        "journal topic sequence drifted from contract"
    );
}

#[test]
fn fixture_derives_expected_run_summary() {
    let recs = replay_journal(FIXTURE).expect("fixture parses");
    let summary = derive_run_summary(&recs);

    assert_eq!(summary.run_id, "lucky-pilot");
    assert_eq!(summary.iterations, 3);
    assert_eq!(summary.stop_reason.as_deref(), Some("max_iterations"));
}

#[test]
fn fixture_payload_event_is_decoded() {
    let recs = replay_journal(FIXTURE).expect("fixture parses");
    let emitted = recs
        .iter()
        .find(|r| r.topic == "tasks.ready")
        .expect("tasks.ready present");
    assert_eq!(emitted.iteration, Some(1));
    assert_eq!(emitted.field("source"), Some("agent"));
    assert_eq!(emitted.field("payload"), Some("Mock backend: tasks ready"));
}

#[test]
fn fixture_split_mid_line_replay_equals_whole_file() {
    let whole = replay_journal(FIXTURE).expect("whole-file parse");

    // Split the journal at every byte and verify chunked replay recovers the
    // exact same complete event set. A partial final line must never be dropped
    // or half-parsed.
    let mut checked = 0usize;
    for split in 1..FIXTURE.len() {
        if !FIXTURE.is_char_boundary(split) {
            continue;
        }
        let (a, b) = FIXTURE.split_at(split);
        let mut replay = JournalReplay::new();
        replay.push(a).expect("first chunk parses");
        replay.push(b).expect("second chunk parses");
        assert_eq!(
            replay.into_records(),
            whole,
            "chunked replay split at byte {split} diverged from whole-file replay"
        );
        checked += 1;
    }
    assert!(checked > 0, "expected at least one split point");
}
