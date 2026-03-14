//! Wave worker prompt builder.
//!
//! Constructs focused prompts for individual wave worker instances,
//! providing task context and constraints to keep workers on track.

use crate::config::HatConfig;
use crate::event_reader::Event;

/// Context for a wave worker instance.
#[derive(Debug)]
pub struct WaveWorkerContext {
    /// Wave correlation ID (e.g., "w-1a2b3c4d").
    pub wave_id: String,
    /// 0-based index of this worker within the wave.
    pub wave_index: u32,
    /// Total number of workers in this wave.
    pub wave_total: u32,
    /// Topics this worker should publish results to.
    pub result_topics: Vec<String>,
}

/// Builds a focused prompt for a wave worker instance.
///
/// The prompt contains:
/// 1. Hat instructions (what the worker does)
/// 2. Wave context (worker identity within the wave)
/// 3. Task payload (the specific work item)
/// 4. Publishing guide (how to emit results)
/// 5. Constraints (nested wave prohibition, focus directive)
pub fn build_wave_worker_prompt(hat: &HatConfig, event: &Event, ctx: &WaveWorkerContext) -> String {
    let mut prompt = String::new();

    // 1. Instructions
    if !hat.instructions.trim().is_empty() {
        prompt.push_str("# Instructions\n\n");
        prompt.push_str(&hat.instructions);
        if !hat.instructions.ends_with('\n') {
            prompt.push('\n');
        }
        prompt.push('\n');
    }

    // 2. Wave context
    prompt.push_str("# Wave Context\n\n");
    prompt.push_str(&format!(
        "You are worker **{}/{}** in wave `{}`.\n\
         Each worker in this wave processes one task independently and in parallel.\n\
         Focus exclusively on your assigned task below.\n\n",
        ctx.wave_index + 1,
        ctx.wave_total,
        ctx.wave_id,
    ));

    // 3. Task payload
    prompt.push_str("# Your Task\n\n");
    if let Some(ref payload) = event.payload {
        prompt.push_str(payload);
    }
    prompt.push_str("\n\n");

    // 4. Publishing results
    if !ctx.result_topics.is_empty() {
        prompt.push_str("# Publishing Results\n\n");
        prompt.push_str("When your work is complete, publish your results using `ralph emit`:\n\n");
        for topic in &ctx.result_topics {
            prompt.push_str(&format!(
                "```bash\nralph emit {} \"<your result payload>\"\n```\n\n",
                topic
            ));
        }
    }

    // 5. Constraints
    prompt.push_str("# Constraints\n\n");
    prompt.push_str(
        "- **DO NOT** use `ralph wave emit` — nested wave dispatch is prohibited.\n\
         - Focus exclusively on your assigned task. Do not attempt work assigned to other workers.\n\
         - Publish exactly one result event when complete.\n",
    );

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hat_config() -> HatConfig {
        let yaml = r#"
            name: "Reviewer"
            triggers: ["review.file"]
            publishes: ["review.done"]
            instructions: "Review the file for bugs and style issues."
        "#;
        serde_yaml::from_str(yaml).unwrap()
    }

    fn make_event(payload: &str) -> Event {
        Event {
            topic: "review.file".to_string(),
            payload: Some(payload.to_string()),
            ts: "2025-01-01T00:00:00Z".to_string(),
            wave_id: Some("w-test1234".to_string()),
            wave_index: Some(0),
            wave_total: Some(3),
        }
    }

    #[test]
    fn test_build_wave_worker_prompt_contains_all_sections() {
        let hat = make_hat_config();
        let event = make_event("src/main.rs");
        let ctx = WaveWorkerContext {
            wave_id: "w-test1234".to_string(),
            wave_index: 0,
            wave_total: 3,
            result_topics: vec!["review.done".to_string()],
        };

        let prompt = build_wave_worker_prompt(&hat, &event, &ctx);

        assert!(prompt.contains("# Instructions"));
        assert!(prompt.contains("Review the file for bugs"));
        assert!(prompt.contains("# Wave Context"));
        assert!(prompt.contains("worker **1/3**"));
        assert!(prompt.contains("w-test1234"));
        assert!(prompt.contains("# Your Task"));
        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("# Publishing Results"));
        assert!(prompt.contains("ralph emit review.done"));
        assert!(prompt.contains("# Constraints"));
        assert!(prompt.contains("DO NOT"));
    }

    #[test]
    fn test_worker_index_is_1_based_in_display() {
        let hat = make_hat_config();
        let event = make_event("file.rs");
        let ctx = WaveWorkerContext {
            wave_id: "w-abc".to_string(),
            wave_index: 2,
            wave_total: 5,
            result_topics: vec![],
        };

        let prompt = build_wave_worker_prompt(&hat, &event, &ctx);
        assert!(prompt.contains("worker **3/5**"));
    }

    #[test]
    fn test_empty_instructions_omitted() {
        let yaml = r#"
            name: "Reviewer"
            triggers: ["review.file"]
            publishes: ["review.done"]
            instructions: ""
        "#;
        let hat: HatConfig = serde_yaml::from_str(yaml).unwrap();
        let event = make_event("payload");
        let ctx = WaveWorkerContext {
            wave_id: "w-abc".to_string(),
            wave_index: 0,
            wave_total: 1,
            result_topics: vec![],
        };

        let prompt = build_wave_worker_prompt(&hat, &event, &ctx);
        assert!(!prompt.contains("# Instructions"));
    }

    #[test]
    fn test_no_result_topics_skips_publishing_section() {
        let hat = make_hat_config();
        let event = make_event("payload");
        let ctx = WaveWorkerContext {
            wave_id: "w-abc".to_string(),
            wave_index: 0,
            wave_total: 1,
            result_topics: vec![],
        };

        let prompt = build_wave_worker_prompt(&hat, &event, &ctx);
        assert!(!prompt.contains("# Publishing Results"));
    }
}
