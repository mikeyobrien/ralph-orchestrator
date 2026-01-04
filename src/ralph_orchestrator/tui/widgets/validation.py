"""Validation prompt modal for gate approval."""

from textual.screen import ModalScreen
from textual.widgets import Static, Button, ListView, ListItem
from textual.containers import Vertical, Horizontal
from textual import on
from rich.text import Text
from typing import List


class ValidationPrompt(ModalScreen[bool | None]):
    """Modal screen for validation gate approval.

    Shows:
    - Gate name and description
    - Evidence list (screenshots, logs, etc.)
    - Approve/Reject/Skip buttons

    Returns:
    - True if approved
    - False if rejected
    - None if skipped
    """

    DEFAULT_CSS = """
    ValidationPrompt {
        align: center middle;
    }

    ValidationPrompt > Vertical {
        width: 70;
        height: auto;
        max-height: 80%;
        background: $surface;
        border: thick $primary;
        padding: 2;
    }

    ValidationPrompt .title {
        text-style: bold;
        color: $primary;
        text-align: center;
        padding-bottom: 1;
        width: 100%;
    }

    ValidationPrompt .description {
        color: $text;
        padding: 1 0;
    }

    ValidationPrompt .evidence-section {
        height: auto;
        max-height: 15;
        background: $surface-darken-1;
        border: solid $primary-darken-3;
        padding: 1;
        margin: 1 0;
    }

    ValidationPrompt .evidence-title {
        text-style: bold;
        color: $text-muted;
        padding-bottom: 1;
    }

    ValidationPrompt ListView {
        height: auto;
        max-height: 10;
        background: transparent;
    }

    ValidationPrompt .button-row {
        height: 3;
        padding-top: 1;
        align: center middle;
    }

    ValidationPrompt Button {
        margin: 0 1;
        min-width: 12;
    }

    ValidationPrompt Button.approve {
        background: $success;
    }

    ValidationPrompt Button.reject {
        background: $error;
    }

    ValidationPrompt Button.skip {
        background: $warning-darken-1;
    }

    ValidationPrompt .shortcut-hint {
        color: $text-muted;
        text-align: center;
        padding-top: 1;
    }
    """

    BINDINGS = [
        ("y", "approve", "Approve"),
        ("n", "reject", "Reject"),
        ("s", "skip", "Skip"),
        ("escape", "skip", "Skip"),
    ]

    def __init__(
        self,
        gate_name: str,
        description: str = "",
        evidence: List[str] = None,
        **kwargs
    ):
        super().__init__(**kwargs)
        self.gate_name = gate_name
        self.description = description
        self.evidence = evidence or []

    def compose(self):
        """Compose the validation prompt layout."""
        with Vertical():
            # Title
            yield Static(f"🔒 Validation Gate: {self.gate_name}", classes="title")

            # Description
            if self.description:
                yield Static(self.description, classes="description")

            # Evidence section
            with Vertical(classes="evidence-section"):
                yield Static("📎 Evidence Captured:", classes="evidence-title")

                if self.evidence:
                    yield ListView(id="evidence-list")
                else:
                    yield Static("No evidence captured", classes="no-evidence")

            # Buttons
            with Horizontal(classes="button-row"):
                yield Button("Approve [y]", id="approve", classes="approve", variant="success")
                yield Button("Reject [n]", id="reject", classes="reject", variant="error")
                yield Button("Skip [s]", id="skip", classes="skip", variant="warning")

            # Shortcut hint
            yield Static(
                "Press [y] to approve, [n] to reject, [s] or [Esc] to skip",
                classes="shortcut-hint"
            )

    def on_mount(self) -> None:
        """Populate evidence list after mount."""
        if self.evidence:
            evidence_list = self.query_one("#evidence-list", ListView)
            for item in self.evidence:
                # Determine icon based on file type
                if item.endswith((".png", ".jpg", ".jpeg")):
                    icon = "🖼"
                elif item.endswith(".json"):
                    icon = "📋"
                elif item.endswith((".mp3", ".wav")):
                    icon = "🔊"
                elif item.endswith(".txt"):
                    icon = "📄"
                else:
                    icon = "📁"

                evidence_list.append(
                    ListItem(Static(f"{icon} {item}"))
                )

    @on(Button.Pressed, "#approve")
    def handle_approve(self) -> None:
        """Handle approve button press."""
        self.dismiss(True)

    @on(Button.Pressed, "#reject")
    def handle_reject(self) -> None:
        """Handle reject button press."""
        self.dismiss(False)

    @on(Button.Pressed, "#skip")
    def handle_skip(self) -> None:
        """Handle skip button press."""
        self.dismiss(None)

    def action_approve(self) -> None:
        """Approve via keyboard shortcut."""
        self.dismiss(True)

    def action_reject(self) -> None:
        """Reject via keyboard shortcut."""
        self.dismiss(False)

    def action_skip(self) -> None:
        """Skip via keyboard shortcut."""
        self.dismiss(None)
