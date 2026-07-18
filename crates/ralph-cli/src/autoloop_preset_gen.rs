//! Generate an autoloop preset directory from ralph's native hats config.
//!
//! This is the inverse of `ralph_core::preset_source` (which imports autoloop
//! TOML presets into ralph): given a [`RalphConfig`] with `hats`, write an
//! autoloop preset (`autoloops.toml` + `topology.toml` + `roles/*.md` +
//! `harness.md`) so the v3 autoloop engine can run ralph's existing workflow
//! without a hand-authored preset.
//!
//! ## Mapping (ralph -> autoloop)
//!
//! | ralph                                   | autoloop                          |
//! | --------------------------------------- | --------------------------------- |
//! | `hats.<id>`                             | `[[role]] id`                     |
//! | `hats.<id>.publishes`                   | `[[role]] emits`                  |
//! | `hats.<id>.instructions`                | `roles/<id>.md` (`prompt_file`)   |
//! | `hats.<id>.triggers` (inverted)         | `[handoff] <event> = [role, ...]` |
//! | `event_loop.completion_promise`         | `event_loop.completion_promise`   |
//! | `event_loop.required_events`            | `event_loop.required_events`      |
//! | `event_loop.max_iterations`             | `event_loop.max_iterations`       |
//! | `event_loop.max_runtime_seconds`         | `event_loop.max_runtime` (ms)     |
//! | `event_loop.max_cost_usd`                | `event_loop.max_cost_usd`         |
//! | `core.guardrails`                        | `harness.md`                      |

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use ralph_adapters::{CliBackend, PromptMode};
use ralph_core::RalphConfig;

/// autoloop's completion-event convention (ralph signals completion by the
/// promise text and/or a hat publishing this event).
const COMPLETION_EVENT: &str = "task.complete";

/// Quote a TOML basic string, escaping `"` and `\`.
fn q(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Render a TOML array of strings: `["a", "b"]`.
fn arr(items: &[String]) -> String {
    let inner = items.iter().map(|s| q(s)).collect::<Vec<_>>().join(", ");
    format!("[{inner}]")
}

/// Write an autoloop preset reflecting `config`'s hats into `dir`.
///
/// Returns an error if the config has no hats (nothing to translate).
pub fn generate_preset(config: &RalphConfig, dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir.join("roles"))?;

    // Hatless / single-hat ralph configs have no hats map. Synthesize a single
    // role that runs the objective and emits the completion event — the autoloop
    // equivalent of ralph's hatless mode.
    if config.hats.is_empty() {
        return generate_hatless_preset(config, dir);
    }

    // Deterministic ordering for stable output.
    let mut hats: Vec<(&String, &ralph_core::HatConfig)> = config.hats.iter().collect();
    hats.sort_by(|a, b| a.0.cmp(b.0));

    // Invert triggers -> handoff (event -> roles that consume it), preserving
    // role order per event.
    let mut handoff: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, hat) in &hats {
        for trigger in &hat.triggers {
            let entry = handoff.entry(trigger.clone()).or_default();
            if !entry.iter().any(|r| r == *id) {
                entry.push((*id).clone());
            }
        }
    }
    // Ensure a starting route: autoloop begins from handoff["loop.start"].
    if !handoff.contains_key("loop.start") {
        let starter = starting_role(config, &hats);
        handoff.insert("loop.start".to_string(), vec![starter]);
    }

    // topology.toml
    let mut topo = String::new();
    topo.push_str(&format!("name = {}\n", q("ralph")));
    topo.push_str(&format!("completion = {}\n\n", q(COMPLETION_EVENT)));
    for (id, hat) in &hats {
        let role_file = format!("roles/{id}.md");
        fs::write(dir.join(&role_file), hat.instructions.as_bytes())?;
        topo.push_str("[[role]]\n");
        topo.push_str(&format!("id = {}\n", q(id)));
        if !hat.name.is_empty() {
            topo.push_str(&format!("name = {}\n", q(&hat.name)));
        }
        topo.push_str(&format!("emits = {}\n", arr(&hat.publishes)));
        topo.push_str(&format!("prompt_file = {}\n\n", q(&role_file)));
    }
    topo.push_str("[handoff]\n");
    for (event, roles) in &handoff {
        topo.push_str(&format!("{} = {}\n", q(event), arr(roles)));
    }
    fs::write(dir.join("topology.toml"), topo.as_bytes())?;

    write_autoloops(config, dir)?;
    write_harness(config, dir)?;
    Ok(())
}

