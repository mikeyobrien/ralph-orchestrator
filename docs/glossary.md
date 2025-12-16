# Glossary

## A

**Agent**
: An AI-powered CLI tool that executes tasks based on prompts. Ralph supports Claude, Gemini, and Q agents.

**Agent Manager**
: Component that detects, selects, and manages AI agents, including automatic fallback when preferred agents are unavailable.

**Archive**
: Storage of historical prompts and iterations in `.agent/prompts/` directory for debugging and analysis.

**Auto-detection**
: Ralph's ability to automatically discover which AI agents are installed and available on the system.

## C

**Checkpoint**
: A Git commit created at regular intervals to save progress and enable recovery. Default interval is every 5 iterations.

**Claude**
: Anthropic's AI assistant, accessible via Claude Code CLI. Known for high context window (200K tokens) and code generation capabilities.

**CLI**
: Command Line Interface - the primary way to interact with Ralph Orchestrator through terminal commands.

**Config**
: Configuration settings stored in `ralph.json` or passed via command line arguments and environment variables.

**Context Window**
: The maximum amount of text/tokens an AI agent can process in a single request. Varies by agent (Claude: 200K, Gemini: 32K, Q: 8K).

**Convergence**
: The process of iterations gradually approaching task completion through successive improvements.

**Completion Marker**
: A special checkbox pattern (`- [x] TASK_COMPLETE`) in the prompt file that signals task completion. When the orchestrator detects this marker, it immediately exits the loop.

## D

**Dry Run**
: Test mode that simulates execution without actually running AI agents. Useful for testing configuration and setup.

**Deterministic Failure**
: The philosophy that it's better to fail predictably than succeed unpredictably - core to the Ralph Wiggum technique.

## E

**Exponential Backoff**
: Retry strategy where wait time doubles after each failure (2, 4, 8, 16 seconds) to handle transient errors.

**Execution Cycle**
: One complete iteration of reading prompt, executing agent, checking completion, and updating state.

## G

**Gemini**
: Google's AI model, accessible via Gemini CLI. Balanced context window (32K tokens) and capabilities.

**Git Integration**
: Ralph's use of Git for checkpointing, history tracking, and recovery from failed states.

## I

**Iteration**
: One complete cycle of the Ralph loop - executing an agent with the current prompt and processing results.

**Iteration Limit**
: Maximum number of iterations before Ralph stops. Default is 100, configurable via `max_iterations`.

## L

**Loop**
: The core Ralph pattern - continuously running an AI agent until task completion or limits are reached.

**Loop Detection**
: Safety feature that detects when an agent is producing repetitive outputs. Uses fuzzy string matching (rapidfuzz) with a 90% similarity threshold to compare recent outputs and prevent infinite loops.

## M

**Metrics**
: Performance and execution data collected in `.agent/metrics/` including timing, errors, and resource usage.

**MkDocs**
: Static site generator used for Ralph's documentation, configured in `mkdocs.yml`.

## O

**Orchestrator**
: The main component that manages the execution loop, agent interaction, and state management.

## P

**Prompt**
: The task description file (usually `PROMPT.md`) that tells the AI agent what to accomplish.

**Prompt Archive**
: Historical storage of all prompt iterations in `.agent/prompts/` for debugging and analysis.

**Plugin**
: Extension mechanism for adding custom agents or commands to Ralph.

## Q

**Q Chat**
: An AI assistant accessible via Q CLI. Smaller context window (8K tokens) but fast execution.

## R

**Ralph Wiggum Technique**
: The software engineering pattern of putting AI agents in a loop until the task is done, created by Geoffrey Huntley.

**Recovery**
: Process of resuming execution from a saved state or Git checkpoint after failure or interruption.

**Retry Logic**
: Automatic retry mechanism with exponential backoff for handling transient failures.

**Runtime Limit**
: Maximum execution time in seconds before Ralph stops. Default is 14400 (4 hours).

## S

**State**
: Current execution status including iteration count, errors, and metrics, saved in `.agent/metrics/state_latest.json`.

**State Manager**
: Component responsible for saving, loading, and updating execution state across iterations.

**Success Criteria**
: The measurable objectives defined in PROMPT.md that guide the orchestrator towards completion.

## T

**Task**
: The work to be accomplished, described in PROMPT.md with requirements and success criteria.

**Task Complete**
: State when the orchestrator reaches its maximum iterations, runtime, or cost limits, having worked towards the defined objectives.

**Timeout**
: Maximum time allowed for a single agent execution. Default is 300 seconds (5 minutes) per iteration.

**Token**
: Unit of text processed by AI models. Roughly 4 characters = 1 token.

## U

**Unpossible**
: Reference to Ralph Wiggum's quote, embodying the philosophy of achieving the seemingly impossible through persistence.

## V

**Verbose Mode**
: Detailed logging mode enabled with `--verbose` flag for debugging and monitoring.

## W

**Working Directory**
: The directory where Ralph executes, containing PROMPT.md and project files. Defaults to current directory.

**Workspace**
: The `.agent/` directory containing Ralph's operational data including metrics, checkpoints, and archives.

## Technical Terms

**API**
: Application Programming Interface - the interface through which Ralph communicates with AI services.

**JSON**
: JavaScript Object Notation - format used for configuration files and state storage.

**Subprocess**
: Separate process spawned to execute AI agents, providing isolation and timeout control.

**YAML**
: YAML Ain't Markup Language - human-readable data format used for some configuration files.

## File Formats

**`.md`**
: Markdown files used for prompts and documentation.

**`.json`**
: JSON files used for configuration and state storage.

**`.yaml`/`.yml`**
: YAML files used for configuration (e.g., `mkdocs.yml`).

**`.log`**
: Log files containing execution history and debugging information.

## Directory Structure

**`.agent/`**
: Ralph's workspace directory containing all operational data.

**`.agent/metrics/`**
: Storage for execution metrics and state files.

**`.agent/prompts/`**
: Archive of historical prompt iterations.

**`.agent/checkpoints/`**
: Markers for Git checkpoints created during execution.

**`.agent/logs/`**
: Execution logs for debugging and analysis.

**`.agent/plans/`**
: Agent planning documents and long-term strategies.

## Command Reference

**`ralph run`**
: Execute the orchestrator with current configuration.

**`ralph init`**
: Initialize a new Ralph project with default structure.

**`ralph status`**
: Check current execution status and metrics.

**`ralph clean`**
: Clean workspace and reset state.

**`ralph agents`**
: List available AI agents on the system.

## Environment Variables

**`RALPH_AGENT`**
: Override default agent selection.

**`RALPH_MAX_ITERATIONS`**
: Set maximum iteration limit.

**`RALPH_MAX_RUNTIME`**
: Set maximum runtime in seconds.

**`RALPH_VERBOSE`**
: Enable verbose logging (true/false).

**`RALPH_DRY_RUN`**
: Enable dry run mode (true/false).

## Exit Codes

**0**: Success - task completed successfully

**1**: General failure - check logs for details

**130**: Interrupted - user pressed Ctrl+C

**137**: Killed - process terminated (often memory issues)

**124**: Timeout - execution time exceeded
