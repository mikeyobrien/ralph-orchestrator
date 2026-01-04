"""Metrics panel widget with sparkline visualizations."""

from textual.widgets import Static, Sparkline
from textual.containers import Horizontal, Vertical
from textual.reactive import reactive
from rich.text import Text
from rich.table import Table
from typing import List
from collections import deque


class MetricWidget(Static):
    """Single metric with value and sparkline history."""

    DEFAULT_CSS = """
    MetricWidget {
        width: 1fr;
        height: 3;
        padding: 0 1;
    }

    MetricWidget .metric-label {
        color: $text-muted;
    }

    MetricWidget .metric-value {
        text-style: bold;
        color: $primary-lighten-2;
    }

    MetricWidget Sparkline {
        height: 1;
        color: $primary;
    }
    """

    value: reactive[float] = reactive(0.0)

    def __init__(
        self,
        label: str,
        icon: str = "",
        unit: str = "",
        max_value: float = 100.0,
        format_spec: str = ".1f",
        **kwargs
    ):
        super().__init__(**kwargs)
        self.label = label
        self.icon = icon
        self.unit = unit
        self.max_value = max_value
        self.format_spec = format_spec
        self._history: deque[float] = deque(maxlen=60)

    def compose(self):
        """Compose metric widget."""
        yield Static(id="label")
        yield Static(id="value")
        yield Sparkline([], id="sparkline")

    def on_mount(self) -> None:
        """Initialize display on mount."""
        self._update_display()

    def update_value(self, value: float) -> None:
        """Update metric value and history."""
        self.value = value
        self._history.append(value)
        self._update_display()

    def _update_display(self) -> None:
        """Refresh the metric display."""
        # Label
        label_text = Text()
        label_text.append(f"{self.icon} ", style="bold")
        label_text.append(self.label, style="dim")
        self.query_one("#label", Static).update(label_text)

        # Value
        formatted = f"{self.value:{self.format_spec}}{self.unit}"
        value_text = Text(formatted, style="bold cyan")
        self.query_one("#value", Static).update(value_text)

        # Sparkline
        sparkline = self.query_one("#sparkline", Sparkline)
        if self._history:
            sparkline.data = list(self._history)

    def watch_value(self, new_value: float) -> None:
        """React to value changes."""
        self._update_display()


class MetricsPanel(Static):
    """Real-time metrics with sparkline visualizations.

    Displays:
    - CPU usage (%)
    - Memory usage (%)
    - Token count (cumulative)
    - Cost ($)
    """

    DEFAULT_CSS = """
    MetricsPanel {
        height: 5;
        background: $surface-darken-1;
        border-top: solid $primary-darken-2;
        padding: 0 1;
    }

    MetricsPanel.hidden {
        display: none;
    }

    MetricsPanel Horizontal {
        height: 100%;
    }
    """

    def __init__(self, **kwargs):
        super().__init__(**kwargs)

    def compose(self):
        """Compose metrics layout."""
        with Horizontal():
            yield MetricWidget(
                "CPU",
                icon="⚙",
                unit="%",
                max_value=100.0,
                format_spec=".0f",
                id="cpu-metric"
            )
            yield MetricWidget(
                "Memory",
                icon="💾",
                unit="%",
                max_value=100.0,
                format_spec=".0f",
                id="memory-metric"
            )
            yield MetricWidget(
                "Tokens",
                icon="📊",
                unit="K",
                max_value=1000.0,
                format_spec=".1f",
                id="tokens-metric"
            )
            yield MetricWidget(
                "Cost",
                icon="$",
                unit="",
                max_value=100.0,
                format_spec=".2f",
                id="cost-metric"
            )

    def update_metrics(
        self,
        cpu: float = 0,
        memory: float = 0,
        tokens: int = 0,
        cost: float = 0.0,
    ) -> None:
        """Update all metrics at once."""
        self.query_one("#cpu-metric", MetricWidget).update_value(cpu)
        self.query_one("#memory-metric", MetricWidget).update_value(memory)
        self.query_one("#tokens-metric", MetricWidget).update_value(tokens / 1000)  # Convert to K

        # Format cost with $ prefix
        cost_widget = self.query_one("#cost-metric", MetricWidget)
        cost_widget.icon = "$"
        cost_widget.update_value(cost)

    def get_current_metrics(self) -> dict:
        """Get current metric values."""
        return {
            "cpu": self.query_one("#cpu-metric", MetricWidget).value,
            "memory": self.query_one("#memory-metric", MetricWidget).value,
            "tokens": self.query_one("#tokens-metric", MetricWidget).value * 1000,
            "cost": self.query_one("#cost-metric", MetricWidget).value,
        }