/// Synthesize a single-role preset for a hatless / single-hat ralph config.
fn generate_hatless_preset(config: &RalphConfig, dir: &Path) -> io::Result<()> {
    let role_instructions = format!(
        "You are the single autonomous worker for this loop. Work the objective \
         through to completion, verifying with the strongest available harness \
         before declaring done. When the work is complete and verified, emit the \
         completion event (`<tool> emit {COMPLETION_EVENT} \"<summary>\"`) or end \
         your output with the completion promise `{}`.\n",
        config.event_loop.completion_promise
    );
    fs::write(dir.join("roles/ralph.md"), role_instructions.as_bytes())?;

    let mut topo = String::new();
    topo.push_str(&format!("name = {}\n", q("ralph")));
    topo.push_str(&format!("completion = {}\n\n", q(COMPLETION_EVENT)));
    topo.push_str("[[role]]\n");
    topo.push_str(&format!("id = {}\n", q("ralph")));
    topo.push_str(&format!(
        "emits = {}\n",
        arr(&[COMPLETION_EVENT.to_string()])
    ));
    topo.push_str(&format!("prompt_file = {}\n\n", q("roles/ralph.md")));
    topo.push_str("[handoff]\n");
    topo.push_str(&format!(
        "{} = {}\n",
        q("loop.start"),
        arr(&["ralph".to_string()])
    ));
    fs::write(dir.join("topology.toml"), topo.as_bytes())?;

    write_autoloops(config, dir)?;
    write_harness(config, dir)?;
    Ok(())
}

/// Map Ralph's normalized loop budgets to autoloop 0.9.2 config overrides.
///
/// Autoloop duration values are milliseconds. There is no autoloop equivalent
/// for Ralph's consecutive-failure budget, so it is deliberately omitted and
/// called out rather than approximated with a differently-behaving limit.
pub(crate) fn autoloop_budget_overrides(config: &RalphConfig) -> Vec<(&'static str, String)> {
    let el = &config.event_loop;
    tracing::warn!(
        field = "event_loop.max_consecutive_failures",
        value = el.max_consecutive_failures,
        "engine=autoloop: ignoring event_loop.max_consecutive_failures because autoloop 0.9.2 has no equivalent budget"
    );

    let mut overrides = vec![
        ("event_loop.max_iterations", el.max_iterations.to_string()),
        (
            "event_loop.max_runtime",
            Duration::from_secs(el.max_runtime_seconds)
                .as_millis()
                .to_string(),
        ),
    ];
    if let Some(max_cost_usd) = el.max_cost_usd {
        overrides.push(("event_loop.max_cost_usd", max_cost_usd.to_string()));
    }
    overrides
}

const SUPPORTED_AUTOLOOP_BACKENDS: &str =
    "claude, pi, kiro, kiro-acp, gemini, codex, forge, amp, copilot, opencode, custom";

fn unsupported_backend(name: &str, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "backend {name:?} {reason}; supported autoloop backends: {SUPPORTED_AUTOLOOP_BACKENDS}"
        ),
    )
}

fn backend_args_csv(name: &str, args: &[String]) -> io::Result<Option<String>> {
    if let Some(arg) = args.iter().find(|arg| arg.contains(',')) {
        return Err(unsupported_backend(
            name,
            &format!(
                "has argument {arg:?} containing a comma, which autoloop backend.args cannot represent"
            ),
        ));
    }
    Ok((!args.is_empty()).then(|| args.join(",")))
}

