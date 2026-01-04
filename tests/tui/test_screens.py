"""Tests for TUI screens."""

import pytest
from ralph_orchestrator.tui.screens.history import HistoryScreen, IterationRecord
from ralph_orchestrator.tui.screens.help import HelpScreen


class TestIterationRecord:
    """Tests for IterationRecord dataclass."""

    def test_create_record(self):
        """Create iteration record with required fields."""
        record = IterationRecord(
            number=1,
            status="success",
            task="Test task",
            duration=10.5,
            tokens=5000,
            cost=0.15,
            tool_calls=3
        )
        assert record.number == 1
        assert record.status == "success"
        assert record.duration == 10.5
        assert record.tokens == 5000
        assert record.cost == 0.15
        assert record.tool_calls == 3


class TestHistoryScreen:
    """Tests for HistoryScreen."""

    def test_create_with_records(self):
        """Create screen with iteration records."""
        records = [
            IterationRecord(
                number=1,
                status="success",
                task="Task 1",
                duration=5.0,
                tokens=1000,
                cost=0.05,
                tool_calls=2
            ),
            IterationRecord(
                number=2,
                status="error",
                task="Task 2",
                duration=3.0,
                tokens=500,
                cost=0.02,
                tool_calls=1
            ),
        ]
        screen = HistoryScreen(records=records)
        assert len(screen.records) == 2

    def test_create_without_records(self):
        """Create screen with no records."""
        screen = HistoryScreen()
        assert screen.records == []

    def test_records_can_be_set(self):
        """Records attribute can be set directly."""
        screen = HistoryScreen()
        new_records = [
            IterationRecord(
                number=1,
                status="success",
                task="Task 1",
                duration=5.0,
                tokens=1000,
                cost=0.05,
                tool_calls=2
            ),
        ]
        # Directly set records (update_records requires mounted widgets)
        screen.records = new_records
        assert len(screen.records) == 1


class TestHelpScreen:
    """Tests for HelpScreen."""

    def test_bindings_defined(self):
        """HelpScreen has expected bindings."""
        screen = HelpScreen()
        binding_keys = [b.key for b in screen.BINDINGS]
        assert "escape" in binding_keys
        assert "q" in binding_keys

    def test_make_shortcuts_table(self):
        """_make_shortcuts_table creates formatted text."""
        screen = HelpScreen()
        shortcuts = [("q", "Quit"), ("?", "Help")]
        text = screen._make_shortcuts_table(shortcuts)
        assert "[q]" in str(text)
        assert "Quit" in str(text)
