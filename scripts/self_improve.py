#!/usr/bin/env python3
# ABOUTME: Pure-Python self-improvement runner using Ralph's Python API directly
# ABOUTME: Enables Ralph to build features into itself with proper SDK usage

"""
RALPH Self-Improvement Runner

Uses Ralph's Python API directly (no CLI subprocess) to implement features.
Supports both predefined features and direct prompt file paths.
All orchestrator parameters are exposed as CLI options.

Usage:
    # Using direct prompt file (recommended)
    python scripts/self_improve.py -P prompts/VALIDATION_FEATURE_PROMPT.md
    python scripts/self_improve.py -P prompts/MY_PROMPT.md --with-web-ui

    # Using predefined features
    python scripts/self_improve.py --feature validation
    python scripts/self_improve.py --feature onboarding --verbose

    # Status
    python scripts/self_improve.py --status
"""

import argparse
import subprocess
import sys
import threading
import time
import webbrowser
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

# Add src to path for direct imports
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from ralph_orchestrator.orchestrator import RalphOrchestrator
from ralph_orchestrator.adapters.claude import ClaudeAdapter
from ralph_orchestrator.output import RalphConsole
from ralph_orchestrator.main import RalphConfig, AgentType


# Defaults optimized for self-improvement tasks
DEFAULT_MAX_ITERATIONS = 100
DEFAULT_MAX_RUNTIME = 14400  # 4 hours
DEFAULT_MAX_COST = 100.0
DEFAULT_CHECKPOINT_INTERVAL = 3
DEFAULT_CONTEXT_WINDOW = 200000  # 200k tokens
DEFAULT_CONTEXT_THRESHOLD = 0.95  # Summarize at 95%
DEFAULT_OUTPUT_PREVIEW = 1000


@dataclass
class FeatureConfig:
    """Configuration for a self-improvement feature."""
    name: str
    branch: str
    prompt_file: str
    title: str
    description: str


# Predefined features - use --prompt for custom prompts
FEATURES = {
    "onboarding": FeatureConfig(
        name="onboarding",
        branch="feature/intelligent-onboarding",
        prompt_file="prompts/ONBOARDING_PROMPT.md",
        title="Intelligent Project Onboarding & Pattern Analysis",
        description="Analyzes project patterns and generates custom configurations",
    ),
    "tui": FeatureConfig(
        name="tui",
        branch="feature/realtime-tui",
        prompt_file="prompts/TUI_PROMPT.md",
        title="Real-Time Terminal User Interface",
        description="Live terminal interface for watching RALPH in action",
    ),
    "validation": FeatureConfig(
        name="validation",
        branch="feat/agnostic-validation-gates",
        prompt_file="prompts/VALIDATION_FEATURE_PROMPT.md",
        title="User-Collaborative Validation Gate System",
        description="Opt-in functional validation with user confirmation before proceeding",
    ),
}