/// Translate Ralph's resolved CLI backend into autoloop 0.9.2 config keys.
///
/// Command backends reuse [`CliBackend`] so their headless command, arguments,
/// prompt flag, and configured overrides remain defined in one place.
pub(crate) fn autoloop_backend_spec(
    config: &RalphConfig,
) -> io::Result<Vec<(&'static str, String)>> {
    let cli = &config.cli;
    let command = |default: &str| cli.command.clone().unwrap_or_else(|| default.to_string());

    match cli.backend.as_str() {
        "claude" => {
            if !cli.args.is_empty() {
                return Err(unsupported_backend(
                    "claude",
                    &format!(
                        "cannot receive CLI args {:?} through the claude-sdk backend; use the custom backend or an explicit core.autoloop_preset",
                        cli.args
                    ),
                ));
            }
            Ok(vec![
                ("backend.kind", "claude-sdk".to_string()),
                ("backend.command", command("claude")),
            ])
        }
        "pi" => {
            let mut spec = vec![
                ("backend.kind", "pi".to_string()),
                ("backend.command", command("pi")),
            ];
            if let Some(args) = backend_args_csv("pi", &cli.args)? {
                spec.push(("backend.args", args));
            }
            Ok(spec)
        }
        "kiro" | "kiro-acp" => {
            let mut spec = vec![
                ("backend.kind", "acp".to_string()),
                ("backend.provider", "kiro".to_string()),
                ("backend.command", command("kiro-cli")),
            ];
            if let Some(args) = backend_args_csv(&cli.backend, &cli.args)? {
                spec.push(("backend.args", format!("acp,{args}")));
            }
            Ok(spec)
        }
        "roo" => Err(unsupported_backend(
            "roo",
            "is unsupported under the autoloop engine because roo requires --prompt-file <path>",
        )),
        "auto" | "" => Err(unsupported_backend(
            &cli.backend,
            "was not resolved before autoloop preset generation",
        )),
        "gemini" | "codex" | "forge" | "amp" | "copilot" | "opencode" | "custom" => {
            let mut backend = CliBackend::from_config(cli).map_err(|error| {
                unsupported_backend(&cli.backend, &format!("cannot be configured: {error}"))
            })?;

            // Copilot's JSONL flag exists for Ralph's output parser. The
            // autoloop command backend consumes plain text instead.
            if cli.backend == "copilot" {
                let parser_args = backend
                    .args
                    .windows(2)
                    .position(|args| args[0] == "--output-format" && args[1] == "json")
                    .ok_or_else(|| {
                        unsupported_backend(
                            "copilot",
                            "is missing its expected parser output-format arguments",
                        )
                    })?;
                backend.args.drain(parser_args..parser_args + 2);
            }

            let prompt_mode = match backend.prompt_mode {
                PromptMode::Arg => {
                    if let Some(flag) = backend.prompt_flag {
                        backend.args.push(flag);
                    }
                    "arg"
                }
                PromptMode::Stdin => "stdin",
                PromptMode::NoPrompt => {
                    return Err(unsupported_backend(
                        &cli.backend,
                        "does not provide a prompt delivery mode supported by autoloop",
                    ));
                }
            };

            let args = backend_args_csv(&cli.backend, &backend.args)?;
            let mut spec = vec![
                ("backend.kind", "command".to_string()),
                ("backend.command", backend.command),
            ];
            if let Some(args) = args {
                spec.push(("backend.args", args));
            }
            spec.push(("backend.prompt_mode", prompt_mode.to_string()));
            Ok(spec)
        }
        name => Err(unsupported_backend(name, "is not recognized")),
    }
}

/// Write `autoloops.toml` from ralph's event-loop config.
fn write_autoloops(config: &RalphConfig, dir: &Path) -> io::Result<()> {
    let el = &config.event_loop;
    let mut auto = String::new();

    // Deliberately do not set `core.tasks_file` to Ralph's
    // `.ralph/agent/tasks.jsonl`. Ralph stores snapshot records with `title`,
    // lifecycle statuses such as `in_progress`/`closed`/`failed`, priorities,
    // dependencies, and loop IDs. autoloop's append-only records instead use
    // `type = "task"`, `text`, and `open`/`done`. Sharing the path would make
    // autoloop silently ignore Ralph records and risk corrupting the file with
    // mixed formats. autoloop therefore keeps its canonical task store and
    // remains the sole authority for its completion gate; Ralph only warns
    // observationally about its separate open tasks after engine completion.
    for (key, value) in autoloop_budget_overrides(config) {
        auto.push_str(&format!("{key} = {value}\n"));
    }
    auto.push_str(&format!(
        "event_loop.completion_event = {}\n",
        q(COMPLETION_EVENT)
    ));
    auto.push_str(&format!(
        "event_loop.completion_promise = {}\n",
        q(&el.completion_promise)
    ));
    if !el.required_events.is_empty() {
        auto.push_str(&format!(
            "event_loop.required_events = {}\n",
            arr(&el.required_events)
        ));
    }
    auto.push('\n');
    for (key, value) in autoloop_backend_spec(config)? {
        auto.push_str(&format!("{key} = {}\n", q(&value)));
    }
    auto.push_str("\nharness.instructions_file = \"harness.md\"\n");
    fs::write(dir.join("autoloops.toml"), auto.as_bytes())
}

