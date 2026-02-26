#!/usr/bin/env python3
"""Validate generated llms.txt shape for docs CI.

Usage:
    python scripts/validate_llms_txt.py [path/to/llms.txt]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REQUIRED_SECTIONS = [
    "Getting Started",
    "Concepts",
    "User Guide",
    "Advanced",
    "API Reference",
    "Examples",
    "Contributing",
    "Reference",
]

LINK_PATTERN = re.compile(r"^-\s+\[[^\]]+\]\([^)]+\)")
H2_PATTERN = re.compile(r"^##\s+(.+?)\s*$")


def fail(message: str) -> int:
    print(f"llms.txt validation failed: {message}", file=sys.stderr)
    return 1


def first_non_empty_line(lines: list[str]) -> str | None:
    for line in lines:
        if line.strip():
            return line.strip()
    return None


def main() -> int:
    target = Path(sys.argv[1] if len(sys.argv) > 1 else "site/llms.txt")

    if not target.exists():
        return fail(f"file not found: {target}")

    contents = target.read_text(encoding="utf-8")
    lines = [line.rstrip() for line in contents.splitlines()]

    if not lines:
        return fail("file is empty")

    first_line = first_non_empty_line(lines)
    if first_line is None or not first_line.startswith("# "):
        return fail("missing H1 title at top of file")

    if not any(line.strip().startswith("> ") for line in lines):
        return fail("missing summary blockquote")

    section_indices: dict[str, int] = {}
    ordered_sections: list[str] = []
    for index, line in enumerate(lines):
        match = H2_PATTERN.match(line.strip())
        if match:
            name = match.group(1)
            section_indices[name] = index
            ordered_sections.append(name)

    if not ordered_sections:
        return fail("no H2 sections found")

    missing_sections = [name for name in REQUIRED_SECTIONS if name not in section_indices]
    if missing_sections:
        return fail(f"missing required sections: {', '.join(missing_sections)}")

    ordered_index_lookup = {name: i for i, name in enumerate(ordered_sections)}

    for section in REQUIRED_SECTIONS:
        start = section_indices[section] + 1
        order_index = ordered_index_lookup[section]
        if order_index + 1 < len(ordered_sections):
            next_section = ordered_sections[order_index + 1]
            end = section_indices[next_section]
        else:
            end = len(lines)

        body = [line.strip() for line in lines[start:end] if line.strip()]
        if not any(LINK_PATTERN.match(line) for line in body):
            return fail(f"section '{section}' has no markdown link list entries")

    print(f"llms.txt validation passed: {target}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
