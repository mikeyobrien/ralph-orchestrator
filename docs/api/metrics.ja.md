# メトリクス API リファレンス

## 概要

メトリクスは `ralph_core::diagnostics` の診断システムを通じて収集されます。診断が有効な
とき、Ralph は次の下に構造化された JSONL ファイルを書き込みます。

```
.ralph/diagnostics/<timestamp>/
```

含まれるファイル:
- `performance.jsonl` — イテレーション/ハットごとのパフォーマンスメトリクス
- `orchestration.jsonl` — オーケストレーションイベント
- `errors.jsonl` — エラーレポート

診断は `RALPH_DIAGNOSTICS=1` を設定することで有効になります。

## パフォーマンスメトリクスを記録する

ファイル管理を気にせずメトリクスを記録するには `DiagnosticsCollector` を使います。

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

## メトリクスを直接書き込む

既に診断セッションのディレクトリがある場合は、`PerformanceLogger` を使います。

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

## メトリクス JSONL を読む

各行は JSON オブジェクトです。`serde_json::Value` へデシリアライズするか、
`PerformanceEntry` の形に合わせたローカルの構造体を使えます。

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
