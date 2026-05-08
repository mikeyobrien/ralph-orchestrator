//! Preset source abstraction — extensible pluggable loaders for hat-collection
//! presets authored in formats other than Ralph's native YAML.
//!
//! This module defines the [`PresetSource`] trait and ships two built-in
//! implementations:
//!
//! - [`YamlPresetSource`] — the canonical Ralph single-file YAML shape.
//! - [`TomlPresetSource`] — imports multi-file TOML presets authored for
//!   the [`@mobrienv/autoloop`](https://github.com/mobrienv/autoloop) runtime
//!   (directory containing `autoloops.toml` + `topology.toml` + `roles/*.md`
//!   + optional `harness.md`).
//!
//! New preset shapes plug in by implementing [`PresetSource::detect`] +
//! [`PresetSource::load`] and registering with [`PresetRegistry`].
//!
//! All impls produce a [`serde_yaml::Value`] that can be consumed by the
//! existing hat-overlay merging pipeline in `ralph-cli`, so downstream code
//! does not need to know which preset format was on disk.
//!
//! ## Autoloop → Ralph mapping
//!
//! | Autoloop                                          | Ralph                                      |
//! | ------------------------------------------------- | ------------------------------------------ |
//! | `[[role]] id`                                     | `hats.<id>` key                            |
//! | `[[role]] prompt_file` (read from disk)           | `hats.<id>.instructions`                   |
//! | `[[role]] emits`                                  | `hats.<id>.publishes`                      |
//! | `[handoff] <event> = [role, ...]` (inverted)      | `hats.<role>.triggers` list                |
//! | `topology.completion`                             | `event_loop.completion_promise`            |
//! | `autoloops.toml` `event_loop.completion_event`    | `event_loop.completion_promise` (fallback) |
//! | `autoloops.toml` `event_loop.required_events`     | `event_loop.required_events`               |
//! | `autoloops.toml` `event_loop.max_iterations`      | `event_loop.max_iterations`                |
//! | `handoff["loop.start"]` (first target's role id)  | `event_loop.starting_event = "loop.start"` |
//! | `harness.md` (whole file)                         | appended to `core.guardrails`              |
//!
//! Fields Ralph has but autoloop lacks (backend, cli, core.specs_dir, etc.)
//! stay unset — the overlay only populates hats/events/event_loop slots, and
//! the caller's `ralph.yml` supplies the rest exactly as it does for builtin
//! presets.

