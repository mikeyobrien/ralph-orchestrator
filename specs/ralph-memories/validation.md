# Validation Plan — Ralph Memories Feature

> **Status:** Pending Implementation

## Task Completion Checklist

| Task | Status | Notes |
|------|--------|-------|
| Core data structures (`Memory`, `MemoryType`) | ⬜ pending | |
| Markdown parser (`memory_parser.rs`) | ⬜ pending | |
| Markdown store (`memory_store.rs`) | ⬜ pending | |
| CLI: `ralph memory add` | ⬜ pending | |
| CLI: `ralph memory search` | ⬜ pending | |
| CLI: `ralph memory list` | ⬜ pending | |
| CLI: `ralph memory show` | ⬜ pending | |
| CLI: `ralph memory delete` | ⬜ pending | |
| CLI: `ralph memory prime` | ⬜ pending | |
| CLI: `ralph memory init` | ⬜ pending | |
| Orchestrator integration | ⬜ pending | |
| Skill auto-injection | ⬜ pending | |
| Unit tests | ⬜ pending | |
| Integration tests | ⬜ pending | |

---

## Automated Validation

### 1. Test Suite

```bash
cargo test -p ralph-core memory
cargo test -p ralph-cli integration_memory
```

**Expected:**
- All parser tests pass
- All store tests pass
- All CLI integration tests pass

### 2. Build

```bash
cargo build
```

**Expected:** Clean compilation, no warnings.

### 3. Lint/Clippy

```bash
cargo clippy -- -D warnings
```

**Expected:** No warnings in production code.

---

## Manual E2E Test Plan

### Step 1: Initialize Memories

```bash
ralph memory init
cat .agent/memories.md
```

**Expected:** File created with section headers for Patterns, Decisions, Fixes, Context.

### Step 2: Add Memories

```bash
ralph memory add "This codebase uses barrel exports" --type pattern --tags "imports,structure"
ralph memory add "Chose Zod over Yup for performance" --type decision --tags "validation"
ralph memory add "ECONNREFUSED means run docker-compose up" --type fix --tags "docker,debugging"
```

**Expected:** Each returns `Memory stored: mem-{id}`. File contains all three memories in correct sections.

### Step 3: Search by Content

```bash
ralph memory search "barrel"
```

**Expected:** Returns the barrel exports memory.

### Step 4: Search by Tags

```bash
ralph memory search --tags docker
```

**Expected:** Returns the ECONNREFUSED memory.

### Step 5: Filter by Type

```bash
ralph memory search --type decision
```

**Expected:** Returns only the Zod decision memory.

### Step 6: List All Memories

```bash
ralph memory list
ralph memory list --last 2
ralph memory list --format json
ralph memory list --format markdown
```

**Expected:** All variations work correctly.

### Step 7: Show Single Memory

```bash
ralph memory show mem-{id}
```

**Expected:** Shows full memory details.

### Step 8: Prime for Injection

```bash
ralph memory prime
ralph memory prime --budget 500
ralph memory prime --type fix
```

**Expected:** Outputs raw markdown, respects budget and filters.

### Step 9: Delete Memory

```bash
ralph memory delete mem-{id}
ralph memory list
```

**Expected:** Memory removed from file.

### Step 10: Error Handling

```bash
ralph memory delete nonexistent-id
echo $?
```

**Expected:** Error message, exit code 1.

### Step 11: Human Editing

1. Manually edit `.agent/memories.md` to add a memory
2. Run `ralph memory list`

**Expected:** Hand-written memory appears in list.

### Step 12: Orchestrator Injection (if implemented)

1. Configure `ralph.yml` with `memories.inject: auto`
2. Run Ralph orchestrator
3. Check that memories appear in agent context

**Expected:** Memories injected at iteration start.

---

## Code Quality Checks

### YAGNI Check

Verify no speculative features:
- [ ] No global memories scope
- [ ] No vector/semantic search
- [ ] No memory expiration/decay
- [ ] No unused parameters or abstractions

### KISS Check

Verify simplest solution:
- [ ] Markdown parsing uses regex, not full AST
- [ ] No unnecessary abstractions
- [ ] Follows existing codebase patterns

### Idiomatic Check

Verify code matches codebase conventions:
- [ ] Color handling via `color_mode.should_use_colors()`
- [ ] Error handling matches existing commands
- [ ] Arg structs use clap derive macros
- [ ] Test structure uses Given-When-Then comments

---

## Summary Template

| Check | Result |
|-------|--------|
| Task Completion | ⬜ |
| Test Suite | ⬜ |
| Build | ⬜ |
| Lint/Clippy | ⬜ |
| YAGNI | ⬜ |
| KISS | ⬜ |
| Idiomatic | ⬜ |
| E2E Manual Test | ⬜ |

**VALIDATION RESULT:** Pending
