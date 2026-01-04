"""Onboarding module for Ralph Orchestrator.

Provides intelligent project onboarding by analyzing Claude Code history,
MCP configurations, and project metadata to generate optimized RALPH configuration.
"""

from ralph_orchestrator.onboarding.agent_analyzer import AgentAnalyzer, AnalysisResult
from ralph_orchestrator.onboarding.scanner import ProjectScanner, ProjectType
from ralph_orchestrator.onboarding.settings_loader import SettingsLoader

__all__ = [
    "AgentAnalyzer",
    "AnalysisResult",
    "ProjectScanner",
    "ProjectType",
    "SettingsLoader",
]
