"""
RALPH TUI - Real-time Terminal User Interface for orchestration monitoring.

Usage:
    ralph tui -P prompt.md  # Run with TUI attached
    ralph watch             # Connect to running orchestrator
"""

from .app import RalphTUI
from .connection import OrchestratorConnection, AttachedConnection, WebSocketConnection

__all__ = [
    "RalphTUI",
    "OrchestratorConnection",
    "AttachedConnection",
    "WebSocketConnection",
]
