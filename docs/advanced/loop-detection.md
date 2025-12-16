# Loop Detection

## Overview

Ralph Orchestrator includes automatic loop detection to prevent agents from getting stuck in repetitive cycles. This feature uses fuzzy string matching to compare recent agent outputs and detect when an agent is producing similar responses repeatedly.

## How It Works

The `SafetyGuard` class maintains a sliding window of the last 5 agent outputs. After each successful iteration, the current output is compared against this history using [rapidfuzz](https://github.com/rapidfuzz/RapidFuzz), a fast fuzzy string matching library.

### Detection Algorithm

1. After each successful iteration, the agent's output is captured
2. The output is compared against the last 5 stored outputs
3. If any comparison exceeds the 90% similarity threshold, a loop is detected
4. The current output is added to the history (oldest removed if at capacity)
5. When a loop is detected, the orchestrator logs a warning and exits

#### Sliding Window Visualization

```
                                         🔄 Sliding Window (deque maxlen=5)

┌────────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐  evicted   ╭───╮
│ New Output │ ──> │ Output 5 │ ──> │ Output 4 │ ──> │ Output 3 │ ──> │ Output 2 │ ──> │ Output 1 │ ─────────> │ X │
└────────────┘     └──────────┘     └──────────┘     └──────────┘     └──────────┘     └──────────┘            ╰───╯
```

<details>
<summary>graph-easy source</summary>

```
graph { label: "🔄 Sliding Window (deque maxlen=5)"; flow: east; }
[ New Output ] -> [ Output 5 ] -> [ Output 4 ] -> [ Output 3 ] -> [ Output 2 ] -> [ Output 1 ]
[ Output 1 ] -- evicted --> [ X ] { shape: rounded; }
```

</details>

### Similarity Threshold

The default threshold is **90% similarity** (0.9 ratio). This was chosen based on industry best practices:

- **0.95**: Too strict - only catches nearly identical outputs
- **0.90**: Balanced - catches repetitive patterns while allowing variation (recommended)
- **0.85**: Loose - higher false positive rate

#### Decision Flow

```
              🔍 Loop Detection Decision

                         ╭────────────────────╮
                         │   Current Output   │
                         ╰────────────────────╯
                           │
                           │
                           ∨
                         ┌────────────────────┐
                         │ Compare to History │ <┐
                         └────────────────────┘  │
                           │                     │
                           │                     │
                           ∨                     │
╔═══════════════╗  yes   ┌────────────────────┐  │
║ LOOP DETECTED ║ <───── │   ratio >= 90%?    │  │ yes
╚═══════════════╝        └────────────────────┘  │
                           │                     │
                           │ no                  │
                           ∨                     │
                         ┌────────────────────┐  │
                         │   More outputs?    │ ─┘
                         └────────────────────┘
                           │
                           │ no
                           ∨
                         ┌────────────────────┐
                         │   Add to History   │
                         └────────────────────┘
                           │
                           │
                           ∨
                         ╭────────────────────╮
                         │      Continue      │
                         ╰────────────────────╯
```

<details>
<summary>graph-easy source</summary>

```
graph { label: "🔍 Loop Detection Decision"; flow: south; }
[ Current Output ] { shape: rounded; } -> [ Compare to History ]
[ Compare to History ] -> [ ratio >= 90%? ]
[ ratio >= 90%? ] -- yes --> [ LOOP DETECTED ] { border: double; }
[ ratio >= 90%? ] -- no --> [ More outputs? ]
[ More outputs? ] -- yes --> [ Compare to History ]
[ More outputs? ] -- no --> [ Add to History ]
[ Add to History ] -> [ Continue ] { shape: rounded; }
```

</details>

## Example

```python
# Example of how loop detection works internally

from rapidfuzz import fuzz

# Agent outputs from iterations 1-3
outputs = [
    "Let me check the database for user information...",
    "I'll query the database to find the user data...",
    "Checking the database for user information...",  # Similar to #1
]

# Similarity check
ratio = fuzz.ratio(outputs[0], outputs[2]) / 100.0
# Result: ~0.91 (91% similar) - LOOP DETECTED
```

## Configuration

Currently, loop detection uses fixed parameters:

| Parameter        | Value | Description                          |
| ---------------- | ----- | ------------------------------------ |
| `loop_threshold` | 0.9   | Similarity threshold (90%)           |
| `recent_outputs` | 5     | Number of outputs to compare against |

Future versions may expose these as configuration options.

## Interaction with Other Safety Features

Loop detection works alongside other safety mechanisms:

1. **Iteration Limit**: Maximum iterations (default: 100)
2. **Runtime Limit**: Maximum time (default: 4 hours)
3. **Cost Limit**: Maximum cost (default: $10)
4. **Consecutive Failure Limit**: Max failures in a row (default: 5)
5. **Loop Detection**: Similarity-based output comparison

The orchestrator exits when **any** of these conditions are met.

### Integration Architecture

The following diagram shows how loop detection integrates with the main orchestration loop:

```
            ⚙️ SafetyGuard in Orchestration Loop

                               ╭─────────────────────╮
  ┌──────────────────────────> │   Start Iteration   │ <┐
  │                            ╰─────────────────────╯  │
  │                              │                      │
  │                              │                      │
  │                              ∨                      │
  │                            ┌─────────────────────┐  │
  │                            │ SafetyGuard.check() │  │
  │                            └─────────────────────┘  │
  │                              │                      │
  │                              │                      │
  │                              ∨                      │
  │  ╔════════════════╗  no    ┌─────────────────────┐  │
  │  ║  STOP: Limit   ║ <───── │     Limits OK?      │  │
  │  ╚════════════════╝        └─────────────────────┘  │
  │                              │                      │
  │                              │ yes                  │
  │                              ∨                      │
  │                            ┌─────────────────────┐  │
  │                            │  Check Completion   │  │
  │                            └─────────────────────┘  │
  │                              │                      │
  │                              │                      │
  │                              ∨                      │
  │  ╔════════════════╗  yes   ┌─────────────────────┐  │
  │  ║   STOP: Done   ║ <───── │   TASK_COMPLETE?    │  │ no
  │  ╚════════════════╝        └─────────────────────┘  │
  │                              │                      │
  └────┐                         │ no                   │
       │                         ∨                      │
       │                       ┌─────────────────────┐  │
       │                       │    Execute Agent    │  │
       │                       └─────────────────────┘  │
       │                         │                      │
       │                         │                      │
       │                         ∨                      │
     ┌────────────────┐  no    ┌─────────────────────┐  │
     │ Handle Failure │ <───── │      Success?       │  │
     └────────────────┘        └─────────────────────┘  │
                                 │                      │
                                 │ yes                  │
                                 ∨                      │
                               ┌─────────────────────┐  │
                               │    detect_loop()    │  │
                               └─────────────────────┘  │
                                 │                      │
                                 │                      │
                                 ∨                      │
                               ┌─────────────────────┐  │
                               │     Loop Found?     │ ─┘
                               └─────────────────────┘
                                 │
                                 │ yes
                                 ∨
                               ╔═════════════════════╗
                               ║     STOP: Loop      ║
                               ╚═════════════════════╝
```

<details>
<summary>graph-easy source</summary>

```
graph { label: "⚙️ SafetyGuard in Orchestration Loop"; flow: south; }
[ Start Iteration ] { shape: rounded; } -> [ SafetyGuard.check() ]
[ SafetyGuard.check() ] -> [ Limits OK? ]
[ Limits OK? ] -- no --> [ STOP: Limit ] { border: double; }
[ Limits OK? ] -- yes --> [ Check Completion ]
[ Check Completion ] -> [ TASK_COMPLETE? ]
[ TASK_COMPLETE? ] -- yes --> [ STOP: Done ] { border: double; }
[ TASK_COMPLETE? ] -- no --> [ Execute Agent ]
[ Execute Agent ] -> [ Success? ]
[ Success? ] -- no --> [ Handle Failure ]
[ Handle Failure ] -> [ Start Iteration ]
[ Success? ] -- yes --> [ detect_loop() ]
[ detect_loop() ] -> [ Loop Found? ]
[ Loop Found? ] -- yes --> [ STOP: Loop ] { border: double; }
[ Loop Found? ] -- no --> [ Start Iteration ]
```

</details>

## When Loop Detection Triggers

Loop detection helps in these scenarios:

- **Agent stuck on same task**: Repeatedly attempting the same action
- **Oscillation**: Agent switching between two similar approaches
- **API errors**: Consistent retry messages
- **Placeholder responses**: Agent returning similar "working on it" messages

## Logging

When loop detection triggers, you'll see:

```
WARNING - Loop detected: 92.3% similarity to previous output
WARNING - Breaking loop due to repetitive agent outputs
```

## Resetting Loop Detection

The loop detection history is automatically cleared when:

- A new orchestration session starts
- `SafetyGuard.reset()` is called
- The orchestrator completes (success or failure)

## Dependencies

Loop detection requires the `rapidfuzz` package:

```bash
pip install "rapidfuzz>=3.0.0,<4.0.0"
```

If rapidfuzz is not installed, loop detection is gracefully skipped with a debug log message.

## Best Practices

1. **Monitor for loops**: Watch for loop detection warnings in logs
2. **Improve prompts**: If loops occur frequently, refine your task description
3. **Check task completeness**: Ensure tasks have clear completion criteria
4. **Use completion markers**: Add `- [x] TASK_COMPLETE` when done

## Related Topics

- [Safety Mechanisms](../guide/overview.md#safety-features)
- [Troubleshooting](../troubleshooting.md)
- [Glossary: Loop Detection](../glossary.md#l)
