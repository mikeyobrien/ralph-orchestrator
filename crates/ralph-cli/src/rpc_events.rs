//! Map Autoloop `--events` records onto Ralph's existing [`RpcEvent`] contract.
//!
//! This is the #343 restore path. The structured plane stays Autoloop's
//! `events.ndjson`. RPC mode only changes the stdout serialization.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use ralph_adapters::AutoloopEvent;
use ralph_proto::json_rpc::{RpcEvent, TerminationReason};

/// Stateful translator from one Autoloop `--events` stream onto `RpcEvent`s.
pub struct RpcEventMapper {
    prompt: String,
    backend: String,
    max_iterations: Option<u32>,
    started_at: u64,
    role_display_names: HashMap<String, String>,
    announced_role: Option<(u32, String)>,
    current_iteration: Option<u32>,
    iteration_started_at: Option<u64>,
    loop_started: bool,
    last_cost_usd: f64,
}

impl RpcEventMapper {
    /// Creates a mapper for one Ralph `--rpc` run.
    pub fn new(
        prompt: impl Into<String>,
        backend: impl Into<String>,
        max_iterations: Option<u32>,
        role_display_names: HashMap<String, String>,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            backend: backend.into(),
            max_iterations,
            started_at: unix_ms(),
            role_display_names,
            announced_role: None,
            current_iteration: None,
            iteration_started_at: None,
            loop_started: false,
            last_cost_usd: 0.0,
        }
    }

    /// Maps one Autoloop event into zero or more `RpcEvent`s.
    pub fn map(&mut self, event: &AutoloopEvent) -> Vec<RpcEvent> {
        match event.kind.as_str() {
            "loop.start" => vec![self.ensure_loop_started()],
            "iteration.banner" => {
                self.observe_banner(event);
                Vec::new()
            }
            "iteration.start" => self.map_iteration_start(event),
            "progress" => self.map_progress(event),
            "backend.output" => self.map_backend_output(event),
            "ask.pending" => self.map_ask(event),
            "loop.finish" | "summary" => self.map_terminal(event),
            "log" => self.map_log(event),
            _ => Vec::new(),
        }
    }

    fn ensure_loop_started(&mut self) -> RpcEvent {
        self.loop_started = true;
        RpcEvent::LoopStarted {
            prompt: self.prompt.clone(),
            max_iterations: self.max_iterations,
            backend: self.backend.clone(),
            workspace_root: None,
            started_at: self.started_at,
        }
    }

    fn observe_banner(&mut self, event: &AutoloopEvent) {
        if let (Some(iteration), Some(role_id)) = (
            event.iteration,
            event.allowed_roles.as_ref().and_then(|roles| roles.first()),
        ) {
            self.announced_role = Some((iteration, self.role_display(role_id)));
        }
        if let Some(max) = event.max_iterations {
            self.max_iterations = Some(max);
        }
    }

    fn map_iteration_start(&mut self, event: &AutoloopEvent) -> Vec<RpcEvent> {
        let mut out = Vec::new();
        if !self.loop_started {
            out.push(self.ensure_loop_started());
        }
        let iteration = event.iteration.unwrap_or(0);
        if let Some(max) = event.max_iterations {
            self.max_iterations = Some(max);
        }
        self.current_iteration = Some(iteration);
        self.iteration_started_at = Some(unix_ms());
        let hat = self.role_id_for(iteration);
        let hat_display = self.role_display(&hat);
        out.push(RpcEvent::IterationStart {
            iteration,
            max_iterations: self.max_iterations,
            hat,
            hat_display,
            backend: self.backend.clone(),
            started_at: self.iteration_started_at.unwrap_or(self.started_at),
        });
        out
    }

    fn map_progress(&mut self, event: &AutoloopEvent) -> Vec<RpcEvent> {
        let iteration = event.iteration.or(self.current_iteration).unwrap_or(0);
        if let Some(cost) = event.cost_usd {
            self.last_cost_usd = cost;
        }
        let duration_ms = self
            .iteration_started_at
            .map(|started| unix_ms().saturating_sub(started))
            .unwrap_or(0);
        let loop_complete_triggered = event
            .outcome
            .as_deref()
            .is_some_and(|outcome| outcome.starts_with("complete"));
        let mut out = vec![RpcEvent::IterationEnd {
            iteration,
            duration_ms,
            cost_usd: event.cost_usd.unwrap_or(0.0),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            context_window: 0,
            context_tokens: 0,
            loop_complete_triggered,
        }];
        if let Some(topic) = event.emitted_topic.as_deref() {
            out.push(RpcEvent::OrchestrationEvent {
                topic: topic.to_string(),
                payload: event.outcome.clone().unwrap_or_default(),
                source: self.announced_role.as_ref().map(|(_, name)| name.clone()),
                target: None,
            });
        }
        out
    }

    fn map_backend_output(&mut self, event: &AutoloopEvent) -> Vec<RpcEvent> {
        let Some(output) = event.output.as_deref() else {
            return Vec::new();
        };
        if output.is_empty() {
            return Vec::new();
        }
        vec![RpcEvent::TextDelta {
            iteration: event.iteration.or(self.current_iteration).unwrap_or(0),
            delta: output.to_string(),
        }]
    }

    fn map_ask(&mut self, event: &AutoloopEvent) -> Vec<RpcEvent> {
        vec![RpcEvent::OrchestrationEvent {
            topic: "human.ask".to_string(),
            payload: event.question.clone().unwrap_or_default(),
            source: None,
            target: None,
        }]
    }

    fn map_terminal(&mut self, event: &AutoloopEvent) -> Vec<RpcEvent> {
        let mut out = Vec::new();
        if !self.loop_started {
            out.push(self.ensure_loop_started());
        }
        if let Some(cost) = event.cost_usd {
            self.last_cost_usd = cost;
        }
        out.push(RpcEvent::LoopTerminated {
            reason: map_rpc_stop_reason(event.stop_reason.as_deref()),
            total_iterations: event.iterations.or(self.current_iteration).unwrap_or(0),
            duration_ms: unix_ms().saturating_sub(self.started_at),
            total_cost_usd: self.last_cost_usd,
            terminated_at: unix_ms(),
        });
        out
    }

    fn map_log(&mut self, event: &AutoloopEvent) -> Vec<RpcEvent> {
        if !matches!(event.level.as_deref(), Some("error" | "ERROR")) {
            return Vec::new();
        }
        vec![RpcEvent::Error {
            iteration: event.iteration.or(self.current_iteration).unwrap_or(0),
            code: "ENGINE_LOG".to_string(),
            message: event.message.clone().unwrap_or_default(),
            recoverable: true,
        }]
    }

    fn role_id_for(&self, iteration: u32) -> String {
        self.announced_role
            .as_ref()
            .filter(|(announced, _)| *announced == iteration)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| "working".to_string())
    }

    fn role_display(&self, role_id: &str) -> String {
        self.role_display_names
            .get(role_id)
            .cloned()
            .unwrap_or_else(|| role_id.to_string())
    }
}