class SelfImprovementRunner:
    """Runs Ralph self-improvement using the Python API directly."""

    def __init__(self, verbose: bool = False):
        self.verbose = verbose
        self.console = RalphConsole()
        self.orchestrator: Optional[RalphOrchestrator] = None
        self.web_monitor = None
        self.web_thread = None

    def show_status(self) -> None:
        """Show current status of self-improvement features."""
        self.console.print_header("RALPH Self-Improvement Status")

        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True, text=True
        )
        current_branch = result.stdout.strip() if result.returncode == 0 else "unknown"

        for name, config in FEATURES.items():
            is_current = current_branch == config.branch
            marker = ">" if is_current else " "

            prompt_path = Path(config.prompt_file)
            if prompt_path.exists():
                content = prompt_path.read_text()
                if "[x] TASK_COMPLETE" in content:
                    status_text = "Complete"
                elif "IN PROGRESS" in content.upper():
                    status_text = "In Progress"
                else:
                    status_text = "Ready"
            else:
                status_text = "Prompt missing"

            print(f"\n{marker} {config.title}")
            print(f"    Branch: {config.branch}")
            print(f"    Status: {status_text}")
            print(f"    Prompt: {config.prompt_file}")

    def create_orchestrator(
        self,
        prompt_file: str,
        max_iterations: int = DEFAULT_MAX_ITERATIONS,
        max_runtime: int = DEFAULT_MAX_RUNTIME,
        max_cost: float = DEFAULT_MAX_COST,
        checkpoint_interval: int = DEFAULT_CHECKPOINT_INTERVAL,
        context_window: int = DEFAULT_CONTEXT_WINDOW,
        context_threshold: float = DEFAULT_CONTEXT_THRESHOLD,
        output_preview_length: int = DEFAULT_OUTPUT_PREVIEW,
        iteration_telemetry: bool = True,
        enable_validation: bool = False,
        validation_interactive: bool = True,
    ) -> RalphOrchestrator:
        """Create a properly configured RalphOrchestrator instance."""
        prompt_path = Path(prompt_file)
        if not prompt_path.exists():
            raise FileNotFoundError(f"Prompt file not found: {prompt_file}")

        # Use RalphConfig for full parameter support
        config = RalphConfig(
            agent=AgentType.CLAUDE,
            prompt_file=str(prompt_path),
            max_iterations=max_iterations,
            max_runtime=max_runtime,
            max_cost=max_cost,
            checkpoint_interval=checkpoint_interval,
            context_window=context_window,
            context_threshold=context_threshold,
            verbose=self.verbose,
        )

        orchestrator = RalphOrchestrator(
            prompt_file_or_config=config,
            iteration_telemetry=iteration_telemetry,
            output_preview_length=output_preview_length,
            enable_validation=enable_validation,
            validation_interactive=validation_interactive,
        )

        # Configure Claude adapter with all tools
        if 'claude' in orchestrator.adapters:
            claude_adapter: ClaudeAdapter = orchestrator.adapters['claude']
            claude_adapter.configure(
                enable_all_tools=True,
                enable_web_search=True,
            )
            if self.verbose:
                self.console.print_success(
                    "Claude configured with all native tools + user's MCP servers"
                )

        return orchestrator

    def start_web_ui(self, port: int = 8000, open_browser: bool = True) -> None:
        """Start the web monitoring UI."""
        try:
            from ralph_orchestrator.web import WebMonitor

            self.web_monitor = WebMonitor(
                host="0.0.0.0",
                port=port,
                enable_auth=False,
            )

            def run_server():
                import uvicorn
                uvicorn.run(
                    self.web_monitor.app,
                    host="0.0.0.0",
                    port=port,
                    log_level="warning" if not self.verbose else "info",
                )

            self.web_thread = threading.Thread(target=run_server, daemon=True)
            self.web_thread.start()
            time.sleep(1)

            self.console.print_success(f"Web UI started at http://localhost:{port}")

            if open_browser:
                try:
                    webbrowser.open(f"http://localhost:{port}")
                except Exception:
                    pass

        except ImportError as e:
            self.console.print_warning(f"Web UI not available: {e}")
            self.web_monitor = None
        except Exception as e:
            self.console.print_warning(f"Could not start web UI: {e}")
            self.web_monitor = None

    def run_prompt(
        self,
        prompt_file: str,
        title: Optional[str] = None,
        description: Optional[str] = None,
        # Orchestration params
        max_iterations: int = DEFAULT_MAX_ITERATIONS,
        max_runtime: int = DEFAULT_MAX_RUNTIME,
        max_cost: float = DEFAULT_MAX_COST,
        checkpoint_interval: int = DEFAULT_CHECKPOINT_INTERVAL,
        context_window: int = DEFAULT_CONTEXT_WINDOW,
        context_threshold: float = DEFAULT_CONTEXT_THRESHOLD,
        output_preview_length: int = DEFAULT_OUTPUT_PREVIEW,
        iteration_telemetry: bool = True,
        enable_validation: bool = False,
        validation_interactive: bool = True,
        # Web UI params
        with_web_ui: bool = False,
        web_port: int = 8000,
        open_browser: bool = True,
    ) -> None:
        """Run Ralph with a prompt file."""
        prompt_path = Path(prompt_file)
        display_title = title or prompt_path.stem
        display_desc = description or f"Running prompt: {prompt_file}"

        self.console.print_header("RALPH Self-Improvement")
        print(f"\n  Implementing: {display_title}")
        print(f"   {display_desc}")
        print(f"   Prompt: {prompt_file}")
        print()
        print("=" * 60)
        print("  Ralph inherits your Claude Code settings (MCP servers, tools)")
        print("=" * 60)
        print()

        if with_web_ui:
            self.start_web_ui(port=web_port, open_browser=open_browser)

        try:
            self.orchestrator = self.create_orchestrator(
                prompt_file=prompt_file,
                max_iterations=max_iterations,
                max_runtime=max_runtime,
                max_cost=max_cost,
                checkpoint_interval=checkpoint_interval,
                context_window=context_window,
                context_threshold=context_threshold,
                output_preview_length=output_preview_length,
                iteration_telemetry=iteration_telemetry,
                enable_validation=enable_validation,
                validation_interactive=validation_interactive,
            )

            if self.web_monitor and self.orchestrator:
                try:
                    self.web_monitor.register_orchestrator(
                        "self-improvement",
                        self.orchestrator
                    )
                except Exception as e:
                    if self.verbose:
                        self.console.print_warning(f"Could not register with web UI: {e}")

            self.orchestrator.run()

        except KeyboardInterrupt:
            self.console.print_warning("\n  Interrupted by user")
        except Exception as e:
            self.console.print_error(f"Error: {e}")
            if self.verbose:
                import traceback
                traceback.print_exc()
            sys.exit(1)

    def run_feature(self, feature_name: str, **kwargs) -> None:
        """Run Ralph to implement a predefined feature."""
        if feature_name not in FEATURES:
            self.console.print_error(f"Unknown feature: {feature_name}")
            self.console.print_info(f"Available: {', '.join(FEATURES.keys())}")
            sys.exit(1)

        feature = FEATURES[feature_name]
        self.run_prompt(
            prompt_file=feature.prompt_file,
            title=feature.title,
            description=feature.description,
            **kwargs,
        )


