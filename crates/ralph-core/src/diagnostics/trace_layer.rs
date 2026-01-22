use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::Subscriber;
use tracing_subscriber::Layer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    pub timestamp: String,
    pub iteration: Option<u32>,
    pub hat: Option<String>,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: serde_json::Value,
}

pub struct DiagnosticTraceLayer {
    writer: Arc<Mutex<BufWriter<File>>>,
    context: Arc<Mutex<TraceContext>>,
}

#[derive(Debug, Default)]
struct TraceContext {
    iteration: Option<u32>,
    hat: Option<String>,
}

impl DiagnosticTraceLayer {
    pub fn new(session_dir: &Path) -> std::io::Result<Self> {
        let trace_file = session_dir.join("trace.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(trace_file)?;

        Ok(Self {
            writer: Arc::new(Mutex::new(BufWriter::new(file))),
            context: Arc::new(Mutex::new(TraceContext::default())),
        })
    }

    pub fn set_context(&self, iteration: u32, hat: &str) {
        let mut ctx = self.context.lock().unwrap();
        ctx.iteration = Some(iteration);
        ctx.hat = Some(hat.to_string());
    }
}

impl<S: Subscriber> Layer<S> for DiagnosticTraceLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();

        // Extract message and fields
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        // Get context
        let ctx = self.context.lock().unwrap();

        let entry = TraceEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
            iteration: ctx.iteration,
            hat: ctx.hat.clone(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            message: visitor.message,
            fields: serde_json::to_value(&visitor.fields).unwrap_or(serde_json::Value::Null),
        };

        // Write to file
        let mut writer = self.writer.lock().unwrap();
        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = writeln!(writer, "{}", json);
            let _ = writer.flush();
        }
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: std::collections::HashMap<String, serde_json::Value>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value).trim_matches('"').to_string();
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(format!("{:?}", value)),
            );
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use tempfile::TempDir;
    use tracing::{debug, error, info, warn};
    use tracing_subscriber::prelude::*;

    #[test]
    fn test_layer_captures_all_levels() {
        let temp_dir = TempDir::new().unwrap();
        let layer = DiagnosticTraceLayer::new(temp_dir.path()).unwrap();

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            info!("info message");
            debug!("debug message");
            warn!("warn message");
            error!("error message");
        });

        // Read trace.jsonl
        let trace_file = temp_dir.path().join("trace.jsonl");
        let file = File::open(trace_file).unwrap();
        let reader = std::io::BufReader::new(file);
        let entries: Vec<TraceEntry> = reader
            .lines()
            .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
            .collect();

        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].level, "INFO");
        assert_eq!(entries[0].message, "info message");
        assert_eq!(entries[1].level, "DEBUG");
        assert_eq!(entries[1].message, "debug message");
        assert_eq!(entries[2].level, "WARN");
        assert_eq!(entries[2].message, "warn message");
        assert_eq!(entries[3].level, "ERROR");
        assert_eq!(entries[3].message, "error message");
    }

    #[test]
    fn test_fields_serialized_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let layer = DiagnosticTraceLayer::new(temp_dir.path()).unwrap();

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            info!(bytes = 1024, status = "ok", "message with fields");
        });

        let trace_file = temp_dir.path().join("trace.jsonl");
        let file = File::open(trace_file).unwrap();
        let reader = std::io::BufReader::new(file);
        let entries: Vec<TraceEntry> = reader
            .lines()
            .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
            .collect();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "message with fields");
        assert_eq!(entries[0].fields["bytes"], 1024);
        assert_eq!(entries[0].fields["status"], "ok");
    }

    #[test]
    fn test_context_included() {
        let temp_dir = TempDir::new().unwrap();
        let layer = DiagnosticTraceLayer::new(temp_dir.path()).unwrap();

        layer.set_context(5, "builder");

        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            info!("test message");
        });

        let trace_file = temp_dir.path().join("trace.jsonl");
        let file = File::open(trace_file).unwrap();
        let reader = std::io::BufReader::new(file);
        let entries: Vec<TraceEntry> = reader
            .lines()
            .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
            .collect();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].iteration, Some(5));
        assert_eq!(entries[0].hat, Some("builder".to_string()));
    }
}
