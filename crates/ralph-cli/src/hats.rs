//! CLI commands for the `ralph hats` namespace.
//!
//! Manage and inspect configured hats.
//!
//! Subcommands:
//! - `list`: Show all configured hats (Name, Description)
//! - `show`: Show detailed configuration for a specific hat

use crate::backend_support;
use crate::display::colors;
use crate::preflight;
use crate::{ConfigSource, HatsSource, is_toml_preset_dir};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use ralph_adapters::{CliBackend, detect_backend_default};
use ralph_core::{HatRegistry, RalphConfig, truncate_with_ellipsis};
use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Manage configured hats.
#[derive(Parser, Debug)]
pub struct HatsArgs {
    #[command(subcommand)]
    pub command: Option<HatsCommands>,
}

#[derive(Subcommand, Debug)]
pub enum HatsCommands {
    /// Validate hat topology and report issues
    Validate,
    /// Display hat topology graph
    Graph {
        /// Output format (unicode, ascii, compact, mermaid)
        #[arg(long, default_value = "unicode")]
        format: GraphFormat,
        /// Backend for AI-generated diagrams (claude, kiro, gemini, codex, forge, amp, copilot, opencode, pi, custom)
        #[arg(short = 'b', long = "backend")]
        backend: Option<String>,
    },
    /// List all configured hats (default if no subcommand)
    List {
        /// Output format (table, json)
        #[arg(long, default_value = "table")]
        format: ListFormat,
    },
    /// Show detailed configuration for a specific hat
    Show(ShowArgs),
    /// List all presets discoverable on this system (both YAML and TOML formats).
    ///
    /// Walks the same resolver paths used by `-H <name>`:
    ///   1. `./presets/<name>/`
    ///   2. `$XDG_CONFIG_HOME/ralph/presets/<name>/`
    ///   3. `$HOME/.config/ralph/presets/<name>/`
    ///   4. `$HOME/.config/autoloop/presets/<name>/` (shared with autoloop CLI)
    ///   5. `$RALPH_PRESETS_DIR/<name>/`
    ///   6. `$AUTOLOOP_PRESETS_DIR/<name>/` (deprecated alias for #5)
    ///
    /// A preset is either a `.yml`/`.yaml` file (ralph's native shape) or a
    /// directory containing `autoloops.toml` + `topology.toml` (multi-file TOML shape).
    ListPresets {
        /// Output format (table, json)
        #[arg(long, default_value = "table")]
        format: ListFormat,
    },
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum GraphFormat {
    /// Unicode box-drawing characters (┌─┐│└┘▶) - best appearance
    #[default]
    Unicode,
    /// Pure ASCII characters (+--| chars) - maximum compatibility
    Ascii,
    /// Compact single-glyph nodes - minimal output
    Compact,
    /// Raw Mermaid syntax - for external rendering tools
    Mermaid,
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum ListFormat {
    #[default]
    Table,
    Json,
}

#[derive(Parser, Debug)]
pub struct ShowArgs {
    /// Name of the hat to show (ID or display name)
    pub name: String,
}

/// Execute a hats command.
pub async fn execute(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    args: HatsArgs,
    use_colors: bool,
) -> Result<()> {
    let mut stdout = std::io::stdout();

    // ListPresets doesn't need a loaded config; it's pure filesystem discovery.
    // Short-circuit before preflight so users can run it outside any ralph
    // workspace (matches `kubectl config get-contexts` style ergonomics).
    if let Some(HatsCommands::ListPresets { format }) = &args.command {
        let presets = discover_presets();
        return match format {
            ListFormat::Table => list_presets_table(&mut stdout, &presets, use_colors),
            ListFormat::Json => list_presets_json(&mut stdout, &presets),
        };
    }

    let config = preflight::load_config_for_preflight(config_sources, hats_source)
        .await
        .context("Failed to load config for hats")?;

    let registry = HatRegistry::from_config(&config);

    match args.command {
        None
        | Some(HatsCommands::List {
            format: ListFormat::Table,
        }) => list_hats(&mut stdout, &registry, use_colors),
        Some(HatsCommands::List {
            format: ListFormat::Json,
        }) => list_hats_json(&mut stdout, &registry),
        Some(HatsCommands::Show(show_args)) => {
            show_hat(&mut stdout, &registry, &show_args.name, use_colors)
        }
        Some(HatsCommands::Validate) => validate_hats(&mut stdout, &config, &registry, use_colors),
        Some(HatsCommands::Graph { format, backend }) => {
            graph_hats(&mut stdout, &config, &registry, format, backend.as_deref())
        }
        Some(HatsCommands::ListPresets { .. }) => unreachable!("handled above"),
    }
}

/// Preset file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PresetFormat {
    /// Native ralph shape: a single `.yml` / `.yaml` file.
    Yaml,
    /// Multi-file TOML shape: a directory with `autoloops.toml` + `topology.toml`.
    Toml,
}

impl PresetFormat {
    fn label(self) -> &'static str {
        match self {
            PresetFormat::Yaml => "yaml",
            PresetFormat::Toml => "toml",
        }
    }
}

/// A single discovered preset (any format).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredPreset {
    /// Preset name (file stem for YAML, directory basename for TOML).
    pub name: String,
    /// Absolute path to the preset file or directory.
    pub path: PathBuf,
    /// Which resolver path class matched (`project`, `xdg`, `home`, `autoloop`, `env`).
    pub source: &'static str,
    /// Format on disk.
    pub format: PresetFormat,
    /// One-line description if discoverable.
    pub description: Option<String>,
}

