# Ralph Memories — Design Document

> **Status:** Draft

## 1. Overview

### Problem Statement

Ralph's scratchpad provides working memory within a session, but knowledge is lost when sessions end. There's no mechanism for **accumulated wisdom** — learnings that compound across many sessions.

### Solution Summary

A persistent learning system using **markdown storage** with:
- CLI commands under `ralph memory` namespace
- Automatic context injection controlled by Ralph orchestrator
- Skill auto-injection to teach agents how to use memories

### Design Principles

| Principle | Implementation |
|-----------|----------------|
| Human + agent authoring | Markdown format — easy to edit manually |
| Agent consumption | Structured parsing with fallback defaults |
| Direct injection | No transformation needed — it's already markdown |
| Orchestrator control | Budget-aware injection at iteration start |
| Self-documenting | Skill injection teaches agents the system |

## 2. Storage Format

### File Location

```
.agent/memories.md
```

Single file at `.agent/` root, matching `.agent/scratchpad.md` pattern.

### Markdown Structure

```markdown
# Memories

## Patterns

### mem-1737372000-a1b2
> This codebase uses barrel exports for all module boundaries.
> Each directory has an index.ts that re-exports public API.
<!-- tags: imports, structure | created: 2025-01-20 -->

### mem-1737372100-c3d4
> Always run `cargo test` before declaring tasks complete.
<!-- tags: workflow, testing | created: 2025-01-20 -->

## Decisions

### mem-1737380000-e5f6
> Chose JSONL over SQLite for event storage: simpler, git-friendly, append-only.
<!-- tags: architecture, storage | created: 2025-01-20 -->

## Fixes

### mem-1737390000-g7h8
> ECONNREFUSED on port 5432 means PostgreSQL isn't running. Fix: `docker-compose up -d`
<!-- tags: docker, debugging, database | created: 2025-01-21 -->

## Context

### mem-1737400000-i9j0
> The `ralph-core` crate is the shared library; `ralph-cli` is the binary entry point.
<!-- tags: architecture, crates | created: 2025-01-21 -->
```

### Structure Rationale

| Element | Purpose |
|---------|---------|
| `# Memories` | Document header, identifies file type |
| `## {Type}` | Section headers group by memory type |
| `### {id}` | Individual memory, parseable ID |
| `> content` | Blockquote = memory content (multi-line supported) |
| `<!-- metadata -->` | HTML comment = machine-readable, invisible when rendered |

### Memory Types

| Type | Section Header | Purpose | Emoji |
|------|----------------|---------|-------|
| `pattern` | `## Patterns` | How this codebase does things | 🔄 |
| `decision` | `## Decisions` | Why something was chosen | ⚖️ |
| `fix` | `## Fixes` | Solution to recurring problem | 🔧 |
| `context` | `## Context` | Project-specific knowledge | 📍 |

### ID Format

```
mem-{unix_timestamp}-{4_hex_chars}
```

Example: `mem-1737372000-a1b2`

- Sortable by creation time
- Unique enough to avoid collisions
- Human-readable in git history

## 3. CLI Commands

### Namespace

All memory commands live under `ralph memory`:

```bash
ralph memory <subcommand>
```

### Commands

#### `ralph memory add`

Store a new memory.

```bash
ralph memory add <content> [options]

Arguments:
  <content>              The memory content to store

Options:
  -t, --type <TYPE>      Memory type [default: pattern]
                         [values: pattern, decision, fix, context]
      --tags <TAGS>      Comma-separated tags
      --format <FORMAT>  Output format [default: table]
                         [values: table, json, quiet]
```

**Examples:**
```bash
ralph memory add "uses barrel exports" --type pattern --tags imports,structure
ralph memory add "chose Zod for validation" -t decision --tags validation
ralph memory add "ECONNREFUSED means start docker" -t fix --tags docker
```

**Output:**
```
📝 Memory stored: mem-1737372000-a1b2
```

With `--format quiet`:
```
mem-1737372000-a1b2
```

#### `ralph memory search`

Find memories by query.

```bash
ralph memory search [query] [options]

Arguments:
  [query]                Search query (fuzzy match on content/tags)

Options:
  -t, --type <TYPE>      Filter by memory type
      --tags <TAGS>      Filter by tags (comma-separated, OR logic)
      --all              Show all results (no limit)
      --format <FORMAT>  Output format [default: table]
                         [values: table, json, markdown]
```

