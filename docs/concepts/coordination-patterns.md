# Coordination Patterns

Ralph's hat system enables sophisticated multi-agent workflows through event-driven coordination. This section covers the architectural patterns, event routing mechanics, and built-in workflow templates.

## How Hat-Based Orchestration Works

### The Event-Driven Model

Hats communicate through a **pub/sub event system**:

1. **Ralph publishes a starting event** (e.g., `task.start`)
2. **The matching hat activates** — the hat subscribed to that event takes over
3. **The hat does its work** and publishes an event when done
4. **The next hat activates** — triggered by the new event
5. **The cycle continues** until a termination event or `LOOP_COMPLETE`

```
┌─────────────────────────────────────────────────────────────────┐
│  task.start → [Test Writer] → test.written → [Implementer] →   │
│  test.passing → [Refactorer] → refactor.done ──┐                │
│                                                │                │
│  ┌─────────────────────────────────────────────┘                │
│  └──→ (loops back to Test Writer for next test)                 │
└─────────────────────────────────────────────────────────────────┘
```

### Ralph as the Constant Coordinator

In hat-based mode, **Ralph is always present**:

- Ralph cannot be removed or replaced
- Custom hats define the **topology** (who triggers whom)
- Ralph executes with **topology awareness** — knowing which hats exist and their relationships
- Ralph serves as the **universal fallback** — orphaned events automatically route to Ralph

This means custom hats don't execute directly. Instead, Ralph reads all pending events across all hats and decides what to do based on the defined topology. Ralph then either:

- Delegates to the appropriate hat by publishing an event
- Handles the work directly if no hat is suited

### Event Routing and Topic Matching

Events route to hats using **glob-style pattern matching**:

| Pattern | Matches |
|---------|---------|
| `task.start` | Exactly `task.start` |
| `build.*` | `build.done`, `build.blocked`, `build.task`, etc. |
| `*.done` | `build.done`, `review.done`, `test.done`, etc. |
| `*` | Everything (global wildcard — used by Ralph as fallback) |

**Priority Rules:**

- Specific patterns take precedence over wildcards
- If multiple hats have specific subscriptions, that's an error (ambiguous routing)
- Global wildcard (`*`) only triggers if no specific handler exists

## Coordination Patterns

Ralph presets implement several proven coordination patterns:

### 1. Linear Pipeline

The simplest pattern — work flows through a sequence of specialists.

```
Input → Hat A → Event → Hat B → Event → Hat C → Output
```

**Example: TDD Red-Green-Refactor** (`tdd-red-green.yml`)

```yaml
hats:
  test_writer:
    triggers: ["tdd.start", "refactor.done"]
    publishes: ["test.written"]

  implementer:
    triggers: ["test.written"]
    publishes: ["test.passing"]

  refactorer:
    triggers: ["test.passing"]
    publishes: ["refactor.done", "cycle.complete"]
```

```
tdd.start → 🔴 Test Writer → test.written → 🟢 Implementer →
test.passing → 🔵 Refactorer → refactor.done ─┐
                                              │
              ┌───────────────────────────────┘
              └──→ (back to Test Writer)
```

**When to use:** Workflows with clear sequential phases where each step builds on the previous.

### 2. Contract-First Pipeline

A variant where work must pass validation gates before proceeding.

**Example: Spec-Driven Development** (`spec-driven.yml`)

```yaml
hats:
  spec_writer:
    triggers: ["spec.start", "spec.rejected"]
    publishes: ["spec.ready"]

  spec_reviewer:
    triggers: ["spec.ready"]
    publishes: ["spec.approved", "spec.rejected"]

  implementer:
    triggers: ["spec.approved", "spec.violated"]
    publishes: ["implementation.done"]

  verifier:
    triggers: ["implementation.done"]
    publishes: ["task.complete", "spec.violated"]
```

```
spec.start → 📋 Spec Writer ──→ spec.ready ──→ 🔎 Spec Critic
                 ↑                                   │
                 └────── spec.rejected ──────────────┤
                                                     ↓
                                               spec.approved
                                                     │
                                                     ↓
task.complete ←── ✅ Verifier ←── impl.done ←── ⚙️ Implementer
                       │                              ↑
                       └──── spec.violated ───────────┘
```

**When to use:** High-stakes changes where the spec must be rock-solid before implementation begins.

