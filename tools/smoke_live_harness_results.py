#!/usr/bin/env python3
"""Fail-closed result parser for the manual live harness smoke."""

from __future__ import annotations

import argparse
import json
import os
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class Backend:
    name: str
    kind: str
    command: str
    handoff: str

    @property
    def tool_sentinel(self) -> str:
        return f"HARNESS_SMOKE:{self.name}"

    @property
    def response_sentinel(self) -> str:
        return f"HARNESS_OK:{self.name}:{self.tool_sentinel}"


BACKENDS = (
    Backend("claude", "claude-sdk", "claude", "smoke.claude.done"),
    Backend("codex", "command", "codex", "smoke.codex.done"),
    Backend("opencode", "command", "opencode", "smoke.opencode.done"),
    Backend("pi", "pi", "pi", "smoke.pi.done"),
    Backend("hermes", "acp", "hermes", "smoke.hermes.done"),
    Backend("kiro", "acp", "kiro-cli", "smoke.complete"),
)


@dataclass
class Row:
    backend: Backend
    tool: bool = False
    response: bool = False
    handoff: bool = False
    errors: list[str] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        return self.tool and self.response and self.handoff and not self.errors


def field(record: dict[str, Any], name: str, default: Any = None) -> Any:
    fields = record.get("fields")
    return fields.get(name, default) if isinstance(fields, dict) else default


def load_journal(path: Path) -> tuple[list[dict[str, Any]], list[str]]:
    records: list[dict[str, Any]] = []
    errors: list[str] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        return [], [f"cannot read native journal {path}: {exc}"]
    if not lines:
        return [], [f"native journal is empty: {path}"]
    for number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            errors.append(f"malformed native journal line {number}: {exc.msg}")
            continue
        if not isinstance(record, dict) or not isinstance(record.get("run"), str) or not isinstance(record.get("topic"), str):
            errors.append(f"malformed native journal line {number}: expected object with string run/topic")
            continue
        records.append(record)
    return records, errors


def exact_evidence(path: Path) -> tuple[list[str], list[str]]:
    expected = [backend.tool_sentinel for backend in BACKENDS]
    try:
        content = path.read_text(encoding="utf-8")
    except OSError as exc:
        return [], [f"cannot read smoke evidence {path}: {exc}"]
    actual = content.splitlines()
    errors: list[str] = []
    expected_content = "".join(f"{line}\n" for line in expected)
    if content != expected_content:
        errors.append(
            "evidence must contain exactly six ordered unique newline-terminated sentinel lines; "
            f"expected={expected!r} actual={actual!r}"
        )
    return actual, errors


