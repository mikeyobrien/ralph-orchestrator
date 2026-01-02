#!/usr/bin/env python3
# ABOUTME: Pure-Python self-improvement runner using Ralph's Python API directly
# ABOUTME: Enables Ralph to build features into itself with proper SDK usage

"""
RALPH Self-Improvement Runner

Uses Ralph's Python API directly (no CLI subprocess) to implement features.
All orchestrator parameters are exposed as CLI options.

Usage:
    # Run with a prompt file
    python scripts/self_improve.py -P prompts/MY_FEATURE_PROMPT.md

    # With web UI monitoring
    python scripts/self_improve.py -P prompts/PROMPT.md --with-web-ui

    # With custom limits
    python scripts/self_improve.py -P prompts/PROMPT.md --max-cost 50 --max-iterations 50

    # Dry run to validate setup
    python scripts/self_improve.py -P prompts/PROMPT.md --dry-run
"""

import argparse
import sys
import threading
import time
import webbrowser
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


class SelfImprovementRunner:
    """Runs Ralph self-improvement using the Python API directly."""

    def __init__(self, verbose: bool = False):
        self.verbose = verbose
        self.console = RalphConsole()
        self.orchestrator: Optional[RalphOrchestrator] = None
        self.web_monitor = None
        self.web_thread = None

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

    def start_web_ui(self, port: int = 8000, open_browser: bool = True) -> bool:
        """Start the web monitoring UI. Returns True if started successfully."""
        try:
            from ralph_orchestrator.web import WebMonitor

            self.web_monitor = WebMonitor(
                host="0.0.0.0",
                port=port,
                enable_auth=False,  # No auth for self-improvement
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

            return True

        except ImportError as e:
            self.console.print_warning(f"Web UI not available: {e}")
            self.web_monitor = None
            return False
        except Exception as e:
            self.console.print_warning(f"Could not start web UI: {e}")
            self.web_monitor = None
            return False

    def run_prompt(
        self,
        prompt_file: str,
        # Orchestration params
        max_iterations: int = DEFAULT_MAX_ITERATIONS,
        max_runtime: int = DEFAULT_MAX_RUNTIME,
        max_cost: float = DEFAULT_MAX_COST,
        checkpoint_interval: int = DEFAULT_CHECKPOINT_INTERVAL,
        context_window: int = DEFAULT_CONTEXT_WINDOW,
        context_threshold: float = DEFAULT_CONTEXT_THRESHOLD,
        output_preview_length: int = DEFAULT_OUTPUT_PREVIEW,
        iteration_telemetry: bool = True,
        # Web UI params
        with_web_ui: bool = False,
        web_port: int = 8000,
        open_browser: bool = True,
    ) -> None:
        """Run Ralph with a prompt file."""
        prompt_path = Path(prompt_file)

        self.console.print_header("RALPH Self-Improvement")
        print(f"\n  Prompt: {prompt_file}")
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


def main():
    parser = argparse.ArgumentParser(
        description="Run RALPH self-improvement to build new features",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Run with a prompt file
  python scripts/self_improve.py -P prompts/MY_FEATURE_PROMPT.md

  # With web UI monitoring
  python scripts/self_improve.py -P prompts/PROMPT.md --with-web-ui

  # With custom limits
  python scripts/self_improve.py -P prompts/PROMPT.md --max-cost 50 --max-iterations 50

  # Dry run to validate setup
  python scripts/self_improve.py -P prompts/PROMPT.md --with-web-ui --dry-run

Note:
  Ralph automatically inherits your Claude Code settings (MCP servers, etc.)
        """
    )

    # Required: prompt file
    parser.add_argument(
        "--prompt", "-P",
        type=str,
        required=True,
        help="Path to prompt file"
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
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show configuration without running (validates setup)"
    )

    args = parser.parse_args()

    runner = SelfImprovementRunner(verbose=args.verbose)

    # Handle dry-run mode
    if args.dry_run:
        prompt_path = Path(args.prompt)

        runner.console.print_header("RALPH Self-Improvement (DRY RUN)")
        print(f"\n  Prompt: {args.prompt}")
        print(f"  Prompt exists: {prompt_path.exists()}")
        print()
        print("  Configuration:")
        print(f"    Max iterations: {args.max_iterations}")
        print(f"    Max runtime: {args.max_runtime}s ({args.max_runtime // 3600}h)")
        print(f"    Max cost: ${args.max_cost:.2f}")
        print(f"    Context window: {args.context_window:,} tokens")
        print(f"    Context threshold: {args.context_threshold:.0%}")
        print(f"    Checkpoint interval: {args.checkpoint_interval}")
        print(f"    Web UI: {'enabled' if args.with_web_ui else 'disabled'}")
        if args.with_web_ui:
            print(f"    Web port: {args.web_port}")
        print()

        if not prompt_path.exists():
            runner.console.print_error(f"Prompt file not found: {args.prompt}")
            sys.exit(1)

        runner.console.print_success("Dry run complete - configuration valid")

        # Start web UI if requested to verify it works
        if args.with_web_ui:
            if runner.start_web_ui(port=args.web_port, open_browser=not args.no_browser):
                print("\n  Web UI started - press Ctrl+C to stop")
                try:
                    while True:
                        time.sleep(1)
                except KeyboardInterrupt:
                    print("\n  Stopped")
        return

    # Build kwargs from CLI args
    run_kwargs = {
        "max_iterations": args.max_iterations,
        "max_runtime": args.max_runtime,
        "max_cost": args.max_cost,
        "checkpoint_interval": args.checkpoint_interval,
        "context_window": args.context_window,
        "context_threshold": args.context_threshold,
        "output_preview_length": args.output_preview_length,
        "iteration_telemetry": not args.no_telemetry,
        "with_web_ui": args.with_web_ui,
        "web_port": args.web_port,
        "open_browser": not args.no_browser,
    }

    runner.run_prompt(prompt_file=args.prompt, **run_kwargs)


if __name__ == "__main__":
    main()
