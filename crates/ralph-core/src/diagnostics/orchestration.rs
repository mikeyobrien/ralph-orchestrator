use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationEntry {
    pub timestamp: String,
    pub iteration: u32,
    pub hat: String,
    pub event: OrchestrationEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestrationEvent {
    IterationStarted,
    HatSelected { hat: String, reason: String },
    EventPublished { topic: String },
    BackpressureTriggered { reason: String },
    LoopTerminated { reason: String },
    TaskAbandoned { reason: String },
}

pub struct OrchestrationLogger {
    writer: BufWriter<File>,
}

impl OrchestrationLogger {
    pub fn new(session_dir: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(session_dir.join("orchestration.jsonl"))?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn log(
        &mut self,
        iteration: u32,
        hat: &str,
        event: OrchestrationEvent,
    ) -> std::io::Result<()> {
        let entry = OrchestrationEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
            iteration,
            hat: hat.to_string(),
            event,
        };
        serde_json::to_writer(&mut self.writer, &entry)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use tempfile::TempDir;

    #[test]
    fn test_all_event_types_serialize() {
        let events = vec![
            OrchestrationEvent::IterationStarted,
            OrchestrationEvent::HatSelected {
                hat: "ralph".to_string(),
                reason: "pending_events".to_string(),
            },
            OrchestrationEvent::EventPublished {
                topic: "build.start".to_string(),
            },
            OrchestrationEvent::BackpressureTriggered {
                reason: "tests failed".to_string(),
            },
            OrchestrationEvent::LoopTerminated {
                reason: "completion_promise".to_string(),
            },
            OrchestrationEvent::TaskAbandoned {
                reason: "max_iterations".to_string(),
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let _: OrchestrationEvent = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn test_iteration_and_hat_captured() {
        let temp_dir = TempDir::new().unwrap();
        let mut logger = OrchestrationLogger::new(temp_dir.path()).unwrap();

        logger
            .log(
                5,
                "builder",
                OrchestrationEvent::HatSelected {
                    hat: "builder".to_string(),
                    reason: "tasks_ready".to_string(),
                },
            )
            .unwrap();

        drop(logger);

        let file = File::open(temp_dir.path().join("orchestration.jsonl")).unwrap();
        let reader = BufReader::new(file);
        let line = reader.lines().next().unwrap().unwrap();
        let entry: OrchestrationEntry = serde_json::from_str(&line).unwrap();

        assert_eq!(entry.iteration, 5);
        assert_eq!(entry.hat, "builder");
    }

    #[test]
    fn test_immediate_flush() {
        let temp_dir = TempDir::new().unwrap();
        let mut logger = OrchestrationLogger::new(temp_dir.path()).unwrap();

        logger
            .log(1, "ralph", OrchestrationEvent::IterationStarted)
            .unwrap();

        // Don't drop logger - verify file has content immediately
        let file = File::open(temp_dir.path().join("orchestration.jsonl")).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<_> = reader.lines().collect();
        assert_eq!(lines.len(), 1);
    }
}
