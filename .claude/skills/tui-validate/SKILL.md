---
name: tui-validate
description: Validates Terminal User Interface (TUI) output using freeze for screenshot capture and LLM-as-judge for semantic validation. Supports both visual (PNG/SVG) and text-based validation modes.
type: anthropic-skill
version: "1.0"
---

# TUI Validate

## Overview

This skill validates Terminal User Interface (TUI) applications by capturing their output and using LLM-as-judge for semantic validation. It leverages [freeze](https://github.com/charmbracelet/freeze) from Charmbracelet for high-fidelity terminal screenshots and provides structured validation criteria.

**Philosophy**: Rather than brittle string matching, this skill uses semantic understanding to validate that TUI output "looks right" - checking layout, content presence, and visual hierarchy without breaking on minor formatting changes.

## When to Use

- Validating TUI rendering after changes
- Checking that UI components display correctly
- Visual regression testing for terminal applications
- Verifying TUI state after specific interactions
- Creating documentation screenshots with validation

## Prerequisites

**Required:**
- `freeze` CLI tool installed (`brew install charmbracelet/tap/freeze`)
- `tmux` for interactive TUI capture (optional, for live applications)

**Verification:**
```bash
# Check freeze is installed
freeze --version

# Check tmux is installed (for interactive capture)
tmux -V
```

## Parameters

- **target** (required): What to validate. One of:
  - `file:<path>` - ANSI output file to validate
  - `command:<cmd>` - Command to execute and capture
  - `tmux:<session>` - Live tmux session to capture
  - `buffer:<text>` - Raw text/ANSI to validate

- **criteria** (required): Validation criteria. Can be:
  - A predefined criteria name (see Built-in Criteria)
  - A custom criteria string describing what to check

- **output_format** (optional, default: "svg"): Screenshot format
  - `svg` - Vector format, best for documentation
  - `png` - Raster format, best for visual diff
  - `text` - Text-only extraction, fastest

- **save_screenshot** (optional, default: false): Whether to save the screenshot
  - If true, saves to `{target_name}.{format}` in current directory

- **judge_mode** (optional, default: "semantic"): Validation approach
  - `semantic` - LLM judges based on meaning and layout
  - `strict` - Also checks exact content presence
  - `visual` - Requires PNG, checks visual appearance

## Built-in Criteria

### `hats-header`
Validates Hats TUI header component:
- Iteration counter in `[iter N]` or `[iter N/M]` format
- Elapsed time in `MM:SS` format
- Hat indicator with emoji and name
- Mode indicator (`▶ auto` or `⏸ paused`)
- Optional scroll mode indicator `[SCROLL]`
- Optional idle countdown `idle: Ns`

### `hats-footer`
Validates Hats TUI footer component:
- Activity indicator (`◉ active`, `◯ idle`, or `■ done`)
- Last event topic display
- Search mode display when active

### `hats-full`
Validates complete Hats TUI layout:
- Header section at top (3 lines)
- Terminal content area (variable height)
- Footer section at bottom (3 lines)
- Proper visual hierarchy and borders

### `tui-basic`
Generic TUI validation:
- Has visible content (not blank)
- No rendering artifacts or broken characters
- Proper terminal dimensions

## Execution Flow

### 1. Capture Phase

Capture TUI output based on target type:

**For file targets:**
```bash
freeze {file_path} -o /tmp/tui-capture.{format}
```

**For command targets:**
```bash
freeze --execute "{command}" -o /tmp/tui-capture.{format}
```

**For tmux targets:**
```bash
tmux capture-pane -pet {session} | freeze -o /tmp/tui-capture.{format}
```

**For buffer targets:**
```bash
echo "{buffer}" | freeze -o /tmp/tui-capture.{format}
```

**Constraints:**
- You MUST verify freeze is installed before attempting capture
- You MUST handle capture failures gracefully and report the error
- You MUST use appropriate freeze flags for the output format
- You SHOULD use `--theme base16` for consistent rendering
- You SHOULD set reasonable dimensions with `--width` and `--height`

### 2. Extraction Phase

Extract content for LLM analysis:

**For text/semantic validation:**
- If format is `text`, use the captured text directly
- If format is `svg` or `png`, also capture text version for content analysis

**For visual validation:**
- Requires PNG format
- Will analyze the image directly using vision capabilities

**Constraints:**
- You MUST extract both visual and text representations when judge_mode is `visual`
- You MUST preserve ANSI escape sequences for color validation when relevant

### 3. Validation Phase

Apply LLM-as-judge with the appropriate criteria:

**Semantic Validation Prompt Template:**
```
Analyze this terminal UI output and determine if it meets the following criteria:

CRITERIA:
{criteria_description}

TERMINAL OUTPUT:
{captured_text}

Evaluate each criterion and provide:
1. PASS or FAIL for each requirement
2. Brief explanation for any failures
3. Overall verdict: PASS or FAIL

Be lenient on exact formatting but strict on:
- Required content presence
- Logical layout and hierarchy
- No rendering errors or artifacts
```

**Visual Validation Prompt Template (with image):**
```
Examine this terminal screenshot and validate:

CRITERIA:
{criteria_description}

Check for:
1. Visual hierarchy and layout
2. Color coding correctness
3. No rendering artifacts or broken characters
4. Proper alignment and spacing

Verdict: PASS or FAIL with explanation
```

**Constraints:**
- You MUST return a clear PASS or FAIL verdict
- You MUST provide specific feedback on failures
- You MUST be lenient on whitespace/formatting differences
- You MUST be strict on content presence and semantic correctness
- You SHOULD note any warnings even on PASS results

### 4. Reporting Phase

Report validation results:

**On PASS:**
```
✅ TUI Validation PASSED

Criteria: {criteria_name}
Target: {target}
Mode: {judge_mode}

All requirements satisfied.
{optional_notes}
```

**On FAIL:**
```
❌ TUI Validation FAILED

Criteria: {criteria_name}
Target: {target}
Mode: {judge_mode}

Issues found:
- {issue_1}
- {issue_2}

Screenshot saved: {path_if_saved}
```

**Constraints:**
- You MUST always provide a clear verdict
- You MUST list specific issues on failure
- You MUST offer the screenshot path if saved
- You SHOULD suggest fixes for common issues

## Examples

### Example 1: Validate Hats Header from File

**Input:**
```
/tui-validate file:test_output.txt criteria:hats-header
```

**Process:**
1. Read `test_output.txt` containing ANSI output
2. Capture with freeze: `freeze test_output.txt -o /tmp/capture.svg`
3. Extract text content
4. Apply `hats-header` criteria via LLM judge
5. Report PASS/FAIL with details

### Example 2: Validate Live TUI in tmux

**Input:**
```
/tui-validate tmux:hats-session criteria:hats-full save_screenshot:true
```

**Process:**
1. Capture tmux pane: `tmux capture-pane -pet hats-session | freeze -o hats-session.svg`
2. Also capture text: `tmux capture-pane -pet hats-session > /tmp/text.txt`
3. Apply `hats-full` criteria checking header, content, and footer
4. Save screenshot to `hats-session.svg`
5. Report validation result

### Example 3: Custom Criteria Validation

**Input:**
```
/tui-validate command:"cargo run --example tui_demo" criteria:"Shows a bordered box with 'Hello World' text centered inside" output_format:png judge_mode:visual
```

**Process:**
1. Execute command and capture: `freeze --execute "cargo run --example tui_demo" -o /tmp/capture.png`
2. Use vision model to analyze PNG
3. Check for bordered box and centered text
4. Report visual validation result

### Example 4: Quick Text Validation

**Input:**
```
/tui-validate buffer:"[iter 3/10] 04:32 | 🔨 Builder | ▶ auto" criteria:hats-header output_format:text
```

**Process:**
1. Analyze text directly (no freeze needed for text mode with buffer)
2. Check for iteration format, elapsed time, hat, and mode indicator
3. Report validation result

## Criteria Definitions

### hats-header (Full Definition)

```yaml
name: hats-header
description: Hats TUI header component validation
requirements:
  - name: iteration_counter
    description: Shows iteration in [iter N] or [iter N/M] format
    required: true
    pattern: '\[iter \d+(/\d+)?\]'

  - name: elapsed_time
    description: Shows elapsed time in MM:SS format
    required: true
    pattern: '\d{2}:\d{2}'

  - name: hat_indicator
    description: Shows current hat with emoji prefix
    required: true
    examples: ["🔨 Builder", "📋 Planner", "🎯 Executor"]

  - name: mode_indicator
    description: Shows loop mode status
    required: true
    values: ["▶ auto", "⏸ paused"]

  - name: scroll_indicator
    description: Shows [SCROLL] when in scroll mode
    required: false
    pattern: '\[SCROLL\]'

  - name: idle_countdown
    description: Shows idle timeout when present
    required: false
    pattern: 'idle: \d+s'
```

### hats-footer (Full Definition)

```yaml
name: hats-footer
description: Hats TUI footer component validation
requirements:
  - name: activity_indicator
    description: Shows current activity state
    required: true
    values: ["◉ active", "◯ idle", "■ done"]

  - name: event_topic
    description: Shows last event topic
    required: false
    examples: ["task.start", "build.done", "loop.terminate"]

  - name: search_display
    description: Shows search query and match count when searching
    required: false
    pattern: 'Search: .+ \d+/\d+'
```

### hats-full (Full Definition)

```yaml
name: hats-full
description: Complete Hats TUI layout validation
requirements:
  - name: header_section
    description: Header at top with iteration, time, hat, and mode
    required: true
    references: hats-header

  - name: content_section
    description: Main terminal content area
    required: true
    checks:
      - Has visible content or is ready for content
      - Properly bounded between header and footer

  - name: footer_section
    description: Footer at bottom with activity status
    required: true
    references: hats-footer

  - name: visual_hierarchy
    description: Clear visual separation between sections
    required: true
    checks:
      - Borders or spacing between sections
      - Consistent width across sections
```

## Troubleshooting

### freeze not found

```bash
# macOS
brew install charmbracelet/tap/freeze

# Linux (via Go)
go install github.com/charmbracelet/freeze@latest

# Verify installation
freeze --version
```

### tmux capture fails

- Ensure the tmux session exists: `tmux list-sessions`
- Verify pane number: `tmux list-panes -t {session}`
- Try capturing specific pane: `tmux capture-pane -pet {session}:{pane}`

### Rendering artifacts in capture

- Try different terminal emulator settings in freeze
- Use `--theme` flag for consistent colors
- Ensure terminal dimensions match TUI expectations

### LLM judge too strict/lenient

- Adjust criteria to be more specific
- Use `strict` mode for exact matching requirements
- Use `semantic` mode for layout/presence checking

## Integration with Tests

This skill can be integrated into test suites:

```rust
// In tests/tui_validation.rs
#[test]
#[ignore] // Run with: cargo test -- --ignored
fn validate_header_rendering() {
    // 1. Render header to buffer
    let output = render_header_to_string(&test_state);

    // 2. Save to temp file
    std::fs::write("/tmp/header_test.txt", &output).unwrap();

    // 3. Run tui-validate skill (via CLI or programmatic)
    // /tui-validate file:/tmp/header_test.txt criteria:hats-header

    // 4. Assert validation passed
}
```

## Best Practices

1. **Use semantic validation for layout checks** - Don't break on minor formatting
2. **Use strict validation for content requirements** - Ensure critical info is present
3. **Save screenshots on failure** - Aids debugging
4. **Test with various terminal sizes** - TUIs should be responsive
5. **Combine with unit tests** - Use this for integration/visual validation, unit tests for logic
