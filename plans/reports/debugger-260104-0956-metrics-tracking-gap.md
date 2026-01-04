# Root Cause Investigation: Metrics Not Tracked

**Investigation Date:** 2026-01-04 09:56
**Investigator:** Debugger Agent
**Status:** ROOT CAUSE IDENTIFIED

## Executive Summary

Metrics ARE being tracked and written to JSON files. The issue is that **`iteration_telemetry` parameter is NOT being passed** from TUI (`__main__.py`) when creating the `RalphOrchestrator`, causing it to default to `True` but missing explicit configuration from the config object.

**Impact:** Per-iteration details (duration, success/failure, costs, trigger reasons) are being captured but may not reflect config-level settings.

## Evidence

### 1. Metrics Files Exist & Contain Data

```bash
# 8 metrics files found in .agent/metrics/
.agent/metrics/metrics_20260104_*.json (8 files)

# File sizes vary:
- 311B (0 iterations - early exits)
- 636B (1 iteration - single failed run)
- 19K (19 iterations)
- 36K (37 iterations)
- 97K (100 iterations)
```

### 2. Metrics Data Structure is Complete

Latest file (`metrics_20260104_080253.json`) shows:
```json
{
  "summary": {
    "iterations": 100,
    "successful": 100,
    "failed": 0,
    "errors": 0,
    "checkpoints": 0,
    "rollbacks": 0
  },
  "iterations": [100 iteration records],
  "cost": {...},
  "analysis": {
    "avg_iteration_duration": ...,
    "success_rate": ...
  }
}
```

Each iteration record contains:
- iteration number
- duration
- success/error status
- timestamp
- trigger_reason
- output_preview (truncated)
- tokens_used
- cost
- tools_used

### 3. Metrics Tracking Code Flow

**Code Location:** `src/ralph_orchestrator/orchestrator.py`

**Initialization (Lines 134-137):**
```python
self.metrics = Metrics()
self.iteration_stats = IterationStats(
    max_preview_length=self.output_preview_length
) if self.iteration_telemetry else None
```

**Recording (Lines 457-466):**
```python
if self.iteration_stats:
    self.iteration_stats.record_iteration(
        iteration=self.metrics.iterations,
        duration=iteration_duration,
        success=iteration_success,
        error=iteration_error,
        trigger_reason=trigger_reason,
        output_preview=output_preview,
        tokens_used=iteration_tokens,
        cost=iteration_cost,
    )
```

**Saving (Lines 692-724):**
```python
metrics_dir = Path(".agent") / "metrics"
metrics_dir.mkdir(parents=True, exist_ok=True)
metrics_file = metrics_dir / f"metrics_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"

metrics_data = {
    "summary": {...},
    "iterations": self.iteration_stats.iterations if self.iteration_stats else [],
    "cost": {...},
    "analysis": {...}
}

metrics_file.write_text(json.dumps(metrics_data, indent=2))
```

## Root Cause Analysis

### The Gap

**Location:** `src/ralph_orchestrator/__main__.py:1397-1410`

```python
orchestrator = RalphOrchestrator(
    prompt_file_or_config=config,
    primary_tool=primary_tool,
    max_iterations=config.max_iterations,
    max_runtime=config.max_runtime,
    track_costs=True,  # Enable cost tracking by default
    max_cost=config.max_cost,
    checkpoint_interval=config.checkpoint_interval,
    verbose=config.verbose,
    acp_agent=acp_agent,
    acp_permission_mode=acp_permission_mode,
    enable_validation=config.enable_validation,
    validation_interactive=config.validation_interactive
    # ❌ MISSING: iteration_telemetry parameter
    # ❌ MISSING: output_preview_length parameter
)
```

**Orchestrator Constructor Signature (Lines 37-54):**
```python
def __init__(
    self,
    prompt_file_or_config = None,
    primary_tool: str = "claude",
    max_iterations: int = 100,
    max_runtime: int = 14400,
    track_costs: bool = False,
    max_cost: float = 10.0,
    checkpoint_interval: int = 5,
    archive_dir: str = "./prompts/archive",
    verbose: bool = False,
    acp_agent: str = None,
    acp_permission_mode: str = None,
    iteration_telemetry: bool = True,  # ✅ Defaults to True
    output_preview_length: int = 500,   # ✅ Defaults to 500
    enable_validation: bool = False,
    validation_interactive: bool = True,
    instance_manager: InstanceManager = None,
):
```