### 3. Cyclic Rotation

Multiple roles take turns, each bringing a different perspective.

**Example: Mob Programming** (`mob-programming.yml`)

```yaml
hats:
  navigator:
    triggers: ["mob.start", "observation.noted"]
    publishes: ["direction.set", "mob.complete"]

  driver:
    triggers: ["direction.set"]
    publishes: ["code.written"]

  observer:
    triggers: ["code.written"]
    publishes: ["observation.noted"]
```

```
mob.start → 🧭 Navigator → direction.set → ⌨️ Driver →
code.written → 👁️ Observer → observation.noted ─┐
                                                │
              ┌─────────────────────────────────┘
              └──→ (back to Navigator)
```

**When to use:** Complex features that benefit from multiple perspectives and continuous feedback.

### 4. Adversarial Review

Two roles with opposing objectives ensure robustness.

**Example: Red Team / Blue Team** (`adversarial-review.yml`)

```yaml
hats:
  builder:
    name: "🔵 Blue Team (Builder)"
    triggers: ["security.review", "fix.applied"]
    publishes: ["build.ready"]

  red_team:
    name: "🔴 Red Team (Attacker)"
    triggers: ["build.ready"]
    publishes: ["vulnerability.found", "security.approved"]

  fixer:
    triggers: ["vulnerability.found"]
    publishes: ["fix.applied"]
```

```
security.review → 🔵 Blue Team → build.ready → 🔴 Red Team
                      ↑                            │
                      │                            ├─→ security.approved ✓
                      │                            │
                      │                            └─→ vulnerability.found
                      │                                        │
                      └────── fix.applied ←── 🛡️ Fixer ←──────┘
```

**When to use:** Security-sensitive code, authentication systems, or any code where adversarial thinking improves quality.

### 5. Hypothesis-Driven Investigation

The scientific method applied to debugging.

**Example: Scientific Method** (`scientific-method.yml`)

```yaml
hats:
  observer:
    triggers: ["science.start", "hypothesis.rejected"]
    publishes: ["observation.made"]

  theorist:
    triggers: ["observation.made"]
    publishes: ["hypothesis.formed"]

  experimenter:
    triggers: ["hypothesis.formed"]
    publishes: ["hypothesis.confirmed", "hypothesis.rejected"]

  fixer:
    triggers: ["hypothesis.confirmed"]
    publishes: ["fix.applied"]
```

```
science.start → 🔬 Observer → observation.made → 🧠 Theorist →
hypothesis.formed → 🧪 Experimenter ──┬─→ hypothesis.confirmed → 🔧 Fixer
                                      │
                                      └─→ hypothesis.rejected ─┐
                                                               │
              ┌────────────────────────────────────────────────┘
              └──→ (back to Observer with new data)
```

**When to use:** Complex bugs where the root cause isn't obvious. Forces systematic investigation over random fixes.

### 6. Coordinator-Specialist (Fan-Out)

A coordinator delegates to specialists based on the work type.

**Example: Gap Analysis** (`gap-analysis.yml`)

```yaml
hats:
  analyzer:
    triggers: ["gap.start", "verify.complete", "report.complete"]
    publishes: ["analyze.spec", "verify.request", "report.request"]

  verifier:
    triggers: ["analyze.spec", "verify.request"]
    publishes: ["verify.complete"]

  reporter:
    triggers: ["report.request"]
    publishes: ["report.complete"]
```

```
                    ┌─→ analyze.spec ──→ 🔍 Verifier ──┐
                    │                                  │
gap.start → 📊 Analyzer ←── verify.complete ──────────┘
                    │
                    └─→ report.request ──→ 📝 Reporter ──→ report.complete
```

**When to use:** Work that naturally decomposes into independent specialist tasks (analysis, verification, reporting).

### 7. Adaptive Entry Point

A bootstrapping hat detects input type and routes to the appropriate workflow.

**Example: Code-Assist** (`code-assist.yml`)

```yaml
hats:
  planner:
    triggers: ["build.start"]
    publishes: ["tasks.ready"]
    # Detects: PDD directory vs. code task file vs. description

  builder:
    triggers: ["tasks.ready", "validation.failed", "task.complete"]
    publishes: ["implementation.ready", "task.complete"]

  validator:
    triggers: ["implementation.ready"]
    publishes: ["validation.passed", "validation.failed"]

  committer:
    triggers: ["validation.passed"]
    publishes: ["commit.complete"]
```