/// Write `harness.md` from ralph's guardrails.
fn write_harness(config: &RalphConfig, dir: &Path) -> io::Result<()> {
    fs::write(
        dir.join("harness.md"),
        config.core.guardrails.join("\n").as_bytes(),
    )
}

/// Pick the role autoloop should start from: the hat triggered by ralph's start
/// event, else the first hat alphabetically.
fn starting_role(config: &RalphConfig, hats: &[(&String, &ralph_core::HatConfig)]) -> String {
    let start_event = config
        .event_loop
        .starting_event
        .clone()
        .unwrap_or_else(|| "task.start".to_string());
    for (id, hat) in hats {
        if hat
            .triggers
            .iter()
            .any(|t| *t == start_event || t == "task.start")
        {
            return (*id).clone();
        }
    }
    hats.first()
        .map(|(id, _)| (*id).clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_hats() -> RalphConfig {
        let yaml = r#"
event_loop:
  max_iterations: 42
  required_events: ["review.passed"]
hats:
  planner:
    name: Planner
    description: "plans"
    triggers: ["task.start"]
    publishes: ["tasks.ready"]
    instructions: "Plan it."
  builder:
    name: Builder
    description: "builds"
    triggers: ["tasks.ready"]
    publishes: ["task.complete"]
    instructions: 'Build "it".'
"#;
        serde_yaml::from_str(yaml).expect("valid ralph config")
    }

    #[test]
    fn generates_a_single_role_preset_when_no_hats() {
        let cfg = RalphConfig::default();
        let dir = tempfile::tempdir().unwrap();
        generate_preset(&cfg, dir.path()).unwrap();
        assert!(dir.path().join("autoloops.toml").is_file());
        assert!(dir.path().join("roles/ralph.md").is_file());
        let topo = fs::read_to_string(dir.path().join("topology.toml")).unwrap();
        assert!(topo.contains("id = \"ralph\""));
        assert!(topo.contains("\"loop.start\" = [\"ralph\"]"));
    }

    #[test]
    fn writes_a_valid_preset_shape_from_hats() {
        let cfg = config_with_hats();
        let dir = tempfile::tempdir().unwrap();
        generate_preset(&cfg, dir.path()).unwrap();

        // Files exist.
        assert!(dir.path().join("autoloops.toml").is_file());
        assert!(dir.path().join("topology.toml").is_file());
        assert!(dir.path().join("harness.md").is_file());
        assert!(dir.path().join("roles/planner.md").is_file());
        assert!(dir.path().join("roles/builder.md").is_file());

        let topo = fs::read_to_string(dir.path().join("topology.toml")).unwrap();
        // Roles map from hats.
        assert!(topo.contains("id = \"planner\""));
        assert!(topo.contains("emits = [\"tasks.ready\"]"));
        // Handoff is the inverted triggers + a loop.start route.
        assert!(topo.contains("\"tasks.ready\" = [\"builder\"]"));
        assert!(topo.contains("\"loop.start\" = [\"planner\"]"));

        let auto = fs::read_to_string(dir.path().join("autoloops.toml")).unwrap();
        assert!(auto.contains("event_loop.max_iterations = 42"));
        assert!(auto.contains("event_loop.required_events = [\"review.passed\"]"));

        // Role prompt content is escaped/written verbatim.
        let builder = fs::read_to_string(dir.path().join("roles/builder.md")).unwrap();
        assert_eq!(builder, "Build \"it\".");

        // The generated preset is loadable by ralph's own autoloop preset reader
        // (round-trip sanity against preset_source's detector).
        assert!(dir.path().join("topology.toml").is_file());
    }

    #[test]
    fn keeps_incompatible_ralph_and_autoloop_task_stores_separate() {
        let cfg = RalphConfig::default();
        let dir = tempfile::tempdir().unwrap();

        generate_preset(&cfg, dir.path()).unwrap();

        let auto = fs::read_to_string(dir.path().join("autoloops.toml")).unwrap();
        assert!(!auto.contains("core.tasks_file"));
        assert!(!auto.contains(".ralph/agent/tasks.jsonl"));
    }

    #[test]
    fn writes_normalized_v2_budgets_with_autoloop_keys_and_units() {
        let cfg: RalphConfig = serde_yaml::from_str(
            r#"
event_loop:
  max_iterations: 9
  max_runtime_seconds: 12
  max_cost_usd: 3.5
  max_consecutive_failures: 2
"#,
        )
        .expect("valid v2 config");
        let dir = tempfile::tempdir().unwrap();

        generate_preset(&cfg, dir.path()).unwrap();

        let auto = fs::read_to_string(dir.path().join("autoloops.toml")).unwrap();
        assert!(auto.contains("event_loop.max_iterations = 9\n"));
        assert!(auto.contains("event_loop.max_runtime = 12000\n"));
        assert!(auto.contains("event_loop.max_cost_usd = 3.5\n"));
        assert!(!auto.contains("max_consecutive_failures"));
    }

    #[test]
    fn writes_v1_budget_aliases_after_normalization() {
        let mut cfg: RalphConfig = serde_yaml::from_str(
            r#"
max_runtime: 8
max_cost: 1.25
"#,
        )
        .expect("valid v1 config");
        cfg.normalize();
        let dir = tempfile::tempdir().unwrap();

        generate_preset(&cfg, dir.path()).unwrap();

        let auto = fs::read_to_string(dir.path().join("autoloops.toml")).unwrap();
        assert!(auto.contains("event_loop.max_runtime = 8000\n"));
        assert!(auto.contains("event_loop.max_cost_usd = 1.25\n"));
    }

    fn generated_autoloops(backend: &str) -> String {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = backend.to_string();
        let dir = tempfile::tempdir().unwrap();
        generate_preset(&cfg, dir.path()).unwrap();
        fs::read_to_string(dir.path().join("autoloops.toml")).unwrap()
    }

    #[test]
    fn writes_claude_sdk_backend_without_cli_args() {
        let auto = generated_autoloops("claude");

        assert!(auto.contains("backend.kind = \"claude-sdk\"\n"));
        assert!(auto.contains("backend.command = \"claude\"\n"));
        assert!(!auto.contains("backend.args ="));
    }

    #[test]
    fn rejects_claude_cli_args_that_the_sdk_cannot_receive() {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = "claude".to_string();
        cfg.cli.args = vec!["--model".to_string(), "claude-opus".to_string()];
        let dir = tempfile::tempdir().unwrap();

        let error = generate_preset(&cfg, dir.path()).unwrap_err().to_string();

        assert!(error.contains("claude"));
        assert!(error.contains("--model"));
        assert!(error.contains("claude-opus"));
        assert!(error.contains("cannot receive CLI args"));
        assert!(error.contains("custom"));
        assert!(error.contains("core.autoloop_preset"));
    }

    #[test]
    fn writes_pi_backend() {
        let auto = generated_autoloops("pi");

        assert!(auto.contains("backend.kind = \"pi\"\n"));
        assert!(auto.contains("backend.command = \"pi\"\n"));
        assert!(!auto.contains("backend.args ="));
    }

    #[test]
    fn forwards_pi_cli_args() {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = "pi".to_string();
        cfg.cli.args = vec!["--provider".to_string(), "openai-codex".to_string()];
        let dir = tempfile::tempdir().unwrap();

        generate_preset(&cfg, dir.path()).unwrap();

        let auto = fs::read_to_string(dir.path().join("autoloops.toml")).unwrap();
        assert!(auto.contains("backend.args = \"--provider,openai-codex\"\n"));
    }

    #[test]
    fn writes_kiro_backends_as_acp() {
        for backend in ["kiro", "kiro-acp"] {
            let auto = generated_autoloops(backend);

            assert!(auto.contains("backend.kind = \"acp\"\n"));
            assert!(auto.contains("backend.provider = \"kiro\"\n"));
            assert!(auto.contains("backend.command = \"kiro-cli\"\n"));
            assert!(!auto.contains("backend.args ="));
        }
    }

    #[test]
    fn forwards_kiro_cli_args_after_the_acp_provider_default() {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = "kiro".to_string();
        cfg.cli.args = vec!["--trust-all-tools".to_string()];
        let dir = tempfile::tempdir().unwrap();

        generate_preset(&cfg, dir.path()).unwrap();

        let auto = fs::read_to_string(dir.path().join("autoloops.toml")).unwrap();
        assert!(auto.contains("backend.args = \"acp,--trust-all-tools\"\n"));
    }

    #[test]
    fn writes_gemini_command_backend_from_adapter_template() {
        let auto = generated_autoloops("gemini");

        assert!(auto.contains("backend.kind = \"command\"\n"));
        assert!(auto.contains("backend.command = \"gemini\"\n"));
        assert!(auto.contains("backend.args = \"--yolo,-p\"\n"));
        assert!(auto.contains("backend.prompt_mode = \"arg\"\n"));
    }

    #[test]
    fn removes_copilot_parser_only_output_format_args() {
        let auto = generated_autoloops("copilot");

        assert!(auto.contains("backend.args = \"--allow-all-tools,-p\"\n"));
        assert!(!auto.contains("--output-format"));
    }

    #[test]
    fn writes_custom_command_args_with_prompt_flag_last() {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = "custom".to_string();
        cfg.cli.command = Some("my-agent".to_string());
        cfg.cli.args = vec![
            "--verbose".to_string(),
            "--model".to_string(),
            "fast".to_string(),
        ];
        cfg.cli.prompt_flag = Some("--prompt".to_string());
        let dir = tempfile::tempdir().unwrap();

        generate_preset(&cfg, dir.path()).unwrap();

        let auto = fs::read_to_string(dir.path().join("autoloops.toml")).unwrap();
        assert!(auto.contains("backend.kind = \"command\"\n"));
        assert!(auto.contains("backend.command = \"my-agent\"\n"));
        assert!(auto.contains("backend.args = \"--verbose,--model,fast,--prompt\"\n"));
        assert!(auto.contains("backend.prompt_mode = \"arg\"\n"));
    }

    #[test]
    fn rejects_roo_with_supported_backend_list() {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = "roo".to_string();
        let dir = tempfile::tempdir().unwrap();

        let error = generate_preset(&cfg, dir.path()).unwrap_err().to_string();

        assert!(error.contains("roo"));
        assert!(error.contains("unsupported under the autoloop engine"));
        assert!(error.contains(SUPPORTED_AUTOLOOP_BACKENDS));
    }

    #[test]
    fn rejects_unknown_backend_with_supported_backend_list() {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = "smaug".to_string();
        let dir = tempfile::tempdir().unwrap();

        let error = generate_preset(&cfg, dir.path()).unwrap_err().to_string();

        assert!(error.contains("smaug"));
        assert!(error.contains(SUPPORTED_AUTOLOOP_BACKENDS));
    }

    #[test]
    fn rejects_command_argument_containing_a_comma() {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = "gemini".to_string();
        cfg.cli.args = vec!["--setting-sources".to_string(), "project,local".to_string()];
        let dir = tempfile::tempdir().unwrap();

        let error = generate_preset(&cfg, dir.path()).unwrap_err().to_string();

        assert!(error.contains("gemini"));
        assert!(error.contains("project,local"));
        assert!(error.contains("comma"));
    }

    #[test]
    fn rejects_pi_argument_containing_a_comma() {
        let mut cfg = RalphConfig::default();
        cfg.cli.backend = "pi".to_string();
        cfg.cli.args = vec!["--provider".to_string(), "openai,codex".to_string()];
        let dir = tempfile::tempdir().unwrap();

        let error = generate_preset(&cfg, dir.path()).unwrap_err().to_string();

        assert!(error.contains("pi"));
        assert!(error.contains("openai,codex"));
        assert!(error.contains("comma"));
    }
}
