#!/usr/bin/env python3
# ABOUTME: CLI entry point for Ralph Orchestrator with all wrapper functionality
# ABOUTME: Provides complete command-line interface including init, status, and clean commands

"""Command-line interface for Ralph Orchestrator."""

import argparse
import sys
import os
import json
import shutil
from pathlib import Path
import logging
import subprocess
from typing import List

# Import the proper orchestrator with adapter support
from .orchestrator import RalphOrchestrator
from .main import (
    RalphConfig, AgentType,
    DEFAULT_MAX_ITERATIONS, DEFAULT_MAX_RUNTIME, DEFAULT_PROMPT_FILE,
    DEFAULT_CHECKPOINT_INTERVAL, DEFAULT_RETRY_DELAY, DEFAULT_MAX_TOKENS,
    DEFAULT_MAX_COST, DEFAULT_CONTEXT_WINDOW, DEFAULT_CONTEXT_THRESHOLD,
    DEFAULT_METRICS_INTERVAL, DEFAULT_MAX_PROMPT_SIZE
)
from .output import RalphConsole

# Global console instance for CLI output
_console = RalphConsole()


def _apply_codex_shortcut(args: argparse.Namespace, parser: argparse.ArgumentParser) -> None:
    """Apply --codex shortcut flags to ACP CLI args.

    --codex is a convenience flag equivalent to:
      --agent acp --acp-agent codex-acp

    It also defaults permission mode to interactive unless explicitly set.
    """
    if not getattr(args, "codex", False):
        return

    # --codex implies ACP mode; reject conflicting explicit agent selection.
    if getattr(args, "agent", "auto") not in ("auto", "acp"):
        parser.error("--codex requires --agent auto or acp (or omit -a/--agent)")

    args.agent = "acp"

    if getattr(args, "acp_agent", None) is None:
        args.acp_agent = "codex-acp"

    if getattr(args, "acp_permission_mode", None) is None:
        args.acp_permission_mode = getattr(args, "codex_permission_mode", None) or "interactive"

    # Optionally push Codex config overrides into the ACP agent args (codex-acp supports -c key=value).
    # Ensure agent_args exists (argparse.REMAINDER yields list, but keep it defensive).
    if not hasattr(args, "agent_args") or args.agent_args is None:
        args.agent_args = []

    codex_model = getattr(args, "codex_model", None)
    if codex_model:
        args.agent_args.extend(["-c", f"model=\"{codex_model}\""])

    codex_reasoning = getattr(args, "codex_reasoning_effort", None)
    if codex_reasoning:
        # Codex config key is `model_reasoning_effort` (NOT the ACP session config option id).
        args.agent_args.extend(["-c", f"model_reasoning_effort=\"{codex_reasoning}\""])


def init_project():
    """Initialize a new Ralph project."""
    _console.print_status("Initializing Ralph project...")

    # Create directories
    dirs = [
        ".agent/prompts",
        ".agent/checkpoints",
        ".agent/metrics",
        ".agent/plans",
        ".agent/memory",
        ".agent/cache"
    ]

    for dir_path in dirs:
        Path(dir_path).mkdir(parents=True, exist_ok=True)

    # Create default PROMPT.md if it doesn't exist
    if not Path("PROMPT.md").exists():
        with open("PROMPT.md", "w") as f:
            f.write("""# Task: [Describe your task here]

## Requirements
- [ ] Requirement 1
- [ ] Requirement 2

## Success Criteria
- All requirements met
- Tests pass
- Code is clean
""")
        _console.print_success("Created PROMPT.md template")
    
    # Create default ralph.yml if it doesn't exist
    if not Path("ralph.yml").exists():
        with open("ralph.yml", "w") as f:
            f.write("""# Ralph Orchestrator Configuration
agent: auto
# Agent selection + fallback ordering (used when agent=auto, and for fallback order)
# Valid values: acp, claude, gemini, qchat (aliases: codex->acp, q->qchat)
agent_priority:
  - claude
  - gemini
  - qchat
  - acp
prompt_file: PROMPT.md
max_iterations: 100
max_runtime: 14400
verbose: false

# Adapter configurations
adapters:
  claude:
    enabled: true
    timeout: 300
  q:
    enabled: true
    timeout: 300
  gemini:
    enabled: true
    timeout: 300
  # ACP (Agent Client Protocol) adapter for Gemini CLI and other ACP-compatible agents
  acp:
    enabled: true
    timeout: 300
    # ACP-specific settings (stored in tool_permissions)
    tool_permissions:
      agent_command: gemini
      agent_args: []
      permission_mode: auto_approve  # auto_approve, deny_all, allowlist, interactive
      permission_allowlist: []       # Patterns for allowlist mode: "fs/*", "/^terminal\\/.*$/"
""")
        _console.print_success("Created ralph.yml configuration")

    # Initialize git if not already
    if not Path(".git").exists():
        subprocess.run(["git", "init"], capture_output=True)
        _console.print_info("Initialized git repository")

    _console.print_success("Ralph project initialized!")
    _console.print_info("Edit ralph.yml to customize configuration")
    _console.print_info("Edit PROMPT.md to define your task")