```
build.start → 📋 Planner ─── (detects input type) ───→ tasks.ready
                                                            │
    ┌───────────────────────────────────────────────────────┘
    │
    ↓
⚙️ Builder ←─────────────── validation.failed ←─────┐
    │                                               │
    ├── task.complete ──→ (loop for PDD mode) ──────┤
    │                                               │
    └── implementation.ready ──→ ✅ Validator ──────┤
                                      │             │
                                      └─→ validation.passed
                                              │
                                              ↓
                                        📦 Committer → commit.complete
```

**When to use:** Workflows that need to handle multiple input formats or adapt their behavior based on context.

## Designing Custom Hat Collections

### Hat Configuration Schema

```yaml
hats:
  my_hat:
    name: "🎯 Display Name"      # Shown in TUI and logs
    description: "What this hat does"  # REQUIRED — Ralph uses this for delegation
    triggers: ["event.a", "event.b"]   # Events that activate this hat
    publishes: ["event.c", "event.d"]  # Events this hat can emit
    default_publishes: "event.c"       # Fallback if hat forgets to emit
    max_activations: 10                # Optional cap on activations
    backend: "claude"                  # Optional backend override
    instructions: |
      Prompt injected when this hat is active.
      Tell the hat what to do, not how to do it.
```

### Design Principles

1. **Description is critical** — Ralph uses hat descriptions to decide when to delegate. Make them clear and specific.

2. **One hat, one responsibility** — Each hat should have a clear, focused purpose. If you're writing "and" in the description, consider splitting.

3. **Events are routing signals, not data** — Keep payloads brief. Store detailed output in files and reference them in events.

4. **Design for recovery** — If a hat fails or forgets to publish, Ralph catches the orphaned event. Your topology should handle unexpected states gracefully.

5. **Test with simple prompts first** — Complex topologies can have emergent behavior. Start simple, validate the flow, then add complexity.

### Validation Rules

Ralph validates hat configurations:

- **Required description**: Every hat must have a description (Ralph needs it for delegation context)
- **Reserved triggers**: `task.start` and `task.resume` are reserved for Ralph
- **No ambiguous routing**: Each trigger pattern must map to exactly one hat

```
ERROR: Ambiguous routing for trigger 'build.done'.
Both 'planner' and 'reviewer' trigger on 'build.done'.
```

## Event Emission

Hats emit events to signal completion or hand off work:

```bash
# Simple event with payload
ralph emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"

# Event with JSON payload
ralph emit "review.done" --json '{"status": "approved", "issues": 0}'

# Direct handoff to specific hat (bypasses routing)
ralph emit "handoff" --target reviewer "Please review the changes"
```

**In agent output**, events are embedded as XML tags:

```xml
<event topic="impl.done">Implementation complete</event>
<event topic="handoff" target="reviewer">Please review</event>
```

## Choosing a Pattern

| Scenario | Recommended Pattern | Preset |
|----------|---------------------|--------|
| Sequential workflow with clear phases | Linear Pipeline | `tdd-red-green` |
| Spec must be approved before coding | Contract-First | `spec-driven` |
| Need multiple perspectives | Cyclic Rotation | `mob-programming` |
| Security review required | Adversarial | `adversarial-review` |
| Debugging complex issues | Hypothesis-Driven | `scientific-method` |
| Work decomposes into specialist tasks | Coordinator-Specialist | `gap-analysis` |
| Multiple input formats | Adaptive Entry | `code-assist` |
| Standard feature development | Basic delegation | `feature` |

## When Not to Use Hats

Hat-based orchestration adds complexity. Use **traditional mode** (no hats) when:

- The task is straightforward and single-focused
- You don't need role separation or handoffs
- You're prototyping and want minimal configuration
- The work doesn't naturally decompose into distinct phases

Traditional mode is just Ralph in a loop until completion — simpler, faster to set up, and often sufficient.

## Next Steps

- Learn about [Hats & Events](hats-and-events.md) basics
- Explore [Presets](../guide/presets.md) for ready-made workflows
- See [Creating Custom Hats](../advanced/custom-hats.md) for implementation details
