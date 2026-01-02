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
- Progress monitoring via web UI
- Inheriting user's Claude Code settings (MCP servers, CLAUDE.md, etc.)

Usage:
    # Implement the Onboarding feature
    python examples/run_self_improvement.py --feature onboarding

    # Implement the TUI feature
    python examples/run_self_improvement.py --feature tui

    # Implement the Validation feature with web UI monitoring
    python examples/run_self_improvement.py --feature validation --with-web-ui

    # Check status only
    python examples/run_self_improvement.py --status
"""

import argparse
import asyncio
import subprocess
import sys
import webbrowser
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
    "validation": {
        "branch": "feat/agnostic-validation-gates",
        "prompt": "prompts/VALIDATION_FEATURE_PROMPT.md",
        "title": "User-Collaborative Validation Gate System",
        "description": "Opt-in functional validation with user confirmation before proceeding",
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


def start_web_monitor(port: int = 8000) -> Optional[subprocess.Popen]:
    """Start the web monitoring server in the background."""
    print(f"\n🖥️  Starting web monitoring dashboard on port {port}...")

    try:
        # Start web server as a background process
        process = subprocess.Popen(
            [sys.executable, "-m", "ralph_orchestrator.web", "--port", str(port)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Give it a moment to start
        import time
        time.sleep(2)

        # Check if it started successfully
        if process.poll() is None:
            print(f"  ✓ Web UI started at http://localhost:{port}")
            print("    Default credentials: admin / ralph-admin-2024")
            return process
        else:
            print("  ✗ Web UI failed to start")
            return None
    except Exception as e:
        print(f"  ✗ Could not start web UI: {e}")
        return None


def run_ralph(feature: str, verbose: bool = True, with_web_ui: bool = False,
              web_port: int = 8000, open_browser: bool = True) -> None:
    """Run RALPH with the appropriate configuration."""
    feature_info = FEATURES[feature]
    web_process = None

    print(f"\n🚀 Starting RALPH to implement: {feature_info['title']}")
    print(f"   {feature_info['description']}")
    print(f"   Prompt: {feature_info['prompt']}")

    # Start web UI if requested
    if with_web_ui:
        web_process = start_web_monitor(web_port)
        if web_process and open_browser:
            try:
                webbrowser.open(f"http://localhost:{web_port}")
            except Exception:
                pass  # Browser open is optional

    print("\n" + "=" * 60)
    print("📝 Note: Ralph inherits your Claude Code settings (MCP servers, tools)")
    print("   This gives Ralph access to all your configured capabilities.")
    print("=" * 60 + "\n")

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
    finally:
        # Clean up web server if we started it
        if web_process:
            print("\n🛑 Stopping web monitoring server...")
            web_process.terminate()
            try:
                web_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                web_process.kill()


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
  python examples/run_self_improvement.py --feature validation --with-web-ui
  python examples/run_self_improvement.py --feature tui --no-setup
  python examples/run_self_improvement.py --status

Web Monitoring:
  When using --with-web-ui, the script starts a web dashboard at http://localhost:8000
  to monitor RALPH's progress in real-time. Default credentials: admin / ralph-admin-2024

Claude Code Integration:
  RALPH automatically inherits your Claude Code settings (MCP servers, CLAUDE.md, etc.)
  This gives RALPH access to all tools you have configured in your Claude Code session.
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

    parser.add_argument(
        "--with-web-ui", "-w",
        action="store_true",
        help="Start web monitoring dashboard for real-time progress tracking"
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
    run_ralph(
        args.feature,
        verbose=not args.quiet,
        with_web_ui=args.with_web_ui,
        web_port=args.web_port,
        open_browser=not args.no_browser
    )


if __name__ == "__main__":
    main()