def show_status():
    """Show current Ralph project status."""
    _console.print_header("Ralph Orchestrator Status")

    # Check for PROMPT.md
    if Path("PROMPT.md").exists():
        _console.print_success("Prompt: PROMPT.md exists")
        _console.print_info("Status: IN PROGRESS")
    else:
        _console.print_warning("Prompt: PROMPT.md not found")

    # Check iterations from metrics
    metrics_dir = Path(".agent/metrics")
    if metrics_dir.exists():
        state_files = sorted(metrics_dir.glob("state_*.json"))
        if state_files:
            latest_state = state_files[-1]
            _console.print_info(f"Latest metrics: {latest_state.name}")
            try:
                with open(latest_state, "r") as f:
                    data = json.load(f)
                    _console.print_info(f"  Iterations: {data.get('iteration_count', 0)}")
                    _console.print_info(f"  Runtime: {data.get('runtime', 0):.1f}s")
                    _console.print_info(f"  Errors: {len(data.get('errors', []))}")
            except json.JSONDecodeError:
                _console.print_warning(f"  Warning: Metrics file {latest_state.name} is corrupted")
            except PermissionError:
                _console.print_warning(f"  Warning: No permission to read {latest_state.name}")
            except Exception as e:
                _console.print_warning(f"  Warning: Could not read metrics - {e}")

    # Check git status
    if Path(".git").exists():
        _console.print_info("Git checkpoints:")
        result = subprocess.run(
            ["git", "log", "--oneline", "-5"],
            capture_output=True,
            text=True
        )
        if result.returncode == 0 and result.stdout:
            _console.print_message(result.stdout.strip())
        else:
            _console.print_info("No checkpoints yet")


def clean_workspace():
    """Clean Ralph workspace."""
    _console.print_status("Cleaning Ralph workspace...")

    # Ask about .agent directory
    response = input("Remove .agent directory? (y/N) ")
    if response.lower() == 'y':
        if Path(".agent").exists():
            shutil.rmtree(".agent")
            _console.print_success("Removed .agent directory")

    # Ask about git reset
    if Path(".git").exists():
        response = input("Reset git to last checkpoint? (y/N) ")
        if response.lower() == 'y':
            subprocess.run(["git", "reset", "--hard", "HEAD"], capture_output=True)
            _console.print_success("Reset to last checkpoint")


