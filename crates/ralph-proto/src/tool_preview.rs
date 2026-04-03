use serde_json::Value;

pub fn format_tool_summary(name: &str, input: &Value) -> Option<String> {
    match name {
        "Read" | "Edit" | "Write" | "read" | "edit" | "write" => input
            .get("file_path")
            .or_else(|| input.get("path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "Bash" | "bash" | "shell" => {
            let cmd = input.get("command")?.as_str()?;
            Some(truncate_preview(cmd, 60))
        }
        "Grep" | "grep" => input.get("pattern")?.as_str().map(|s| s.to_string()),
        "Glob" | "glob" | "ls" => input
            .get("pattern")
            .or_else(|| input.get("path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        "Task" => input.get("description")?.as_str().map(|s| s.to_string()),
        "WebFetch" | "web_fetch" => input.get("url")?.as_str().map(|s| s.to_string()),
        "WebSearch" | "web_search" => input.get("query")?.as_str().map(|s| s.to_string()),
        "LSP" => {
            let op = input.get("operation")?.as_str()?;
            let file = input.get("filePath")?.as_str()?;
            Some(format!("{} @ {}", op, file))
        }
        "NotebookEdit" => input.get("notebook_path")?.as_str().map(|s| s.to_string()),
        "TodoWrite" => Some("updating todo list".to_string()),
        _ => input
            .get("path")
            .or_else(|| input.get("file_path"))
            .or_else(|| input.get("command"))
            .or_else(|| input.get("pattern"))
            .or_else(|| input.get("url"))
            .or_else(|| input.get("query"))
            .and_then(|v| v.as_str())
            .map(|s| truncate_preview(s, 60)),
    }
}

pub fn format_tool_result(output: &str) -> String {
    let Ok(val) = serde_json::from_str::<Value>(output) else {
        return summarize_plain_text(output);
    };
    let Some(items) = val.get("items").and_then(|v| v.as_array()) else {
        return output.to_string();
    };
    let Some(item) = items.first() else {
        return String::new();
    };

    if let Some(text) = item.get("Text").and_then(|v| v.as_str()) {
        return summarize_plain_text(text);
    }

    if let Some(json) = item.get("Json") {
        if let Some(stdout) = json.get("stdout").and_then(|v| v.as_str()) {
            let stderr = json.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
            let exit = json
                .get("exit_status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let failed = !exit.contains("status: 0");
            return if failed && !stderr.is_empty() {
                summarize_plain_text(stderr)
            } else if !stdout.is_empty() {
                summarize_plain_text(stdout)
            } else {
                summarize_plain_text(stderr)
            };
        }

        if let Some(paths) = json.get("filePaths").and_then(|v| v.as_array()) {
            let total = json
                .get("totalFiles")
                .and_then(|v| v.as_u64())
                .unwrap_or(paths.len() as u64);
            let names: Vec<&str> = paths
                .iter()
                .filter_map(|p| p.as_str())
                .map(|p| p.rsplit('/').next().unwrap_or(p))
                .collect();
            let shown = names.iter().take(3).copied().collect::<Vec<_>>();
            return if total > shown.len() as u64 {
                format!(
                    "{} files: {} (+{} more)",
                    total,
                    shown.join(", "),
                    total - shown.len() as u64
                )
            } else {
                format!("{} files: {}", total, shown.join(", "))
            };
        }

        if let Some(results) = json.get("results").and_then(|v| v.as_array()) {
            let num_matches = json.get("numMatches").and_then(|v| v.as_u64()).unwrap_or(0);
            let first_match = results.first().and_then(|r| {
                let file = r.get("file").and_then(|v| v.as_str()).unwrap_or("");
                let basename = file.rsplit('/').next().unwrap_or(file);
                let matches = r.get("matches").and_then(|v| v.as_array())?;
                let first = matches.first().and_then(|m| m.as_str())?;
                Some(format!("{}: {}", basename, first.trim()))
            });
            return match first_match {
                Some(m) => format!("{} matches: {}", num_matches, m),
                None => format!("{} matches", num_matches),
            };
        }

        return json.to_string();
    }

    summarize_plain_text(output)
}

fn summarize_plain_text(output: &str) -> String {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lines: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if lines.len() <= 1 {
        return trimmed.to_string();
    }

    let short_lines = lines.len() <= 4 && lines.iter().all(|line| line.chars().count() <= 60);
    if short_lines {
        return lines.join(" • ");
    }

    let first = lines.first().copied().unwrap_or(trimmed);
    let second = lines.get(1).copied();
    let remaining = lines.len().saturating_sub(2);

    match (second, remaining) {
        (Some(next), 0) => format!("{} • {}", first, next),
        (Some(next), more) => format!("{} • {} (+{} more lines)", first, next, more),
        (None, _) => first.to_string(),
    }
}

fn truncate_preview(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let byte_idx = s
            .char_indices()
            .nth(max_len)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        format!("{}...", &s[..byte_idx])
    }
}