use serde_yaml::{Mapping, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Error returned by a [`PresetSource::load`] implementation.
#[derive(Debug, thiserror::Error)]
pub enum PresetSourceError {
    #[error("i/o error reading preset at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("malformed preset at {path}: {message}")]
    Malformed { path: PathBuf, message: String },
    #[error("unsupported preset shape at {path}")]
    Unsupported { path: PathBuf },
}

impl PresetSourceError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn malformed(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Malformed {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Pluggable loader for a preset shape.
///
/// Implementations are stateless; a single instance is reused across loads.
/// Detection is cheap and path-based so callers can probe multiple sources
/// without incurring the cost of a full parse.
pub trait PresetSource: Send + Sync {
    /// Short identifier used in error messages and logs (e.g., `"yaml"`,
    /// `"toml"`).
    fn id(&self) -> &'static str;

    /// Returns `true` iff this source can handle the preset at `path`.
    ///
    /// Callers walk through registered sources in order; the first one whose
    /// `detect` returns `true` is chosen. Detection MUST be side-effect free
    /// (read-only fs access is fine).
    fn detect(&self, path: &Path) -> bool;

    /// Parse the preset at `path` into a hat-overlay YAML value.
    ///
    /// The returned [`Value`] MUST be a mapping with a `hats:` key (and
    /// optionally `events:` / `event_loop:`). It is merged into the core
    /// Ralph config by the caller.
    fn load(&self, path: &Path) -> Result<Value, PresetSourceError>;
}

/// Ordered registry of [`PresetSource`] impls.
///
/// Sources registered earlier win detection ties. The default registry is
/// `[TomlPresetSource, YamlPresetSource]` — TOML-dir first because its
/// detection is strict (requires a directory with two specific TOML files),
/// YAML second as the permissive fallback.
pub struct PresetRegistry {
    sources: Vec<Box<dyn PresetSource>>,
}

impl PresetRegistry {
    /// Empty registry. Use [`PresetRegistry::default`] for the shipped sources.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Append a source to the registry.
    pub fn register(mut self, source: Box<dyn PresetSource>) -> Self {
        self.sources.push(source);
        self
    }

    /// Find the first registered source whose `detect` returns true, then
    /// invoke its `load`.
    pub fn load(&self, path: &Path) -> Result<Value, PresetSourceError> {
        for source in &self.sources {
            if source.detect(path) {
                return source.load(path);
            }
        }
        Err(PresetSourceError::Unsupported {
            path: path.to_path_buf(),
        })
    }

    /// Peek which source handles `path` without loading. Returns the source's
    /// `id()` string, or `None` if no source matches.
    pub fn detect(&self, path: &Path) -> Option<&'static str> {
        self.sources.iter().find(|s| s.detect(path)).map(|s| s.id())
    }
}

impl Default for PresetRegistry {
    fn default() -> Self {
        Self::new()
            .register(Box::new(TomlPresetSource::new()))
            .register(Box::new(YamlPresetSource::new()))
    }
}

// ──────────────────────────────────────────────────────────────────────────
// YAML source — the native Ralph shape.
// ──────────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct YamlPresetSource;

impl YamlPresetSource {
    pub fn new() -> Self {
        Self
    }
}

impl PresetSource for YamlPresetSource {
    fn id(&self) -> &'static str {
        "yaml"
    }

    fn detect(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yml" | "yaml")
        )
    }

    fn load(&self, path: &Path) -> Result<Value, PresetSourceError> {
        let text = fs::read_to_string(path).map_err(|e| PresetSourceError::io(path, e))?;
        serde_yaml::from_str(&text).map_err(|e| PresetSourceError::malformed(path, e.to_string()))
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Autoloop source — multi-file TOML preset directory.
// ──────────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct TomlPresetSource;

impl TomlPresetSource {
    pub fn new() -> Self {
        Self
    }
}

impl PresetSource for TomlPresetSource {
    fn id(&self) -> &'static str {
        "toml"
    }

    fn detect(&self, path: &Path) -> bool {
        path.is_dir()
            && path.join("topology.toml").is_file()
            && path.join("autoloops.toml").is_file()
    }

    fn load(&self, path: &Path) -> Result<Value, PresetSourceError> {
        let topology = read_toml(&path.join("topology.toml"))?;
        let autoloops = read_toml(&path.join("autoloops.toml"))?;
        let harness_text = maybe_read_text(&path.join("harness.md"))?;

        build_overlay(path, &topology, &autoloops, harness_text.as_deref())
    }
}

fn read_toml(path: &Path) -> Result<toml::Value, PresetSourceError> {
    let text = fs::read_to_string(path).map_err(|e| PresetSourceError::io(path, e))?;
    toml::from_str(&text).map_err(|e| PresetSourceError::malformed(path, e.to_string()))
}

fn maybe_read_text(path: &Path) -> Result<Option<String>, PresetSourceError> {
    if !path.is_file() {
        return Ok(None);
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|e| PresetSourceError::io(path, e))
}

fn build_overlay(
    preset_dir: &Path,
    topology: &toml::Value,
    autoloops: &toml::Value,
    harness: Option<&str>,
) -> Result<Value, PresetSourceError> {
    let topology_table = topology
        .as_table()
        .ok_or_else(|| PresetSourceError::malformed(preset_dir, "topology.toml must be a table"))?;

    // ── Extract topology fields ────────────────────────────────────────────
    let preset_name = topology_table
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let completion_event = topology_table
        .get("completion")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    let roles = extract_roles(preset_dir, topology_table)?;
    let handoff = extract_handoff(preset_dir, topology_table)?;

    // Invert the handoff map: Ralph defines triggers per hat, autoloop
    // defines handoff per event → role list. Pattern keys (`/regex/`) are
    // passed through verbatim — Ralph matches raw strings for triggers so
    // regex handoffs become literal trigger strings. Acceptable because
    // 15/16 autoloop presets use exact-match handoffs.
    let triggers_by_role = invert_handoff(&handoff);

    // ── Build hats mapping ─────────────────────────────────────────────────
    let mut hats = Mapping::new();
    for role in &roles {
        let mut hat = Mapping::new();
        insert_str(&mut hat, "name", &role.name);
        // Ralph requires non-empty `description`. Autoloop presets don't have
        // this field, so synthesize one from id+first emit.
        let description = role
            .description
            .clone()
            .unwrap_or_else(|| match role.emits.first() {
                Some(ev) => format!("Autoloop role `{}` — emits {}", role.id, ev),
                None => format!("Autoloop role `{}`", role.id),
            });
        insert_str(&mut hat, "description", &description);
        insert_str_list(
            &mut hat,
            "triggers",
            triggers_by_role.get(&role.id).map(Vec::as_slice),
        );
        insert_str_list(&mut hat, "publishes", Some(&role.emits));
        insert_str(&mut hat, "instructions", &role.prompt);
        if let Some(default) = role.emits.first() {
            insert_str(&mut hat, "default_publishes", default);
        }
        hats.insert(Value::String(role.id.clone()), Value::Mapping(hat));
    }

    // ── Build event_loop overlay ───────────────────────────────────────────
    let mut event_loop = Mapping::new();
    let autoloops_event_loop = autoloops
        .get("event_loop")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();

    // Completion: topology.completion wins, then autoloops event_loop.completion_event.
    let completion = completion_event.or_else(|| {
        autoloops_event_loop
            .get("completion_event")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
    });
    if let Some(c) = completion {
        insert_str(&mut event_loop, "completion_promise", &c);
    }

    if let Some(max_iters) = autoloops_event_loop
        .get("max_iterations")
        .and_then(toml_int)
    {
        event_loop.insert(
            Value::String("max_iterations".into()),
            Value::Number(max_iters.into()),
        );
    }

    if let Some(required) = autoloops_event_loop
        .get("required_events")
        .and_then(|v| v.as_array())
    {
        let items: Vec<Value> = required
            .iter()
            .filter_map(|v| v.as_str().map(|s| Value::String(s.to_string())))
            .collect();
        event_loop.insert(
            Value::String("required_events".into()),
            Value::Sequence(items),
        );
    }

    // Starting event: autoloop preset convention is `loop.start`. If the
    // handoff defines a route out of `loop.start`, honor it; otherwise leave
    // unset so Ralph derives from the hat topology.
    if handoff.iter().any(|(event, _)| event == "loop.start") {
        insert_str(&mut event_loop, "starting_event", "loop.start");
    }

    // ── Overlay envelope ───────────────────────────────────────────────────
    let mut overlay = Mapping::new();
    if !preset_name.is_empty() {
        insert_str(&mut overlay, "name", &preset_name);
    }
    insert_str(
        &mut overlay,
        "description",
        &format!(
            "Imported autoloop preset{}",
            if preset_name.is_empty() {
                String::new()
            } else {
                format!(": {}", preset_name)
            }
        ),
    );
    overlay.insert(Value::String("hats".into()), Value::Mapping(hats));
    if !event_loop.is_empty() {
        overlay.insert(
            Value::String("event_loop".into()),
            Value::Mapping(event_loop),
        );
    }

    // Harness text goes into guardrails as a single entry prefixed with a
    // marker. The caller's `load_hats_value` restricts overlays to
    // hats/events/event_loop by default, but the `name`+`description`+harness
    // payload is captured in each hat's instructions as well so nothing is
    // lost when the strict overlay filter drops the top-level extras.
    if let Some(harness_text) = harness {
        prepend_harness_into_hats(&mut overlay, harness_text);
    }

    Ok(Value::Mapping(overlay))
}

/// Prepend the `harness.md` content (global autoloop rules) to every hat's
/// `instructions`. This is the only slot in Ralph's hats-overlay schema where
/// the text survives hat-overlay extraction — top-level keys like `core:` are
/// dropped by `hats_disallowed_keys`.
fn prepend_harness_into_hats(overlay: &mut Mapping, harness: &str) {
    let Some(hats) = overlay
        .get_mut(Value::String("hats".into()))
        .and_then(Value::as_mapping_mut)
    else {
        return;
    };

    let harness_block = format!(
        "## Shared harness rules (imported from autoloop `harness.md`)\n\n{}\n\n---\n\n",
        harness.trim_end()
    );

    for (_k, v) in hats.iter_mut() {
        let Some(hat) = v.as_mapping_mut() else {
            continue;
        };
        let key = Value::String("instructions".into());
        let merged = match hat.get(&key).and_then(Value::as_str) {
            Some(existing) => format!("{}{}", harness_block, existing),
            None => harness_block.clone(),
        };
        hat.insert(key, Value::String(merged));
    }
}

struct AutoloopRole {
    id: String,
    name: String,
    description: Option<String>,
    emits: Vec<String>,
    prompt: String,
}

fn extract_roles(
    preset_dir: &Path,
    topology: &toml::map::Map<String, toml::Value>,
) -> Result<Vec<AutoloopRole>, PresetSourceError> {
    let raw_roles = topology
        .get("role")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut roles = Vec::with_capacity(raw_roles.len());
    for role_value in raw_roles {
        let role_table = role_value.as_table().ok_or_else(|| {
            PresetSourceError::malformed(preset_dir, "every [[role]] must be a TOML table")
        })?;

        let id = role_table
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PresetSourceError::malformed(preset_dir, "role missing `id`"))?
            .to_string();

        let emits: Vec<String> = role_table
            .get("emits")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let inline_prompt = role_table
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        let prompt_file = role_table
            .get("prompt_file")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        let prompt = resolve_role_prompt(preset_dir, inline_prompt, prompt_file.as_deref())?;

        let name = role_table
            .get("name")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| humanize_role_id(&id));

        let description = role_table
            .get("description")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);

        roles.push(AutoloopRole {
            id,
            name,
            description,
            emits,
            prompt,
        });
    }

    Ok(roles)
}