/// Walk the preset resolver paths and return every preset found.
///
/// Covers BOTH preset formats:
/// - YAML single-file presets (`<root>/*.yml`)
/// - TOML multi-file preset directories (`<root>/<name>/autoloops.toml + topology.toml`)
///
/// Roots, in order:
/// 1. `./presets/` (project-local)
/// 2. `$XDG_CONFIG_HOME/ralph/presets/` (user, canonical)
/// 3. `$HOME/.config/ralph/presets/` (user, fallback for #2)
/// 4. `$HOME/.config/autoloop/presets/` (shared with autoloop CLI; back-compat)
/// 5. `$RALPH_PRESETS_DIR/` (explicit override)
/// 6. `$AUTOLOOP_PRESETS_DIR/` (deprecated alias for #5)
///
/// First-wins on name collisions.
pub(crate) fn discover_presets() -> Vec<DiscoveredPreset> {
    let mut roots: Vec<(&'static str, PathBuf)> = Vec::new();

    roots.push(("project", PathBuf::from("presets")));
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        roots.push(("xdg", PathBuf::from(xdg).join("ralph/presets")));
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        roots.push(("home", home.join(".config/ralph/presets")));
        roots.push(("autoloop", home.join(".config/autoloop/presets")));
    }
    if let Ok(explicit) = std::env::var("RALPH_PRESETS_DIR") {
        roots.push(("env", PathBuf::from(explicit)));
    }
    if let Ok(explicit) = std::env::var("AUTOLOOP_PRESETS_DIR") {
        roots.push(("env", PathBuf::from(explicit)));
    }

    discover_in_roots(&roots)
}

/// Inner discovery that takes explicit roots so tests can exercise the logic
/// without mutating process-wide environment variables (crate forbids unsafe,
/// which `std::env::set_var` requires).
fn discover_in_roots(roots: &[(&'static str, PathBuf)]) -> Vec<DiscoveredPreset> {
    // Preserve first-wins semantics by keying on name and skipping duplicates.
    let mut by_name: BTreeMap<String, DiscoveredPreset> = BTreeMap::new();
    for (source, root) in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(discovered) = classify_entry(&path, source) else {
                continue;
            };
            // Skip if already discovered in an earlier root (first-wins).
            if by_name.contains_key(&discovered.name) {
                continue;
            }
            by_name.insert(discovered.name.clone(), discovered);
        }
    }
    by_name.into_values().collect()
}

/// Classify a filesystem entry as a YAML preset, TOML preset dir, or neither.
fn classify_entry(path: &Path, source: &'static str) -> Option<DiscoveredPreset> {
    if is_toml_preset_dir(path) {
        let name = path.file_name()?.to_str()?.to_string();
        return Some(DiscoveredPreset {
            name,
            path: path.to_path_buf(),
            source,
            format: PresetFormat::Toml,
            description: read_toml_preset_description(path),
        });
    }
    if is_yaml_preset_file(path) {
        let name = path.file_stem()?.to_str()?.to_string();
        return Some(DiscoveredPreset {
            name,
            path: path.to_path_buf(),
            source,
            format: PresetFormat::Yaml,
            description: read_yaml_preset_description(path),
        });
    }
    None
}

/// True if `path` is a readable `.yml` / `.yaml` file.
fn is_yaml_preset_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("yml" | "yaml")
    )
}

/// Read a one-line description from a TOML preset directory: `README.md`
/// first prose line, falling back to the first `#` comment in `autoloops.toml`.
fn read_toml_preset_description(preset_dir: &Path) -> Option<String> {
    if let Some(desc) = first_readme_prose_line(&preset_dir.join("README.md")) {
        return Some(desc);
    }
    let autoloops = preset_dir.join("autoloops.toml");
    if let Ok(contents) = std::fs::read_to_string(&autoloops) {
        for line in contents.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("# ") {
                let rest = rest.trim();
                if !rest.is_empty() {
                    return Some(truncate_with_ellipsis(rest, 80));
                }
            }
        }
    }
    None
}

/// Read a one-line description from a YAML preset: first `#` comment line
/// in the file (skipping shebang / blank / ABOUTME preamble).
fn read_yaml_preset_description(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let rest = rest.trim();
            if rest.is_empty() || rest.eq_ignore_ascii_case("ralph.yml") {
                continue;
            }
            return Some(truncate_with_ellipsis(rest, 80));
        }
        // First non-comment line means no header block — give up.
        return None;
    }
    None
}

/// Return the first non-empty, non-heading line of `README.md`, or the first
/// heading's text if no prose line exists.
fn first_readme_prose_line(readme: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(readme).ok()?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return Some(truncate_with_ellipsis(trimmed, 80));
    }
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return Some(truncate_with_ellipsis(rest.trim(), 80));
        }
    }
    None
}