fn map_rpc_stop_reason(reason: Option<&str>) -> TerminationReason {
    match reason {
        Some("completed" | "completion_event" | "completion_promise" | "verdict_exit") => {
            TerminationReason::Completed
        }
        Some("max_iterations") => TerminationReason::MaxIterations,
        Some("interrupted") => TerminationReason::Interrupted,
        Some(_) => TerminationReason::Error,
        None => TerminationReason::Error,
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_adapters::parse_events;

    fn mapper() -> RpcEventMapper {
        let mut names = HashMap::new();
        names.insert("planner".to_string(), "Planning Lead".to_string());
        names.insert("builder".to_string(), "Build Crew".to_string());
        RpcEventMapper::new("do the work", "claude", Some(2), names)
    }

    #[test]
    fn maps_banner_start_progress_and_finish_onto_rpc_events() {
        let events = parse_events(concat!(
            r#"{"type":"iteration.banner","runId":"r1","iteration":1,"maxIterations":2,"allowedRoles":["planner"]}"#,
            "\n",
            r#"{"type":"iteration.start","runId":"r1","iteration":1,"maxIterations":2}"#,
            "\n",
            r#"{"type":"progress","runId":"r1","iteration":1,"emittedTopic":"build.task","outcome":"continue:routed_event","costUsd":0.01}"#,
            "\n",
            r#"{"type":"loop.finish","runId":"r1","iterations":1,"stopReason":"completed","costUsd":0.01}"#,
            "\n",
        ));
        let mut mapper = mapper();
        let mapped: Vec<RpcEvent> = events.iter().flat_map(|event| mapper.map(event)).collect();

        assert!(matches!(
            &mapped[0],
            RpcEvent::LoopStarted {
                prompt,
                backend,
                max_iterations: Some(2),
                ..
            } if prompt == "do the work" && backend == "claude"
        ));
        assert!(matches!(
            &mapped[1],
            RpcEvent::IterationStart {
                iteration: 1,
                hat_display,
                ..
            } if hat_display == "Planning Lead"
        ));
        assert!(matches!(
            &mapped[2],
            RpcEvent::IterationEnd {
                iteration: 1,
                loop_complete_triggered: false,
                ..
            }
        ));
        assert!(matches!(
            &mapped[3],
            RpcEvent::OrchestrationEvent { topic, .. } if topic == "build.task"
        ));
        assert!(matches!(
            &mapped[4],
            RpcEvent::LoopTerminated {
                reason: TerminationReason::Completed,
                total_iterations: 1,
                ..
            }
        ));
    }

    #[test]
    fn maps_backend_output_and_ask_without_a_second_events_plane() {
        let events = parse_events(concat!(
            r#"{"type":"iteration.start","runId":"r1","iteration":1}"#,
            "\n",
            r#"{"type":"backend.output","iteration":1,"output":"hello from the agent"}"#,
            "\n",
            r#"{"type":"ask.pending","runId":"r1","questionId":"q1","question":"ship it?"}"#,
            "\n",
        ));
        let mut mapper = mapper();
        let mapped: Vec<RpcEvent> = events.iter().flat_map(|event| mapper.map(event)).collect();
        assert!(
            mapped
                .iter()
                .any(|event| matches!(event, RpcEvent::TextDelta { delta, .. } if delta == "hello from the agent"))
        );
        assert!(mapped.iter().any(|event| matches!(
            event,
            RpcEvent::OrchestrationEvent { topic, payload, .. }
                if topic == "human.ask" && payload == "ship it?"
        )));
    }

    #[test]
    fn unknown_autoloop_kinds_do_not_emit_rpc_events() {
        let events = parse_events(r#"{"type":"progress.internal","runId":"r1"}"#);
        let mut mapper = mapper();
        assert!(mapper.map(&events[0]).is_empty());
    }
}
