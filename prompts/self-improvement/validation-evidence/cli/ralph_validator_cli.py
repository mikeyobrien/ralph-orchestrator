#!/usr/bin/env python3
"""
Ralph Validation CLI Tool

A command-line tool demonstrating Ralph Orchestrator's validation capabilities.
This tool performs file analysis, data transformation, and formatted output.

Usage:
    ralph-validator --help
    ralph-validator info
    ralph-validator analyze <file>
    ralph-validator transform <input> --format [json|yaml|text]
"""

import argparse
import json
import os
import sys
from datetime import datetime
from pathlib import Path


def get_version():
    """Return version string."""
    return "1.0.0"


def cmd_info(args):
    """Display tool information."""
    info = {
        "name": "Ralph Validation CLI",
        "version": get_version(),
        "description": "Ralph Validation Test - CLI Tool",
        "author": "Ralph Orchestrator",
        "timestamp": datetime.now().isoformat(),
        "python_version": sys.version.split()[0],
        "platform": sys.platform,
    }

    print("=" * 50)
    print("       RALPH VALIDATION CLI TOOL")
    print("=" * 50)
    print()
    for key, value in info.items():
        print(f"  {key.replace('_', ' ').title()}: {value}")
    print()
    print("=" * 50)
    return 0


def cmd_analyze(args):
    """Analyze a file and report statistics."""
    file_path = Path(args.file)

    if not file_path.exists():
        print(f"Error: File not found: {file_path}", file=sys.stderr)
        return 1

    if not file_path.is_file():
        print(f"Error: Not a file: {file_path}", file=sys.stderr)
        return 1

    # Read and analyze file
    content = file_path.read_text()
    lines = content.splitlines()
    words = content.split()

    analysis = {
        "file": str(file_path),
        "size_bytes": file_path.stat().st_size,
        "lines": len(lines),
        "words": len(words),
        "characters": len(content),
        "non_empty_lines": len([l for l in lines if l.strip()]),
        "extension": file_path.suffix or "(none)",
    }

    print()
    print("FILE ANALYSIS REPORT")
    print("-" * 40)
    for key, value in analysis.items():
        label = key.replace('_', ' ').title()
        print(f"  {label}: {value}")
    print("-" * 40)
    print()

    return 0


def cmd_transform(args):
    """Transform input data to specified format."""
    data = {
        "input": args.input,
        "format": args.format,
        "timestamp": datetime.now().isoformat(),
        "tool": "Ralph Validation CLI",
        "validation_marker": "Ralph Validation Test",
    }

    if args.format == "json":
        print(json.dumps(data, indent=2))
    elif args.format == "yaml":
        # Simple YAML output without external dependency
        print("---")
        for key, value in data.items():
            print(f"{key}: {value}")
    else:  # text
        print("TRANSFORMED DATA")
        print("=" * 40)
        for key, value in data.items():
            print(f"{key}: {value}")
        print("=" * 40)

    return 0


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        prog="ralph-validator",
        description="Ralph Validation CLI - A tool for testing Ralph Orchestrator validation",
        epilog="Example: ralph-validator info"
    )
    parser.add_argument(
        "--version", "-v",
        action="version",
        version=f"ralph-validator {get_version()}"
    )

    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # info command
    info_parser = subparsers.add_parser("info", help="Display tool information")
    info_parser.set_defaults(func=cmd_info)

    # analyze command
    analyze_parser = subparsers.add_parser("analyze", help="Analyze a file")
    analyze_parser.add_argument("file", help="File to analyze")
    analyze_parser.set_defaults(func=cmd_analyze)

    # transform command
    transform_parser = subparsers.add_parser("transform", help="Transform data")
    transform_parser.add_argument("input", help="Input data to transform")
    transform_parser.add_argument(
        "--format", "-f",
        choices=["json", "yaml", "text"],
        default="text",
        help="Output format (default: text)"
    )
    transform_parser.set_defaults(func=cmd_transform)

    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        return 0

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
