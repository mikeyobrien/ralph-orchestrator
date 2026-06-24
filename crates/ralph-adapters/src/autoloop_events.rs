//! Parser for autoloop's `--events` NDJSON `LoopEvent` stream (v3 contract #30).
//!
//! `autoloop run --events <path>` appends one JSON object per line, each a
//! `LoopEvent` discriminated by a `type` field. This is the *structured*
//! observability channel ralph drives autoloop through: unlike the journal, it
//! carries the resolved `progress` event (routing + outcome) and a final
//! machine-readable run result (`loop.finish` / `summary`).
//!
//! Rather than model the full union, we decode each line into one flat
//! [`AutoloopEvent`] keyed by `kind` — every field ralph consumes is optional,
//! so unknown event types decode losslessly and forward-compatibly.

use serde::Deserialize;

/// One decoded `LoopEvent` line. `kind` is the event `type`; the remaining
/// fields are present only for the event types that carry them.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AutoloopEvent {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "runId", default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub iteration: Option<u32>,
    /// Present on `loop.finish` / `summary`.
    #[serde(default)]
    pub iterations: Option<u32>,
    #[serde(rename = "stopReason", default)]
    pub stop_reason: Option<String>,
    /// Cumulative run cost in USD; present on `loop.finish` / `summary`.
    #[serde(rename = "costUsd", default)]
    pub cost_usd: Option<f64>,
    /// Present on `progress`.
    #[serde(rename = "recentEvent", default)]
    pub recent_event: Option<String>,
    #[serde(rename = "emittedTopic", default)]
    pub emitted_topic: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    /// Present on `ask.pending` / `ask.answered`.
    #[serde(rename = "questionId", default)]
    pub question_id: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub answer: Option<String>,
}

impl AutoloopEvent {
    /// The run is paused on a human question (HITL).
    pub fn ask_pending(&self) -> Option<PendingAsk> {
        if self.kind != "ask.pending" {
            return None;
        }
        Some(PendingAsk {
            run_id: self.run_id.clone()?,
            question_id: self.question_id.clone()?,
            question: self.question.clone().unwrap_or_default(),
        })
    }

    /// The terminal run result, from `loop.finish` (or `summary`).
    pub fn run_result(&self) -> Option<RunResult> {
        if self.kind != "loop.finish" && self.kind != "summary" {
            return None;
        }
        Some(RunResult {
            run_id: self.run_id.clone()?,
            iterations: self.iterations?,
            stop_reason: self.stop_reason.clone()?,
            // Older autoloop builds omit costUsd; default to 0.0.
            cost_usd: self.cost_usd.unwrap_or(0.0),
        })
    }
}

/// A human question the loop is blocked on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAsk {
    pub run_id: String,
    pub question_id: String,
    pub question: String,
}

/// The machine-readable run result carried by the terminal event.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub run_id: String,
    pub iterations: u32,
    pub stop_reason: String,
    /// Cumulative run cost in USD (`0.0` if the backend reported no usage or the
    /// running autoloop predates the `costUsd` event field).
    pub cost_usd: f64,
}

/// Parse a `--events` NDJSON stream. Complete lines that fail to decode are
/// skipped (forward-compatible); a partial trailing line is ignored.
pub fn parse_events(content: &str) -> Vec<AutoloopEvent> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(ev) = serde_json::from_str::<AutoloopEvent>(trimmed) {
            out.push(ev);
        }
    }
    out
}

/// The first pending human ask in the stream, if any.
pub fn first_pending_ask(events: &[AutoloopEvent]) -> Option<PendingAsk> {
    events.iter().find_map(AutoloopEvent::ask_pending)
}

/// The terminal run result in the stream, if present (last wins).
pub fn run_result(events: &[AutoloopEvent]) -> Option<RunResult> {
    events.iter().rev().find_map(AutoloopEvent::run_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r1"}"#,
        "\n",
        r#"{"type":"progress","runId":"r1","iteration":1,"recentEvent":"loop.start","allowedRoles":["planner"],"emittedTopic":"tasks.ready","outcome":"continue:routed_event"}"#,
        "\n",
        r#"{"type":"summary","runId":"r1","iterations":2,"stopReason":"max_iterations","journalFile":"/j","memoryFile":"/m","reviewEvery":10,"toolPath":"/t"}"#,
        "\n",
        r#"{"type":"loop.finish","iterations":2,"stopReason":"max_iterations","runId":"r1","costUsd":0.08}"#,
        "\n",
    );

    #[test]
    fn parses_known_events_and_ignores_unknown_fields() {
        let events = parse_events(SAMPLE);
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].kind, "iteration.start");
        let progress = &events[1];
        assert_eq!(progress.kind, "progress");
        assert_eq!(progress.emitted_topic.as_deref(), Some("tasks.ready"));
        assert_eq!(progress.outcome.as_deref(), Some("continue:routed_event"));
    }

    #[test]
    fn derives_the_machine_readable_run_result() {
        let result = run_result(&parse_events(SAMPLE)).unwrap();
        assert_eq!(result.run_id, "r1");
        assert_eq!(result.iterations, 2);
        assert_eq!(result.stop_reason, "max_iterations");
        assert_eq!(result.cost_usd, 0.08);
    }

    #[test]
    fn detects_a_pending_human_ask() {
        let content = concat!(
            r#"{"type":"iteration.start","iteration":1,"maxIterations":3,"runId":"r9"}"#,
            "\n",
            r#"{"type":"ask.pending","runId":"r9","iteration":1,"questionId":"ask_r9_1","question":"A or B?"}"#,
            "\n",
        );
        let events = parse_events(content);
        let ask = first_pending_ask(&events).unwrap();
        assert_eq!(ask.run_id, "r9");
        assert_eq!(ask.question_id, "ask_r9_1");
        assert_eq!(ask.question, "A or B?");
        // No terminal result yet — the run is still blocked.
        assert!(run_result(&events).is_none());
    }

    #[test]
    fn skips_malformed_and_partial_lines() {
        let content = concat!(
            r#"{"type":"iteration.start","iteration":1,"maxIterations":1,"runId":"r1"}"#,
            "\n",
            "{ not json }\n",
            r#"{"type":"loop.finish","iterations":1,"stopReason":"completed","runId":"r1"}"#,
            "\n",
            r#"{"type":"ask.pending","runId":"r1""#, // truncated, no newline
        );
        let events = parse_events(content);
        // The two valid complete lines decode; malformed + partial are skipped.
        assert_eq!(events.len(), 2);
        assert_eq!(run_result(&events).unwrap().stop_reason, "completed");
    }
}