**Examples:**
```bash
ralph memory search "authentication"
ralph memory search --type fix --tags docker
ralph memory search --format json
```

#### `ralph memory list`

List all memories.

```bash
ralph memory list [options]

Options:
  -t, --type <TYPE>      Filter by memory type
      --last <N>         Show only last N memories
      --format <FORMAT>  Output format [default: table]
                         [values: table, json, markdown]
```

#### `ralph memory show`

Show a single memory by ID.

```bash
ralph memory show <id> [options]

Arguments:
  <id>                   Memory ID (e.g., mem-1737372000-a1b2)

Options:
      --format <FORMAT>  Output format [default: table]
                         [values: table, json, markdown]
```

#### `ralph memory delete`

Delete a memory by ID.

```bash
ralph memory delete <id>

Arguments:
  <id>                   Memory ID to delete
```

**Output:**
```
🗑️  Memory deleted: mem-1737372000-a1b2
```

**Error (not found):**
```
Error: Memory not found: mem-1737372000-a1b2
```
Exit code: 1

#### `ralph memory prime`

Output memories for context injection. Used by orchestrator internally.

```bash
ralph memory prime [options]

Options:
      --budget <TOKENS>  Maximum tokens to include
  -t, --type <TYPES>     Filter by types (comma-separated)
      --tags <TAGS>      Filter by tags (comma-separated)
      --recent <DAYS>    Only memories from last N days
      --format <FORMAT>  Output format [default: markdown]
                         [values: markdown, json]
```

**Examples:**
```bash
ralph memory prime                        # All memories as markdown
ralph memory prime --budget 2000          # Truncate to ~2k tokens
ralph memory prime --type fix,pattern     # Only fixes and patterns
ralph memory prime --recent 30            # Last 30 days
```

**Output:** Raw markdown, suitable for direct context injection.

#### `ralph memory init`

Initialize memories file (creates `.agent/memories.md` with template).

```bash
ralph memory init [options]

Options:
      --force            Overwrite existing file
```

### Output Formats

| Format | Use Case |
|--------|----------|
| `table` | Human-readable, default for interactive use |
| `json` | Programmatic consumption, piping to other tools |
| `markdown` | Direct context injection (prime command) |
| `quiet` | ID-only output for scripting |

## 4. Orchestrator Integration

### Configuration

```yaml
# ralph.yml
memories:
  enabled: true
  inject: auto           # auto | manual | none
  budget: 2000           # max tokens to inject (0 = unlimited)
  filter:
    types: []            # empty = all types
    tags: []             # empty = all tags
    recent: 0            # 0 = no time limit, otherwise days
```

### Injection Modes

| Mode | Behavior |
|------|----------|
| `auto` | Ralph injects memories at start of each iteration |
| `manual` | Agent must explicitly run `ralph memory search` |
| `none` | Memories feature disabled |

### Injection Flow (auto mode)

```
┌─────────────────────────────────────────────────────────────┐
│  Ralph Orchestrator: Start of Iteration                     │
│                                                             │
│  1. Check config: memories.enabled && inject == "auto"      │
│  2. Run: ralph memory prime --budget {config.budget}        │
│  3. Prepend output to system prompt                         │
│  4. If memories.skill_injection, also inject skill          │
└─────────────────────────────────────────────────────────────┘
```

### Implementation in EventLoop

```rust
// In crates/ralph-core/src/event_loop.rs

fn build_iteration_context(&self, config: &RalphConfig) -> String {
    let mut context = String::new();

    // Inject memories if enabled
    if config.memories.enabled && config.memories.inject == InjectMode::Auto {
        if let Ok(memories) = self.prime_memories(config) {
            context.push_str(&memories);
            context.push_str("\n\n");
        }
    }

    // Inject memory skill if enabled
    if config.memories.enabled && config.memories.skill_injection {
        context.push_str(MEMORY_SKILL_CONTENT);
        context.push_str("\n\n");
    }

    context
}
```

## 5. Skill Auto-Injection

When memories are enabled, Ralph automatically injects a skill that teaches agents how to use the memory system.

### Skill Content