fn list_presets_table<W: Write>(
    writer: &mut W,
    presets: &[DiscoveredPreset],
    _use_colors: bool,
) -> Result<()> {
    if presets.is_empty() {
        writeln!(
            writer,
            "No presets found. Searched:\n  - ./presets/\n  - $XDG_CONFIG_HOME/ralph/presets/\n  - $HOME/.config/ralph/presets/\n  - $HOME/.config/autoloop/presets/\n  - $RALPH_PRESETS_DIR/\n  - $AUTOLOOP_PRESETS_DIR/ (deprecated)\n\nDrop a YAML preset or TOML-dir preset in any of the above. Example:\n  mkdir -p ~/.config/ralph/presets\n  ln -s /path/to/autoloop/packages/presets/presets/autocode ~/.config/ralph/presets/\n\nOr set:  export RALPH_PRESETS_DIR=/path/to/your/presets"
        )?;
        return Ok(());
    }

    writeln!(
        writer,
        "{:<22} {:<6} {:<9} DESCRIPTION",
        "PRESET", "FORMAT", "SOURCE"
    )?;
    writeln!(writer, "{}", "-".repeat(80))?;

    for p in presets {
        let desc = p.description.as_deref().unwrap_or("-");
        let desc = truncate_with_ellipsis(desc, 40);
        writeln!(
            writer,
            "{:<22} {:<6} {:<9} {}",
            p.name,
            p.format.label(),
            p.source,
            desc
        )?;
    }
    writeln!(writer)?;
    writeln!(
        writer,
        "Run a preset with:  ralph run -H <PRESET> -P PROMPT.md"
    )?;
    Ok(())
}

fn list_presets_json<W: Write>(writer: &mut W, presets: &[DiscoveredPreset]) -> Result<()> {
    serde_json::to_writer_pretty(&mut *writer, presets)?;
    writeln!(writer)?;
    Ok(())
}

fn list_hats_json<W: Write>(writer: &mut W, registry: &HatRegistry) -> Result<()> {
    let hats: Vec<_> = registry.all().collect();
    serde_json::to_writer_pretty(&mut *writer, &hats)?;
    writeln!(writer)?;
    Ok(())
}

fn list_hats<W: Write>(writer: &mut W, registry: &HatRegistry, _use_colors: bool) -> Result<()> {
    if registry.is_empty() {
        writeln!(
            writer,
            "No custom hats configured (using default HatlessRalph coordination)."
        )?;
        return Ok(());
    }

    writeln!(writer, "{:<20} DESCRIPTION", "HAT")?;
    writeln!(writer, "{}", "-".repeat(80))?;

    // Sort by name for consistent output
    let mut hats: Vec<_> = registry.all().collect();
    hats.sort_by(|a, b| a.name.cmp(&b.name));

    for hat in hats {
        let desc = if hat.description.is_empty() {
            "-"
        } else {
            &hat.description
        };

        // Truncate desc if too long
        let desc = truncate_with_ellipsis(desc, 55);

        writeln!(writer, "{:<20} {}", hat.name, desc)?;
    }
    Ok(())
}

fn validate_hats<W: Write>(
    writer: &mut W,
    config: &RalphConfig,
    registry: &HatRegistry,
    use_colors: bool,
) -> Result<()> {
    writeln!(writer, "Hat Topology Validation")?;
    writeln!(writer, "=======================")?;
    writeln!(writer)?;

    if registry.is_empty() {
        writeln!(writer, "No hats configured (solo mode).")?;
        return Ok(());
    }

    writeln!(writer, "Hats: {} configured", registry.len())?;
    if let Some(start) = &config.event_loop.starting_event {
        writeln!(writer, "Entry: task.start -> {}", start)?;
    } else {
        writeln!(writer, "Entry: task.start (Ralph coordinates)")?;
    }
    writeln!(writer)?;

    writeln!(writer, "Checks:")?;

    let mut warnings = 0;
    let mut errors = 0;

    // 1. Starting event validation
    if let Some(start) = &config.event_loop.starting_event {
        if registry.has_subscriber(start) {
            let hat = registry.get_for_topic(start).unwrap();
            print_check(
                writer,
                CheckResult::Ok,
                &format!("Starting event '{}' has subscriber ({})", start, hat.name),
                use_colors,
            )?;
        } else {
            print_check(
                writer,
                CheckResult::Error,
                &format!("starting_event '{}' has no subscribers", start),
                use_colors,
            )?;
            errors += 1;
        }
    }

    // 2. Orphan event detection (published but no subscribers)
    for hat in registry.all() {
        for pub_event in &hat.publishes {
            let topic = pub_event.as_str();
            // Ignore loop completion promise
            if topic == config.event_loop.completion_promise {
                continue;
            }
            // Ignore if Ralph subscribes (task.start, etc - though Ralph usually PUBLISHES task.start)
            // Ralph conceptually subscribes to everything as fallback, but we want to warn if no SPECIFIC hat handles it.
            if !registry.has_subscriber(topic) {
                print_check(
                    writer,
                    CheckResult::Warn,
                    &format!(
                        "Event '{}' published by '{}' has no hat subscribers",
                        topic, hat.name
                    ),
                    use_colors,
                )?;
                warnings += 1;
            }
        }
    }

    // 3. Dead end detection
    let mut dead_ends = 0;
    for hat in registry.all() {
        if hat.publishes.is_empty() {
            // It's okay to be a dead end if it's the Summarizer (which outputs completion promise via stdout/file, not event)
            // But usually they publish something.
            // Just info.
            // print_check(CheckResult::Ok, &format!("Hat '{}' is a dead end (publishes nothing)", hat.name), use_colors);
            dead_ends += 1;
        }
    }
    if dead_ends == 0 {
        print_check(writer, CheckResult::Ok, "No dead-end hats", use_colors)?;
    }

    writeln!(writer)?;
    if errors > 0 {
        writeln!(
            writer,
            "Result: Invalid ({} errors, {} warnings)",
            errors, warnings
        )?;
        // Return error to propagate failure to main
        return Err(anyhow::anyhow!("Validation failed with {} errors", errors));
    } else if warnings > 0 {
        writeln!(writer, "Result: Valid ({} warnings)", warnings)?;
    } else {
        writeln!(writer, "Result: Valid")?;
    }
    Ok(())
}

