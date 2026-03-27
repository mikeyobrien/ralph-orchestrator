"""Benchmark harness for rich TUI output with controlled adapter fixtures.

This test intentionally uses a deterministic mock Pi backend instead of a live model.
It launches Ralph in tmux with TUI enabled, captures ANSI output, validates the
capture with the LLM judge helper, and prints a machine-readable metric summary.
"""

import asyncio
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
import textwrap
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Optional

import pytest

from .helpers import IterationCapture, LLMJudge, TmuxSession


RICH_OUTPUT_CRITERIA = """
Analyze this ANSI-preserved Ralph TUI capture. Score each dimension 1-10.
ANSI color escapes and box-drawing characters are normal and expected.

Score each criterion on a 1-10 scale:

1. **agent_text_readability** (1-10)
   - 10: Assistant prose is beautifully formatted, well-spaced, easy to scan
   - 5: Readable but cramped or poorly formatted
   - 1: Garbled, lost between status lines, or unreadable

2. **tool_call_specificity** (1-10)
   - 10: Tool call shows full context (file path, command args, query text) with smart formatting
   - 5: Shows tool name and partial context
   - 1: Only tool name visible, no meaningful summary

3. **tool_result_informativeness** (1-10)
   - 10: Result preview shows exactly the right amount of content with smart truncation
   - 5: Shows some content but truncation is too aggressive or too loose
   - 1: Empty success marker or no useful preview at all

4. **chronological_flow** (1-10)
   - 10: Perfect ordering: text → tool call → result → text, with clear visual separation
   - 5: Correct order but boundaries between sections are unclear
   - 1: Jumbled or impossible to follow the sequence

5. **visual_polish** (1-10)
   - 10: Intentional design — colors, borders, alignment, whitespace all harmonious
   - 5: Functional but bland or slightly misaligned
   - 1: Ugly, broken borders, clashing colors, misaligned columns

6. **information_density** (1-10)
   - 10: Screen real estate used optimally — no wasted space, no overwhelming walls
   - 5: Some wasted space or slightly too dense
   - 1: Mostly empty or overwhelming wall of text

7. **error_state_handling** (1-10)
   - 10: Errors/warnings clearly distinguished with color and icons
   - 5: Errors visible but not clearly differentiated from success
   - 1: Errors silent or confusing (score 5 if no errors present to evaluate)

8. **multi_tool_clarity** (1-10)
   - 10: Multiple tool calls clearly distinguishable with individual results
   - 5: Distinguishable but requires effort to match calls to results
   - 1: Ambiguous which result belongs to which call (score 5 if only one tool call)

9. **tui_structure** (1-10)
   - 10: Header, footer, progress indicators all present and informative
   - 5: Basic structure present but missing useful metadata
   - 1: No visible structure, raw text dump

10. **progressive_disclosure** (1-10)
    - 10: Default view is clean, detail available on demand or via scrolling
    - 5: Reasonable default but no way to get more detail
    - 1: Either too sparse or dumps everything at once

Respond with ONLY valid JSON (no markdown, no prose outside JSON):
{
  "pass": true/false,
  "total_score": <sum of all scores, 0-100>,
  "checks": {
    "agent_text_readability": {"score": <1-10>, "reason": "..."},
    "tool_call_specificity": {"score": <1-10>, "reason": "..."},
    "tool_result_informativeness": {"score": <1-10>, "reason": "..."},
    "chronological_flow": {"score": <1-10>, "reason": "..."},
    "visual_polish": {"score": <1-10>, "reason": "..."},
    "information_density": {"score": <1-10>, "reason": "..."},
    "error_state_handling": {"score": <1-10>, "reason": "..."},
    "multi_tool_clarity": {"score": <1-10>, "reason": "..."},
    "tui_structure": {"score": <1-10>, "reason": "..."},
    "progressive_disclosure": {"score": <1-10>, "reason": "..."}
  },
  "overall_reason": "..."
}
"""

ANSI_RE = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]")


@dataclass
class CaptureArtifact:
    mode: str
    content: str
    artifact_path: Path


def strip_ansi(text: str) -> str:
    return ANSI_RE.sub("", text)


def normalize_line(line: str) -> str:
    return " ".join(strip_ansi(line).split())


def extract_matching_lines(content: str, markers: tuple[str, ...]) -> list[str]:
    return [
        normalize_line(line)
        for line in strip_ansi(content).splitlines()
        if any(marker in line for marker in markers)
    ]