```markdown
## Using Project Memories

This project uses Ralph's memory system for persistent learnings across sessions.

### Reading Memories

Memories are automatically included in your context. Review the `# Memories` section above for:
- **Patterns**: How this codebase does things
- **Decisions**: Why architectural choices were made
- **Fixes**: Solutions to recurring problems
- **Context**: Project-specific knowledge

### Storing New Memories

When you discover something worth remembering:

```bash
ralph memory add "<learning>" --type <type> --tags <tags>
```

**When to create memories:**
- You discover a codebase pattern others should follow
- You make an architectural decision with rationale
- You solve a problem that might recur
- You learn project-specific context

**Examples:**
```bash
ralph memory add "API routes use kebab-case, handlers use camelCase" --type pattern --tags api,naming
ralph memory add "Chose Postgres over SQLite for concurrent write support" --type decision --tags database
ralph memory add "CORS errors mean nginx config needs update" --type fix --tags nginx,cors
```

### Searching Memories

For targeted recall beyond auto-injected context:

```bash
ralph memory search "authentication"
ralph memory search --type fix --tags docker
```

### Best Practices

1. **Be specific**: "Uses barrel exports" not "Has good patterns"
2. **Include context**: "Chose X because Y" not just "Uses X"
3. **Tag consistently**: Use existing tags when possible
4. **One concept per memory**: Split complex learnings into multiple memories
```

### Configuration

```yaml
# ralph.yml
memories:
  enabled: true
  inject: auto
  skill_injection: true   # Inject the "how to use memories" skill
```

## 6. Parsing Implementation

### Rust Data Structures

```rust
// crates/ralph-core/src/memory.rs

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Pattern,
    Decision,
    Fix,
    Context,
}

impl MemoryType {
    pub fn section_name(&self) -> &'static str {
        match self {
            Self::Pattern => "Patterns",
            Self::Decision => "Decisions",
            Self::Fix => "Fixes",
            Self::Context => "Context",
        }
    }

    pub fn from_section(s: &str) -> Option<Self> {
        match s {
            "Patterns" => Some(Self::Pattern),
            "Decisions" => Some(Self::Decision),
            "Fixes" => Some(Self::Fix),
            "Context" => Some(Self::Context),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub tags: Vec<String>,
    pub created: String,
}
```

### Parser Implementation

```rust
// crates/ralph-core/src/memory_parser.rs

use regex::Regex;
use once_cell::sync::Lazy;

static SECTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^## (Patterns|Decisions|Fixes|Context)").unwrap()
});

static MEMORY_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^### (mem-\d+-[0-9a-f]{4})").unwrap()
});

static CONTENT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^> (.+)$").unwrap()
});

static METADATA_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<!-- tags: ([^|]*) \| created: (\d{4}-\d{2}-\d{2}) -->").unwrap()
});

pub fn parse_memories(markdown: &str) -> Vec<Memory> {
    let mut memories = Vec::new();
    let mut current_type = MemoryType::Pattern;
    let mut current_id: Option<String> = None;
    let mut current_content: Vec<String> = Vec::new();
    let mut current_tags: Vec<String> = Vec::new();
    let mut current_created: Option<String> = None;

    for line in markdown.lines() {
        if let Some(caps) = SECTION_RE.captures(line) {
            flush_memory(&mut memories, &mut current_id, &current_type,
                        &mut current_content, &mut current_tags, &mut current_created);
            current_type = MemoryType::from_section(&caps[1]).unwrap_or(MemoryType::Pattern);
        } else if let Some(caps) = MEMORY_ID_RE.captures(line) {
            flush_memory(&mut memories, &mut current_id, &current_type,
                        &mut current_content, &mut current_tags, &mut current_created);
            current_id = Some(caps[1].to_string());
        } else if let Some(caps) = CONTENT_RE.captures(line) {
            current_content.push(caps[1].to_string());
        } else if let Some(caps) = METADATA_RE.captures(line) {
            current_tags = caps[1].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            current_created = Some(caps[2].to_string());
        }
    }

    flush_memory(&mut memories, &mut current_id, &current_type,
                &mut current_content, &mut current_tags, &mut current_created);

    memories
}

