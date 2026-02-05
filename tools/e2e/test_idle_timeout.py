"""E2E tests for Hats idle timeout functionality.

These tests validate that:
1. Idle timeout triggers correctly after inactivity
2. TUI state is captured properly
3. LLM-as-judge validation works
4. Evidence files are preserved
"""

import asyncio
import json
from datetime import datetime
from pathlib import Path

import pytest

from .helpers import TmuxSession, FreezeCapture, LLMJudge, JudgeResult
from .helpers.llm_judge import IDLE_TIMEOUT_CRITERIA


@pytest.mark.asyncio
@pytest.mark.e2e
@pytest.mark.requires_tmux
@pytest.mark.requires_freeze
@pytest.mark.requires_claude
async def test_idle_timeout_triggers_after_inactivity(
    tmux_session: TmuxSession,
    freeze_capture: FreezeCapture,
    llm_judge: LLMJudge,
    hats_binary: Path,
    hats_config_path: Path,
    evidence_dir: Path,
):
    """Test that idle timeout triggers correctly and TUI captures properly.

    This test:
    1. Starts Hats in interactive mode with a 5-second idle timeout
    2. Sends a simple prompt
    3. Waits for the idle timeout to trigger
    4. Captures the final TUI state
    5. Validates the output with LLM-as-judge
    6. Preserves evidence files
    """
    # Build the command
    cmd = (
        f"{hats_binary} run -i "
        f"--idle-timeout 5 "
        f"-c {hats_config_path} "
        f'-p "Say hello and nothing else"'
    )

    # Start Hats in the tmux session
    await tmux_session.send_keys(cmd)

    # Wait for Claude to respond and idle timeout to trigger
    # 5s timeout + ~5s buffer for response + cleanup
    await asyncio.sleep(12)

    # Capture the final TUI state
    raw_output = await tmux_session.capture_pane()

    # Create screenshot with freeze
    capture_result = await freeze_capture.capture_buffer(
        raw_output,
        name_prefix="idle_timeout",
        formats=("svg", "png", "text"),
    )

    # Validate with LLM-as-judge
    judge_result = await llm_judge.validate(raw_output, IDLE_TIMEOUT_CRITERIA)

    # Save evidence
    _save_evidence(evidence_dir, capture_result, judge_result)

    # Assert validation passed
    assert judge_result.passed, (
        f"LLM-as-judge validation failed:\n"
        f"Reason: {judge_result.overall_reason}\n"
        f"Checks: {json.dumps({k: v.reason for k, v in judge_result.checks.items()}, indent=2)}"
    )


@pytest.mark.asyncio
@pytest.mark.e2e
@pytest.mark.requires_tmux
async def test_tmux_session_captures_output(tmux_session: TmuxSession):
    """Test that tmux session can capture command output."""
    # Send a simple command
    await tmux_session.send_keys("echo 'Hello from tmux test'")

    # Small delay for command execution
    await asyncio.sleep(0.5)

    # Capture output
    output = await tmux_session.capture_pane()

    # Verify output contains our echo
    assert "Hello from tmux test" in output


@pytest.mark.asyncio
@pytest.mark.e2e
@pytest.mark.requires_freeze
async def test_freeze_capture_produces_files(
    freeze_capture: FreezeCapture,
    evidence_dir: Path,
):
    """Test that freeze produces valid output files."""
    test_content = """
╭─────────────────────────────────────────────╮
│ [iter 1] 00:05 | 🔨 Test | ▶ auto          │
├─────────────────────────────────────────────┤
│ Hello, this is a test of TUI capture!       │
│                                             │
│ Testing freeze integration...               │
├─────────────────────────────────────────────┤
│ ◉ active | test.start                       │
╰─────────────────────────────────────────────╯
"""

    result = await freeze_capture.capture_buffer(
        test_content,
        name_prefix="test_freeze",
        formats=("svg", "png", "text"),
    )

    # Verify files were created
    assert result.text_path.exists(), "Text file not created"
    assert result.svg_path is not None and result.svg_path.exists(), "SVG not created"
    # PNG is optional - may fail without rasterization dependencies
    if result.png_path:
        print(f"PNG created: {result.png_path.exists()}")

    # Verify text content
    saved_text = result.text_path.read_text()
    assert "Hello, this is a test" in saved_text


@pytest.mark.asyncio
@pytest.mark.e2e
@pytest.mark.requires_claude
async def test_llm_judge_validates_content(llm_judge: LLMJudge):
    """Test that LLM judge can validate terminal content."""
    # Sample terminal output that should pass validation
    valid_content = """
user@machine:~/project$ hats run --tui --idle-timeout 5 -p "Say hello"
[Starting Hats orchestrator...]

Hello! I'm here to help.

[Session completed - idle timeout reached]
user@machine:~/project$
"""

    result = await llm_judge.validate(valid_content, IDLE_TIMEOUT_CRITERIA)

    # The judge should be able to parse and return a result
    assert isinstance(result, JudgeResult)
    assert result.raw_response, "Judge should return a response"

    # Log the result for debugging
    print(f"Judge result: passed={result.passed}")
    print(f"Reason: {result.overall_reason}")
    for check_name, check in result.checks.items():
        print(f"  {check_name}: {'PASS' if check.passed else 'FAIL'} - {check.reason}")


@pytest.mark.asyncio
@pytest.mark.e2e
async def test_evidence_directory_structure(evidence_dir: Path):
    """Test that evidence directory is created with proper structure."""
    assert evidence_dir.exists()
    assert evidence_dir.is_dir()

    # Verify we can write to it
    test_file = evidence_dir / "test_write.txt"
    test_file.write_text("test")
    assert test_file.exists()
    test_file.unlink()  # Clean up


def _save_evidence(
    evidence_dir: Path,
    capture_result,
    judge_result: JudgeResult,
) -> None:
    """Save all evidence files for the test run."""
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

    # Save judge result as JSON
    judge_path = evidence_dir / f"judge_result_{timestamp}.json"
    judge_path.write_text(json.dumps(judge_result.to_dict(), indent=2))

    # Log evidence locations
    print(f"\nEvidence saved to: {evidence_dir}")
    print(f"  - Text: {capture_result.text_path}")
    if capture_result.svg_path:
        print(f"  - SVG: {capture_result.svg_path}")
    if capture_result.png_path:
        print(f"  - PNG: {capture_result.png_path}")
    print(f"  - Judge: {judge_path}")