def generate_prompt(rough_ideas: List[str], output_file: str = "PROMPT.md", interactive: bool = False, agent: str = "auto"):
    """Generate a structured prompt from rough ideas using AI agent."""

    # Collect ideas if interactive mode
    if interactive:
        _console.print_info("Enter your rough ideas (one per line, press Enter twice to finish):")
        ideas = []
        while True:
            try:
                line = input("> ").strip()
                if not line:
                    if ideas:  # Exit if we have ideas and empty line
                        break
                else:
                    ideas.append(line)
            except KeyboardInterrupt:
                _console.print_warning("Cancelled.")
                return
        rough_ideas = ideas

    if not rough_ideas:
        _console.print_warning("No ideas provided.")
        return
    
    # Determine the project root and create prompts directory
    current_dir = Path(os.getcwd())
    
    # Parse the output file path
    output_path = Path(output_file)
    
    # If the output path is absolute or contains directory separators, use it as-is
    # Otherwise, put it in the prompts directory
    if output_path.is_absolute() or len(output_path.parts) > 1:
        # User specified a full path or relative path with directories
        # Convert relative paths to absolute based on current directory
        if not output_path.is_absolute():
            output_path = current_dir / output_path
        # Create parent directories if needed
        output_path.parent.mkdir(parents=True, exist_ok=True)
    else:
        # Just a filename, put it in prompts directory
        # Look for the project root (where .git is located)
        project_root = current_dir
        found_git = False
        while project_root.parent != project_root:
            if (project_root / '.git').exists():
                found_git = True
                break
            project_root = project_root.parent

        # If no .git found, fall back to current directory
        if not found_git:
            project_root = current_dir

        # Create prompts directory in project root
        prompts_dir = project_root / 'prompts'
        prompts_dir.mkdir(exist_ok=True)
        
        # Update output path to be in prompts directory
        output_path = prompts_dir / output_file
    if output_path.exists():
        response = input(f"{output_path} already exists. Overwrite? (y/N) ")
        if response.lower() != 'y':
            _console.print_warning("Cancelled.")
            return

    _console.print_status("Generating structured prompt using AI...")

    try:
        # Use the specified agent to generate the prompt
        # The agent will create/edit the file directly
        success = generate_prompt_with_agent(rough_ideas, agent, str(output_path))

        if success and output_path.exists():
            _console.print_success(f"Generated structured prompt: {output_path}")
            # Calculate relative path for the command suggestion
            try:
                rel_path = output_path.relative_to(current_dir)
                _console.print_info(f"You can now run: ralph run -p {rel_path}")
            except ValueError:
                _console.print_info(f"You can now run: ralph run -p {output_path}")
        else:
            _console.print_error(f"Failed to generate prompt. Please check if {output_path} was created.")

    except Exception as e:
        _console.print_error(f"Error generating prompt: {e}")
        return


def generate_prompt_with_agent(rough_ideas: List[str], agent: str = "auto", output_file: str = "PROMPT.md") -> bool:
    """Use AI agent to generate structured prompt from rough ideas.
    
    Returns:
        bool: True if the prompt was successfully generated, False otherwise
    """
    
    # Map shorthand to full agent names
    agent_name_map = {
        "c": "claude",
        "g": "gemini", 
        "q": "qchat",
        "claude": "claude",
        "gemini": "gemini",
        "qchat": "qchat",
        "auto": "auto"
    }
    agent = agent_name_map.get(agent, agent)
    
    # Create a generation prompt for the AI
    ideas_text = "\n".join(f"- {idea}" for idea in rough_ideas)
    
    generation_prompt = f"""Convert these rough ideas into a structured PROMPT.md file and WRITE it to {output_file}:

ROUGH IDEAS:
{ideas_text}

INSTRUCTIONS:
1. Create or overwrite the file {output_file} with the structured task prompt
2. Use your file writing tools to create the file
3. The file should contain ONLY the structured markdown with no extra commentary

The file content should follow this EXACT format:

# Task: [Clear, actionable task title]

[Brief description of what needs to be built/accomplished]

## Requirements

- [ ] [Specific requirement 1]
- [ ] [Specific requirement 2]  
- [ ] [Additional requirements based on the ideas]
- [ ] [More requirements as needed]

## Technical Specifications

- [Technical detail 1]
- [Technical detail 2]
- [Framework/technology suggestions if appropriate]
- [More technical details as needed]

## Success Criteria

- [Measurable success criterion 1]
- [Measurable success criterion 2]
- [How to know when task is complete]

IMPORTANT: 
1. WRITE the content to {output_file} using your file writing tools
2. Make requirements specific and actionable with checkboxes
3. Include relevant technical specifications for the task type
4. Make success criteria measurable and clear
5. The file should contain ONLY the structured markdown"""

    # Try to use the specified agent or auto-detect
    success = False
    
    # Import adapters
    try:
        from .adapters.claude import ClaudeAdapter
        from .adapters.qchat import QChatAdapter
        from .adapters.gemini import GeminiAdapter
    except ImportError:
        pass
    
    # Try specified agent first
    if agent == "claude" or agent == "auto":
        try:
            adapter = ClaudeAdapter()
            if adapter.available:
                # Enable file tools and WebSearch for the agent to write PROMPT.md and research if needed
                result = adapter.execute(
                    generation_prompt,
                    enable_all_tools=True,
                    enable_web_search=True,
                    allowed_tools=['Write', 'Edit', 'MultiEdit', 'WebSearch', 'Read', 'Grep']
                )
                if result.success:
                    success = True
                    # Check if the file was created
                    return Path(output_file).exists()
        except Exception as e:
            if agent != "auto":
                _console.print_error(f"Claude adapter failed: {e}")

    if not success and (agent == "gemini" or agent == "auto"):
        try:
            adapter = GeminiAdapter()
            if adapter.available:
                result = adapter.execute(generation_prompt)
                if result.success:
                    success = True
                    # Check if the file was created
                    return Path(output_file).exists()
        except Exception as e:
            if agent != "auto":
                _console.print_error(f"Gemini adapter failed: {e}")

    if not success and (agent == "qchat" or agent == "auto"):
        try:
            adapter = QChatAdapter()
            if adapter.available:
                result = adapter.execute(generation_prompt)
                if result.success:
                    success = True
                    # Check if the file was created
                    return Path(output_file).exists()
        except Exception as e:
            if agent != "auto":
                _console.print_error(f"QChat adapter failed: {e}")
    
    # If no adapter succeeded, return False
    return False




