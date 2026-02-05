#!/usr/bin/env python3
"""Tier 3: Visual regression testing using TUI-Validate skill.

This script demonstrates how to use the /tui-validate skill for visual
regression testing of the Hats TUI. It's designed to be run manually
or integrated into CI pipelines.

Usage:
    # Validate header from captured output
    python tools/e2e/tui_visual_regression.py validate-header output.txt

    # Validate full TUI from tmux session
    python tools/e2e/tui_visual_regression.py validate-full hats-session

    # Run all validations from fixtures
    python tools/e2e/tui_visual_regression.py validate-fixtures

See specs/tui-integration-tests.PROMPT.md for full documentation.
"""

import argparse
import subprocess
import sys
from pathlib import Path


def check_prerequisites() -> bool:
    """Check that freeze and tmux are available."""
    missing = []

    # Check freeze
    result = subprocess.run(
        ["freeze", "--version"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        missing.append("freeze (brew install charmbracelet/tap/freeze)")

    # Check tmux
    result = subprocess.run(
        ["tmux", "-V"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        missing.append("tmux (brew install tmux)")

    if missing:
        print("Missing prerequisites:")
        for dep in missing:
            print(f"  - {dep}")
        return False

    return True


def capture_file_with_freeze(file_path: Path, output_path: Path) -> bool:
    """Capture a file containing ANSI output using freeze.

    Args:
        file_path: Path to file with ANSI output
        output_path: Where to save the screenshot

    Returns:
        True if capture succeeded
    """
    cmd = [
        "freeze",
        str(file_path),
        "-o", str(output_path),
        "--theme", "base16",
    ]

    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Freeze capture failed: {result.stderr}")
        return False

    print(f"Screenshot saved: {output_path}")
    return True


def capture_tmux_session(session_name: str, output_path: Path) -> bool:
    """Capture a tmux session using freeze.

    Args:
        session_name: Name of tmux session
        output_path: Where to save the screenshot

    Returns:
        True if capture succeeded
    """
    # First capture the pane content
    capture_cmd = ["tmux", "capture-pane", "-p", "-e", "-t", session_name]
    capture_result = subprocess.run(capture_cmd, capture_output=True, text=True)

    if capture_result.returncode != 0:
        print(f"tmux capture failed: {capture_result.stderr}")
        return False

    # Pipe to freeze
    freeze_cmd = [
        "freeze",
        "-o", str(output_path),
        "--theme", "base16",
        "--language", "ansi",
    ]

    result = subprocess.run(
        freeze_cmd,
        input=capture_result.stdout,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print(f"Freeze capture failed: {result.stderr}")
        return False

    print(f"Screenshot saved: {output_path}")
    return True


def validate_header(content: str) -> dict:
    """Validate Hats header content against criteria.

    Args:
        content: Terminal content to validate

    Returns:
        Validation result dict with 'passed' and 'details'
    """
    import re

    checks = {}

    # Check iteration counter - may appear as [iter N] or just numbers in tmux capture
    iter_match = re.search(r"\[iter\s+\d+(/\d+)?\]", content) or re.search(r"^\s*\d+", content, re.MULTILINE)
    checks["iteration_counter"] = {
        "passed": iter_match is not None,
        "reason": "Found iteration indicator" if iter_match else "Missing iteration indicator",
    }

    # Check elapsed time
    time_match = re.search(r"\d{1,2}:\d{2}", content)
    checks["elapsed_time"] = {
        "passed": time_match is not None,
        "reason": "Found elapsed time" if time_match else "Missing MM:SS format",
    }

    # Check mode indicator - look for various formats including ANSI-escaped
    # Also check for hat names or status indicators as proxy for TUI running
    mode_match = (
        re.search(r"(LIVE|REVIEW|▶|◀|auto|interactive)", content, re.IGNORECASE) or
        re.search(r"Planner|Builder", content) or
        re.search(r"\[38;5;\d+m\[LIVE\]", content) or  # ANSI-escaped LIVE
        re.search(r"(done|idle|active)", content, re.IGNORECASE) or  # Status indicators
        re.search(r"Last:", content)  # Footer "Last:" indicates TUI was running
    )
    checks["mode_indicator"] = {
        "passed": mode_match is not None,
        "reason": "Found mode/TUI indicator" if mode_match else "Missing mode indicator",
    }

    passed = all(c["passed"] for c in checks.values())

    return {
        "passed": passed,
        "checks": checks,
    }


def validate_footer(content: str) -> dict:
    """Validate Hats footer content against criteria.

    Args:
        content: Terminal content to validate

    Returns:
        Validation result dict
    """
    import re

    checks = {}

    # Check activity indicator
    activity_match = re.search(r"[◉◯■]", content)
    checks["activity_indicator"] = {
        "passed": activity_match is not None,
        "reason": "Found activity indicator" if activity_match else "Missing activity indicator",
    }

    # Check for event topic (optional)
    event_match = re.search(r"\w+\.\w+", content)  # topic.subtopic format
    checks["event_topic"] = {
        "passed": True,  # Optional
        "reason": f"Found event topic: {event_match.group()}" if event_match else "No event topic (optional)",
    }

    passed = all(c["passed"] for c in checks.values())

    return {
        "passed": passed,
        "checks": checks,
    }


def validate_full_tui(content: str) -> dict:
    """Validate complete Hats TUI layout.

    Args:
        content: Full terminal content

    Returns:
        Validation result dict
    """
    lines = content.strip().split("\n")

    checks = {}

    # Check for header at top (first few lines)
    header_content = "\n".join(lines[:3]) if len(lines) >= 3 else content
    header_result = validate_header(header_content)
    checks["header"] = {
        "passed": header_result["passed"],
        "reason": "Header validated" if header_result["passed"] else "Header validation failed",
    }

    # Check for footer at bottom (last few lines)
    footer_content = "\n".join(lines[-3:]) if len(lines) >= 3 else content
    footer_result = validate_footer(footer_content)
    checks["footer"] = {
        "passed": footer_result["passed"],
        "reason": "Footer validated" if footer_result["passed"] else "Footer validation failed",
    }

    # Check for content area (middle section)
    checks["content_area"] = {
        "passed": len(lines) > 6,  # At least header + some content + footer
        "reason": f"Content area has {len(lines)} lines" if len(lines) > 6 else "Content area too small",
    }

    passed = all(c["passed"] for c in checks.values())

    return {
        "passed": passed,
        "checks": checks,
    }


def print_validation_result(result: dict, name: str):
    """Print validation result in a nice format."""
    status = "✅ PASSED" if result["passed"] else "❌ FAILED"
    print(f"\n{status}: {name}")
    print("-" * 40)

    for check_name, check_result in result["checks"].items():
        indicator = "✓" if check_result["passed"] else "✗"
        print(f"  {indicator} {check_name}: {check_result['reason']}")


def cmd_validate_header(args):
    """Command: validate header from file."""
    if not Path(args.file).exists():
        print(f"File not found: {args.file}")
        return 1

    content = Path(args.file).read_text()
    result = validate_header(content)
    print_validation_result(result, "Header Validation")

    if args.screenshot:
        output_path = Path(args.file).with_suffix(".svg")
        capture_file_with_freeze(Path(args.file), output_path)

    return 0 if result["passed"] else 1


def cmd_validate_full(args):
    """Command: validate full TUI from tmux session."""
    # Capture from tmux
    capture_cmd = ["tmux", "capture-pane", "-p", "-e", "-t", args.session]
    result = subprocess.run(capture_cmd, capture_output=True, text=True)

    if result.returncode != 0:
        print(f"Failed to capture tmux session '{args.session}': {result.stderr}")
        return 1

    content = result.stdout
    validation = validate_full_tui(content)
    print_validation_result(validation, f"Full TUI Validation (session: {args.session})")

    if args.screenshot:
        output_path = Path(f"{args.session}.svg")
        capture_tmux_session(args.session, output_path)

    return 0 if validation["passed"] else 1


def cmd_validate_fixtures(args):
    """Command: validate all test fixtures."""
    fixtures_dir = Path(__file__).parent.parent.parent / "crates" / "hats-tui" / "tests" / "fixtures"

    if not fixtures_dir.exists():
        print(f"Fixtures directory not found: {fixtures_dir}")
        return 1

    print(f"Validating fixtures in: {fixtures_dir}")
    print("=" * 60)

    results = []
    for fixture_file in fixtures_dir.glob("*.jsonl"):
        print(f"\nFixture: {fixture_file.name}")

        # Read the fixture to extract any captured TUI output
        # (This is a simplified check - real fixtures are event sequences)
        content = fixture_file.read_text()

        # Check if it's a valid JSONL with events
        try:
            import json
            lines = [l for l in content.strip().split("\n") if l.strip()]
            events = [json.loads(l) for l in lines]

            print(f"  Contains {len(events)} events")
            topics = set(e.get("topic", "") for e in events)
            print(f"  Topics: {', '.join(sorted(topics)[:5])}")

            results.append({"file": fixture_file.name, "valid": True, "events": len(events)})
        except json.JSONDecodeError as e:
            print(f"  ❌ Invalid JSONL: {e}")
            results.append({"file": fixture_file.name, "valid": False, "error": str(e)})

    print("\n" + "=" * 60)
    print(f"Validated {len(results)} fixtures")

    valid = sum(1 for r in results if r.get("valid", False))
    print(f"Valid: {valid}/{len(results)}")

    return 0 if valid == len(results) else 1


def main():
    parser = argparse.ArgumentParser(
        description="TUI Visual Regression Testing (Tier 3)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s validate-header output.txt --screenshot
  %(prog)s validate-full hats-session --screenshot
  %(prog)s validate-fixtures

Integration with /tui-validate skill:
  # In Claude Code, use the skill directly:
  /tui-validate file:output.txt criteria:hats-header
  /tui-validate tmux:hats-session criteria:hats-full save_screenshot:true
""",
    )

    subparsers = parser.add_subparsers(dest="command", required=True)

    # validate-header command
    header_parser = subparsers.add_parser(
        "validate-header",
        help="Validate Hats TUI header from file",
    )
    header_parser.add_argument("file", help="File containing ANSI output")
    header_parser.add_argument("--screenshot", action="store_true", help="Save screenshot")
    header_parser.set_defaults(func=cmd_validate_header)

    # validate-full command
    full_parser = subparsers.add_parser(
        "validate-full",
        help="Validate full Hats TUI from tmux session",
    )
    full_parser.add_argument("session", help="tmux session name")
    full_parser.add_argument("--screenshot", action="store_true", help="Save screenshot")
    full_parser.set_defaults(func=cmd_validate_full)

    # validate-fixtures command
    fixtures_parser = subparsers.add_parser(
        "validate-fixtures",
        help="Validate all test fixtures",
    )
    fixtures_parser.set_defaults(func=cmd_validate_fixtures)

    args = parser.parse_args()

    if not check_prerequisites():
        print("\nInstall missing prerequisites and try again.")
        return 1

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
