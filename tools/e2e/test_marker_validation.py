"""Simple test file created by Hats iteration.

This test validates the marker value from the scratchpad.
"""

import pytest
from pathlib import Path


def test_scratchpad_marker_exists():
    """Verify the scratchpad contains a marker value."""
    scratchpad_path = Path(__file__).parent.parent.parent / ".agent" / "scratchpad.md"

    assert scratchpad_path.exists(), "Scratchpad file should exist"

    content = scratchpad_path.read_text()
    assert "Marker:" in content, "Scratchpad should contain a Marker line"


def test_marker_value_is_beta_updated():
    """Verify the marker value matches expected updated state."""
    scratchpad_path = Path(__file__).parent.parent.parent / ".agent" / "scratchpad.md"

    content = scratchpad_path.read_text()
    assert "MARKER_BETA_UPDATED" in content, "Marker should be MARKER_BETA_UPDATED"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