fn flush_memory(
    memories: &mut Vec<Memory>,
    current_id: &mut Option<String>,
    current_type: &MemoryType,
    current_content: &mut Vec<String>,
    current_tags: &mut Vec<String>,
    current_created: &mut Option<String>,
) {
    if let Some(id) = current_id.take() {
        if !current_content.is_empty() {
            memories.push(Memory {
                id,
                memory_type: current_type.clone(),
                content: current_content.join("\n"),
                tags: std::mem::take(current_tags),
                created: current_created.take().unwrap_or_else(||
                    chrono::Utc::now().format("%Y-%m-%d").to_string()
                ),
            });
        }
    }
    current_content.clear();
}
```

### Writer Implementation

```rust
// crates/ralph-core/src/memory_store.rs

impl MarkdownMemoryStore {
    pub fn append(&self, memory: &Memory) -> io::Result<()> {
        let content = fs::read_to_string(&self.path).unwrap_or_else(|_| self.template());

        let section = format!("## {}", memory.memory_type.section_name());
        let memory_block = format!(
            "\n### {}\n> {}\n<!-- tags: {} | created: {} -->\n",
            memory.id,
            memory.content.replace('\n', "\n> "),
            memory.tags.join(", "),
            memory.created,
        );

        let new_content = if let Some(pos) = self.find_section_insert_point(&content, &section) {
            format!("{}{}{}", &content[..pos], memory_block, &content[pos..])
        } else {
            // Section doesn't exist, append section + memory at end
            format!("{}\n{}\n{}", content.trim_end(), section, memory_block)
        };

        fs::write(&self.path, new_content)
    }

    fn template(&self) -> String {
        "# Memories\n\n## Patterns\n\n## Decisions\n\n## Fixes\n\n## Context\n".to_string()
    }
}
```

## 7. Error Handling

| Error Case | Behavior |
|------------|----------|
| File doesn't exist (read) | Return empty Vec |
| File doesn't exist (write) | Create file with template |
| Malformed memory block | Skip with warning, continue parsing |
| Memory not found (delete) | Print error, exit code 1 |
| Invalid memory type | Reject with error message |
| Permission denied | Propagate IO error |

## 8. Testing Strategy

### Unit Tests (ralph-core)

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_single_memory() { ... }

    #[test]
    fn test_parse_multiple_sections() { ... }

    #[test]
    fn test_parse_multiline_content() { ... }

    #[test]
    fn test_parse_missing_metadata_uses_defaults() { ... }

    #[test]
    fn test_parse_ignores_malformed_blocks() { ... }

    #[test]
    fn test_append_to_existing_section() { ... }

    #[test]
    fn test_append_creates_section_if_missing() { ... }

    #[test]
    fn test_search_matches_content() { ... }

    #[test]
    fn test_search_matches_tags() { ... }

    #[test]
    fn test_delete_removes_memory_block() { ... }
}
```

### Integration Tests (ralph-cli)

```rust
#[test]
fn test_memory_add_creates_file() { ... }

#[test]
fn test_memory_search_finds_by_content() { ... }

#[test]
fn test_memory_list_shows_all() { ... }

#[test]
fn test_memory_delete_removes_entry() { ... }

#[test]
fn test_memory_prime_outputs_markdown() { ... }

#[test]
fn test_memory_prime_respects_budget() { ... }
```

## 9. File Structure

```
crates/ralph-core/src/
├── lib.rs                    # Export memory modules
├── memory.rs                 # Memory, MemoryType structs
├── memory_parser.rs          # Markdown parsing
└── memory_store.rs           # MarkdownMemoryStore

crates/ralph-cli/src/
├── main.rs                   # Add memory subcommand routing
└── commands/
    └── memory.rs             # Memory command implementations

.agent/
├── memories.md               # Memory storage (new)
└── scratchpad.md             # Existing working memory
```

## 10. Implementation Tasks

1. **Core data structures** — Update `memory.rs` with section helpers
2. **Markdown parser** — Create `memory_parser.rs`
3. **Markdown store** — Replace `memory_store.rs` with markdown implementation
4. **CLI restructure** — Move commands under `ralph memory` namespace
5. **Prime command** — Implement budget-aware context output
6. **Orchestrator integration** — Add memory injection to event loop
7. **Skill injection** — Embed memory usage skill
8. **Unit tests** — Parser and store tests
9. **Integration tests** — CLI command tests
10. **Documentation** — Update CLAUDE.md with memory commands
