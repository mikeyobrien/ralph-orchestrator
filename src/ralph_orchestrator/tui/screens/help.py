"""Help overlay screen showing keyboard shortcuts."""

from textual.screen import ModalScreen
from textual.widgets import Static
from textual.containers import Vertical
from textual.binding import Binding
from rich.text import Text


class HelpScreen(ModalScreen):
    """Modal overlay showing keyboard shortcuts and help.

    Displays all available keyboard shortcuts grouped by function.
    """

    DEFAULT_CSS = """
    HelpScreen {
        align: center middle;
    }

    HelpScreen > Vertical {
        width: 60;
        height: auto;
        max-height: 80%;
        background: $surface;
        border: thick $primary;
        padding: 2;
    }

    HelpScreen .title {
        text-style: bold;
        color: $primary;
        text-align: center;
        padding-bottom: 1;
        width: 100%;
    }

    HelpScreen .section-title {
        text-style: bold;
        color: $text-muted;
        padding: 1 0 0 0;
    }

    HelpScreen .shortcuts-table {
        padding: 0 2;
    }

    HelpScreen .dismiss-hint {
        color: $text-muted;
        text-align: center;
        padding-top: 1;
    }
    """

    BINDINGS = [
        Binding("escape", "dismiss", "Close"),
        Binding("question_mark", "dismiss", "Close"),
        Binding("q", "dismiss", "Close"),
    ]

    def compose(self):
        """Compose help screen layout."""
        with Vertical():
            yield Static("❓ RALPH TUI Help", classes="title")

            # Navigation shortcuts
            yield Static("Navigation", classes="section-title")
            yield Static(self._make_shortcuts_table([
                ("q", "Quit TUI"),
                ("Esc", "Go back / Close modal"),
                ("h", "Show iteration history"),
                ("?", "Show this help"),
            ]), classes="shortcuts-table")

            # Control shortcuts
            yield Static("Controls", classes="section-title")
            yield Static(self._make_shortcuts_table([
                ("p", "Pause / Resume orchestration"),
                ("c", "Create checkpoint"),
                ("f", "Toggle auto-scroll (follow)"),
            ]), classes="shortcuts-table")

            # Validation shortcuts
            yield Static("Validation Gates", classes="section-title")
            yield Static(self._make_shortcuts_table([
                ("y", "Approve validation gate"),
                ("n", "Reject validation gate"),
                ("s", "Skip validation gate"),
            ]), classes="shortcuts-table")

            # Panel shortcuts
            yield Static("Panels", classes="section-title")
            yield Static(self._make_shortcuts_table([
                ("l", "Toggle log panel"),
                ("t", "Toggle task sidebar"),
                ("m", "Toggle metrics panel"),
                ("d", "Toggle details view"),
            ]), classes="shortcuts-table")

            yield Static("Press [Esc] or [?] to close", classes="dismiss-hint")

    def _make_shortcuts_table(self, shortcuts: list) -> Text:
        """Create formatted shortcuts text."""
        text = Text()
        for key, description in shortcuts:
            text.append(f"  [{key}]", style="bold cyan")
            text.append(f"  {description}\n", style="")
        return text

    def action_dismiss(self) -> None:
        """Close help screen."""
        self.app.pop_screen()
