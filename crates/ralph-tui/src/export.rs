//! Export helpers for TUI iteration buffers.

use crate::state::IterationBuffer;
use anyhow::{Context, Result, bail};
use chrono::Utc;
use ratatui::text::Line;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

const EXPORT_DIR: &str = ".ralph/tui-exports";

/// Which set of iteration buffers to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    /// Export only the currently viewed iteration.
    Current,
    /// Export every iteration currently held in memory.
    All,
}

impl ExportScope {
    fn file_slug(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::All => "all",
        }
    }

    /// Human-readable label for status messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Current => "current iteration",
            Self::All => "all iterations",
        }
    }
}

/// Plain-text snapshot of one iteration buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IterationExport {
    /// Iteration number as shown in the TUI.
    pub number: u32,
    /// Hat display captured for the iteration.
    pub hat_display: Option<String>,
    /// Backend captured for the iteration.
    pub backend: Option<String>,
    /// Plain text lines from the buffer.
    pub lines: Vec<String>,
}

impl IterationExport {
    /// Creates a plain-text snapshot from a live iteration buffer.
    pub fn from_buffer(buffer: &IterationBuffer) -> Result<Self> {
        let lines = buffer
            .lines
            .lock()
            .map_err(|_| anyhow::anyhow!("iteration buffer lock poisoned"))?
            .iter()
            .map(line_to_text)
            .collect();

        Ok(Self {
            number: buffer.number,
            hat_display: buffer.hat_display.clone(),
            backend: buffer.backend.clone(),
            lines,
        })
    }
}

/// Returns the TUI export directory for a workspace root.
pub fn export_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(EXPORT_DIR)
}

/// Writes a collision-safe export file and returns its path.
pub fn write_export(
    workspace_root: &Path,
    scope: ExportScope,
    iterations: &[IterationExport],
) -> Result<PathBuf> {
    if iterations.is_empty() {
        bail!("no iteration buffers to export");
    }

    let dir = export_dir(workspace_root);
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;

    let stem = format!(
        "ralph-tui-{}-{}",
        scope.file_slug(),
        Utc::now().format("%Y%m%dT%H%M%S%.6fZ")
    );
    write_collision_safe(&dir, &stem, &format_iterations(iterations))
}

/// Formats iteration snapshots as a stable plain-text artifact.
pub fn format_iterations(iterations: &[IterationExport]) -> String {
    let mut output = String::new();
    output.push_str("# Ralph TUI Export\n");
    output.push_str(&format!("Iterations: {}\n\n", iterations.len()));

    for (idx, iteration) in iterations.iter().enumerate() {
        if idx > 0 {
            output.push('\n');
        }

        output.push_str(
            "================================================================================\n",
        );
        output.push_str(&format!("Iteration {}\n", iteration.number));
        output.push_str(&format!(
            "Hat: {}\n",
            iteration.hat_display.as_deref().unwrap_or("unknown")
        ));
        output.push_str(&format!(
            "Backend: {}\n",
            iteration.backend.as_deref().unwrap_or("unknown")
        ));
        output.push_str(&format!("Lines: {}\n", iteration.lines.len()));
        output.push_str(
            "================================================================================\n",
        );

        for line in &iteration.lines {
            output.push_str(line);
            output.push('\n');
        }
    }

    output
}

fn line_to_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

fn write_collision_safe(dir: &Path, stem: &str, content: &str) -> Result<PathBuf> {
    for suffix in 0..1000 {
        let filename = if suffix == 0 {
            format!("{stem}.txt")
        } else {
            format!("{stem}-{suffix:03}.txt")
        };
        let path = dir.join(filename);

        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| format!("failed to create {}", path.display()));
            }
        };

        file.write_all(content.as_bytes())
            .with_context(|| format!("failed to write {}", path.display()))?;
        return Ok(path);
    }

    bail!(
        "could not allocate a unique TUI export path in {}",
        dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::{Line, Span};

    #[test]
    fn format_iterations_uses_clear_stable_headers() {
        let text = format_iterations(&[
            IterationExport {
                number: 1,
                hat_display: Some("Planner".to_string()),
                backend: Some("claude".to_string()),
                lines: vec!["first line".to_string(), "second line".to_string()],
            },
            IterationExport {
                number: 2,
                hat_display: None,
                backend: None,
                lines: vec!["done".to_string()],
            },
        ]);

        assert_eq!(
            text,
            "# Ralph TUI Export\n\
Iterations: 2\n\
\n\
================================================================================\n\
Iteration 1\n\
Hat: Planner\n\
Backend: claude\n\
Lines: 2\n\
================================================================================\n\
first line\n\
second line\n\
\n\
================================================================================\n\
Iteration 2\n\
Hat: unknown\n\
Backend: unknown\n\
Lines: 1\n\
================================================================================\n\
done\n"
        );
    }

    #[test]
    fn from_buffer_flattens_styled_spans_to_text() {
        let mut buffer = IterationBuffer::new(7);
        buffer.hat_display = Some("Builder".to_string());
        buffer.backend = Some("codex".to_string());
        buffer.append_line(Line::from(vec![Span::raw("hello "), Span::raw("world")]));

        let export = IterationExport::from_buffer(&buffer).unwrap();

        assert_eq!(export.number, 7);
        assert_eq!(export.hat_display.as_deref(), Some("Builder"));
        assert_eq!(export.backend.as_deref(), Some("codex"));
        assert_eq!(export.lines, vec!["hello world"]);
    }

    #[test]
    fn export_dir_is_under_workspace_ralph_directory() {
        let root = Path::new("/tmp/workspace");

        assert_eq!(
            export_dir(root),
            PathBuf::from("/tmp/workspace/.ralph/tui-exports")
        );
    }

    #[test]
    fn write_collision_safe_adds_suffix_when_name_exists() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("ralph-tui-current-fixed.txt");
        fs::write(&existing, "existing").unwrap();

        let written = write_collision_safe(dir.path(), "ralph-tui-current-fixed", "new").unwrap();

        assert_eq!(
            written.file_name().and_then(|name| name.to_str()),
            Some("ralph-tui-current-fixed-001.txt")
        );
        assert_eq!(fs::read_to_string(written).unwrap(), "new");
        assert_eq!(fs::read_to_string(existing).unwrap(), "existing");
    }
}
