use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEntry {
    pub timestamp: DateTime<Utc>,
    pub iteration: u32,
    pub hat: String,
    pub metric: PerformanceMetric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PerformanceMetric {
    IterationDuration { duration_ms: u64 },
    AgentLatency { duration_ms: u64 },
    TokenCount { input: usize, output: usize },
}

pub struct PerformanceLogger {
    writer: BufWriter<File>,
}

impl PerformanceLogger {
    pub fn new(session_dir: &Path) -> std::io::Result<Self> {
        let log_file = session_dir.join("performance.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;
        let writer = BufWriter::new(file);
        Ok(Self { writer })
    }

    pub fn log(
        &mut self,
        iteration: u32,
        hat: &str,
        metric: PerformanceMetric,
    ) -> std::io::Result<()> {
        let entry = PerformanceEntry {
            timestamp: Utc::now(),
            iteration,
            hat: hat.to_string(),
            metric,
        };

        serde_json::to_writer(&mut self.writer, &entry)?;
        writeln!(&mut self.writer)?;
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use tempfile::TempDir;

    #[test]
    fn test_performance_logger_creates_file() {
        let temp = TempDir::new().unwrap();
        let _logger = PerformanceLogger::new(temp.path()).unwrap();

        let log_file = temp.path().join("performance.jsonl");
        assert!(log_file.exists(), "performance.jsonl should be created");
    }

    #[test]
    fn test_all_metric_types_serialize() {
        let temp = TempDir::new().unwrap();
        let mut logger = PerformanceLogger::new(temp.path()).unwrap();

        // Log all metric types
        logger
            .log(
                1,
                "ralph",
                PerformanceMetric::IterationDuration { duration_ms: 1500 },
            )
            .unwrap();
        logger
            .log(
                1,
                "builder",
                PerformanceMetric::AgentLatency { duration_ms: 800 },
            )
            .unwrap();
        logger
            .log(
                1,
                "builder",
                PerformanceMetric::TokenCount {
                    input: 1000,
                    output: 500,
                },
            )
            .unwrap();

        // Read back and verify
        drop(logger);
        let log_file = temp.path().join("performance.jsonl");
        let file = File::open(log_file).unwrap();
        let reader = std::io::BufReader::new(file);
        let lines: Vec<_> = reader.lines().collect::<Result<_, _>>().unwrap();

        assert_eq!(lines.len(), 3, "Should have 3 log entries");

        // Verify each line is valid JSON
        let entry1: PerformanceEntry = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(entry1.iteration, 1);
        assert_eq!(entry1.hat, "ralph");
        assert!(matches!(
            entry1.metric,
            PerformanceMetric::IterationDuration { duration_ms: 1500 }
        ));

        let entry2: PerformanceEntry = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(entry2.iteration, 1);
        assert_eq!(entry2.hat, "builder");
        assert!(matches!(
            entry2.metric,
            PerformanceMetric::AgentLatency { duration_ms: 800 }
        ));

        let entry3: PerformanceEntry = serde_json::from_str(&lines[2]).unwrap();
        assert_eq!(entry3.iteration, 1);
        assert_eq!(entry3.hat, "builder");
        assert!(matches!(
            entry3.metric,
            PerformanceMetric::TokenCount {
                input: 1000,
                output: 500
            }
        ));
    }
}
