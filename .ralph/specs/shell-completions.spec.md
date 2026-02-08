---
status: implemented
gap_analysis: 2026-02-03
related: []
---

# Shell Completions Support

## Overview

Ralph CLI should support generating shell completion scripts for popular shells (bash, zsh, fish, PowerShell). This improves the user experience by enabling tab completion for commands, arguments, and flags.

## Design

### Subcommand

Add a new `completions` subcommand to the CLI:

```
ralph completions <SHELL>
```

Where `<SHELL>` is one of:
- `bash`
- `zsh` 
- `fish`
- `powershell`

### Output

The command prints the completion script to stdout, which users can redirect to the appropriate location:

```bash
# Bash
ralph completions bash > ~/.local/share/bash-completion/completions/ralph

# Zsh
ralph completions zsh > ~/.zsh/completions/_ralph

# Fish
ralph completions fish > ~/.config/fish/completions/ralph.fish
```

### Implementation

Use `clap_complete` crate which integrates with clap to generate completions from the existing `Cli` struct definition. This ensures completions stay in sync with the CLI.

## Acceptance Criteria

### Bash Completions
- **Given** Ralph CLI with completions subcommand
- **When** User runs `ralph completions bash`
- **Then** A valid bash completion script is printed to stdout

### Zsh Completions  
- **Given** Ralph CLI with completions subcommand
- **When** User runs `ralph completions zsh`
- **Then** A valid zsh completion script is printed to stdout

### Fish Completions
- **Given** Ralph CLI with completions subcommand
- **When** User runs `ralph completions fish`
- **Then** A valid fish completion script is printed to stdout

### PowerShell Completions
- **Given** Ralph CLI with completions subcommand
- **When** User runs `ralph completions powershell`
- **Then** A valid PowerShell completion script is printed to stdout

### Invalid Shell Error
- **Given** Ralph CLI with completions subcommand
- **When** User runs `ralph completions invalid`
- **Then** An error message is shown listing valid shells

### Help Text
- **Given** Ralph CLI with completions subcommand
- **When** User runs `ralph completions --help`
- **Then** Help text explains usage and supported shells

## Dependencies

Add to `crates/ralph-cli/Cargo.toml`:
```toml
[dependencies]
clap_complete = "4.5"
```