enum CheckResult {
    Ok,
    Warn,
    Error,
}

fn print_check<W: Write>(
    writer: &mut W,
    result: CheckResult,
    msg: &str,
    use_colors: bool,
) -> Result<()> {
    if use_colors {
        match result {
            CheckResult::Ok => {
                writeln!(writer, "  [{}ok{}] {}", colors::GREEN, colors::RESET, msg)?
            }
            CheckResult::Warn => writeln!(
                writer,
                "  [{}warn{}] {}",
                colors::YELLOW,
                colors::RESET,
                msg
            )?,
            CheckResult::Error => {
                writeln!(writer, "  [{}err{}] {}", colors::RED, colors::RESET, msg)?
            }
        }
    } else {
        match result {
            CheckResult::Ok => writeln!(writer, "  [ok] {}", msg)?,
            CheckResult::Warn => writeln!(writer, "  [warn] {}", msg)?,
            CheckResult::Error => writeln!(writer, "  [err] {}", msg)?,
        }
    }
    Ok(())
}

fn graph_hats<W: Write>(
    writer: &mut W,
    config: &RalphConfig,
    registry: &HatRegistry,
    format: GraphFormat,
    backend_override: Option<&str>,
) -> Result<()> {
    match format {
        GraphFormat::Mermaid => {
            writeln!(writer, "```mermaid")?;
            write!(writer, "{}", generate_mermaid_string(registry))?;
            writeln!(writer, "```")?;
        }
        GraphFormat::Compact => {
            write!(writer, "{}", generate_compact_graph(registry))?;
        }
        GraphFormat::Unicode | GraphFormat::Ascii => {
            // Generate diagram via AI backend
            let rendered = render_hat_dag_via_ai(config, registry, backend_override)?;
            write!(writer, "{}", rendered)?;
        }
    }
    Ok(())
}

/// Render hat topology as ASCII DAG by calling an AI backend.
///
/// Shows the logical flow: task.start -> Ralph -> Hats
/// Uses the configured backend (or auto-detects) to generate the diagram.
fn render_hat_dag_via_ai(
    config: &RalphConfig,
    registry: &HatRegistry,
    backend_override: Option<&str>,
) -> Result<String> {
    if registry.is_empty() {
        return Ok("No hats configured.\n".to_string());
    }

    // Resolve backend: CLI flag > config > auto-detect
    let backend_name = resolve_backend(backend_override, config)?;

    // Build the prompt describing the graph
    let prompt = build_diagram_prompt(registry);

    // Create backend and generate diagram
    let backend = CliBackend::from_name(&backend_name)
        .map_err(|e| anyhow::anyhow!("Failed to create backend '{}': {}", backend_name, e))?;

    // Show spinner while generating
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("valid template"),
    );
    spinner.set_message(format!("Generating diagram via {}...", backend_name));
    spinner.enable_steady_tick(Duration::from_millis(100));

    // Build command for non-interactive mode
    let (command, args, stdin_input, _temp_file) = backend.build_command(&prompt, false);

    // Spawn and capture output
    let mut child = Command::new(&command)
        .args(&args)
        .stdin(if stdin_input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn backend command: {}", command))?;

    // Send stdin if needed
    if let Some(input) = stdin_input
        && let Some(mut stdin) = child.stdin.take()
    {
        use std::io::Write;
        stdin.write_all(input.as_bytes())?;
    }

    // Wait for completion
    let output = child
        .wait_with_output()
        .context("Failed to wait for backend")?;

    spinner.finish_and_clear();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Backend '{}' failed (exit code: {:?}):\n{}",
            backend_name,
            output.status.code(),
            stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "Backend '{}' returned empty output",
            backend_name
        ));
    }

    // Extract just the ASCII diagram from the response
    Ok(extract_diagram(&stdout))
}

/// Resolves which backend to use for diagram generation.
///
/// Precedence (highest to lowest):
/// 1. CLI flag (`--backend`)
/// 2. Config file (`cli.backend` in ralph.yml)
/// 3. Auto-detect (first available from claude → kiro → gemini → codex → amp)
fn resolve_backend(flag_override: Option<&str>, config: &RalphConfig) -> Result<String> {
    // 1. CLI flag takes precedence
    if let Some(backend) = flag_override {
        validate_backend_name(backend)?;
        return Ok(backend.to_string());
    }

    // 2. Check config (if not "auto")
    if config.cli.backend != "auto" {
        return Ok(config.cli.backend.clone());
    }

    // 3. Auto-detect
    detect_backend_default().map_err(|e| anyhow::anyhow!("{}", e))
}

/// Validates a backend name.
fn validate_backend_name(name: &str) -> Result<()> {
    if !backend_support::is_known_backend(name) {
        return Err(anyhow::anyhow!(
            "{}",
            backend_support::unknown_backend_message(name)
        ));
    }

    Ok(())
}