fn resolve_role_prompt(
    preset_dir: &Path,
    inline: Option<String>,
    prompt_file: Option<&str>,
) -> Result<String, PresetSourceError> {
    if let Some(inline) = inline
        && !inline.trim().is_empty()
    {
        return Ok(inline);
    }
    let Some(rel) = prompt_file else {
        return Ok(String::new());
    };
    let full = preset_dir.join(rel);
    if !full.is_file() {
        return Ok(String::new());
    }
    fs::read_to_string(&full).map_err(|e| PresetSourceError::io(full, e))
}

fn extract_handoff(
    preset_dir: &Path,
    topology: &toml::map::Map<String, toml::Value>,
) -> Result<Vec<(String, Vec<String>)>, PresetSourceError> {
    let Some(raw) = topology.get("handoff") else {
        return Ok(Vec::new());
    };
    let table = raw
        .as_table()
        .ok_or_else(|| PresetSourceError::malformed(preset_dir, "handoff must be a TOML table"))?;

    let mut out = Vec::with_capacity(table.len());
    for (event, value) in table {
        let targets: Vec<String> = match value {
            toml::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect(),
            toml::Value::String(s) => vec![s.clone()],
            _ => continue,
        };
        out.push((event.clone(), targets));
    }
    Ok(out)
}

