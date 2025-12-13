# ABOUTME: Pytest configuration and shared fixtures for the test suite
# ABOUTME: Defines markers for test categorization (integration, slow, etc.)

"""Pytest configuration and fixtures."""

import os
import pytest


def pytest_configure(config):
    """Register custom markers."""
    config.addinivalue_line(
        "markers",
        "integration: marks tests as integration tests (require external services or APIs)"
    )
    config.addinivalue_line(
        "markers",
        "slow: marks tests as slow (may take longer than usual)"
    )


def pytest_collection_modifyitems(config, items):
    """Auto-skip integration tests when required environment variables are missing."""
    skip_integration = pytest.mark.skip(reason="GOOGLE_API_KEY not set")

    for item in items:
        if "integration" in item.keywords:
            # Check if required API key is present for integration tests
            if "acp" in item.fspath.basename or "gemini" in item.fspath.basename:
                if not os.environ.get("GOOGLE_API_KEY"):
                    item.add_marker(skip_integration)


@pytest.fixture
def temp_workspace(tmp_path):
    """Create a temporary workspace directory."""
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    return workspace


@pytest.fixture
def google_api_key():
    """Get Google API key from environment or skip."""
    api_key = os.environ.get("GOOGLE_API_KEY")
    if not api_key:
        pytest.skip("GOOGLE_API_KEY not set")
    return api_key
