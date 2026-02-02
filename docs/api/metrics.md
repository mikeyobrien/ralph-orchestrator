# Metrics API Reference

## Overview

Metrics are collected through the diagnostics system in `ralph_core::diagnostics`.
When diagnostics are enabled, Ralph writes structured JSONL files under:

```
.ralph/diagnostics/<timestamp>/
```

Files include:
- `performance.jsonl` — performance metrics per iteration/hat
- `orchestration.jsonl` — orchestration events
- `errors.jsonl` — error reports

Diagnostics are enabled by setting `RALPH_DIAGNOSTICS=1`.

## Log Performance Metrics

Use `DiagnosticsCollector` to log metrics without worrying about file management:

```rust
use ralph_core::DiagnosticsCollector;
use ralph_core::diagnostics::PerformanceMetric;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(".");
    let collector = DiagnosticsCollector::with_enabled(base, true)?;

    collector.log_performance(
        1,
        "planner",
        PerformanceMetric::IterationDuration { duration_ms: 1200 },
    );

    collector.log_performance(
        1,
        "builder",
        PerformanceMetric::TokenCount { input: 1450, output: 620 },
    );

    Ok(())
}
```

## Write Metrics Directly

If you already have a diagnostics session directory, use `PerformanceLogger`:

```rust
use ralph_core::diagnostics::{PerformanceLogger, PerformanceMetric};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session_dir = Path::new(".ralph/diagnostics/2026-01-31T12-00-00");
    let mut logger = PerformanceLogger::new(session_dir)?;

    logger.log(
        2,
        "reviewer",
        PerformanceMetric::AgentLatency { duration_ms: 830 },
    )?;

    Ok(())
}
```

## Reading Metrics JSONL

Each line is a JSON object. You can deserialize into `serde_json::Value`
or a local struct that matches the `PerformanceEntry` shape.

```rust
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(".ralph/diagnostics/2026-01-31T12-00-00/performance.jsonl")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let entry: Value = serde_json::from_str(&line?)?;
        let iteration = entry.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0);
        let hat = entry.get("hat").and_then(|v| v.as_str()).unwrap_or("unknown");
        println!("iteration={iteration} hat={hat}");
    }

    Ok(())
}
```
