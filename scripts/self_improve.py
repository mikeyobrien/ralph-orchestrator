#!/usr/bin/env python3
# ABOUTME: Pure-Python self-improvement runner using Ralph's Python API directly
# ABOUTME: Enables Ralph to build features into itself with proper SDK usage

"""
RALPH Self-Improvement Runner

Uses Ralph's Python API directly (no CLI subprocess) to implement features.
Supports both predefined features and direct prompt file paths.

Usage:
    # Using predefined features
    python scripts/self_improve.py --feature validation
    python scripts/self_improve.py --feature onboarding --verbose

    # Using direct prompt file (more flexible)
    python scripts/self_improve.py --prompt prompts/VALIDATION_FEATURE_PROMPT.md
    python scripts/self_improve.py -P prompts/MY_CUSTOM_PROMPT.md --with-web-ui

    # Status
    python scripts/self_improve.py --status
"""

import argparse
import asyncio
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


@dataclass
class FeatureConfig:
    """Configuration for a self-improvement feature."""
    name: str
    branch: str
    prompt_file: str
    title: str
    description: str
    # Orchestration settings
    max_iterations: int = 100
    max_runtime: int = 14400  # 4 hours
    max_cost: float = 100.0
    checkpoint_interval: int = 3


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

    def show_status(self) -> None:
        """Show current status of self-improvement features."""
        self.console.print_header("RALPH Self-Improvement Status")

        # Get current git branch
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True, text=True
        )
        current_branch = result.stdout.strip() if result.returncode == 0 else "unknown"

        for name, config in FEATURES.items():
            is_current = current_branch == config.branch
            marker = "→" if is_current else " "

            # Check prompt file status
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
        max_iterations: int = 100,
        max_runtime: int = 14400,
        max_cost: float = 100.0,
        checkpoint_interval: int = 3,
    ) -> RalphOrchestrator:
        """Create a properly configured RalphOrchestrator instance."""
        prompt_path = Path(prompt_file)
        if not prompt_path.exists():
            raise FileNotFoundError(f"Prompt file not found: {prompt_file}")

        # Create orchestrator with Python parameters (no YAML)
        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(prompt_path),
            primary_tool="claude",
            max_iterations=max_iterations,
            max_runtime=max_runtime,
            track_costs=True,
            max_cost=max_cost,
            checkpoint_interval=checkpoint_interval,
            verbose=self.verbose,
            iteration_telemetry=True,
            output_preview_length=1000,
        )

        # Configure Claude adapter with all tools enabled
        if 'claude' in orchestrator.adapters:
            claude_adapter: ClaudeAdapter = orchestrator.adapters['claude']
            claude_adapter.configure(
                enable_all_tools=True,
                enable_web_search=True,
                # inherit_user_settings is True by default in ClaudeAdapter
                # This loads user's MCP servers from ~/.claude/settings.json
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
                enable_auth=False,  # Disable for local development
            )

            # Start web server in background thread
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

            # Give server time to start
            time.sleep(1)

            self.console.print_success(f"Web UI started at http://localhost:{port}")

            if open_browser:
                try:
                    webbrowser.open(f"http://localhost:{port}")
                except Exception:
                    pass  # Browser open is optional

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
        max_iterations: int = 100,
        max_runtime: int = 14400,
        max_cost: float = 100.0,
        checkpoint_interval: int = 3,
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
        print("   This gives Ralph access to all your configured capabilities.")
        print("=" * 60)
        print()

        # Start web UI if requested
        if with_web_ui:
            self.start_web_ui(port=web_port, open_browser=open_browser)

        try:
            # Create and run orchestrator
            self.orchestrator = self.create_orchestrator(
                prompt_file=prompt_file,
                max_iterations=max_iterations,
                max_runtime=max_runtime,
                max_cost=max_cost,
                checkpoint_interval=checkpoint_interval,
            )

            # Register with web monitor if available
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
            max_iterations=feature.max_iterations,
            max_runtime=feature.max_runtime,
            max_cost=feature.max_cost,
            checkpoint_interval=feature.checkpoint_interval,
            **kwargs,
        )


def main():
    parser = argparse.ArgumentParser(
        description="Run RALPH self-improvement to build new features",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Using predefined features
  python scripts/self_improve.py --feature validation
  python scripts/self_improve.py --feature onboarding --verbose

  # Using direct prompt file (more flexible)
  python scripts/self_improve.py --prompt prompts/VALIDATION_FEATURE_PROMPT.md
  python scripts/self_improve.py -P prompts/MY_CUSTOM_PROMPT.md --with-web-ui

  # With web monitoring dashboard
  python scripts/self_improve.py -P prompts/PROMPT.md --with-web-ui --web-port 9000

  # Check status
  python scripts/self_improve.py --status

Note:
  Ralph automatically inherits your Claude Code settings (MCP servers, etc.)
  giving it access to all tools you have configured.
        """
    )

    # Input options (mutually exclusive)
    input_group = parser.add_mutually_exclusive_group()
    input_group.add_argument(
        "--feature", "-f",
        choices=list(FEATURES.keys()),
        help="Predefined feature to implement"
    )
    input_group.add_argument(
        "--prompt", "-P",
        type=str,
        help="Direct path to prompt file (more flexible than --feature)"
    )
    input_group.add_argument(
        "--status", "-s",
        action="store_true",
        help="Show status of predefined features"
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

    # Orchestration options
    parser.add_argument(
        "--max-iterations",
        type=int,
        default=100,
        help="Maximum iterations (default: 100)"
    )
    parser.add_argument(
        "--max-runtime",
        type=int,
        default=14400,
        help="Maximum runtime in seconds (default: 14400 = 4 hours)"
    )
    parser.add_argument(
        "--max-cost",
        type=float,
        default=100.0,
        help="Maximum cost in dollars (default: 100.0)"
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
        print("\n  Please specify --feature or --prompt")
        sys.exit(1)

    # Common kwargs for both modes
    run_kwargs = {
        "with_web_ui": args.with_web_ui,
        "web_port": args.web_port,
        "open_browser": not args.no_browser,
    }

    if args.prompt:
        # Direct prompt file mode
        runner.run_prompt(
            prompt_file=args.prompt,
            max_iterations=args.max_iterations,
            max_runtime=args.max_runtime,
            max_cost=args.max_cost,
            **run_kwargs,
        )
    else:
        # Predefined feature mode
        runner.run_feature(args.feature, **run_kwargs)


if __name__ == "__main__":
    main()
