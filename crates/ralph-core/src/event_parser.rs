//! Event parsing from CLI output.
//!
//! Parses XML-style event tags from agent output:
//! ```text
//! <event topic="impl.done">payload</event>
//! <event topic="handoff" target="reviewer">payload</event>
//! ```

use ralph_proto::{Event, HatId};

/// Strips ANSI escape sequences from a string.
///
/// Handles CSI sequences (\x1b[...m), OSC sequences (\x1b]...\x07),
/// and simple escape sequences (\x1b followed by a single char).
fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            // ESC character - start of escape sequence
            i += 1;
            if i >= bytes.len() {
                break;
            }

            match bytes[i] {
                b'[' => {
                    // CSI sequence: ESC [ ... (final byte in 0x40-0x7E range)
                    i += 1;
                    while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1; // Skip final byte
                    }
                }
                b']' => {
                    // OSC sequence: ESC ] ... (terminated by BEL or ST)
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                _ => {
                    // Simple escape sequence: ESC + single char
                    i += 1;
                }
            }
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    String::from_utf8_lossy(&result).into_owned()
}

/// Evidence of backpressure checks for build.done events.
#[derive(Debug, Clone, PartialEq)]
pub struct BackpressureEvidence {
    pub tests_passed: bool,
    pub lint_passed: bool,
    pub typecheck_passed: bool,
    pub audit_passed: bool,
    pub coverage_passed: bool,
    pub complexity_score: Option<f64>,
    pub duplication_passed: bool,
    pub performance_regression: Option<bool>,
    pub mutants: Option<MutationEvidence>,
    /// Whether spec acceptance criteria have been verified against passing tests.
    ///
    /// `None` means specs evidence was not included in the payload (optional gate).
    /// `Some(true)` means all spec criteria are satisfied.
    /// `Some(false)` means some spec criteria are unsatisfied — blocks build.done.
    pub specs_verified: Option<bool>,
}

impl BackpressureEvidence {
    /// Returns true if all required checks passed.
    ///
    /// Mutation testing evidence is warning-only and does not affect this result.
    /// Spec verification blocks when explicitly reported as failed (`Some(false)`),
    /// but is optional — omitting it (`None`) does not block.
    pub fn all_passed(&self) -> bool {
        self.tests_passed
            && self.lint_passed
            && self.typecheck_passed
            && self.audit_passed
            && self.coverage_passed
            && self
                .complexity_score
                .is_some_and(|value| value <= QualityReport::COMPLEXITY_THRESHOLD)
            && self.duplication_passed
            && !matches!(self.performance_regression, Some(true))
            && !matches!(self.specs_verified, Some(false))
    }
}

/// Status of mutation testing evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationStatus {
    Pass,
    Warn,
    Fail,
    Unknown,
}

/// Evidence of mutation testing for build.done payloads.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationEvidence {
    pub status: MutationStatus,
    pub score_percent: Option<f64>,
}

/// Evidence of verification for review.done events.
///
/// Enforces that review hats actually ran verification commands rather
/// than just asserting "looks good". At minimum, tests must have been run.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewEvidence {
    pub tests_passed: bool,
    pub build_passed: bool,
}

impl ReviewEvidence {
    /// Returns true if the review has sufficient verification.
    ///
    /// Both tests and build must pass to constitute a verified review.
    pub fn is_verified(&self) -> bool {
        self.tests_passed && self.build_passed
    }
}

/// Structured quality report for verifier events.
#[derive(Debug, Clone, PartialEq)]
pub struct QualityReport {
    pub tests_passed: Option<bool>,
    pub lint_passed: Option<bool>,
    pub audit_passed: Option<bool>,
    pub coverage_percent: Option<f64>,
    pub mutation_percent: Option<f64>,
    pub complexity_score: Option<f64>,
    /// Whether spec acceptance criteria are satisfied by passing tests.
    ///
    /// `None` means not reported (optional — does not fail thresholds).
    /// `Some(false)` means spec criteria are unsatisfied — fails thresholds.
    pub specs_verified: Option<bool>,
}

impl QualityReport {
    pub const COVERAGE_THRESHOLD: f64 = 80.0;
    pub const MUTATION_THRESHOLD: f64 = 70.0;
    pub const COMPLEXITY_THRESHOLD: f64 = 10.0;

    pub fn meets_thresholds(&self) -> bool {
        self.tests_passed == Some(true)
            && self.lint_passed == Some(true)
            && self.audit_passed == Some(true)
            && self
                .coverage_percent
                .is_some_and(|value| value >= Self::COVERAGE_THRESHOLD)
            && self
                .mutation_percent
                .is_some_and(|value| value >= Self::MUTATION_THRESHOLD)
            && self
                .complexity_score
                .is_some_and(|value| value <= Self::COMPLEXITY_THRESHOLD)
            && !matches!(self.specs_verified, Some(false))
    }