/// Builds the prompt for diagram generation.
fn build_diagram_prompt(registry: &HatRegistry) -> String {
    let mut prompt = String::from(
        "Generate an ASCII diagram showing this directed acyclic graph.\n\
         Use simple box-drawing characters that work in any terminal.\n\
         Show clear arrows between nodes.\n\n\
         Nodes and edges:\n",
    );

    prompt.push_str("- task.start → Ralph\n");

    // Collect all hats sorted for deterministic output
    let mut hats: Vec<_> = registry.all().collect();
    hats.sort_by(|a, b| a.name.cmp(&b.name));

    // Ralph -> Hats (based on subscriptions)
    for hat in &hats {
        for sub in &hat.subscriptions {
            prompt.push_str(&format!(
                "- Ralph → {} (triggers on: {})\n",
                hat.name,
                sub.as_str()
            ));
        }
    }

    // Hats -> Ralph (based on publishes)
    for hat in &hats {
        for pub_event in &hat.publishes {
            prompt.push_str(&format!(
                "- {} → Ralph (publishes: {})\n",
                hat.name,
                pub_event.as_str()
            ));
        }
    }

    // Hat -> Hat (direct flows)
    for source in &hats {
        for pub_event in &source.publishes {
            for target in &hats {
                if target.id == source.id {
                    continue;
                }
                if target
                    .subscriptions
                    .iter()
                    .any(|s| s.as_str() == pub_event.as_str())
                {
                    prompt.push_str(&format!(
                        "- {} → {} (via event: {})\n",
                        source.name,
                        target.name,
                        pub_event.as_str()
                    ));
                }
            }
        }
    }

    prompt.push_str("\nOutput ONLY the ASCII diagram, no explanation or markdown fences.");
    prompt
}

/// Extracts the ASCII diagram from the AI response.
/// Removes any markdown fences or explanatory text.
fn extract_diagram(response: &str) -> String {
    let mut lines: Vec<&str> = response.lines().collect();

    // Remove leading/trailing markdown fences
    if lines.first().is_some_and(|l| l.starts_with("```")) {
        lines.remove(0);
    }
    if lines.last().is_some_and(|l| l.starts_with("```")) {
        lines.pop();
    }

    // Remove any leading blank lines or "Here is" type intros
    while lines
        .first()
        .is_some_and(|l| l.trim().is_empty() || l.to_lowercase().starts_with("here"))
    {
        lines.remove(0);
    }

    let result = lines.join("\n");
    if result.ends_with('\n') {
        result
    } else {
        format!("{}\n", result)
    }
}

fn generate_compact_graph(registry: &HatRegistry) -> String {
    if registry.is_empty() {
        return "No hats configured.\n".to_string();
    }

    let mut output = String::new();
    output.push_str("Graph:\n");
    output.push_str("  task.start -> Ralph\n");

    // Sort hats for deterministic output
    let mut hats: Vec<_> = registry.all().collect();
    hats.sort_by(|a, b| a.name.cmp(&b.name));

    for hat in &hats {
        output.push_str(&format!("  Ralph -> {}\n", hat.name));

        for publish in &hat.publishes {
            output.push_str(&format!("    {} => {}\n", hat.name, publish.as_str()));
        }

        for subscription in &hat.subscriptions {
            output.push_str(&format!("    {} <= {}\n", hat.name, subscription.as_str()));
        }
    }

    if !output.ends_with('\n') {
        output.push('\n');
    }

    output
}

/// Generate Mermaid flowchart syntax for the hat topology.
fn generate_mermaid_string(registry: &HatRegistry) -> String {
    let mut output = String::new();
    output.push_str("flowchart LR\n");
    output.push_str("    Start[task.start] --> Ralph\n");

    // Reconstruct Ralph's publishes (what hats subscribe to)
    let mut ralph_publishes: HashSet<String> = HashSet::new();
    for hat in registry.all() {
        for sub in &hat.subscriptions {
            ralph_publishes.insert(sub.as_str().to_string());
        }
    }

    // Ralph -> Hats
    for hat in registry.all() {
        let node_id = sanitize_id(&hat.name);
        for sub in &hat.subscriptions {
            output.push_str(&format!("    Ralph -->|{}| {}\n", sub.as_str(), node_id));
        }
    }

    // Hats -> Ralph
    for hat in registry.all() {
        let node_id = sanitize_id(&hat.name);
        for pub_event in &hat.publishes {
            output.push_str(&format!(
                "    {} -->|{}| Ralph\n",
                node_id,
                pub_event.as_str()
            ));
        }
    }

    // Hat -> Hat (direct flow visualization)
    // Even though everything goes through Ralph, it's useful to see A -> B
    for source in registry.all() {
        let source_id = sanitize_id(&source.name);
        for pub_event in &source.publishes {
            // Find hats that subscribe to this
            for target in registry.all() {
                if target.id == source.id {
                    continue;
                }
                if target
                    .subscriptions
                    .iter()
                    .any(|s| s.as_str() == pub_event.as_str())
                {
                    let target_id = sanitize_id(&target.name);
                    output.push_str(&format!(
                        "    {} -.->|{}| {}\n",
                        source_id,
                        pub_event.as_str(),
                        target_id
                    ));
                }
            }
        }
    }

    output
}

fn sanitize_id(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).collect()
}

