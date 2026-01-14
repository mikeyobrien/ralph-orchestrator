# Current Implementation Analysis

Research into the existing ralph-core implementation to understand what needs to change for the hatless Ralph architecture.

---

## Event Loop (`event_loop.rs`)

**Location:** `crates/ralph-core/src/event_loop.rs` (~1200 lines)

### Current Flow

```
1. initialize() → publishes task.start
2. next_hat() → finds hat with pending events
3. build_prompt() → builds hat-specific prompt
4. process_output() → parses events from output, routes them
5. check_termination() → checks safeguards
```

### Key Findings

| Aspect | Current | Problem for New Architecture |
|--------|---------|------------------------------|
| **Routing** | Events route to hats by subscription | ✅ Works - just need fallback to Ralph |
| **Fallback** | `inject_fallback_event()` sends `task.resume` to planner | ❌ Fragile - planner might not exist |
| **Completion** | Only planner hat can output `LOOP_COMPLETE` | ⚠️ Need to change to hatless Ralph only |
| **Event parsing** | XML tags parsed from agent output | ❌ Replace with JSONL on disk |
| **Hat selection** | `build_prompt()` checks if planner/builder by ID | ❌ Remove special-casing for planner |

### Relevant Code

```rust
// Current fallback logic (line 286-302)
pub fn inject_fallback_event(&mut self) -> bool {
    let planner_id = HatId::new("planner");
    if let Some(hat) = self.registry.get(&planner_id) {
        if hat.subscriptions.iter().any(|t| t.as_str() == "task.resume") {
            // ... inject fallback
        }
    }
    false  // If no planner, no recovery!
}
```

This is exactly the brittleness we're fixing. If user doesn't define a `planner` hat, fallback fails silently.

---

## Hat Registry (`hat_registry.rs`)

**Location:** `crates/ralph-core/src/hat_registry.rs` (~170 lines)

### Current Behavior

```rust
pub fn from_config(config: &RalphConfig) -> Self {
    if config.hats.is_empty() {
        // Default: planner + builder
        registry.register(Hat::default_planner());
        registry.register(Hat::default_builder());
    } else {
        // Custom hats REPLACE defaults entirely
        for (id, hat_config) in &config.hats {
            registry.register(Self::hat_from_config(id, hat_config));
        }
    }
}
```

### The Problem

Custom hats **replace** defaults. This means:
- User defines `reviewer` only → no planner, no builder
- User defines `planner` with different triggers → breaks event flow
- No safety net for misconfigured presets

### Change Needed

- Ralph is NOT in the registry
- Registry holds only user-defined hats (Ralph's team)
- Ralph exists separately, always present

---

## Event Parser (`event_parser.rs`)

**Location:** `crates/ralph-core/src/event_parser.rs` (~407 lines)

### Current Parsing

Parses XML-style tags from agent output:
```xml
<event topic="build.done">
tests: pass
lint: pass
typecheck: pass
</event>
```

### Problems

1. **Agents forget** - common failure mode
2. **Regex parsing** - fragile, can match incorrectly
3. **No fallback** - if no event parsed, loop stalls

### Change Needed

Replace with JSONL file reading:
```rust
// New approach
pub fn read_events_from_disk(path: &Path) -> Vec<Event> {
    // Read .agent/events.jsonl
    // Parse each line as JSON
    // Return new events since last read
}
```

---

## Instruction Builder (`instructions.rs`)

**Location:** `crates/ralph-core/src/instructions.rs` (~560 lines)

### Current Methods

| Method | Purpose | Hat |
|--------|---------|-----|
| `build_coordinator()` | Planning prompt | Planner |
| `build_ralph()` | Building prompt | Builder |
| `build_custom_hat()` | Generic hat prompt | Custom hats |

### Key Insight

The "planner" and "builder" prompts are actually specialized—they have different structures:
- Planner: Gap analysis, scratchpad ownership, dispatch work
- Builder: Pick task, implement, backpressure, commit

### Change Needed

New method: `build_hatless_ralph()` that:
1. Has core behaviors (always)
2. Conditionally injects "MY TEAM" with hat topology (if hats exist)
3. Conditionally injects "SOLO MODE" (if no hats)
4. Owns completion promise

---

## Hat Proto (`hat.rs`)

**Location:** `crates/ralph-proto/src/hat.rs` (~189 lines)

### Current Structure

```rust
pub struct Hat {
    pub id: HatId,
    pub name: String,
    pub subscriptions: Vec<Topic>,
    pub publishes: Vec<Topic>,
    pub instructions: String,
}
```

### Factory Methods

- `default_planner()` - planner hat
- `default_builder()` - builder hat
- `default_single()` - deprecated

### Change Needed

- Remove `default_planner()` concept - Ralph is the planner now
- Add `default_publishes: Option<Topic>` field for fallback events
- Keep `default_builder()` as a convenience for common preset

---

## Summary of Changes Required

### New Concepts

| Concept | Location | Description |
|---------|----------|-------------|
| **Hatless Ralph** | `event_loop.rs` | New entity, not a hat, always present |
| **JSONL events** | New file? | Read events from `.agent/events.jsonl` |
| **Default publishes** | `hat.rs` | Fallback event per hat |
| **Hat topology injection** | `instructions.rs` | Table of available hats for Ralph |

### Removed/Changed Concepts

| Concept | Change |
|---------|--------|
| `default_planner()` | Remove - Ralph IS the planner |
| `inject_fallback_event()` | Replace with Ralph fallback |
| `EventParser::parse()` | Replace with JSONL reading |
| Completion from planner hat | Change to hatless Ralph only |

### Files to Modify

| File | Changes |
|------|---------|
| `event_loop.rs` | Add hatless Ralph, change routing, JSONL reading |
| `hat_registry.rs` | Remove default planner/builder auto-creation |
| `event_parser.rs` | Add JSONL parsing, deprecate XML parsing |
| `instructions.rs` | Add `build_hatless_ralph()`, hat topology injection |
| `hat.rs` | Add `default_publishes` field |
| `config.rs` | Update hat config schema |

---

## Config (`config.rs`)

**Location:** `crates/ralph-core/src/config.rs` (~1370 lines)

### Current HatConfig

```rust
pub struct HatConfig {
    pub name: String,
    pub triggers: Vec<String>,
    pub publishes: Vec<String>,
    pub instructions: String,
}
```

### Missing: `default_publishes`

The field doesn't exist yet. Needs to be added:

```rust
pub struct HatConfig {
    pub name: String,
    pub triggers: Vec<String>,
    pub publishes: Vec<String>,
    pub default_publishes: Option<String>,  // NEW
    pub instructions: String,
}
```

### Preflight Validation Already Exists

The `preflight_check()` method already validates:
- At least one hat exists
- Every published event has a subscriber
- Initial events have handlers
- Git availability

**This is good** — we can extend it to validate Ralph's requirements.

### The Brittleness (Confirmed)

```rust
fn get_effective_hats(&self) -> HashMap<String, HatConfig> {
    if self.hats.is_empty() {
        // Default planner + builder
    } else {
        // Custom hats REPLACE defaults entirely ← THE PROBLEM
        self.hats.clone()
    }
}
```

With hatless Ralph, this changes to:
- Ralph is always present (not in hats map)
- `hats` map contains only user-defined team members
- Empty `hats` = solo mode (Ralph does everything)