def main():
    parser = argparse.ArgumentParser(
        description="Run RALPH self-improvement to build new features",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Direct prompt file (recommended)
  python scripts/self_improve.py -P prompts/VALIDATION_FEATURE_PROMPT.md
  python scripts/self_improve.py -P prompts/MY_PROMPT.md --with-web-ui

  # With custom limits
  python scripts/self_improve.py -P prompts/PROMPT.md --max-cost 50 --max-iterations 50

  # Predefined features
  python scripts/self_improve.py --feature validation
  python scripts/self_improve.py --feature onboarding --verbose

  # Check status
  python scripts/self_improve.py --status

Note:
  Ralph automatically inherits your Claude Code settings (MCP servers, etc.)
        """
    )

    # Input options (mutually exclusive)
    input_group = parser.add_mutually_exclusive_group()
    input_group.add_argument(
        "--prompt", "-P",
        type=str,
        help="Path to prompt file"
    )
    input_group.add_argument(
        "--feature", "-f",
        choices=list(FEATURES.keys()),
        help="Predefined feature to implement"
    )
    input_group.add_argument(
        "--status", "-s",
        action="store_true",
        help="Show status of predefined features"
    )

    # Orchestration limits
    parser.add_argument(
        "--max-iterations",
        type=int,
        default=DEFAULT_MAX_ITERATIONS,
        help=f"Maximum iterations (default: {DEFAULT_MAX_ITERATIONS})"
    )
    parser.add_argument(
        "--max-runtime",
        type=int,
        default=DEFAULT_MAX_RUNTIME,
        help=f"Maximum runtime in seconds (default: {DEFAULT_MAX_RUNTIME} = 4 hours)"
    )
    parser.add_argument(
        "--max-cost",
        type=float,
        default=DEFAULT_MAX_COST,
        help=f"Maximum cost in dollars (default: {DEFAULT_MAX_COST})"
    )
    parser.add_argument(
        "--checkpoint-interval",
        type=int,
        default=DEFAULT_CHECKPOINT_INTERVAL,
        help=f"Git checkpoint frequency (default: {DEFAULT_CHECKPOINT_INTERVAL})"
    )

    # Context settings
    parser.add_argument(
        "--context-window",
        type=int,
        default=DEFAULT_CONTEXT_WINDOW,
        help=f"Context window size in tokens (default: {DEFAULT_CONTEXT_WINDOW:,})"
    )
    parser.add_argument(
        "--context-threshold",
        type=float,
        default=DEFAULT_CONTEXT_THRESHOLD,
        help=f"Context summarization threshold (default: {DEFAULT_CONTEXT_THRESHOLD})"
    )

    # Telemetry settings
    parser.add_argument(
        "--output-preview-length",
        type=int,
        default=DEFAULT_OUTPUT_PREVIEW,
        help=f"Max chars for output preview in telemetry (default: {DEFAULT_OUTPUT_PREVIEW})"
    )
    parser.add_argument(
        "--no-telemetry",
        action="store_true",
        help="Disable per-iteration telemetry"
    )

    # Validation settings
    parser.add_argument(
        "--enable-validation",
        action="store_true",
        help="Enable functional validation (opt-in, Claude-only)"
    )
    parser.add_argument(
        "--no-validation-interactive",
        action="store_true",
        help="Skip user confirmation for validation strategy"
    )

    # Web UI options
    parser.add_argument(
        "--with-web-ui", "-w",
        action="store_true",
        help="Start web monitoring dashboard"
    )
    parser.add_argument(
        "--web-port",
        type=int,
        default=8000,
        help="Port for web monitoring dashboard (default: 8000)"
    )
    parser.add_argument(
        "--no-browser",
        action="store_true",
        help="Don't automatically open browser when starting web UI"
    )

    # Output options
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Enable verbose output"
    )

    args = parser.parse_args()

    runner = SelfImprovementRunner(verbose=args.verbose)

    if args.status:
        runner.show_status()
        return

    if not args.feature and not args.prompt:
        parser.print_help()
        print("\n  Please specify --prompt or --feature")
        sys.exit(1)

    # Build kwargs from all CLI args
    run_kwargs = {
        # Orchestration
        "max_iterations": args.max_iterations,
        "max_runtime": args.max_runtime,
        "max_cost": args.max_cost,
        "checkpoint_interval": args.checkpoint_interval,
        "context_window": args.context_window,
        "context_threshold": args.context_threshold,
        "output_preview_length": args.output_preview_length,
        "iteration_telemetry": not args.no_telemetry,
        "enable_validation": args.enable_validation,
        "validation_interactive": not args.no_validation_interactive,
        # Web UI
        "with_web_ui": args.with_web_ui,
        "web_port": args.web_port,
        "open_browser": not args.no_browser,
    }

    if args.prompt:
        runner.run_prompt(prompt_file=args.prompt, **run_kwargs)
    else:
        runner.run_feature(args.feature, **run_kwargs)


if __name__ == "__main__":
    main()