fn show_hat<W: Write>(
    writer: &mut W,
    registry: &HatRegistry,
    name: &str,
    use_colors: bool,
) -> Result<()> {
    // Try to find by ID first, then by display name
    let hat = registry
        .all()
        .find(|h| h.id.as_str() == name || h.name == name);

    let hat = hat.context(format!("Hat '{}' not found", name))?;

    if use_colors {
        writeln!(writer, "{}{}{}", colors::BOLD, hat.name, colors::RESET)?;
    } else {
        writeln!(writer, "{}", hat.name)?;
    }

    if !hat.description.is_empty() {
        writeln!(writer, "{}", hat.description)?;
    }
    writeln!(writer)?;

    writeln!(writer, "ID: {}", hat.id)?;

    writeln!(writer, "\nTriggers On:")?;
    if hat.subscriptions.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for trigger in &hat.subscriptions {
            writeln!(writer, "  - {}", trigger.as_str())?;
        }
    }

    writeln!(writer, "\nPublishes:")?;
    if hat.publishes.is_empty() {
        writeln!(writer, "  (none)")?;
    } else {
        for topic in &hat.publishes {
            writeln!(writer, "  - {}", topic.as_str())?;
        }
    }

    if !hat.instructions.is_empty() {
        writeln!(writer, "\nInstructions:")?;
        for line in hat.instructions.lines() {
            writeln!(writer, "  {}", line)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_proto::Hat;

    fn mock_hat(name: &str, subs: &[&str], pubs: &[&str]) -> Hat {
        let mut hat = Hat::new(sanitize_id(name), name);
        hat.description = format!("Description for {}", name);
        hat.subscriptions = subs.iter().map(|s| (*s).into()).collect();
        hat.publishes = pubs.iter().map(|s| (*s).into()).collect();
        hat
    }

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("My Hat"), "MyHat");
        assert_eq!(sanitize_id("cool-hat"), "coolhat");
        assert_eq!(sanitize_id("Hat!@#"), "Hat");
        assert_eq!(sanitize_id("123"), "123");
    }

    #[test]
    fn test_list_hats_empty() {
        let registry = HatRegistry::new();
        let mut buf = Vec::new();
        list_hats(&mut buf, &registry, false).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("No custom hats configured"));
    }

    #[test]
    fn test_list_hats_with_entries() {
        let mut registry = HatRegistry::new();
        registry.register(mock_hat("Builder", &["build.task"], &["build.done"]));
        registry.register(mock_hat("Planner", &["plan.start"], &["build.task"]));

        let mut buf = Vec::new();
        list_hats(&mut buf, &registry, false).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("HAT                  DESCRIPTION"));
        assert!(output.contains("Builder"));
        assert!(output.contains("Planner"));
    }

    #[test]
    fn test_validate_hats_orphan() {
        let mut registry = HatRegistry::new();
        // Builder publishes build.done, but no one listens
        registry.register(mock_hat("Builder", &["build.task"], &["build.done"]));

        let config = RalphConfig::default();
        let mut buf = Vec::new();

        // Validation might exit process on error, so we test warning scenario
        validate_hats(&mut buf, &config, &registry, false).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Should warn about build.done having no subscribers
        assert!(
            output.contains("Event 'build.done' published by 'Builder' has no hat subscribers")
        );
        assert!(output.contains("Result: Valid (1 warnings)"));
    }

    #[test]
    fn test_graph_hats_compact() {
        let mut registry = HatRegistry::new();
        registry.register(mock_hat("Builder", &["build.task"], &["build.done"]));
        registry.register(mock_hat("Planner", &["planner.start"], &["planner.done"]));

        let config = RalphConfig::default();
        let mut buf = Vec::new();

        graph_hats(&mut buf, &config, &registry, GraphFormat::Compact, None).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Graph:"));
        assert!(output.contains("task.start -> Ralph"));
        assert!(output.contains("Ralph -> Builder"));
        assert!(
            output.contains("Builder => build.task") || output.contains("Builder <= build.task")
        );
    }

    #[test]
    #[ignore = "requires live AI backend"]
    fn test_graph_hats_ascii() {
        let mut registry = HatRegistry::new();
        registry.register(mock_hat("Builder", &["build.task"], &["build.done"]));

        let config = RalphConfig::default();
        let mut buf = Vec::new();

        graph_hats(&mut buf, &config, &registry, GraphFormat::Ascii, None).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // AI-generated output should contain the node names
        assert!(output.contains("Builder") || output.contains("Ralph"));
    }

    #[test]
    #[ignore = "requires live AI backend"]
    fn test_graph_hats_unicode() {
        let mut registry = HatRegistry::new();
        registry.register(mock_hat("Coder", &["code.task"], &["code.done"]));

        let config = RalphConfig::default();
        let mut buf = Vec::new();

        graph_hats(&mut buf, &config, &registry, GraphFormat::Unicode, None).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // AI-generated output should contain node names
        assert!(output.contains("Coder") || output.contains("Ralph"));
    }

    #[test]
    fn test_generate_mermaid_string() {
        let mut registry = HatRegistry::new();
        registry.register(mock_hat("A", &["start"], &["mid"]));
        registry.register(mock_hat("B", &["mid"], &["end"]));

        let output = generate_mermaid_string(&registry);

        assert!(output.contains("flowchart LR"));
        assert!(output.contains("Ralph -->|start| A"));
        assert!(output.contains("A -->|mid| Ralph"));
        assert!(output.contains("Ralph -->|mid| B"));
        // Hat-to-hat connection (A publishes mid, B subscribes to mid)
        assert!(output.contains("A -.->|mid| B"));
    }

    #[test]
    fn test_show_hat_found() {
        let mut registry = HatRegistry::new();
        registry.register(mock_hat("Builder", &["build.task"], &["build.done"]));

        let mut buf = Vec::new();
        show_hat(&mut buf, &registry, "Builder", false).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Builder"));
        assert!(output.contains("Triggers On:"));
        assert!(output.contains("build.task"));
        assert!(output.contains("Publishes:"));
        assert!(output.contains("build.done"));
    }

    #[test]
    fn test_show_hat_not_found() {
        let registry = HatRegistry::new();
        let mut buf = Vec::new();
        let result = show_hat(&mut buf, &registry, "Nonexistent", false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_validate_hats_empty_registry() {
        let registry = HatRegistry::new();
        let config = RalphConfig::default();
        let mut buf = Vec::new();

        validate_hats(&mut buf, &config, &registry, false).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("No hats configured"));
    }

    #[test]
    fn test_validate_hats_valid_topology() {
        let mut registry = HatRegistry::new();
        // Create a closed loop: A subscribes to start, publishes mid; B subscribes to mid
        registry.register(mock_hat("A", &["start"], &["mid"]));
        registry.register(mock_hat("B", &["mid"], &[]));

        let config = RalphConfig::default();
        let mut buf = Vec::new();

        validate_hats(&mut buf, &config, &registry, false).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("No dead-end hats") || output.contains("Result: Valid"));
    }

    #[test]
    fn test_list_hats_json() {
        let mut registry = HatRegistry::new();
        registry.register(mock_hat("Builder", &["build.task"], &["build.done"]));

        let mut buf = Vec::new();
        list_hats_json(&mut buf, &registry).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_print_check_ok() {
        let mut buf = Vec::new();
        print_check(&mut buf, CheckResult::Ok, "Test passed", false).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[ok]"));
        assert!(output.contains("Test passed"));
    }

    #[test]
    fn test_print_check_warn() {
        let mut buf = Vec::new();
        print_check(&mut buf, CheckResult::Warn, "Warning message", false).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[warn]"));
        assert!(output.contains("Warning message"));
    }

    #[test]
    fn test_print_check_error() {
        let mut buf = Vec::new();
        print_check(&mut buf, CheckResult::Error, "Error message", false).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[err]"));
        assert!(output.contains("Error message"));
    }

    #[test]
    fn test_print_check_colored() {
        let mut buf = Vec::new();
        print_check(&mut buf, CheckResult::Ok, "Color test", true).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // Should contain ANSI color codes
        assert!(output.contains("\x1b["));
    }

    #[test]
    fn test_list_hats_truncates_long_description() {
        let mut registry = HatRegistry::new();
        let mut hat = mock_hat("LongDesc", &["start"], &["end"]);
        hat.description = "A".repeat(100); // Very long description
        registry.register(hat);

        let mut buf = Vec::new();
        list_hats(&mut buf, &registry, false).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Description should be truncated with "..."
        assert!(output.contains("..."));
    }

    #[test]
    fn test_build_diagram_prompt() {
        let mut registry = HatRegistry::new();
        registry.register(mock_hat("Builder", &["build.task"], &["build.done"]));
        registry.register(mock_hat("Tester", &["test.task"], &["test.done"]));

        let prompt = build_diagram_prompt(&registry);

        // Should contain the key elements
        assert!(prompt.contains("task.start → Ralph"));
        assert!(prompt.contains("Ralph → Builder"));
        assert!(prompt.contains("build.task"));
        assert!(prompt.contains("build.done"));
        assert!(prompt.contains("Ralph → Tester"));
        assert!(prompt.contains("Output ONLY the ASCII diagram"));
    }

    #[test]
    fn test_extract_diagram_plain() {
        let response = "┌─────┐\n│Ralph│\n└─────┘";
        let diagram = extract_diagram(response);
        assert!(diagram.contains("Ralph"));
        assert!(diagram.ends_with('\n'));
    }

    #[test]
    fn test_extract_diagram_with_markdown_fences() {
        let response = "```\n┌─────┐\n│Ralph│\n└─────┘\n```";
        let diagram = extract_diagram(response);
        assert!(diagram.contains("Ralph"));
        assert!(!diagram.contains("```"));
    }

    #[test]
    fn test_extract_diagram_with_intro() {
        let response = "Here is the diagram:\n\n┌─────┐\n│Ralph│\n└─────┘";
        let diagram = extract_diagram(response);
        assert!(diagram.contains("Ralph"));
        assert!(!diagram.to_lowercase().contains("here"));
    }

    #[test]
    fn test_validate_backend_name_valid() {
        assert!(validate_backend_name("claude").is_ok());
        assert!(validate_backend_name("kiro").is_ok());
        assert!(validate_backend_name("gemini").is_ok());
        assert!(validate_backend_name("codex").is_ok());
        assert!(validate_backend_name("amp").is_ok());
        assert!(validate_backend_name("custom").is_ok());
    }

    #[test]
    fn test_validate_backend_name_invalid() {
        let result = validate_backend_name("unknown-backend");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unknown backend"));
        assert!(err.contains("Valid backends"));
    }

    #[test]
    fn test_resolve_backend_flag_override() {
        let config = RalphConfig::default();
        let result = resolve_backend(Some("kiro"), &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "kiro");
    }

    #[test]
    fn test_resolve_backend_from_config() {
        let mut config = RalphConfig::default();
        config.cli.backend = "gemini".to_string();

        let result = resolve_backend(None, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "gemini");
    }

    /// Write a minimal autoloop preset skeleton under `parent/<name>/`.
    fn write_preset_skeleton(parent: &std::path::Path, name: &str, readme: Option<&str>) {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("autoloops.toml"), "# test preset\n").unwrap();
        std::fs::write(dir.join("topology.toml"), "name = \"test\"\n").unwrap();
        if let Some(body) = readme {
            std::fs::write(dir.join("README.md"), body).unwrap();
        }
    }

    #[test]
    fn test_discover_in_roots_finds_presets_and_skips_nonpresets() {
        let tmp = tempfile::tempdir().unwrap();
        write_preset_skeleton(
            tmp.path(),
            "autotest-fixture",
            Some("Fixture preset for tests.\n"),
        );
        write_preset_skeleton(tmp.path(), "another-fixture", None);
        // Non-preset dir should be skipped.
        std::fs::create_dir_all(tmp.path().join("not-a-preset")).unwrap();

        let roots: Vec<(&'static str, PathBuf)> = vec![("env", tmp.path().to_path_buf())];
        let presets = discover_in_roots(&roots);
        let names: Vec<_> = presets.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"autotest-fixture"), "got {:?}", names);
        assert!(names.contains(&"another-fixture"), "got {:?}", names);
        assert!(!names.contains(&"not-a-preset"), "got {:?}", names);

        let with_readme = presets
            .iter()
            .find(|p| p.name == "autotest-fixture")
            .unwrap();
        assert_eq!(with_readme.source, "env");
        assert_eq!(
            with_readme.description.as_deref(),
            Some("Fixture preset for tests.")
        );

        let without_readme = presets
            .iter()
            .find(|p| p.name == "another-fixture")
            .unwrap();
        // Fixture's autoloops.toml starts with `# test preset` — description should
        // fall back to that when there's no README.md.
        assert_eq!(without_readme.description.as_deref(), Some("test preset"));
    }

    #[test]
    fn test_discover_in_roots_respects_first_wins_across_roots() {
        let root_a = tempfile::tempdir().unwrap();
        let root_b = tempfile::tempdir().unwrap();
        write_preset_skeleton(root_a.path(), "shared", Some("From root A.\n"));
        write_preset_skeleton(root_b.path(), "shared", Some("From root B.\n"));
        write_preset_skeleton(root_b.path(), "b-only", None);

        let roots: Vec<(&'static str, PathBuf)> = vec![
            ("project", root_a.path().to_path_buf()),
            ("env", root_b.path().to_path_buf()),
        ];
        let presets = discover_in_roots(&roots);
        let shared = presets.iter().find(|p| p.name == "shared").unwrap();
        assert_eq!(shared.source, "project", "first-wins should pick root A");
        assert_eq!(shared.description.as_deref(), Some("From root A."));
        assert!(presets.iter().any(|p| p.name == "b-only"));
    }

    #[test]
    fn test_discover_in_roots_missing_roots_are_ignored() {
        let roots: Vec<(&'static str, PathBuf)> = vec![
            ("project", PathBuf::from("/nonexistent/ralph/presets-a")),
            ("env", PathBuf::from("/nonexistent/ralph/presets-b")),
        ];
        let presets = discover_in_roots(&roots);
        assert!(presets.is_empty());
    }

    #[test]
    fn test_list_presets_table_empty_prints_search_paths() {
        let mut buf = Vec::new();
        list_presets_table(&mut buf, &[], false).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("No presets found"));
        assert!(output.contains("./presets/"));
        assert!(output.contains("XDG_CONFIG_HOME"));
        assert!(output.contains("RALPH_PRESETS_DIR"));
        assert!(output.contains("AUTOLOOP_PRESETS_DIR"));
    }

    #[test]
    fn test_list_presets_json_is_valid_array() {
        let sample = vec![DiscoveredPreset {
            name: "demo".into(),
            path: PathBuf::from("/tmp/demo"),
            source: "env",
            format: PresetFormat::Toml,
            description: Some("demo preset".into()),
        }];
        let mut buf = Vec::new();
        list_presets_json(&mut buf, &sample).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "demo");
        assert_eq!(arr[0]["source"], "env");
        assert_eq!(arr[0]["format"], "toml");
    }

    #[test]
    fn test_read_toml_preset_description_falls_back_to_toml_comment() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("p");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("autoloops.toml"),
            "# This preset does a thing.\nevent_loop.max_iterations = 10\n",
        )
        .unwrap();
        std::fs::write(dir.join("topology.toml"), "name = \"p\"\n").unwrap();
        let desc = read_toml_preset_description(&dir);
        assert_eq!(desc.as_deref(), Some("This preset does a thing."));
    }

    #[test]
    fn test_discover_in_roots_finds_yaml_presets_alongside_toml() {
        let tmp = tempfile::tempdir().unwrap();
        // TOML preset dir
        write_preset_skeleton(tmp.path(), "toml-preset", Some("A TOML preset.\n"));
        // YAML file preset
        std::fs::write(
            tmp.path().join("yaml-preset.yml"),
            "# A YAML preset for tests.\nhats: {}\n",
        )
        .unwrap();
        // Random file that isn't a preset
        std::fs::write(tmp.path().join("random.txt"), "ignore me").unwrap();

        let roots: Vec<(&'static str, PathBuf)> = vec![("env", tmp.path().to_path_buf())];
        let presets = discover_in_roots(&roots);

        let toml_p = presets.iter().find(|p| p.name == "toml-preset").unwrap();
        assert_eq!(toml_p.format, PresetFormat::Toml);
        assert_eq!(toml_p.description.as_deref(), Some("A TOML preset."));

        let yaml_p = presets.iter().find(|p| p.name == "yaml-preset").unwrap();
        assert_eq!(yaml_p.format, PresetFormat::Yaml);
        assert_eq!(
            yaml_p.description.as_deref(),
            Some("A YAML preset for tests.")
        );

        assert!(!presets.iter().any(|p| p.name == "random"));
    }
}