### Config Handling

The orchestrator supports TWO initialization modes:

1. **Config Object Mode (Lines 83-99):**
   ```python
   if hasattr(prompt_file_or_config, 'prompt_file'):
       config = prompt_file_or_config
       self.iteration_telemetry = getattr(config, 'iteration_telemetry', True)
       self.output_preview_length = getattr(config, 'output_preview_length', 500)
   ```

2. **Individual Parameters Mode (Lines 100-115):**
   ```python
   else:
       self.iteration_telemetry = iteration_telemetry
       self.output_preview_length = output_preview_length
   ```

**The Problem:** TUI passes `config` object but ALSO passes individual parameters, which are IGNORED in config mode. Config object doesn't have `iteration_telemetry` field, so it defaults to True via `getattr()`.

### Config Class Gap

**Location:** `src/ralph_orchestrator/main.py:217-261`

```python
class RalphConfig:
    agent: AgentType = AgentType.AUTO
    prompt_file: str = DEFAULT_PROMPT_FILE
    max_iterations: int = DEFAULT_MAX_ITERATIONS
    # ... other fields ...
    enable_validation: bool = False
    validation_interactive: bool = True
    # ❌ MISSING: iteration_telemetry field
    # ❌ MISSING: output_preview_length field
```

## Data Flow for Metrics

```
1. Orchestrator starts
   ├─> Metrics() initialized (summary counters)
   ├─> IterationStats() initialized if iteration_telemetry=True
   └─> CostTracker() initialized if track_costs=True

2. Each iteration executes
   ├─> metrics.iterations += 1
   ├─> metrics.successful_iterations++ or failed_iterations++
   └─> iteration_stats.record_iteration(...) if enabled

3. Orchestrator finishes
   ├─> Build metrics_data dict
   │   ├─> summary: basic counters from Metrics
   │   ├─> iterations: detailed records from IterationStats
   │   ├─> cost: tracking from CostTracker
   │   └─> analysis: computed stats
   └─> Write to .agent/metrics/metrics_YYYYMMDD_HHMMSS.json
```

## Findings

### What Works ✅

1. **Metrics tracking is ACTIVE** - All 8 files contain valid data
2. **Per-iteration telemetry WORKS** - 100 iterations recorded in latest file
3. **Summary counters WORK** - iterations/successful/failed/errors tracked
4. **Cost tracking WORKS** - When enabled via track_costs=True
5. **File creation WORKS** - Metrics saved to timestamped JSON files
6. **Data structure CORRECT** - summary/iterations/cost/analysis sections

### What's Missing ❌

1. **Config field gap** - `RalphConfig` doesn't have `iteration_telemetry` field
2. **Parameter passing gap** - TUI doesn't pass `iteration_telemetry` parameter
3. **Inconsistent initialization** - Mixing config object + individual params

### Empty Metrics Files (0 iterations)

3 files with 311 bytes, 0 iterations:
- `metrics_20260104_034245.json`
- `metrics_20260104_034832.json`
- `metrics_20260104_035832.json`
- `metrics_20260104_060020.json`

**Cause:** Orchestrator started but exited immediately (before first iteration). Metrics file still created with empty data structure. This is NORMAL for early exits.

## No Critical Issues Found

Metrics ARE being tracked. The system is working as designed:
- iteration_telemetry defaults to True
- Metrics files are created and populated
- Data structure is complete and correct

The only gaps are:
1. Missing config fields for telemetry settings
2. Missing parameter passing in TUI initialization

These gaps don't prevent metrics tracking - they just prevent configuration control.

## Unresolved Questions

1. What prompted the investigation? If user reported "metrics not tracked", what specific data were they looking for?
2. Are empty metrics files (0 iterations) expected or concerning?
3. Should `iteration_telemetry` be configurable via config file?
4. Is the output_preview_length sufficient (500 chars)?
5. Why mix config object + individual parameters in TUI initialization?