fn invert_handoff(
    handoff: &[(String, Vec<String>)],
) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut by_role: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (event, targets) in handoff {
        for role in targets {
            let entry = by_role.entry(role.clone()).or_default();
            if !entry.iter().any(|e| e == event) {
                entry.push(event.clone());
            }
        }
    }
    by_role
}

fn humanize_role_id(id: &str) -> String {
    if id.is_empty() {
        return String::new();
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn insert_str(map: &mut Mapping, key: &str, value: &str) {
    map.insert(Value::String(key.into()), Value::String(value.to_string()));
}

fn insert_str_list(map: &mut Mapping, key: &str, values: Option<&[String]>) {
    let list = values
        .map(|xs| {
            xs.iter()
                .map(|v| Value::String(v.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    map.insert(Value::String(key.into()), Value::Sequence(list));
}

fn toml_int(v: &toml::Value) -> Option<i64> {
    match v {
        toml::Value::Integer(i) => Some(*i),
        toml::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_preset(dir: &Path, files: &[(&str, &str)]) {
        for (rel, content) in files {
            let full = dir.join(rel);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full, content).unwrap();
        }
    }

    fn minimal_preset(dir: &Path) {
        write_preset(
            dir,
            &[
                (
                    "autoloops.toml",
                    r#"
event_loop.max_iterations = 42
event_loop.completion_event = "task.complete"
event_loop.required_events = ["review.passed"]
"#,
                ),
                (
                    "topology.toml",
                    r#"
name = "demo"
completion = "task.complete"

[[role]]
id = "planner"
emits = ["tasks.ready"]
prompt_file = "roles/planner.md"

[[role]]
id = "builder"
emits = ["review.ready"]
prompt_file = "roles/builder.md"

[[role]]
id = "critic"
emits = ["review.passed", "review.rejected"]
prompt_file = "roles/critic.md"

[handoff]
"loop.start" = ["planner"]
"tasks.ready" = ["builder"]
"review.ready" = ["critic"]
"review.rejected" = ["builder"]
"#,
                ),
                ("roles/planner.md", "Plan the work."),
                ("roles/builder.md", "Build the work."),
                ("roles/critic.md", "Criticize the work."),
                ("harness.md", "Always be honest.\n"),
            ],
        );
    }

    #[test]
    fn yaml_source_detects_yml_files() {
        let tmp = TempDir::new().unwrap();
        let yml = tmp.path().join("x.yml");
        fs::write(&yml, "event_loop: {}").unwrap();
        let src = YamlPresetSource::new();
        assert!(src.detect(&yml));
    }

    #[test]
    fn yaml_source_rejects_directories() {
        let tmp = TempDir::new().unwrap();
        assert!(!YamlPresetSource::new().detect(tmp.path()));
    }

    #[test]
    fn autoloop_source_detects_valid_preset_dir() {
        let tmp = TempDir::new().unwrap();
        minimal_preset(tmp.path());
        assert!(TomlPresetSource::new().detect(tmp.path()));
    }

    #[test]
    fn autoloop_source_rejects_files() {
        let tmp = TempDir::new().unwrap();
        let yml = tmp.path().join("x.yml");
        fs::write(&yml, "").unwrap();
        assert!(!TomlPresetSource::new().detect(&yml));
    }

    #[test]
    fn autoloop_source_rejects_dir_missing_topology() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("autoloops.toml"), "").unwrap();
        assert!(!TomlPresetSource::new().detect(tmp.path()));
    }

    #[test]
    fn autoloop_source_loads_preset_with_inverted_handoffs() {
        let tmp = TempDir::new().unwrap();
        minimal_preset(tmp.path());

        let overlay = TomlPresetSource::new().load(tmp.path()).unwrap();
        let map = overlay.as_mapping().unwrap();

        // Hats are populated for each role.
        let hats = map
            .get(Value::String("hats".into()))
            .and_then(Value::as_mapping)
            .unwrap();
        assert_eq!(hats.len(), 3);

        // Builder hat has triggers derived from inverted handoff.
        let builder = hats
            .get(Value::String("builder".into()))
            .and_then(Value::as_mapping)
            .unwrap();
        let triggers: Vec<String> = builder
            .get(Value::String("triggers".into()))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(ToString::to_string))
            .collect();
        assert!(triggers.contains(&"tasks.ready".to_string()));
        assert!(triggers.contains(&"review.rejected".to_string()));

        // Builder publishes its emits.
        let publishes: Vec<String> = builder
            .get(Value::String("publishes".into()))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(ToString::to_string))
            .collect();
        assert_eq!(publishes, vec!["review.ready".to_string()]);

        // Instructions include both the harness block and the role prompt.
        let instructions = builder
            .get(Value::String("instructions".into()))
            .and_then(Value::as_str)
            .unwrap();
        assert!(instructions.contains("Always be honest"));
        assert!(instructions.contains("Build the work."));
    }

    #[test]
    fn autoloop_source_populates_event_loop() {
        let tmp = TempDir::new().unwrap();
        minimal_preset(tmp.path());

        let overlay = TomlPresetSource::new().load(tmp.path()).unwrap();
        let event_loop = overlay
            .as_mapping()
            .unwrap()
            .get(Value::String("event_loop".into()))
            .and_then(Value::as_mapping)
            .unwrap();

        assert_eq!(
            event_loop
                .get(Value::String("completion_promise".into()))
                .and_then(Value::as_str),
            Some("task.complete")
        );
        assert_eq!(
            event_loop
                .get(Value::String("max_iterations".into()))
                .and_then(Value::as_i64),
            Some(42)
        );
        assert_eq!(
            event_loop
                .get(Value::String("starting_event".into()))
                .and_then(Value::as_str),
            Some("loop.start")
        );

        let required: Vec<String> = event_loop
            .get(Value::String("required_events".into()))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(ToString::to_string))
            .collect();
        assert_eq!(required, vec!["review.passed".to_string()]);
    }

    #[test]
    fn autoloop_completion_falls_back_to_event_loop_config() {
        let tmp = TempDir::new().unwrap();
        write_preset(
            tmp.path(),
            &[
                (
                    "autoloops.toml",
                    r#"event_loop.completion_event = "done.fire""#,
                ),
                (
                    "topology.toml",
                    r#"
name = "x"
[[role]]
id = "one"
emits = ["done.fire"]
prompt = "be done"
[handoff]
"loop.start" = ["one"]
"#,
                ),
            ],
        );

        let overlay = TomlPresetSource::new().load(tmp.path()).unwrap();
        let cp = overlay
            .as_mapping()
            .unwrap()
            .get(Value::String("event_loop".into()))
            .and_then(Value::as_mapping)
            .unwrap()
            .get(Value::String("completion_promise".into()))
            .and_then(Value::as_str)
            .unwrap();
        assert_eq!(cp, "done.fire");
    }

    #[test]
    fn registry_default_picks_autoloop_for_preset_dirs_and_yaml_for_files() {
        let registry = PresetRegistry::default();

        let tmp = TempDir::new().unwrap();
        minimal_preset(tmp.path());
        assert_eq!(registry.detect(tmp.path()), Some("toml"));

        let yml = tmp.path().join("out.yml");
        fs::write(&yml, "event_loop: {}").unwrap();
        assert_eq!(registry.detect(&yml), Some("yaml"));
    }

    #[test]
    fn registry_reports_unsupported_for_unknown_shape() {
        let registry = PresetRegistry::default();
        let tmp = TempDir::new().unwrap();
        let weird = tmp.path().join("weird.txt");
        fs::write(&weird, "").unwrap();

        let err = registry.load(&weird).unwrap_err();
        assert!(matches!(err, PresetSourceError::Unsupported { .. }));
    }

    #[test]
    fn handoff_inversion_preserves_event_order_per_role() {
        let handoff = vec![
            ("a.first".to_string(), vec!["r1".to_string()]),
            (
                "a.second".to_string(),
                vec!["r1".to_string(), "r2".to_string()],
            ),
            ("a.third".to_string(), vec!["r1".to_string()]),
        ];
        let inverted = invert_handoff(&handoff);
        assert_eq!(
            inverted.get("r1").unwrap(),
            &vec![
                "a.first".to_string(),
                "a.second".to_string(),
                "a.third".to_string()
            ]
        );
        assert_eq!(inverted.get("r2").unwrap(), &vec!["a.second".to_string()]);
    }

    /// Smoke test against the real autoloop `autocode` preset shipped in the
    /// sibling workspace. Skipped when the fixtures aren't present so CI on a
    /// bare clone still passes.
    #[test]
    fn autoloop_source_loads_real_autocode_fixture_when_available() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../autoloop/packages/presets/presets/autocode");
        if !fixture.is_dir() {
            eprintln!("skip: {} not present", fixture.display());
            return;
        }

        let overlay = TomlPresetSource::new()
            .load(&fixture)
            .expect("real autocode preset must load");

        let hats = overlay
            .as_mapping()
            .unwrap()
            .get(Value::String("hats".into()))
            .and_then(Value::as_mapping)
            .expect("hats mapping populated");

        for expected in ["planner", "builder", "critic", "finalizer"] {
            assert!(
                hats.contains_key(Value::String(expected.into())),
                "missing hat: {expected}"
            );
        }

        let event_loop = overlay
            .as_mapping()
            .unwrap()
            .get(Value::String("event_loop".into()))
            .and_then(Value::as_mapping)
            .expect("event_loop overlay populated");
        assert_eq!(
            event_loop
                .get(Value::String("completion_promise".into()))
                .and_then(Value::as_str),
            Some("task.complete")
        );
    }
}
