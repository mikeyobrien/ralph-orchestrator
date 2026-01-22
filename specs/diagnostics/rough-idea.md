# Rough Idea: Diagnostic Logging for Ralph

## Original Description

When running in `--tui` or non-tui mode, any diagnostic information and agent output should be logged that could help with diagnosing issues.

## Initial Context

- Ralph is an orchestrator that runs AI agents (Claude CLI, Kiro, etc.)
- Currently runs in two modes: TUI (terminal UI) and non-TUI (streaming)
- Diagnostic logging would help debug issues in both modes
- Logs should capture information useful for troubleshooting agent behavior, orchestration issues, and user-reported problems
