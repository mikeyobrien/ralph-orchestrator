# Hat System & Config Research

## HatConfig Schema

**File:** `crates/ralph-core/src/config.rs:1229-1310`

```rust
pub struct HatConfig {
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<String>,
    pub publishes: Vec<String>,
    pub instructions: String,
    pub extra_instructions: Vec<String>,
    pub backend: Option<HatBackend>,
    pub backend_args: Option<Vec<String>>,
    pub default_publishes: Option<String>,
    pub max_activations: Option<u32>,
    pub disallowed_tools: Vec<String>,
}
```

New fields needed for waves: `concurrency`, `aggregate`, `isolation`.

## Hatless Ralph Architecture

**File:** `crates/ralph-core/src/hatless_ralph.rs`

Key insight: **Ralph is always the executor**. Custom hats define topology (pub/sub contracts) that Ralph uses for coordination context, but Ralph handles all iterations.

- Solo mode (no custom hats): Ralph runs directly
- Multi-hat mode: Ralph always executes, custom hats are "personas" with filtered instructions

## Hat Selection & Activation

**File:** `crates/ralph-core/src/event_loop/mod.rs:658-672`

- `next_hat()` returns first hat with pending events
- In multi-hat mode, always returns "ralph"
- `HatRegistry::get_for_topic()` matches events to hats

## Prompt Building

**File:** `crates/ralph-core/src/hatless_ralph.rs:258-309`

Injection order:
1. Core prompt (guardrails, orientation, scratchpad)
2. Skill index
3. OBJECTIVE section
4. ROBOT GUIDANCE
5. PENDING EVENTS (all hat events requiring handling)
6. Workflow section (unless hat has custom instructions)
7. **HATS table** (full topology for delegation)
8. EVENT WRITING guide
9. DONE section

## HATS Table & Context Injection

**File:** `crates/ralph-core/src/hatless_ralph.rs:596-711`

Already generates a topology table:
```
| Hat | Triggers On | Publishes | Description |
```

Also generates Mermaid flowchart of event flow.

**Existing downstream resolution:** For each active hat's `publishes`, shows which downstream hats receive each event. This is exactly the mechanism needed for NL-driven wave dispatch — inject downstream hat descriptions into the dispatcher's prompt.

## Hat Lifecycle

1. **Registration** — HatRegistry loads from config
2. **Activation** — When pending events exist, check max_activations
3. **Execution** — Backend runs with hat-specific prompt context
4. **Scope enforcement** — Hat can only publish declared topics; violations → `<hat_id>.scope_violation`
5. **Default publish** — If hat publishes nothing, inject `default_publishes` event
6. **Exhaustion** — When activation count >= max_activations, emit `<hat_id>.exhausted`

## Where New Config Fields Go

1. **HatConfig struct** — `crates/ralph-core/src/config.rs:1229-1310`
2. **Hat proto** — `crates/ralph-proto/src/hat.rs:43-63`
3. **HatTopology** — `crates/ralph-core/src/hatless_ralph.rs:101-150`
4. **Prompt injection** — `crates/ralph-core/src/hatless_ralph.rs:596-711`
5. **Validation** — `crates/ralph-core/src/config.rs:366-494` (`RalphConfig::validate()`)