def parity_pass(default_capture: str, legacy_capture: str) -> tuple[bool, dict[str, str]]:
    markers = ("[Read]", "[Bash]", "[Write]", "✓")
    default_sequence = extract_matching_lines(default_capture, markers)
    legacy_sequence = extract_matching_lines(legacy_capture, markers)
    default_errors = extract_matching_lines(default_capture, ("Error:", "EXECUTION_ERROR"))
    legacy_errors = extract_matching_lines(legacy_capture, ("Error:", "EXECUTION_ERROR"))

    details = {
        "default_sequence": "\n".join(default_sequence),
        "legacy_sequence": "\n".join(legacy_sequence),
        "default_errors": "\n".join(default_errors),
        "legacy_errors": "\n".join(legacy_errors),
    }

    return bool(default_sequence) and default_sequence == legacy_sequence, details


def find_repo_venv_python() -> Optional[Path]:
    candidate = Path(__file__).resolve().parents[2] / ".venv" / "bin" / "python"
    return candidate if candidate.exists() else None


def rerun_benchmark_in_repo_venv() -> bool:
    if os.environ.get("RALPH_RICH_TUI_INNER") == "1":
        return False

    venv_python = find_repo_venv_python()
    if venv_python is None:
        return False

    env = os.environ.copy()
    env["RALPH_RICH_TUI_INNER"] = "1"
    command = [
        str(venv_python),
        "-m",
        "pytest",
        "-q",
        "-s",
        f"{Path(__file__)}::test_tui_rich_output_benchmark",
    ]
    result = subprocess.run(
        command,
        cwd=Path(__file__).resolve().parents[2],
        env=env,
        capture_output=True,
        text=True,
    )

    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr)

    assert result.returncode == 0, (
        "rich-TUI benchmark fallback in .venv failed with exit code "
        f"{result.returncode}"
    )
    assert "PRIMARY_METRIC=" in result.stdout, (
        "rich-TUI benchmark fallback in .venv did not produce PRIMARY_METRIC output"
    )
    return True


