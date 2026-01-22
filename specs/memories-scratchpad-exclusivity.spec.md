# Memories and Scratchpad Mutual Exclusivity

## Problem Statement

When memories are enabled, the scratchpad instructions are still included in the prompt. This is redundant and potentially confusing because:

1. Memories provide persistent context across sessions
2. Scratchpad was designed for per-iteration state before memories existed
3. Including both wastes tokens and creates ambiguity about which to use

## Solution

Make memories and scratchpad mutually exclusive. When `memories.enabled: true`, remove the scratchpad instructions from the prompt.

## Implementation

### Phase 1: Add Flag to HatlessRalph

**File:** `crates/ralph-core/src/hatless_ralph.rs`

Add a field to control scratchpad inclusion:

```rust
pub struct HatlessRalph {
    core: CoreConfig,
    completion_promise: String,
    include_scratchpad: bool,  // NEW
}
```

Update constructor:

```rust
impl HatlessRalph {
    pub fn new(core: CoreConfig, completion_promise: &str) -> Self {
        Self {
            core,
            completion_promise: completion_promise.to_string(),
            include_scratchpad: true,  // Default to true for backwards compatibility
        }
    }

    pub fn with_scratchpad(mut self, include: bool) -> Self {
        self.include_scratchpad = include;
        self
    }
}
```

### Phase 2: Conditionally Include Scratchpad Section

**File:** `crates/ralph-core/src/hatless_ralph.rs`

In `core_prompt()`, wrap the scratchpad section in a conditional:

```rust
fn core_prompt(&self) -> String {
    let mut prompt = format!(
        r#"I'm Ralph. Fresh context each iteration.

### 0a. ORIENTATION
Study `{}` to understand requirements.
Don't assume features aren't implemented—search first.
"#,
        self.core.specs_dir
    );

    // Only include scratchpad section if enabled
    if self.include_scratchpad {
        prompt.push_str(&format!(
            r#"
### 0b. SCRATCHPAD
Study `{}`. It's shared state. It's memory.

Task markers:
- `[ ]` pending
- `[x]` done
- `[~]` cancelled (with reason)
"#,
            self.core.scratchpad
        ));
    }

    prompt
}
```

### Phase 3: Wire Up in EventLoop

**File:** `crates/ralph-core/src/event_loop.rs`

In `EventLoop::new()`, set the flag based on memories config:

```rust
let ralph = HatlessRalph::new(config.core.clone(), &config.event_loop.completion_promise)
    .with_scratchpad(!config.memories.enabled);  // Disable scratchpad when memories enabled
```

## Acceptance Criteria

1. When `memories.enabled: false` (default), scratchpad instructions are included
2. When `memories.enabled: true`, scratchpad instructions are excluded
3. Unit tests verify both modes
4. Existing tests pass: `cargo test`

## Test Plan

```bash
# Unit tests
cargo test -p ralph-core

# Verify prompt content with memories disabled
MEMORIES_ENABLED=false cargo test -p ralph-core -- hatless_ralph

# Verify prompt content with memories enabled
MEMORIES_ENABLED=true cargo test -p ralph-core -- hatless_ralph
```

## Files Changed

- `crates/ralph-core/src/hatless_ralph.rs` — Add `include_scratchpad` flag and conditional
- `crates/ralph-core/src/event_loop.rs` — Set flag based on memories config
