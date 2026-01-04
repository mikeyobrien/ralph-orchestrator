#!/usr/bin/env python3
# ABOUTME: Script to run RALPH self-improvement - RALPH enhancing RALPH
# ABOUTME: Handles fork setup, branch creation, and orchestration for new features

"""
RALPH Self-Improvement Runner

This script helps set up and run RALPH to implement new features on itself.
It handles:
- Fork verification and sync
- Feature branch creation
- RALPH orchestration with proper configuration
- Progress monitoring

Usage:
    # Implement the Onboarding feature
    python examples/run_self_improvement.py --feature onboarding

    # Implement the TUI feature
    python examples/run_self_improvement.py --feature tui

    # Check status only
    python examples/run_self_improvement.py --status
"""

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Optional


FEATURES = {
    "onboarding": {
        "branch": "feature/intelligent-onboarding",
        "prompt": "prompts/ONBOARDING_PROMPT.md",
        "title": "Intelligent Project Onboarding & Pattern Analysis",
        "description": "Analyzes project patterns and generates custom configurations",
    },
    "tui": {
        "branch": "feature/realtime-tui",
        "prompt": "prompts/TUI_PROMPT.md",
        "title": "Real-Time Terminal User Interface",
        "description": "Live terminal interface for watching RALPH in action",
    },
}


def run_command(cmd: list[str], check: bool = True, capture: bool = False) -> Optional[str]:
    """Run a shell command with proper error handling."""
    print(f"  → {' '.join(cmd)}")
    try:
        result = subprocess.run(
            cmd,
            check=check,
            capture_output=capture,
            text=True,
        )
        if capture:
            return result.stdout.strip()
        return None
    except subprocess.CalledProcessError as e:
        print(f"  ✗ Command failed: {e}")
        if capture and e.stderr:
            print(f"    Error: {e.stderr}")
        return None


def check_git_status() -> dict:
    """Check the current git repository status."""
    status = {}

    # Check if we're in a git repo
    result = run_command(["git", "rev-parse", "--git-dir"], check=False, capture=True)
    status["is_git_repo"] = bool(result)

    if not status["is_git_repo"]:
        return status

    # Get current branch
    status["branch"] = run_command(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        check=False,
        capture=True
    )

    # Check for upstream remote
    remotes = run_command(["git", "remote"], check=False, capture=True) or ""
    status["has_upstream"] = "upstream" in remotes

    # Check for uncommitted changes
    diff_result = run_command(["git", "status", "--porcelain"], check=False, capture=True)
    status["has_changes"] = bool(diff_result)

    return status


def setup_fork(feature: str) -> bool:
    """Ensure fork is properly set up for the feature."""
    feature_info = FEATURES[feature]

    print("\n📋 Checking repository status...")
    status = check_git_status()

    if not status.get("is_git_repo"):
        print("  ✗ Not in a git repository")
        return False

    print(f"  ✓ Git repository detected")
    print(f"  ✓ Current branch: {status.get('branch', 'unknown')}")

    # Add upstream if missing
    if not status.get("has_upstream"):
        print("\n🔗 Adding upstream remote...")
        run_command([
            "git", "remote", "add", "upstream",
            "https://github.com/mikeyobrien/ralph-orchestrator.git"
        ], check=False)

    # Warn about uncommitted changes
    if status.get("has_changes"):
        print("\n⚠️  Warning: You have uncommitted changes")
        response = input("   Continue anyway? [y/N]: ")
        if response.lower() != 'y':
            return False

    # Create or switch to feature branch
    target_branch = feature_info["branch"]
    current_branch = status.get("branch")

    if current_branch != target_branch:
        print(f"\n🌿 Setting up feature branch: {target_branch}")

        # Check if branch exists
        branches = run_command(["git", "branch", "--list", target_branch], capture=True)

        if branches:
            print(f"  Switching to existing branch...")
            run_command(["git", "checkout", target_branch])
        else:
            print(f"  Creating new branch from main...")
            run_command(["git", "checkout", "-b", target_branch], check=False)

    print(f"  ✓ On branch: {target_branch}")
    return True


def run_ralph(feature: str, verbose: bool = True) -> None:
    """Run RALPH with the appropriate configuration."""
    feature_info = FEATURES[feature]

    print(f"\n🚀 Starting RALPH to implement: {feature_info['title']}")
    print(f"   {feature_info['description']}")
    print(f"   Prompt: {feature_info['prompt']}")
    print("\n" + "=" * 60)

    cmd = [
        "ralph", "run",
        "-P", feature_info["prompt"],
        "-c", "examples/ralph-self-improvement.yml",
    ]

    if verbose:
        cmd.append("-v")

    # Run RALPH - this will take over the terminal
    try:
        subprocess.run(cmd, check=True)
    except subprocess.CalledProcessError:
        print("\n⚠️  RALPH exited with errors")
    except KeyboardInterrupt:
        print("\n\n⏹️  RALPH interrupted by user")


def show_status() -> None:
    """Show current status of self-improvement features."""
    print("\n📊 RALPH Self-Improvement Status")
    print("=" * 60)

    status = check_git_status()
    current_branch = status.get("branch", "unknown")

    for name, info in FEATURES.items():
        is_current = current_branch == info["branch"]
        marker = "→" if is_current else " "

        # Check if prompt file exists and has progress
        prompt_path = Path(info["prompt"])
        status_text = "Not started"

        if prompt_path.exists():
            content = prompt_path.read_text()
            if "[x] TASK_COMPLETE" in content:
                status_text = "✅ Complete"
            elif "IN PROGRESS" in content.upper():
                status_text = "🔄 In Progress"
            else:
                status_text = "📋 Ready"

        print(f"\n{marker} {info['title']}")
        print(f"    Branch: {info['branch']}")
        print(f"    Status: {status_text}")
        print(f"    Prompt: {info['prompt']}")


def main():
    parser = argparse.ArgumentParser(
        description="Run RALPH self-improvement to build new features",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python examples/run_self_improvement.py --feature onboarding
  python examples/run_self_improvement.py --feature tui --no-setup
  python examples/run_self_improvement.py --status
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
        "--no-setup",
        action="store_true",
        help="Skip fork/branch setup, run RALPH directly"
    )

    parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Run RALPH without verbose output"
    )

    args = parser.parse_args()

    print("\n🤖 RALPH Self-Improvement Runner")
    print("=" * 60)

    if args.status:
        show_status()
        return

    if not args.feature:
        parser.print_help()
        print("\n⚠️  Please specify a feature with --feature")
        sys.exit(1)

    # Setup fork/branch if requested
    if not args.no_setup:
        if not setup_fork(args.feature):
            print("\n❌ Setup failed, aborting")
            sys.exit(1)

    # Run RALPH
    run_ralph(args.feature, verbose=not args.quiet)


if __name__ == "__main__":
    main()