def build_mock_pi_fixture(tmp_root: Path) -> tuple[Path, Path, Path, Path]:
    workspace = tmp_root / "workspace"
    workspace.mkdir(parents=True, exist_ok=True)
    (workspace / "README.md").write_text(
        "# Demo README\n\nSecond line of context.\n",
        encoding="utf-8",
    )
    (workspace / "notes.txt").write_text(
        "- deterministic fixture\n- rich tui benchmark\n",
        encoding="utf-8",
    )

    script_path = tmp_root / "mock-pi.sh"
    script_path.write_text(
        textwrap.dedent(
            """\
            #!/usr/bin/env bash
            set -euo pipefail

            sleep 0.2
            printf '%s\n' \
              '{"type":"session","version":3,"id":"test","timestamp":"2026-01-01T00:00:00Z","cwd":"/tmp"}' \
              '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Inspecting README, checking the workspace, and preparing a short summary.\\n"}}' \
              '{"type":"tool_execution_start","toolCallId":"toolu_read","toolName":"Read","args":{"path":"README.md"}}' \
              '{"type":"tool_execution_end","toolCallId":"toolu_read","toolName":"Read","result":{"content":[{"type":"text","text":"# Demo README\\nSecond line of context."}]},"isError":false}' \
              '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"README captured. Verifying the workspace file list before writing the summary.\\n"}}' \
              '{"type":"tool_execution_start","toolCallId":"toolu_bash","toolName":"Bash","args":{"command":"ls -1 README.md notes.txt"}}' \
              '{"type":"tool_execution_end","toolCallId":"toolu_bash","toolName":"Bash","result":{"content":[{"type":"text","text":"README.md\\nnotes.txt"}]},"isError":false}' \
              '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Workspace looks right. Writing summary.md with the key points now.\\n"}}' \
              '{"type":"tool_execution_start","toolCallId":"toolu_write_ok","toolName":"Write","args":{"path":"summary.md"}}' \
              '{"type":"tool_execution_end","toolCallId":"toolu_write_ok","toolName":"Write","result":{"content":[{"type":"text","text":"Created summary.md with 2 bullet points."}]},"isError":false}' \
              '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Summary saved. Intentionally attempting one failing backup write to exercise error handling.\\n"}}' \
              '{"type":"tool_execution_start","toolCallId":"toolu_write_err","toolName":"Write","args":{"path":"/root/summary.md"}}' \
              '{"type":"tool_execution_end","toolCallId":"toolu_write_err","toolName":"Write","result":{"content":[{"type":"text","text":"permission denied: /root/summary.md"}]},"isError":true}' \
              '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Recovered from the failed backup write and finalized the response. LOOP_COMPLETE"}}' \
              '{"type":"turn_end","message":{"role":"assistant","content":[],"provider":"anthropic","model":"mock-pi","usage":{"input":10,"output":20,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.01}},"stopReason":"stop"}}'
            """
        ),
        encoding="utf-8",
    )
    script_path.chmod(0o755)

    config_path = tmp_root / "ralph.mock.yml"
    config_path.write_text(
        textwrap.dedent(
            f"""\
            cli:
              backend: pi
              command: {script_path}

            event_loop:
              completion_promise: LOOP_COMPLETE
              max_iterations: 1
              max_runtime_seconds: 30
              idle_timeout_secs: 5

            memories:
              enabled: false

            tasks:
              enabled: false
            """
        ),
        encoding="utf-8",
    )

    evidence_dir = (
        Path(__file__).resolve().parents[2]
        / "tui-validation"
        / "rich-output"
        / f"run_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    )
    evidence_dir.mkdir(parents=True, exist_ok=True)

    return workspace, script_path, config_path, evidence_dir


async def run_tui_capture(
    *,
    ralph_binary: Path,
    workspace: Path,
    config_path: Path,
    evidence_dir: Path,
    mode: str,
    legacy_tui: bool,
) -> CaptureArtifact:
    session = TmuxSession(name=f"ralph-rich-{uuid.uuid4().hex[:8]}", width=120, height=40)
    prompt = "Summarize the README using the controlled adapter fixture."

    command_parts = [
        shlex.quote(str(ralph_binary)),
        "run",
        "-c",
        shlex.quote(str(config_path)),
        "--skip-preflight",
    ]
    if legacy_tui:
        command_parts.append("--legacy-tui")
    command_parts.extend(["-p", shlex.quote(prompt)])
    command = " ".join(command_parts)

    async with session:
        await session.send_keys(f"cd {shlex.quote(str(workspace))} && {command}")

        started = await session.wait_for_alternate_screen(timeout=10.0)
        assert started, f"{mode}: TUI never reached alternate screen"

        capture = IterationCapture(session=session)
        exited, content = await capture.wait_for_process_exit(timeout=20.0, check_interval=0.5)
        assert exited, f"{mode}: Ralph did not exit in time"
        assert content.strip(), f"{mode}: final TUI capture was empty"

    artifact_path = evidence_dir / f"{mode}.ansi.txt"
    artifact_path.write_text(content, encoding="utf-8")
    return CaptureArtifact(mode=mode, content=content, artifact_path=artifact_path)


@pytest.mark.e2e
@pytest.mark.requires_tmux
@pytest.mark.requires_claude
def test_tui_rich_output_benchmark(ralph_binary: Path):
    """Benchmark rich TUI output for a deterministic controlled-adapter scenario."""
    if not TmuxSession.is_available():
        pytest.skip("tmux not available")
    if not LLMJudge.is_available() and rerun_benchmark_in_repo_venv():
        return
    if not LLMJudge.is_available():
        pytest.skip("Claude Agent SDK not available")

    async def run_benchmark() -> None:
        with tempfile.TemporaryDirectory(prefix="ralph-rich-tui-") as tmp_dir:
            workspace, _script_path, config_path, evidence_dir = build_mock_pi_fixture(Path(tmp_dir))

            default_capture = await run_tui_capture(
                ralph_binary=ralph_binary,
                workspace=workspace,
                config_path=config_path,
                evidence_dir=evidence_dir,
                mode="default",
                legacy_tui=False,
            )
            legacy_capture = await run_tui_capture(
                ralph_binary=ralph_binary,
                workspace=workspace,
                config_path=config_path,
                evidence_dir=evidence_dir,
                mode="legacy",
                legacy_tui=True,
            )

            judge = LLMJudge()
            judge_result = await judge.validate(default_capture.content, RICH_OUTPUT_CRITERIA)
            parity_ok, parity_details = parity_pass(default_capture.content, legacy_capture.content)

            # Extract per-criterion scores (1-10 each, 10 criteria, max 100)
            criterion_scores = {}
            for name, check in judge_result.checks.items():
                criterion_scores[name] = check.score if check.score is not None else 0

            primary_metric = judge_result.total_score  # 0-100 scored metric
            judge_pass = primary_metric >= 70 and all(
                s >= 3 for s in criterion_scores.values()
            )

            summary = {
                "metric_name": "rich_tui_judge_score",
                "primary_metric": primary_metric,
                "judge_pass": judge_pass,
                "criterion_scores": criterion_scores,
                "parity": {
                    "pass": parity_ok,
                    "details": parity_details,
                },
                "judge": judge_result.to_dict(),
                "captures": {
                    "default": str(default_capture.artifact_path),
                    "legacy": str(legacy_capture.artifact_path),
                },
            }

            print(json.dumps(summary, indent=2))
            print(f"PRIMARY_METRIC={primary_metric}")

            assert default_capture.content.strip(), "default capture should not be empty"
            assert legacy_capture.content.strip(), "legacy capture should not be empty"

    asyncio.run(run_benchmark())
