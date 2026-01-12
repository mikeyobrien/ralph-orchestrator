# ABOUTME: Ralph Orchestrator package for AI agent orchestration
# ABOUTME: Implements the Ralph Wiggum technique with multi-tool support

"""Ralph Orchestrator - Simple AI agent orchestration."""

__version__ = "1.2.3"

from .orchestrator import RalphOrchestrator
from .metrics import Metrics, CostTracker, IterationStats
from .error_formatter import ClaudeErrorFormatter, ErrorMessage
from .verbose_logger import VerboseLogger
from .output import DiffStats, DiffFormatter, RalphConsole

__all__ = [
    "RalphOrchestrator",
    "Metrics",
    "CostTracker",
    "IterationStats",
    "ClaudeErrorFormatter",
    "ErrorMessage",
    "VerboseLogger",
    "DiffStats",
    "DiffFormatter",
    "RalphConsole",
]