    pub fn failed_dimensions(&self) -> Vec<&'static str> {
        let mut failed = Vec::new();

        if self.tests_passed != Some(true) {
            failed.push("tests");
        }
        if self.lint_passed != Some(true) {
            failed.push("lint");
        }
        if self.audit_passed != Some(true) {
            failed.push("audit");
        }
        if self
            .coverage_percent
            .is_none_or(|value| value < Self::COVERAGE_THRESHOLD)
        {
            failed.push("coverage");
        }
        if self
            .mutation_percent
            .is_none_or(|value| value < Self::MUTATION_THRESHOLD)
        {
            failed.push("mutation");
        }
        if self
            .complexity_score
            .is_none_or(|value| value > Self::COMPLEXITY_THRESHOLD)
        {
            failed.push("complexity");
        }
        if matches!(self.specs_verified, Some(false)) {
            failed.push("specs");
        }

        failed
    }
}

/// Parser for extracting events from CLI output.
#[derive(Debug, Default)]
pub struct EventParser {
    /// The source hat ID to attach to parsed events.
    source: Option<HatId>,
}

impl EventParser {
    /// Creates a new event parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the source hat for parsed events.
    pub fn with_source(mut self, source: impl Into<HatId>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Parses events from CLI output text.
    ///
    /// Returns a list of parsed events.
    pub fn parse(&self, output: &str) -> Vec<Event> {
        let mut events = Vec::new();
        let mut remaining = output;

        while let Some(start_idx) = remaining.find("<event ") {
            let after_start = &remaining[start_idx..];

            // Find the end of the opening tag
            let Some(tag_end) = after_start.find('>') else {
                remaining = &remaining[start_idx + 7..];
                continue;
            };

            let opening_tag = &after_start[..tag_end + 1];

            // Parse attributes from opening tag
            let topic = Self::extract_attr(opening_tag, "topic");
            let target = Self::extract_attr(opening_tag, "target");

            let Some(topic) = topic else {
                remaining = &remaining[start_idx + tag_end + 1..];
                continue;
            };

            // Find the closing tag
            let content_start = &after_start[tag_end + 1..];
            let Some(close_idx) = content_start.find("</event>") else {
                remaining = &remaining[start_idx + tag_end + 1..];
                continue;
            };

            let payload = content_start[..close_idx].trim().to_string();

            let mut event = Event::new(topic, payload);

            if let Some(source) = &self.source {
                event = event.with_source(source.clone());
            }

            if let Some(target) = target {
                event = event.with_target(target);
            }

            events.push(event);

            // Move past this event
            let total_consumed = start_idx + tag_end + 1 + close_idx + 8; // 8 = "</event>".len()
            remaining = &remaining[total_consumed..];
        }

        events
    }

    /// Extracts an attribute value from an XML-like tag.
    fn extract_attr(tag: &str, attr: &str) -> Option<String> {
        let pattern = format!("{attr}=\"");
        let start = tag.find(&pattern)?;
        let value_start = start + pattern.len();
        let rest = &tag[value_start..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    /// Parses backpressure evidence from build.done event payload.
    ///
    /// Expected format:
    /// ```text
    /// tests: pass
    /// lint: pass
    /// typecheck: pass
    /// audit: pass
    /// coverage: pass
    /// complexity: 7           # required (<=10)
    /// duplication: pass       # required
    /// performance: pass       # optional (regression blocks)
    /// mutants: pass (82%)   # optional, warning-only
    /// specs: pass            # optional (fail blocks)
    /// ```
    ///
    /// Note: ANSI escape codes are stripped before parsing to handle
    /// colorized CLI output.
    pub fn parse_backpressure_evidence(payload: &str) -> Option<BackpressureEvidence> {
        // Strip ANSI codes before checking for evidence strings
        let clean_payload = strip_ansi(payload);

        let tests_passed = clean_payload.contains("tests: pass");
        let lint_passed = clean_payload.contains("lint: pass");
        let typecheck_passed = clean_payload.contains("typecheck: pass");
        let audit_passed = clean_payload.contains("audit: pass");
        let coverage_passed = clean_payload.contains("coverage: pass");
        let complexity_score = Self::parse_complexity_evidence(&clean_payload);
        let duplication_passed = Self::parse_duplication_evidence(&clean_payload).unwrap_or(false);
        let performance_regression = Self::parse_performance_regression(&clean_payload);
        let mutants = Self::parse_mutation_evidence(&clean_payload);
        let specs_verified = Self::parse_specs_evidence(&clean_payload);

        // Only return evidence if at least one check is mentioned
        if clean_payload.contains("tests:")
            || clean_payload.contains("lint:")
            || clean_payload.contains("typecheck:")
            || clean_payload.contains("audit:")
            || clean_payload.contains("coverage:")
            || clean_payload.contains("complexity:")
            || clean_payload.contains("duplication:")
            || clean_payload.contains("performance:")
            || clean_payload.contains("perf:")
            || clean_payload.contains("mutants:")
            || clean_payload.contains("specs:")
        {
            Some(BackpressureEvidence {
                tests_passed,
                lint_passed,
                typecheck_passed,
                audit_passed,
                coverage_passed,
                complexity_score,
                duplication_passed,
                performance_regression,
                mutants,
                specs_verified,
            })
        } else {
            None
        }
    }

    fn parse_mutation_evidence(clean_payload: &str) -> Option<MutationEvidence> {
        let segment = clean_payload
            .split(|c| c == '\n' || c == ',')
            .map(str::trim)
            .find(|segment| segment.contains("mutants:"))?;

        let normalized = segment.to_lowercase();
        let status = if normalized.contains("mutants: pass") {
            MutationStatus::Pass
        } else if normalized.contains("mutants: warn") {
            MutationStatus::Warn
        } else if normalized.contains("mutants: fail") {
            MutationStatus::Fail
        } else {
            MutationStatus::Unknown
        };

        let score_percent = Self::extract_percentage(segment);

        Some(MutationEvidence {
            status,
            score_percent,
        })
    }

    fn parse_complexity_evidence(clean_payload: &str) -> Option<f64> {
        let segment = clean_payload
            .split(|c| c == '\n' || c == ',')
            .map(str::trim)
            .find(|segment| segment.to_lowercase().starts_with("complexity:"))?;

        Self::extract_first_number(segment)
    }

    fn parse_duplication_evidence(clean_payload: &str) -> Option<bool> {
        let segment = clean_payload
            .split(|c| c == '\n' || c == ',')
            .map(str::trim)
            .find(|segment| segment.to_lowercase().starts_with("duplication:"))?;

        let normalized = segment.to_lowercase();
        if normalized.contains("duplication: pass") {
            Some(true)
        } else if normalized.contains("duplication: fail") {
            Some(false)
        } else {
            None
        }
    }

    fn parse_performance_regression(clean_payload: &str) -> Option<bool> {
        let segment = clean_payload
            .split(|c| c == '\n' || c == ',')
            .map(str::trim)
            .find(|segment| {
                let normalized = segment.to_lowercase();
                normalized.starts_with("performance:") || normalized.starts_with("perf:")
            })?;

        let normalized = segment.to_lowercase();
        if normalized.contains("regression") || normalized.contains("fail") {
            Some(true)
        } else if normalized.contains("pass")
            || normalized.contains("ok")
            || normalized.contains("improved")
        {
            Some(false)
        } else {
            None
        }
    }

    /// Parses spec acceptance criteria verification evidence.
    ///
    /// Returns `Some(true)` for `specs: pass`, `Some(false)` for `specs: fail`,
    /// and `None` if no specs evidence is present.
    fn parse_specs_evidence(clean_payload: &str) -> Option<bool> {
        let segment = clean_payload
            .split(|c| c == '\n' || c == ',')
            .map(str::trim)
            .find(|segment| segment.to_lowercase().starts_with("specs:"))?;

        let normalized = segment.to_lowercase();
        if normalized.contains("specs: pass") {
            Some(true)
        } else if normalized.contains("specs: fail") {
            Some(false)
        } else {
            None
        }
    }

    fn extract_percentage(segment: &str) -> Option<f64> {
        let percent_idx = segment.find('%')?;
        let bytes = segment.as_bytes();
        let mut start = percent_idx;

        while start > 0 {
            let prev = bytes[start - 1];
            if prev.is_ascii_digit() || prev == b'.' {
                start -= 1;
            } else {
                break;
            }
        }

        if start == percent_idx {
            return None;
        }

        segment[start..percent_idx].trim().parse::<f64>().ok()
    }

    fn extract_first_number(segment: &str) -> Option<f64> {
        let bytes = segment.as_bytes();
        let mut start = None;
        let mut end = None;

        for (idx, &byte) in bytes.iter().enumerate() {
            if byte.is_ascii_digit() {
                if start.is_none() {
                    start = Some(idx);
                }
                end = Some(idx + 1);
            } else if byte == b'.' && start.is_some() {
                end = Some(idx + 1);
            } else if start.is_some() {
                break;
            }
        }

        let start = start?;
        let end = end?;
        segment[start..end].trim().parse::<f64>().ok()
    }

    fn parse_quality_pass_fail(segment: &str) -> Option<bool> {
        if segment.contains("pass") {
            Some(true)
        } else if segment.contains("fail") {
            Some(false)
        } else {
            None
        }
    }

    /// Parses review evidence from review.done event payload.
    ///
    /// Expected format (subset of backpressure evidence):
    /// ```text
    /// tests: pass
    /// build: pass
    /// ```
    ///
    /// Note: ANSI escape codes are stripped before parsing.
    pub fn parse_review_evidence(payload: &str) -> Option<ReviewEvidence> {
        let clean_payload = strip_ansi(payload);

        let tests_passed = clean_payload.contains("tests: pass");
        let build_passed = clean_payload.contains("build: pass");

        // Only return evidence if at least one check is mentioned
        if clean_payload.contains("tests:") || clean_payload.contains("build:") {
            Some(ReviewEvidence {
                tests_passed,
                build_passed,
            })
        } else {
            None
        }
    }

    /// Parses quality report evidence from verify.* event payloads.
    ///
    /// Expected format:
    /// ```text
    /// quality.tests: pass
    /// quality.coverage: 82%
    /// quality.lint: pass
    /// quality.audit: pass
    /// quality.mutation: 71%
    /// quality.complexity: 7
    /// quality.specs: pass         # optional (fail blocks)
    /// ```
    ///
    /// Note: ANSI escape codes are stripped before parsing.
    pub fn parse_quality_report(payload: &str) -> Option<QualityReport> {
        let clean_payload = strip_ansi(payload);
        let mut report = QualityReport {
            tests_passed: None,
            lint_passed: None,
            audit_passed: None,
            coverage_percent: None,
            mutation_percent: None,
            complexity_score: None,
            specs_verified: None,
        };
        let mut seen = false;

        for segment in clean_payload
            .split(|c| c == '\n' || c == ',')
            .map(str::trim)
        {
            if segment.is_empty() {
                continue;
            }
            let normalized = segment.to_lowercase();

            if normalized.starts_with("quality.tests:") {
                report.tests_passed = Self::parse_quality_pass_fail(&normalized);
                seen = true;
            } else if normalized.starts_with("quality.lint:") {
                report.lint_passed = Self::parse_quality_pass_fail(&normalized);
                seen = true;
            } else if normalized.starts_with("quality.audit:") {
                report.audit_passed = Self::parse_quality_pass_fail(&normalized);
                seen = true;
            } else if normalized.starts_with("quality.coverage:") {
                report.coverage_percent = Self::extract_percentage(segment)
                    .or_else(|| Self::extract_first_number(segment));
                seen = true;
            } else if normalized.starts_with("quality.mutation:") {
                report.mutation_percent = Self::extract_percentage(segment)
                    .or_else(|| Self::extract_first_number(segment));
                seen = true;
            } else if normalized.starts_with("quality.complexity:") {
                report.complexity_score = Self::extract_first_number(segment);
                seen = true;
            } else if normalized.starts_with("quality.specs:") {
                report.specs_verified = Self::parse_quality_pass_fail(&normalized);
                seen = true;
            }
        }

        if seen { Some(report) } else { None }
    }

    /// Checks if output contains the completion promise.
    ///
    /// Per spec: The promise must appear in the agent's final output,
    /// not inside an `<event>` tag payload. This function:
    /// 1. Returns false if the promise appears inside ANY event tag
    ///    (prevents accidental completion when agents discuss the promise)
    /// 2. Otherwise, checks that the promise is the final non-empty line
    ///    in the stripped output (prevents prompt echo false positives)
    pub fn contains_promise(output: &str, promise: &str) -> bool {
        let promise = promise.trim();
        if promise.is_empty() {
            return false;
        }

        // Safety check: if promise appears inside any event tag, never complete
        if Self::promise_in_event_tags(output, promise) {
            return false;
        }
        let stripped = Self::strip_event_tags(output);

        for line in stripped.lines().rev() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            return trimmed == promise;
        }

        false
    }

    /// Checks if the promise appears inside any event tag payload.
    pub fn promise_in_event_tags(output: &str, promise: &str) -> bool {
        let mut remaining = output;

        while let Some(start_idx) = remaining.find("<event ") {
            let after_start = &remaining[start_idx..];

            // Find the end of the opening tag
            let Some(tag_end) = after_start.find('>') else {
                remaining = &remaining[start_idx + 7..];
                continue;
            };

            // Find the closing tag
            let content_start = &after_start[tag_end + 1..];
            let Some(close_idx) = content_start.find("</event>") else {
                remaining = &remaining[start_idx + tag_end + 1..];
                continue;
            };

            let payload = &content_start[..close_idx];
            if payload.contains(promise) {
                return true;
            }

            // Move past this event
            let total_consumed = start_idx + tag_end + 1 + close_idx + 8;
            remaining = &remaining[total_consumed..];
        }

        false
    }

    /// Strips all `<event ...>...</event>` blocks from output.
    ///
    /// Returns the output with event tags removed, leaving only
    /// the "final output" text that should be checked for promises.
    fn strip_event_tags(output: &str) -> String {
        let mut result = String::with_capacity(output.len());
        let mut remaining = output;

        while let Some(start_idx) = remaining.find("<event ") {
            // Add everything before this event tag
            result.push_str(&remaining[..start_idx]);

            let after_start = &remaining[start_idx..];

            // Find the closing tag
            if let Some(close_idx) = after_start.find("</event>") {
                // Skip past the entire event block
                remaining = &after_start[close_idx + 8..]; // 8 = "</event>".len()
            } else {
                // Malformed: no closing tag, keep the rest and stop
                result.push_str(after_start);
                remaining = "";
                break;
            }
        }

        // Add any remaining content after the last event
        result.push_str(remaining);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_event() {
        let output = r#"
Some preamble text.
<event topic="impl.done">
Implemented the authentication module.
</event>
Some trailing text.
"#;
        let parser = EventParser::new();
        let events = parser.parse(output);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].topic.as_str(), "impl.done");
        assert!(events[0].payload.contains("authentication module"));
    }

    #[test]
    fn test_parse_event_with_target() {
        let output = r#"<event topic="handoff" target="reviewer">Please review</event>"#;
        let parser = EventParser::new();
        let events = parser.parse(output);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target.as_ref().unwrap().as_str(), "reviewer");
    }