def iteration_number(record: dict[str, Any]) -> int | None:
    value = record.get("iteration")
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def validate(records: list[dict[str, Any]], evidence: list[str]) -> tuple[list[Row], list[str], str, str, str, str]:
    rows = [Row(backend) for backend in BACKENDS]
    errors: list[str] = []
    run_ids = {record["run"] for record in records}
    run_id = next(iter(run_ids), "unknown")
    if len(run_ids) != 1:
        errors.append(f"native journal must contain exactly one run ID; found {sorted(run_ids)!r}")

    starts = [record for record in records if record["topic"] == "backend.start"]
    finishes = [record for record in records if record["topic"] == "backend.finish"]
    emitted = [record for record in records if record["topic"] in {backend.handoff for backend in BACKENDS}]
    expected_topics = [backend.handoff for backend in BACKENDS]
    actual_topics = [record["topic"] for record in emitted]
    if actual_topics != expected_topics:
        errors.append(f"handoffs must occur exactly once in order; expected={expected_topics!r} actual={actual_topics!r}")

    for index, row in enumerate(rows, 1):
        row.tool = evidence == [backend.tool_sentinel for backend in BACKENDS]
        matching = [record for record in emitted if record["topic"] == row.backend.handoff]
        row.handoff = len(matching) == 1 and iteration_number(matching[0]) == index
        event_response = (
            row.handoff
            and matching[0].get("payload") == row.backend.response_sentinel
            and matching[0].get("source") == "agent"
        )
        finish_response = False
        if len(starts) <= index - 1:
            row.errors.append("missing backend.start")
        else:
            start = starts[index - 1]
            actual_command = os.path.basename(str(field(start, "command", "")))
            if (
                iteration_number(start) != index
                or field(start, "backend_kind") != row.backend.kind
                or actual_command != row.backend.command
            ):
                row.errors.append(
                    "backend.start mismatch "
                    f"(iteration={start.get('iteration')!r}, kind={field(start, 'backend_kind')!r}, "
                    f"command={actual_command!r})"
                )
        if len(finishes) <= index - 1:
            row.errors.append("missing backend.finish/tool response")
        else:
            finish = finishes[index - 1]
            timed_out = field(finish, "timed_out", False)
            exit_code = str(field(finish, "exit_code", "missing"))
            finish_response = str(field(finish, "output", "")).strip() == row.backend.response_sentinel
            if iteration_number(finish) != index or exit_code != "0" or timed_out in (True, "true", "1", 1):
                row.errors.append(
                    f"backend.finish failed (iteration={finish.get('iteration')!r}, exit_code={exit_code}, timed_out={timed_out})"
                )
            if row.handoff and len(starts) > index - 1:
                if not (records.index(starts[index - 1]) < records.index(matching[0]) < records.index(finish)):
                    row.errors.append("journal order must be backend.start, handoff, backend.finish")
        row.response = event_response and finish_response
        if not row.handoff:
            row.errors.append("missing or misordered handoff")
        if not row.response:
            row.errors.append("missing exact agent response sentinel")
        if not row.tool:
            row.errors.append("missing exact ordered tool sentinel evidence")

    if len(starts) != len(BACKENDS):
        errors.append(f"expected exactly six backend.start records, found {len(starts)}")
    if len(finishes) != len(BACKENDS):
        errors.append(f"expected exactly six backend.finish records, found {len(finishes)}")

    completions = [record for record in records if record["topic"] == "smoke.complete"]
    if len(completions) != 1:
        errors.append(
            "native journal must contain exactly one literal emitted topic smoke.complete; "
            f"found {len(completions)} (completion_event stop reason is not evidence)"
        )

    start_records = [record for record in records if record["topic"] == "loop.start"]
    stop_records = [record for record in records if record["topic"] == "loop.stop"]
    stop_detail = "missing loop.stop"
    if len(start_records) != 1:
        errors.append(f"native journal must contain exactly one loop.start; found {len(start_records)}")
    if len(stop_records) != 1:
        errors.append(f"native journal must contain exactly one loop.stop; found {len(stop_records)}")
    if stop_records:
        stop = stop_records[-1]
        stop_detail = ", ".join(f"{key}={value}" for key, value in sorted((stop.get("fields") or {}).items())) or "loop.stop"
        if field(stop, "reason") != "completion_event":
            errors.append(f"loop.stop reason must be completion_event; found {field(stop, 'reason')!r}")

    last_output = "unavailable"
    for finish in reversed(finishes):
        output = field(finish, "output")
        if output:
            last_output = str(output).strip().replace("\n", " | ")[-500:]
            break
    autoloop_code = "unknown"
    if finishes:
        autoloop_code = str(field(finishes[-1], "exit_code", "unknown"))
    return rows, errors, run_id, stop_detail, last_output, autoloop_code


def print_table(rows: list[Row]) -> None:
    print("BACKEND   KIND         TOOL_SENTINEL   RESPONSE_SENTINEL   HANDOFF   RESULT")
    for row in rows:
        print(
            f"{row.backend.name:<9} {row.backend.kind:<12} "
            f"{'yes' if row.tool else 'no':<15} {'yes' if row.response else 'no':<19} "
            f"{'yes' if row.handoff else 'no':<9} {'PASS' if row.passed else 'FAIL'}"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--journal", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--rerun", required=True)
    parser.add_argument("--ralph-status", type=int, required=True)
    parser.add_argument("--timed-out", action="store_true")
    args = parser.parse_args()

    records, journal_errors = load_journal(args.journal)
    evidence, evidence_errors = exact_evidence(args.evidence)
    rows, validation_errors, run_id, stop_detail, last_output, autoloop_code = validate(records, evidence)
    if last_output == "unavailable":
        try:
            last_output = args.output.read_text(encoding="utf-8", errors="replace").strip().replace("\n", " | ")[-500:] or "unavailable"
        except OSError:
            pass
    errors = journal_errors + evidence_errors + validation_errors
    if args.ralph_status != 0:
        errors.append(f"Ralph exited with status {args.ralph_status}")
    if args.timed_out:
        errors.append("outer live-smoke watchdog timed out")
    for row in rows:
        errors.extend(f"{row.backend.name} ({row.backend.kind}): {error}" for error in row.errors)

    print_table(rows)
    if errors:
        failing = next((row for row in rows if not row.passed), rows[0])
        print("", file=sys.stderr)
        print(f"ERROR: live harness smoke failed at {failing.backend.name} ({failing.backend.kind})", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        print(f"Ralph/autoloop exit: {args.ralph_status}/{autoloop_code}", file=sys.stderr)
        print(f"Run ID: {run_id}", file=sys.stderr)
        print(f"Retained workspace: {args.workspace}", file=sys.stderr)
        print(f"Native journal: {args.journal}", file=sys.stderr)
        print(f"Journal stop detail: {stop_detail}", file=sys.stderr)
        print(f"Last relevant backend output: {last_output}", file=sys.stderr)
        print(f"Ralph output: {args.output}", file=sys.stderr)
        print(f"Rerun: {args.rerun}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
