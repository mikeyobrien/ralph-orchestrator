#!/usr/bin/env python3
# ABOUTME: Pure-Python self-improvement runner using Ralph's Python API directly
# ABOUTME: Enables Ralph to build features into itself with proper SDK usage

"""
RALPH Self-Improvement Runner

Uses Ralph's Python API directly (no CLI subprocess) to implement features.
Configuration is pure Python - no YAML mixing that causes parameter conflicts.

Usage:
    python scripts/self_improve.py --feature validation
    python scripts/self_improve.py --feature onboarding --verbose
    python scripts/self_improve.py --status
"""

import argparse
import asyncio
import sys
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


# Feature definitions - pure Python, no YAML
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

    def show_status(self) -> None:
        """Show current status of self-improvement features."""
        self.console.print_header("RALPH Self-Improvement Status")

        # Get current git branch
        import subprocess
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
                    status_text = "✅ Complete"
                elif "IN PROGRESS" in content.upper():
                    status_text = "🔄 In Progress"
                else:
                    status_text = "📋 Ready"
            else:
                status_text = "❌ Prompt missing"

            print(f"\n{marker} {config.title}")
            print(f"    Branch: {config.branch}")
            print(f"    Status: {status_text}")
            print(f"    Prompt: {config.prompt_file}")

    def create_orchestrator(self, feature: FeatureConfig) -> RalphOrchestrator:
        """Create a properly configured RalphOrchestrator instance."""
        prompt_path = Path(feature.prompt_file)
        if not prompt_path.exists():
            raise FileNotFoundError(f"Prompt file not found: {feature.prompt_file}")

        # Create orchestrator with Python parameters (no YAML)
        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(prompt_path),
            primary_tool="claude",
            max_iterations=feature.max_iterations,
            max_runtime=feature.max_runtime,
            track_costs=True,
            max_cost=feature.max_cost,
            checkpoint_interval=feature.checkpoint_interval,
            verbose=self.verbose,
            iteration_telemetry=True,
            output_preview_length=1000,
        )

        # Configure Claude adapter with all tools enabled
        # This gives Ralph access to all Claude Code capabilities
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

    def run_feature(self, feature_name: str) -> None:
        """Run Ralph to implement a feature."""
        if feature_name not in FEATURES:
            self.console.print_error(f"Unknown feature: {feature_name}")
            self.console.print_info(f"Available: {', '.join(FEATURES.keys())}")
            sys.exit(1)

        feature = FEATURES[feature_name]

        self.console.print_header("RALPH Self-Improvement")
        print(f"\n🚀 Implementing: {feature.title}")
        print(f"   {feature.description}")
        print(f"   Prompt: {feature.prompt_file}")
        print()
        print("=" * 60)
        print("📝 Ralph inherits your Claude Code settings (MCP servers, tools)")
        print("   This gives Ralph access to all your configured capabilities.")
        print("=" * 60)
        print()

        try:
            # Create and run orchestrator
            self.orchestrator = self.create_orchestrator(feature)
            self.orchestrator.run()

        except KeyboardInterrupt:
            self.console.print_warning("\n⏹️  Interrupted by user")
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
  python scripts/self_improve.py --feature validation
  python scripts/self_improve.py --feature onboarding --verbose
  python scripts/self_improve.py --status

Features:
  onboarding  - Intelligent project onboarding and pattern analysis
  tui         - Real-time terminal user interface
  validation  - User-collaborative validation gate system

Note:
  Ralph automatically inherits your Claude Code settings (MCP servers, etc.)
  giving it access to all tools you have configured.
        """
    )

    parser.add_argument(
        "--feature", "-f",
        choices=list(FEATURES.keys()),
        help="Feature to implement"
    )

    parser.add_argument(
        "--status", "-s",
        action="store_true",
        help="Show status of all features"
    )

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

    if not args.feature:
        parser.print_help()
        print("\n⚠️  Please specify a feature with --feature")
        sys.exit(1)

    runner.run_feature(args.feature)


if __name__ == "__main__":
    main()