def main():
    """Main CLI entry point."""
    parser = argparse.ArgumentParser(
        prog="ralph",
        description="Ralph Orchestrator - Put AI in a loop until done",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Commands:
    ralph               Run the orchestrator (default)
    ralph init          Initialize a new Ralph project  
    ralph status        Show current Ralph status
    ralph clean         Clean up agent workspace
    ralph prompt        Generate structured prompt from rough ideas

Configuration:
    Use -c/--config to load settings from a YAML file.
    CLI arguments override config file settings.

Examples:
    ralph                           # Run with auto-detected agent
    ralph -c ralph.yml              # Use configuration file
    ralph -a claude                 # Use Claude agent
    ralph -p task.md -i 50          # Custom prompt, max 50 iterations
    ralph -t 3600 --dry-run         # Test mode with 1 hour timeout
    ralph --max-cost 10.00          # Limit spending to $10
    ralph init                      # Set up new project
    ralph status                    # Check current progress
    ralph clean                     # Clean agent workspace
    ralph prompt "build a web API"  # Generate API prompt
    ralph prompt -i                 # Interactive prompt creation
    ralph prompt -o task.md "scrape data" "save to CSV"  # Custom output
"""
    )
    
    # Add subcommands
    subparsers = parser.add_subparsers(dest='command', help='Commands')
    
    # Init command
    subparsers.add_parser('init', help='Initialize a new Ralph project')
    
    # Status command
    subparsers.add_parser('status', help='Show current Ralph status')
    
    # Clean command
    subparsers.add_parser('clean', help='Clean up agent workspace')
    
    # Prompt command
    prompt_parser = subparsers.add_parser('prompt', help='Generate structured prompt from rough ideas')
    prompt_parser.add_argument(
        'ideas',
        nargs='*',
        help='Rough ideas for the task (if none provided, enters interactive mode)'
    )
    prompt_parser.add_argument(
        '-o', '--output',
        default='PROMPT.md',
        help='Output file name (default: PROMPT.md)'
    )
    prompt_parser.add_argument(
        '-i', '--interactive',
        action='store_true',
        help='Interactive mode to collect ideas'
    )
    prompt_parser.add_argument(
        '-a', '--agent',
        choices=['claude', 'c', 'gemini', 'g', 'qchat', 'q', 'auto'],
        default='auto',
        help='AI agent to use: claude/c, gemini/g, qchat/q, auto (default: auto)'
    )
    
    # Run command (default) - add all the run options
    run_parser = subparsers.add_parser('run', help='Run the orchestrator')
    
    # Core arguments (also at root level for backward compatibility)
    for p in [parser, run_parser]:
        p.add_argument(
            "-c", "--config",
            help="Configuration file (YAML format)"
        )
        
        p.add_argument(
            "-a", "--agent",
            choices=["claude", "q", "gemini", "acp", "auto"],
            default="auto",
            help="AI agent to use (default: auto)"
        )

        p.add_argument(
            "--acp-agent",
            default=None,
            help="ACP agent binary/command (default: gemini)"
        )

        p.add_argument(
            "--acp-permission-mode",
            choices=["auto_approve", "deny_all", "allowlist", "interactive"],
            default=None,
            help="ACP permission mode (default: auto_approve)"
        )

        p.add_argument(
            "--codex",
            action="store_true",
            help="Shortcut for --agent acp --acp-agent codex-acp (Codex via ACP)",
        )

        p.add_argument(
            "--codex-permission-mode",
            choices=["auto_approve", "deny_all", "allowlist", "interactive"],
            default=None,
            help="Permission mode shortcut for --codex (default: interactive)",
        )

        p.add_argument(
            "--codex-model",
            default=None,
            help="Model to use with --codex (passed to codex-acp as -c model=...)",
        )

        p.add_argument(
            "--codex-reasoning-effort",
            choices=["low", "medium", "high", "xhigh"],
            default=None,
            help="Reasoning effort to use with --codex (passed to codex-acp as -c model_reasoning_effort=...)",
        )
        
        p.add_argument(
            "-P", "--prompt-file",
            default=DEFAULT_PROMPT_FILE,
            dest="prompt",
            help=f"Prompt file (default: {DEFAULT_PROMPT_FILE})"
        )

        p.add_argument(
            "-p", "--prompt-text",
            default=None,
            help="Direct prompt text (overrides --prompt-file)"
        )
        
        p.add_argument(
            "-i", "--iterations", "--max-iterations",
            type=int,
            default=DEFAULT_MAX_ITERATIONS,
            dest="max_iterations",
            help=f"Maximum iterations (default: {DEFAULT_MAX_ITERATIONS})"
        )
        
        p.add_argument(
            "-t", "--time", "--max-runtime",
            type=int,
            default=DEFAULT_MAX_RUNTIME,
            dest="max_runtime",
            help=f"Maximum runtime in seconds (default: {DEFAULT_MAX_RUNTIME})"
        )
        
        p.add_argument(
            "-v", "--verbose",
            action="store_true",
            help="Enable verbose output"
        )
        
        p.add_argument(
            "-d", "--dry-run",
            action="store_true",
            help="Dry run mode (test without execution)"
        )
        
        # Advanced options
        p.add_argument(
            "--max-tokens",
            type=int,
            default=DEFAULT_MAX_TOKENS,
            help=f"Maximum total tokens (default: {DEFAULT_MAX_TOKENS})"
        )
        
        p.add_argument(
            "--max-cost",
            type=float,
            default=DEFAULT_MAX_COST,
            help=f"Maximum cost in USD (default: {DEFAULT_MAX_COST})"
        )
        
        p.add_argument(
            "--context-window",
            type=int,
            default=DEFAULT_CONTEXT_WINDOW,
            help=f"Context window size (default: {DEFAULT_CONTEXT_WINDOW})"
        )
        
        p.add_argument(
            "--context-threshold",
            type=float,
            default=DEFAULT_CONTEXT_THRESHOLD,
            help=f"Context summarization threshold (default: {DEFAULT_CONTEXT_THRESHOLD})"
        )
        
        p.add_argument(
            "--checkpoint-interval",
            type=int,
            default=DEFAULT_CHECKPOINT_INTERVAL,
            help=f"Git checkpoint interval (default: {DEFAULT_CHECKPOINT_INTERVAL})"
        )
        
        p.add_argument(
            "--retry-delay",
            type=int,
            default=DEFAULT_RETRY_DELAY,
            help=f"Retry delay on errors (default: {DEFAULT_RETRY_DELAY})"
        )
        
        p.add_argument(
            "--metrics-interval",
            type=int,
            default=DEFAULT_METRICS_INTERVAL,
            help=f"Metrics logging interval (default: {DEFAULT_METRICS_INTERVAL})"
        )
        
        p.add_argument(
            "--max-prompt-size",
            type=int,
            default=DEFAULT_MAX_PROMPT_SIZE,
            help=f"Max prompt file size (default: {DEFAULT_MAX_PROMPT_SIZE})"
        )
        
        p.add_argument(
            "--no-git",
            action="store_true",
            help="Disable git checkpointing"
        )
        
        p.add_argument(
            "--no-archive",
            action="store_true",
            help="Disable prompt archiving"
        )
        
        p.add_argument(
            "--no-metrics",
            action="store_true",
            help="Disable metrics collection"
        )
        
        p.add_argument(
            "--allow-unsafe-paths",
            action="store_true",
            help="Allow potentially unsafe prompt paths"
        )
        
        # Collect remaining arguments for agent
        p.add_argument(
            "agent_args",
            nargs=argparse.REMAINDER,
            help="Additional arguments to pass to the AI agent"
        )
    
    # Parse arguments
    args = parser.parse_args()

    # Apply convenience shortcuts (may adjust args.agent/acp settings)
    _apply_codex_shortcut(args, parser)
    
    # Handle commands
    command = args.command if args.command else 'run'
    
    if command == 'init':
        init_project()
        sys.exit(0)
    
    if command == 'status':
        show_status()
        sys.exit(0)
    
    if command == 'clean':
        clean_workspace()
        sys.exit(0)
    
    if command == 'prompt':
        # Use interactive mode if no ideas provided or -i flag used
        interactive_mode = args.interactive or not args.ideas
        generate_prompt(args.ideas, args.output, interactive_mode, args.agent)
        sys.exit(0)
    
    # Run command (default)
    # Set up logging
    log_level = logging.DEBUG if args.verbose else logging.INFO
    logging.basicConfig(
        level=log_level,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    # Map agent string to enum (including shorthand)
    agent_map = {
        "claude": AgentType.CLAUDE,
        "c": AgentType.CLAUDE,
        "q": AgentType.Q,
        "qchat": AgentType.Q,
        "gemini": AgentType.GEMINI,
        "g": AgentType.GEMINI,
        "acp": AgentType.ACP,
        "auto": AgentType.AUTO
    }
    
    # Create config - load from YAML if provided, otherwise use CLI args.
    # If no --config is provided, auto-load ./ralph.yml when present.
    auto_config_path: str | None = None
    if not getattr(args, "config", None) and Path("ralph.yml").exists():
        auto_config_path = "ralph.yml"

    config_path = getattr(args, "config", None) or auto_config_path

    if config_path:
        try:
            config = RalphConfig.from_yaml(config_path)
            if auto_config_path and config_path == auto_config_path:
                _console.print_info(f"Using config: {config_path}")
            # Override with any CLI arguments that were explicitly provided
            if hasattr(args, 'agent') and args.agent != 'auto':
                config.agent = agent_map[args.agent]
            if hasattr(args, 'verbose') and args.verbose:
                config.verbose = args.verbose
            if hasattr(args, 'dry_run') and args.dry_run:
                config.dry_run = args.dry_run
        except Exception as e:
            _console.print_error(f"Error loading config file: {e}")
            sys.exit(1)
    else:
        # Create config from CLI arguments
        config = RalphConfig(
            agent=agent_map[args.agent],
            prompt_file=args.prompt,
            prompt_text=args.prompt_text,
            max_iterations=args.max_iterations,
            max_runtime=args.max_runtime,
            checkpoint_interval=args.checkpoint_interval,
            retry_delay=args.retry_delay,
            archive_prompts=not args.no_archive,
            git_checkpoint=not args.no_git,
            verbose=args.verbose,
            dry_run=args.dry_run,
            max_tokens=args.max_tokens,
            max_cost=args.max_cost,
            context_window=args.context_window,
            context_threshold=args.context_threshold,
            metrics_interval=args.metrics_interval,
            enable_metrics=not args.no_metrics,
            max_prompt_size=args.max_prompt_size,
            allow_unsafe_paths=args.allow_unsafe_paths,
            agent_args=args.agent_args if hasattr(args, 'agent_args') else []
        )

    # Validate prompt source exists and has content (before dry-run check)
    if config.prompt_text is not None:
        # Validate prompt_text is not empty or whitespace-only
        if not config.prompt_text.strip():
            _console.print_error("Prompt text cannot be empty or whitespace-only")
            _console.print_info("Please provide actual task text with --prompt-text")
            _console.print_info("Or use --prompt-file to load from a file")
            sys.exit(1)
    else:
        # Validate prompt file exists
        prompt_path = Path(config.prompt_file)
        if not prompt_path.exists():
            _console.print_error(f"Prompt file '{config.prompt_file}' not found")
            _console.print_info("Please create a PROMPT.md file with your task description.")
            _console.print_info("Or use --prompt-text to provide a prompt directly.")
            _console.print_info("Example content:")
            _console.print_message("""---
# Task: Build a simple web server

## Requirements
- Use Python
- Include basic routing
- Add tests
---""")
            sys.exit(1)

    if config.dry_run:
        _console.print_info("Dry run mode - no tools will be executed")
        _console.print_info("Configuration:")
        if config.prompt_text:
            preview = config.prompt_text[:100] + "..." if len(config.prompt_text) > 100 else config.prompt_text
            _console.print_info(f"  Prompt text: {preview}")
        else:
            _console.print_info(f"  Prompt file: {config.prompt_file}")
        _console.print_info(f"  Agent: {config.agent.value}")
        _console.print_info(f"  Max iterations: {config.max_iterations}")
        _console.print_info(f"  Max runtime: {config.max_runtime}s")
        _console.print_info(f"  Max cost: ${config.max_cost:.2f}")
        sys.exit(0)
    
    try:
        # Create and run orchestrator
        _console.print_header("Starting Ralph Orchestrator")
        _console.print_info(f"Agent: {config.agent.value}")
        if config.prompt_text:
            preview = config.prompt_text[:80] + "..." if len(config.prompt_text) > 80 else config.prompt_text
            _console.print_info(f"Prompt: (text) {preview}")
        else:
            _console.print_info(f"Prompt: {config.prompt_file}")
        _console.print_info(f"Max iterations: {config.max_iterations}")
        _console.print_info("Press Ctrl+C to stop gracefully")
        _console.print_separator()

        # Map CLI agent names to orchestrator tool names
        agent_name = config.agent.value if hasattr(config.agent, 'value') else str(config.agent)
        tool_name_map = {
            "q": "qchat",
            "claude": "claude",
            "gemini": "gemini",
            "acp": "acp",
            "auto": "auto"
        }
        primary_tool = tool_name_map.get(agent_name, agent_name)

        # Pass ACP-specific CLI arguments if using ACP adapter
        acp_agent = getattr(args, 'acp_agent', None)
        acp_permission_mode = getattr(args, 'acp_permission_mode', None)
        acp_agent_args = getattr(args, 'agent_args', None)

        # Pass full config to orchestrator so prompt_text is available
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
            acp_agent_args=acp_agent_args,
        )

        # Enable all tools for Claude adapter (including WebSearch)
        if primary_tool == 'claude' and 'claude' in orchestrator.adapters:
            claude_adapter = orchestrator.adapters['claude']
            claude_adapter.configure(enable_all_tools=True, enable_web_search=True)
            if config.verbose:
                _console.print_success("Claude configured with all native tools including WebSearch")

        orchestrator.run()

        _console.print_separator()
        _console.print_success("Ralph Orchestrator completed successfully")

    except KeyboardInterrupt:
        _console.print_warning("Received interrupt signal, shutting down gracefully...")
        sys.exit(0)
    except Exception as e:
        _console.print_error(f"Error: {e}")
        if config.verbose:
            import traceback
            traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
