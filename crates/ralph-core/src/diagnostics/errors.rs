use chrono::Utc;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct ErrorEntry {
    ts: String,
    iteration: u32,
    hat: String,
    error_type: String,
    message: String,
    context: serde_json::Value,
}

#[derive(Debug)]
pub enum DiagnosticError {
    ParseError {
        source: String,
        message: String,
        input: String,
    },
    ValidationFailure {
        rule: String,
        message: String,
        evidence: String,
    },
    BackendError {
        backend: String,
        message: String,
    },
    Timeout {
        operation: String,
        duration_ms: u64,
    },
    MalformedEvent {
        line: String,
        error: String,
    },
    TelegramSendError {
        operation: String,
        error: String,
        retry_count: u32,
    },
}

impl DiagnosticError {
    fn error_type(&self) -> &str {
        match self {
            Self::ParseError { .. } => "parse_error",
            Self::ValidationFailure { .. } => "validation_failure",
            Self::BackendError { .. } => "backend_error",
            Self::Timeout { .. } => "timeout",
            Self::MalformedEvent { .. } => "malformed_event",
            Self::TelegramSendError { .. } => "telegram_send_error",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::ParseError { message, .. } => message.clone(),
            Self::ValidationFailure { message, .. } => message.clone(),
            Self::BackendError { message, .. } => message.clone(),
            Self::Timeout { operation, .. } => format!("Operation timed out: {}", operation),
            Self::MalformedEvent { error, .. } => error.clone(),
            Self::TelegramSendError { error, .. } => error.clone(),
        }
    }

    fn context(&self) -> serde_json::Value {
        match self {
            Self::ParseError {
                source,
                message: _,
                input,
            } => serde_json::json!({
                "source": source,
                "input": input,
            }),
            Self::ValidationFailure {
                rule,
                message: _,
                evidence,
            } => serde_json::json!({
                "rule": rule,
                "evidence": evidence,
            }),
            Self::BackendError {
                backend,
                message: _,
            } => serde_json::json!({
                "backend": backend,
            }),
            Self::Timeout {
                operation,
                duration_ms,
            } => serde_json::json!({
                "operation": operation,
                "duration_ms": duration_ms,
            }),
            Self::MalformedEvent { line, error: _ } => serde_json::json!({
                "line": line,
            }),
            Self::TelegramSendError {
                operation,
                error: _,
                retry_count,
            } => serde_json::json!({
                "operation": operation,
                "retry_count": retry_count,
            }),
        }
    }
}

pub struct ErrorLogger {
    file: BufWriter<File>,
    iteration: u32,
    hat: String,
}

impl ErrorLogger {
    pub fn new(session_dir: &Path) -> io::Result<Self> {
        let file_path = session_dir.join("errors.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        Ok(Self {
            file: BufWriter::new(file),
            iteration: 0,
            hat: String::from("unknown"),
        })
    }

    pub fn set_context(&mut self, iteration: u32, hat: &str) {
        self.iteration = iteration;
        self.hat = hat.to_string();
    }

    pub fn log(&mut self, error: DiagnosticError) {
        let entry = ErrorEntry {
            ts: Utc::now().to_rfc3339(),
            iteration: self.iteration,
            hat: self.hat.clone(),
            error_type: error.error_type().to_string(),
            message: error.message(),
            context: error.context(),
        };

        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = writeln!(self.file, "{}", json);
            let _ = self.file.flush();
        }
    }

    pub fn flush(&mut self) {
        let _ = self.file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_error_logger_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let session_dir = temp_dir.path();

        let logger = ErrorLogger::new(session_dir);
        assert!(logger.is_ok());

        let file_path = session_dir.join("errors.jsonl");
        assert!(file_path.exists());
    }

    #[test]
    fn test_all_error_types_serialize() {
        let temp_dir = TempDir::new().unwrap();
        let session_dir = temp_dir.path();
        let mut logger = ErrorLogger::new(session_dir).unwrap();
        logger.set_context(1, "ralph");

        let errors = vec![
            DiagnosticError::ParseError {
                source: "agent_output".to_string(),
                message: "Invalid JSON".to_string(),
                input: "{invalid".to_string(),
            },
            DiagnosticError::ValidationFailure {
                rule: "tests_required".to_string(),
                message: "Missing test evidence".to_string(),
                evidence: "tests: missing".to_string(),
            },
            DiagnosticError::BackendError {
                backend: "claude".to_string(),
                message: "API error".to_string(),
            },
            DiagnosticError::Timeout {
                operation: "agent_execution".to_string(),
                duration_ms: 30000,
            },
            DiagnosticError::MalformedEvent {
                line: "<event topic=".to_string(),
                error: "Incomplete tag".to_string(),
            },
        ];

        for error in errors {
            logger.log(error);
        }

        let file_path = session_dir.join("errors.jsonl");
        let content = fs::read_to_string(file_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 5);

        // Verify each line is valid JSON
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed.get("ts").is_some());
            assert_eq!(parsed.get("iteration").unwrap(), 1);
            assert_eq!(parsed.get("hat").unwrap(), "ralph");
            assert!(parsed.get("error_type").is_some());
            assert!(parsed.get("message").is_some());
            assert!(parsed.get("context").is_some());
        }
    }

    #[test]
    fn test_error_logger_integration() {
        let temp_dir = TempDir::new().unwrap();
        let session_dir = temp_dir.path();
        let mut logger = ErrorLogger::new(session_dir).unwrap();

        logger.set_context(1, "builder");
        logger.log(DiagnosticError::ValidationFailure {
            rule: "lint_pass".to_string(),
            message: "Lint failed".to_string(),
            evidence: "lint: fail".to_string(),
        });

        logger.set_context(2, "validator");
        logger.log(DiagnosticError::ParseError {
            source: "event_parser".to_string(),
            message: "Malformed event".to_string(),
            input: "<event>".to_string(),
        });

        let file_path = session_dir.join("errors.jsonl");
        let content = fs::read_to_string(file_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2);

        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first.get("iteration").unwrap(), 1);
        assert_eq!(first.get("hat").unwrap(), "builder");
        assert_eq!(first.get("error_type").unwrap(), "validation_failure");

        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second.get("iteration").unwrap(), 2);
        assert_eq!(second.get("hat").unwrap(), "validator");
        assert_eq!(second.get("error_type").unwrap(), "parse_error");
    }
}
