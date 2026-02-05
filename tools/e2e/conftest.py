"""Pytest configuration and fixtures for E2E tests."""

import asyncio
import os
import uuid
import tempfile
from pathlib import Path
from datetime import datetime
from typing import AsyncGenerator

import pytest
import pytest_asyncio

from .helpers import TmuxSession, FreezeCapture, LLMJudge, IterationCapture


# Configure pytest-asyncio
pytest_plugins = ("pytest_asyncio",)


def pytest_configure(config):
    """Configure pytest with custom markers."""
    config.addinivalue_line(
        "markers", "e2e: mark test as an end-to-end test"
    )
    config.addinivalue_line(
        "markers", "requires_tmux: mark test as requiring tmux"
    )
    config.addinivalue_line(
        "markers", "requires_freeze: mark test as requiring freeze CLI"
    )
    config.addinivalue_line(
        "markers", "requires_claude: mark test as requiring Claude Agent SDK"
    )
    config.addinivalue_line(
        "markers", "slow: mark test as slow-running (requires live Hats)"
    )


@pytest.fixture(scope="session")
def project_root() -> Path:
    """Get the project root directory."""
    return Path(__file__).parent.parent.parent


@pytest.fixture(scope="session")
def hats_binary(project_root: Path) -> Path:
    """Get the Hats binary path."""
    release_path = project_root / "target" / "release" / "hats"
    debug_path = project_root / "target" / "debug" / "hats"

    if release_path.exists():
        return release_path
    elif debug_path.exists():
        return debug_path
    else:
        pytest.skip("Hats binary not found. Run 'cargo build' first.")


@pytest.fixture(scope="session")
def evidence_base_dir(project_root: Path) -> Path:
    """Get the base evidence directory."""
    evidence_dir = project_root / "tui-validation" / "idle-timeout"
    evidence_dir.mkdir(parents=True, exist_ok=True)
    return evidence_dir


@pytest.fixture
def evidence_dir(evidence_base_dir: Path) -> Path:
    """Get a timestamped evidence directory for this test run."""
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    run_dir = evidence_base_dir / f"run_{timestamp}"
    run_dir.mkdir(parents=True, exist_ok=True)
    return run_dir


@pytest.fixture
def tmux_session_name() -> str:
    """Generate a unique tmux session name."""
    return f"hats-e2e-{uuid.uuid4().hex[:8]}"


@pytest_asyncio.fixture
async def tmux_session(tmux_session_name: str) -> AsyncGenerator[TmuxSession, None]:
    """Create and manage a tmux session for testing.

    Automatically creates the session on entry and kills it on exit.
    """
    if not TmuxSession.is_available():
        pytest.skip("tmux not available")

    session = TmuxSession(name=tmux_session_name)
    async with session:
        yield session


@pytest.fixture
def freeze_capture(evidence_dir: Path) -> FreezeCapture:
    """Create a FreezeCapture instance for the test.

    Outputs are saved to the evidence directory.
    """
    if not FreezeCapture.is_available():
        pytest.skip("freeze CLI not available")

    return FreezeCapture(output_dir=evidence_dir)


@pytest.fixture
def llm_judge() -> LLMJudge:
    """Create an LLMJudge instance for validation."""
    if not LLMJudge.is_available():
        pytest.skip("Claude Agent SDK not available")

    return LLMJudge()


@pytest.fixture
def hats_config_path(project_root: Path) -> Path:
    """Get a valid Hats config file path."""
    # Look for common config files
    candidates = [
        "hats.yml",
        "hats.yaml",
        "hats.claude.yml",
        ".hats.yml",
    ]

    for candidate in candidates:
        config_path = project_root / candidate
        if config_path.exists():
            return config_path

    # Create a minimal config for testing
    test_config = project_root / "hats.test.yml"
    test_config.write_text("""
cli:
  backend: claude
  default_mode: interactive
  idle_timeout_secs: 5

orchestrator:
  max_iterations: 1
""")
    return test_config


# ============================================================================
# Iteration Lifecycle Test Fixtures
# ============================================================================


@pytest.fixture(scope="session")
def iteration_evidence_base_dir(project_root: Path) -> Path:
    """Get the base evidence directory for iteration lifecycle tests."""
    evidence_dir = project_root / "tui-validation" / "iteration-lifecycle"
    evidence_dir.mkdir(parents=True, exist_ok=True)
    return evidence_dir


@pytest.fixture
def iteration_evidence_dir(iteration_evidence_base_dir: Path) -> Path:
    """Get a timestamped evidence directory for this test run."""
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    run_dir = iteration_evidence_base_dir / f"run_{timestamp}"
    run_dir.mkdir(parents=True, exist_ok=True)
    return run_dir


@pytest_asyncio.fixture
async def iteration_capture(tmux_session: TmuxSession) -> IterationCapture:
    """Create an IterationCapture instance for the test.

    Requires an active tmux session.
    """
    return IterationCapture(session=tmux_session, poll_interval=0.5)


@pytest.fixture
def iteration_freeze_capture(iteration_evidence_dir: Path) -> FreezeCapture:
    """Create a FreezeCapture instance for iteration tests.

    Outputs are saved to the iteration evidence directory.
    """
    if not FreezeCapture.is_available():
        pytest.skip("freeze CLI not available")

    return FreezeCapture(output_dir=iteration_evidence_dir)


def create_iteration_test_config(
    project_root: Path,
    max_iterations: int = 100,
    max_runtime_seconds: int = 300,
    idle_timeout_secs: int = 30,
) -> Path:
    """Create a Hats config file for iteration testing.

    Args:
        project_root: Project root directory
        max_iterations: Maximum iterations before termination
        max_runtime_seconds: Maximum runtime in seconds
        idle_timeout_secs: Idle timeout in seconds

    Returns:
        Path to the created config file
    """
    config_content = f"""# Hats E2E test config
cli:
  backend: claude
  prompt_mode: arg

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: {max_iterations}
  max_runtime_seconds: {max_runtime_seconds}
  idle_timeout_secs: {idle_timeout_secs}
"""
    config_path = project_root / "hats.iteration-test.yml"
    config_path.write_text(config_content)
    return config_path


@pytest.fixture
def iteration_config_factory(project_root: Path):
    """Factory fixture for creating iteration test configs.

    Usage:
        config_path = iteration_config_factory(max_iterations=3)
    """
    created_configs = []

    def _factory(
        max_iterations: int = 100,
        max_runtime_seconds: int = 300,
        idle_timeout_secs: int = 30,
    ) -> Path:
        config_path = create_iteration_test_config(
            project_root=project_root,
            max_iterations=max_iterations,
            max_runtime_seconds=max_runtime_seconds,
            idle_timeout_secs=idle_timeout_secs,
        )
        created_configs.append(config_path)
        return config_path

    yield _factory

    # Cleanup created configs
    for config_path in created_configs:
        if config_path.exists():
            config_path.unlink()
