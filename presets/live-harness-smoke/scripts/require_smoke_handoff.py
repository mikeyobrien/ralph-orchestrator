#!/usr/bin/env python3
"""Abort the paid smoke immediately when a provider turn misses its contract."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

BACKENDS = (
    ("claude", "claude-sdk", "claude", "smoke.claude.done"),
    ("codex", "command", "codex", "smoke.codex.done"),
    ("opencode", "command", "opencode", "smoke.opencode.done"),
    ("pi", "pi", "pi", "smoke.pi.done"),
    ("hermes", "acp", "hermes", "smoke.hermes.done"),
    ("kiro", "acp", "kiro-cli", "smoke.complete"),
)


def fail(message: str) -> "NoReturn":
    print(f"smoke handoff gate failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def field(record: dict[str, object], key: str, default: object = "") -> object:
    fields = record.get("fields")
    return fields.get(key, default) if isinstance(fields, dict) else default


def main() -> None:
    try:
        iteration = int(os.environ["AUTOLOOP_ITERATION"])
        run_id = os.environ["AUTOLOOP_RUN_ID"]
        project_dir = Path(os.environ["AUTOLOOP_PROJECT_DIR"])
        backend = BACKENDS[iteration - 1]
    except (KeyError, ValueError, IndexError) as error:
        fail(f"invalid hook context: {error}")

    journal = project_dir / ".autoloop/journal.jsonl"
    state_dir = project_dir / ".autoloop/runs" / run_id
    evidence = state_dir / "smoke-evidence.txt"
    if not journal.is_file():
        fail(f"missing native journal: {journal}")

    records: list[dict[str, object]] = []
    try:
        for line in journal.read_text(encoding="utf-8").splitlines():
            if line.strip():
                record = json.loads(line)
                if record.get("run") == run_id and str(record.get("iteration", "")) == str(iteration):
                    records.append(record)
    except (OSError, json.JSONDecodeError) as error:
        fail(f"unreadable native journal: {error}")

    name, kind, command, handoff = backend
    response = f"HARNESS_OK:{name}:HARNESS_SMOKE:{name}"
    starts = [record for record in records if record.get("topic") == "backend.start"]
    finishes = [record for record in records if record.get("topic") == "backend.finish"]
    handoffs = [record for record in records if record.get("topic") == handoff]

    if len(starts) != 1:
        fail(f"iteration {iteration} expected one backend.start, found {len(starts)}")
    if field(starts[0], "backend_kind") != kind or Path(str(field(starts[0], "command"))).name != command:
        fail(f"iteration {iteration} selected the wrong backend")
    if len(handoffs) != 1:
        fail(f"iteration {iteration} expected one {handoff}, found {len(handoffs)}")
    if handoffs[0].get("source") != "agent" or handoffs[0].get("payload") != response:
        fail(f"iteration {iteration} handoff payload/source mismatch")
    if len(finishes) != 1:
        fail(f"iteration {iteration} expected one backend.finish, found {len(finishes)}")
    if (
        str(field(finishes[0], "exit_code", "missing")) != "0"
        or field(finishes[0], "timed_out", False) in (True, "true", "1", 1)
        or not str(field(finishes[0], "output", "")).strip().endswith(response)
    ):
        fail(f"iteration {iteration} backend result was not the exact success response")

    expected_evidence = [f"HARNESS_SMOKE:{item[0]}" for item in BACKENDS[:iteration]]
    try:
        actual_evidence = evidence.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        fail(f"iteration {iteration} cannot read evidence: {error}")
    if actual_evidence != expected_evidence:
        fail(
            f"iteration {iteration} evidence mismatch: "
            f"expected={expected_evidence!r} actual={actual_evidence!r}"
        )

    print(
        f"smoke probe invocation/result gate passed: "
        f"iteration={iteration} backend={name} sentinel=HARNESS_SMOKE:{name}"
    )


if __name__ == "__main__":
    main()