    #[test]
    fn test_parse_multiple_events() {
        let output = r#"
<event topic="impl.started">Starting work</event>
Working on implementation...
<event topic="impl.done">Finished</event>
"#;
        let parser = EventParser::new();
        let events = parser.parse(output);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].topic.as_str(), "impl.started");
        assert_eq!(events[1].topic.as_str(), "impl.done");
    }

    #[test]
    fn test_parse_with_source() {
        let output = r#"<event topic="impl.done">Done</event>"#;
        let parser = EventParser::new().with_source("implementer");
        let events = parser.parse(output);

        assert_eq!(events[0].source.as_ref().unwrap().as_str(), "implementer");
    }

    #[test]
    fn test_no_events() {
        let output = "Just regular output with no events.";
        let parser = EventParser::new();
        let events = parser.parse(output);

        assert!(events.is_empty());
    }

    #[test]
    fn test_contains_promise_requires_last_line() {
        assert!(EventParser::contains_promise(
            "LOOP_COMPLETE",
            "LOOP_COMPLETE"
        ));
        assert!(EventParser::contains_promise(
            "All done!\nLOOP_COMPLETE",
            "LOOP_COMPLETE"
        ));
        assert!(EventParser::contains_promise(
            "LOOP_COMPLETE   \n\n",
            "LOOP_COMPLETE"
        ));
        assert!(!EventParser::contains_promise(
            "prefix LOOP_COMPLETE suffix",
            "LOOP_COMPLETE"
        ));
        assert!(!EventParser::contains_promise(
            "LOOP_COMPLETE\nMore text",
            "LOOP_COMPLETE"
        ));
        assert!(!EventParser::contains_promise("Any output", "   "));
        assert!(!EventParser::contains_promise(
            "No promise here",
            "LOOP_COMPLETE"
        ));
    }

    #[test]
    fn test_contains_promise_ignores_event_payloads() {
        // Promise inside event payload should NOT be detected
        let output = r#"<event topic="build.task">Fix LOOP_COMPLETE detection</event>"#;
        assert!(!EventParser::contains_promise(output, "LOOP_COMPLETE"));

        // Promise inside event with acceptance criteria mentioning LOOP_COMPLETE
        let output = r#"<event topic="build.task">
## Task: Fix completion promise detection
- Given LOOP_COMPLETE appears inside an event tag
- Then it should be ignored
</event>"#;
        assert!(!EventParser::contains_promise(output, "LOOP_COMPLETE"));
    }

    #[test]
    fn test_contains_promise_detects_outside_events() {
        // Promise outside event tags should be detected
        let output = r#"<event topic="build.done">Task complete</event>
All done!
LOOP_COMPLETE"#;
        assert!(EventParser::contains_promise(output, "LOOP_COMPLETE"));

        // Promise before event tags
        let output = r#"LOOP_COMPLETE
<event topic="summary">Final summary</event>"#;
        assert!(EventParser::contains_promise(output, "LOOP_COMPLETE"));
    }

    #[test]
    fn test_contains_promise_mixed_content() {
        // Promise only in event payload, not in surrounding text
        let output = r#"Working on task...
<event topic="build.task">Fix LOOP_COMPLETE bug</event>
Still working..."#;
        assert!(!EventParser::contains_promise(output, "LOOP_COMPLETE"));

        // Promise in both event and surrounding text - should NOT complete
        // because promise appears inside an event tag (safety mechanism)
        let output = r#"All tasks done. LOOP_COMPLETE
<event topic="summary">Completed LOOP_COMPLETE task</event>"#;
        assert!(!EventParser::contains_promise(output, "LOOP_COMPLETE"));
    }

    #[test]
    fn test_promise_in_event_tags() {
        // Promise inside event payload
        let output = r#"<event topic="build.task">Fix LOOP_COMPLETE bug</event>"#;
        assert!(EventParser::promise_in_event_tags(output, "LOOP_COMPLETE"));

        // Promise not in any event
        let output = r#"<event topic="build.done">Task complete</event>"#;
        assert!(!EventParser::promise_in_event_tags(output, "LOOP_COMPLETE"));

        // No events at all
        let output = "Just regular text with LOOP_COMPLETE";
        assert!(!EventParser::promise_in_event_tags(output, "LOOP_COMPLETE"));

        // Multiple events, promise in second
        let output = r#"<event topic="a">first</event>
<event topic="b">contains LOOP_COMPLETE</event>"#;
        assert!(EventParser::promise_in_event_tags(output, "LOOP_COMPLETE"));
    }

    #[test]
    fn test_strip_event_tags() {
        // Single event
        let output = r#"before <event topic="test">payload</event> after"#;
        let stripped = EventParser::strip_event_tags(output);
        assert_eq!(stripped, "before  after");
        assert!(!stripped.contains("payload"));

        // Multiple events
        let output =
            r#"start <event topic="a">one</event> middle <event topic="b">two</event> end"#;
        let stripped = EventParser::strip_event_tags(output);
        assert_eq!(stripped, "start  middle  end");

        // No events
        let output = "just plain text";
        let stripped = EventParser::strip_event_tags(output);
        assert_eq!(stripped, "just plain text");
    }

    #[test]
    fn test_parse_backpressure_evidence_all_pass() {
        let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: pass";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert!(evidence.tests_passed);
        assert!(evidence.lint_passed);
        assert!(evidence.typecheck_passed);
        assert!(evidence.audit_passed);
        assert!(evidence.coverage_passed);
        assert_eq!(evidence.complexity_score, Some(7.0));
        assert!(evidence.duplication_passed);
        assert_eq!(evidence.performance_regression, Some(false));
        assert!(evidence.all_passed());
    }

    #[test]
    fn test_parse_backpressure_evidence_some_fail() {
        let payload = "tests: pass\nlint: fail\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: pass";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert!(evidence.tests_passed);
        assert!(!evidence.lint_passed);
        assert!(evidence.typecheck_passed);
        assert!(evidence.audit_passed);
        assert!(evidence.coverage_passed);
        assert_eq!(evidence.complexity_score, Some(7.0));
        assert!(evidence.duplication_passed);
        assert_eq!(evidence.performance_regression, Some(false));
        assert!(!evidence.all_passed());
    }

    #[test]
    fn test_parse_backpressure_evidence_missing() {
        let payload = "Task completed successfully";
        let evidence = EventParser::parse_backpressure_evidence(payload);
        assert!(evidence.is_none());
    }

    #[test]
    fn test_parse_backpressure_evidence_partial() {
        let payload = "tests: pass\nSome other text";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert!(evidence.tests_passed);
        assert!(!evidence.lint_passed);
        assert!(!evidence.typecheck_passed);
        assert!(!evidence.audit_passed);
        assert!(!evidence.coverage_passed);
        assert!(evidence.complexity_score.is_none());
        assert!(!evidence.duplication_passed);
        assert!(evidence.performance_regression.is_none());
        assert!(!evidence.all_passed());
    }

    #[test]
    fn test_parse_backpressure_evidence_with_ansi_codes() {
        let payload = "\x1b[0mtests: pass\x1b[0m\n\x1b[32mlint: pass\x1b[0m\ntypecheck: pass\n\x1b[34maudit: pass\x1b[0m\n\x1b[35mcoverage: pass\x1b[0m\n\x1b[36mcomplexity: 7\x1b[0m\n\x1b[31mduplication: pass\x1b[0m\n\x1b[33mperformance: pass\x1b[0m";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert!(evidence.tests_passed);
        assert!(evidence.lint_passed);
        assert!(evidence.typecheck_passed);
        assert!(evidence.audit_passed);
        assert!(evidence.coverage_passed);
        assert_eq!(evidence.complexity_score, Some(7.0));
        assert!(evidence.duplication_passed);
        assert_eq!(evidence.performance_regression, Some(false));
        assert!(evidence.all_passed());
    }

    #[test]
    fn test_parse_backpressure_evidence_with_mutants_pass() {
        let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: pass\nmutants: pass (82%)";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        let mutants = evidence
            .mutants
            .as_ref()
            .expect("mutants evidence should parse");
        assert_eq!(mutants.status, MutationStatus::Pass);
        assert_eq!(mutants.score_percent, Some(82.0));
        assert_eq!(evidence.performance_regression, Some(false));
        assert!(evidence.all_passed());
    }

    #[test]
    fn test_parse_backpressure_evidence_with_mutants_warn() {
        let payload = "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass, complexity: 7, duplication: pass, performance: pass, mutants: warn (65%)";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        let mutants = evidence
            .mutants
            .as_ref()
            .expect("mutants evidence should parse");
        assert_eq!(mutants.status, MutationStatus::Warn);
        assert_eq!(mutants.score_percent, Some(65.0));
        assert_eq!(evidence.performance_regression, Some(false));
        assert!(evidence.all_passed());
    }

    #[test]
    fn test_parse_backpressure_evidence_with_performance_regression() {
        let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: regression";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert_eq!(evidence.performance_regression, Some(true));
        assert!(!evidence.all_passed());
    }

    #[test]
    fn test_parse_review_evidence_all_pass() {
        let payload = "tests: pass\nbuild: pass";
        let evidence = EventParser::parse_review_evidence(payload).unwrap();
        assert!(evidence.tests_passed);
        assert!(evidence.build_passed);
        assert!(evidence.is_verified());
    }

    #[test]
    fn test_parse_review_evidence_tests_fail() {
        let payload = "tests: fail\nbuild: pass";
        let evidence = EventParser::parse_review_evidence(payload).unwrap();
        assert!(!evidence.tests_passed);
        assert!(evidence.build_passed);
        assert!(!evidence.is_verified());
    }

    #[test]
    fn test_parse_review_evidence_build_fail() {
        let payload = "tests: pass\nbuild: fail";
        let evidence = EventParser::parse_review_evidence(payload).unwrap();
        assert!(evidence.tests_passed);
        assert!(!evidence.build_passed);
        assert!(!evidence.is_verified());
    }

    #[test]
    fn test_parse_review_evidence_missing() {
        let payload = "Looks good, approved!";
        let evidence = EventParser::parse_review_evidence(payload);
        assert!(evidence.is_none());
    }

    #[test]
    fn test_parse_review_evidence_partial() {
        let payload = "tests: pass\nLGTM";
        let evidence = EventParser::parse_review_evidence(payload).unwrap();
        assert!(evidence.tests_passed);
        assert!(!evidence.build_passed);
        assert!(!evidence.is_verified());
    }

    #[test]
    fn test_parse_review_evidence_with_ansi_codes() {
        let payload = "\x1b[32mtests: pass\x1b[0m\n\x1b[32mbuild: pass\x1b[0m";
        let evidence = EventParser::parse_review_evidence(payload).unwrap();
        assert!(evidence.tests_passed);
        assert!(evidence.build_passed);
        assert!(evidence.is_verified());
    }

    #[test]
    fn test_parse_quality_report_passes_thresholds() {
        let payload = "quality.tests: pass\nquality.coverage: 82% (>=80%)\nquality.lint: pass\nquality.audit: pass\nquality.mutation: 71% (>=70%)\nquality.complexity: 7 (<=10)";
        let report = EventParser::parse_quality_report(payload).unwrap();
        assert_eq!(report.tests_passed, Some(true));
        assert_eq!(report.lint_passed, Some(true));
        assert_eq!(report.audit_passed, Some(true));
        assert_eq!(report.coverage_percent, Some(82.0));
        assert_eq!(report.mutation_percent, Some(71.0));
        assert_eq!(report.complexity_score, Some(7.0));
        assert!(report.meets_thresholds());
    }

    #[test]
    fn test_parse_quality_report_fails_thresholds() {
        let payload = "quality.tests: pass\nquality.coverage: 60%\nquality.lint: fail\nquality.audit: pass\nquality.mutation: 50%\nquality.complexity: 12";
        let report = EventParser::parse_quality_report(payload).unwrap();
        assert!(!report.meets_thresholds());
    }

    #[test]
    fn test_parse_quality_report_missing() {
        let payload = "Looks good, approved!";
        let report = EventParser::parse_quality_report(payload);
        assert!(report.is_none());
    }

    #[test]
    fn test_extract_first_number_quality_line() {
        let value = EventParser::extract_first_number("quality.complexity: 7 (<=10)");
        assert_eq!(value, Some(7.0));
    }

    #[test]
    fn test_parse_backpressure_evidence_with_specs_pass() {
        let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: pass\nspecs: pass";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert_eq!(evidence.specs_verified, Some(true));
        assert!(evidence.all_passed());
    }

    #[test]
    fn test_parse_backpressure_evidence_with_specs_fail() {
        let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: pass\nspecs: fail";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert_eq!(evidence.specs_verified, Some(false));
        assert!(
            !evidence.all_passed(),
            "specs: fail should block build.done"
        );
    }

    #[test]
    fn test_parse_backpressure_evidence_specs_omitted_does_not_block() {
        // When specs evidence is not included, it should not block
        let payload = "tests: pass\nlint: pass\ntypecheck: pass\naudit: pass\ncoverage: pass\ncomplexity: 7\nduplication: pass\nperformance: pass";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert_eq!(evidence.specs_verified, None);
        assert!(
            evidence.all_passed(),
            "missing specs should not block build.done"
        );
    }

    #[test]
    fn test_parse_backpressure_evidence_specs_comma_separated() {
        let payload = "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass, complexity: 7, duplication: pass, performance: pass, specs: pass";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert_eq!(evidence.specs_verified, Some(true));
        assert!(evidence.all_passed());
    }

    #[test]
    fn test_parse_specs_evidence_only() {
        // specs: alone should be recognized as evidence
        let payload = "specs: pass";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert_eq!(evidence.specs_verified, Some(true));
    }

    #[test]
    fn test_quality_report_with_specs_pass() {
        let payload = "quality.tests: pass\nquality.coverage: 82%\nquality.lint: pass\nquality.audit: pass\nquality.mutation: 71%\nquality.complexity: 7\nquality.specs: pass";
        let report = EventParser::parse_quality_report(payload).unwrap();
        assert_eq!(report.specs_verified, Some(true));
        assert!(report.meets_thresholds());
    }

    #[test]
    fn test_quality_report_with_specs_fail() {
        let payload = "quality.tests: pass\nquality.coverage: 82%\nquality.lint: pass\nquality.audit: pass\nquality.mutation: 71%\nquality.complexity: 7\nquality.specs: fail";
        let report = EventParser::parse_quality_report(payload).unwrap();
        assert_eq!(report.specs_verified, Some(false));
        assert!(
            !report.meets_thresholds(),
            "specs: fail should fail quality thresholds"
        );
        assert!(report.failed_dimensions().contains(&"specs"));
    }

    #[test]
    fn test_quality_report_specs_omitted_passes() {
        let payload = "quality.tests: pass\nquality.coverage: 82%\nquality.lint: pass\nquality.audit: pass\nquality.mutation: 71%\nquality.complexity: 7";
        let report = EventParser::parse_quality_report(payload).unwrap();
        assert_eq!(report.specs_verified, None);
        assert!(
            report.meets_thresholds(),
            "missing specs should not fail quality thresholds"
        );
        assert!(!report.failed_dimensions().contains(&"specs"));
    }

    #[test]
    fn test_strip_ansi_function() {
        // Test the internal strip_ansi function via parse_backpressure_evidence
        // Simple CSI reset sequence
        let payload = "\x1b[0mtests: pass\x1b[0m";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert!(evidence.tests_passed);

        // Bold green text
        let payload = "\x1b[1m\x1b[32mtests: pass\x1b[0m";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert!(evidence.tests_passed);

        // Multiple sequences mixed with content
        let payload = "\x1b[31mtests: fail\x1b[0m\n\x1b[32mlint: pass\x1b[0m";
        let evidence = EventParser::parse_backpressure_evidence(payload).unwrap();
        assert!(!evidence.tests_passed); // "tests: fail" not "tests: pass"
        assert!(evidence.lint_passed);
        assert!(!evidence.coverage_passed);
    }
}
