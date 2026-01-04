"""Tests for AgentAnalyzer - uses Claude with user's MCP servers for intelligent analysis.

Following TDD: Writing tests FIRST, then implementing to make them pass.

AgentAnalyzer uses Claude with the user's inherited MCP servers to analyze a project
and extract patterns, workflows, and configuration recommendations.
"""

import json
import pytest
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional
from unittest.mock import AsyncMock, MagicMock, patch

# Import will fail until we implement the module - that's expected for TDD
from ralph_orchestrator.onboarding.agent_analyzer import (
    AgentAnalyzer,
    AnalysisResult,
)
from ralph_orchestrator.onboarding.settings_loader import SettingsLoader


class TestAgentAnalyzerInit:
    """Tests for AgentAnalyzer initialization."""

    def test_init_with_project_path_and_settings(self, tmp_path: Path):
        """AgentAnalyzer initializes with project path and settings loader."""
        settings = SettingsLoader(tmp_path)
        analyzer = AgentAnalyzer(tmp_path, settings)

        assert analyzer.project_path == tmp_path
        assert analyzer.settings is settings

    def test_init_creates_settings_loader_if_not_provided(self, tmp_path: Path):
        """AgentAnalyzer creates SettingsLoader if not provided."""
        analyzer = AgentAnalyzer(tmp_path)

        assert analyzer.project_path == tmp_path
        assert isinstance(analyzer.settings, SettingsLoader)

    def test_init_with_inherit_user_settings_flag(self, tmp_path: Path):
        """AgentAnalyzer respects inherit_user_settings flag."""
        analyzer = AgentAnalyzer(tmp_path, inherit_user_settings=True)
        assert analyzer.inherit_user_settings is True

        analyzer = AgentAnalyzer(tmp_path, inherit_user_settings=False)
        assert analyzer.inherit_user_settings is False


class TestAnalysisResult:
    """Tests for AnalysisResult data class."""

    def test_analysis_result_has_expected_fields(self):
        """AnalysisResult contains project analysis data."""
        result = AnalysisResult(
            project_type="python",
            frameworks=["fastapi", "pytest"],
            common_tools={"Read": 0.95, "Write": 0.88, "Bash": 0.75},
            workflows=[["read", "edit", "test", "commit"]],
            recommended_config={"max_iterations": 50},
            raw_response="...",
        )

        assert result.project_type == "python"
        assert "fastapi" in result.frameworks
        assert result.common_tools["Read"] == 0.95
        assert len(result.workflows) == 1
        assert result.recommended_config["max_iterations"] == 50

    def test_analysis_result_defaults(self):
        """AnalysisResult has sensible defaults."""
        result = AnalysisResult()

        assert result.project_type == "unknown"
        assert result.frameworks == []
        assert result.common_tools == {}
        assert result.workflows == []
        assert result.recommended_config == {}
        assert result.raw_response == ""


class TestBuildAnalysisPrompt:
    """Tests for building the analysis prompt."""

    def test_builds_analysis_prompt_with_project_path(self, tmp_path: Path):
        """Analysis prompt includes project path."""
        analyzer = AgentAnalyzer(tmp_path)
        prompt = analyzer._build_analysis_prompt()

        assert str(tmp_path) in prompt
        assert "analyze" in prompt.lower()

    def test_prompt_includes_memory_instructions_when_available(self, tmp_path: Path):
        """Analysis prompt mentions memory plugin when available."""
        settings = MagicMock(spec=SettingsLoader)
        settings.has_memory_plugin.return_value = True

        analyzer = AgentAnalyzer(tmp_path, settings)
        prompt = analyzer._build_analysis_prompt()

        assert "memory" in prompt.lower()

    def test_prompt_skips_memory_when_not_available(self, tmp_path: Path):
        """Analysis prompt doesn't mention memory when not available."""
        settings = MagicMock(spec=SettingsLoader)
        settings.has_memory_plugin.return_value = False

        analyzer = AgentAnalyzer(tmp_path, settings)
        prompt = analyzer._build_analysis_prompt()

        # Should NOT contain episodic memory instructions
        assert "episodic memory" not in prompt.lower()

    def test_prompt_requests_structured_output(self, tmp_path: Path):
        """Analysis prompt requests JSON structured output."""
        analyzer = AgentAnalyzer(tmp_path)
        prompt = analyzer._build_analysis_prompt()

        # Prompt should request structured JSON response
        assert "json" in prompt.lower() or "structured" in prompt.lower()


class TestParseAnalysisResult:
    """Tests for parsing Claude's analysis response."""

    def test_parses_valid_json_response(self, tmp_path: Path):
        """Parses valid JSON response from Claude."""
        analyzer = AgentAnalyzer(tmp_path)

        response = json.dumps({
            "project_type": "python",
            "frameworks": ["fastapi", "pytest"],
            "common_tools": {"Read": 0.95, "Write": 0.88},
            "workflows": [["test", "fix", "commit"]],
            "recommended_config": {"max_iterations": 75}
        })

        result = analyzer._parse_analysis_result(response)

        assert result.project_type == "python"
        assert "fastapi" in result.frameworks
        assert result.common_tools["Read"] == 0.95
        assert result.raw_response == response

    def test_extracts_json_from_markdown_code_block(self, tmp_path: Path):
        """Extracts JSON from markdown code block in response."""
        analyzer = AgentAnalyzer(tmp_path)

        response = """Here's my analysis:

```json
{
    "project_type": "nodejs",
    "frameworks": ["react", "next"],
    "common_tools": {},
    "workflows": [],
    "recommended_config": {}
}
```

That's the analysis."""

        result = analyzer._parse_analysis_result(response)

        assert result.project_type == "nodejs"
        assert "react" in result.frameworks

    def test_handles_invalid_json_gracefully(self, tmp_path: Path):
        """Returns empty result when JSON parsing fails."""
        analyzer = AgentAnalyzer(tmp_path)

        response = "This is not valid JSON at all"

        result = analyzer._parse_analysis_result(response)

        assert result.project_type == "unknown"
        assert result.frameworks == []
        assert result.raw_response == response

    def test_handles_partial_json_response(self, tmp_path: Path):
        """Handles response with only some fields."""
        analyzer = AgentAnalyzer(tmp_path)

        response = json.dumps({
            "project_type": "rust",
            "frameworks": ["actix-web"]
            # Missing other fields
        })

        result = analyzer._parse_analysis_result(response)

        assert result.project_type == "rust"
        assert result.frameworks == ["actix-web"]
        assert result.common_tools == {}  # Defaults for missing fields


class TestAnalyze:
    """Tests for the main analyze method."""

    @pytest.mark.asyncio
    async def test_analyze_calls_claude_with_inherited_settings(self, tmp_path: Path):
        """analyze() runs Claude with user's inherited MCP servers."""
        settings = MagicMock(spec=SettingsLoader)
        settings.has_memory_plugin.return_value = False

        analyzer = AgentAnalyzer(tmp_path, settings, inherit_user_settings=True)

        # Mock the adapter
        mock_adapter = AsyncMock()
        mock_adapter.aexecute.return_value = MagicMock(
            success=True,
            output=json.dumps({
                "project_type": "python",
                "frameworks": [],
                "common_tools": {},
                "workflows": [],
                "recommended_config": {}
            })
        )

        with patch.object(analyzer, '_get_adapter', return_value=mock_adapter):
            result = await analyzer.analyze()

        # Verify Claude was called
        mock_adapter.aexecute.assert_called_once()
        assert result.project_type == "python"

    @pytest.mark.asyncio
    async def test_analyze_returns_result_on_success(self, tmp_path: Path):
        """analyze() returns AnalysisResult on successful execution."""
        analyzer = AgentAnalyzer(tmp_path)

        mock_adapter = AsyncMock()
        mock_adapter.aexecute.return_value = MagicMock(
            success=True,
            output=json.dumps({
                "project_type": "expo",
                "frameworks": ["react-native", "expo"],
                "common_tools": {"Edit": 0.9},
                "workflows": [],
                "recommended_config": {}
            })
        )

        with patch.object(analyzer, '_get_adapter', return_value=mock_adapter):
            result = await analyzer.analyze()

        assert isinstance(result, AnalysisResult)
        assert result.project_type == "expo"
        assert "expo" in result.frameworks

    @pytest.mark.asyncio
    async def test_analyze_handles_adapter_failure(self, tmp_path: Path):
        """analyze() returns empty result when adapter fails."""
        analyzer = AgentAnalyzer(tmp_path)

        mock_adapter = AsyncMock()
        mock_adapter.aexecute.return_value = MagicMock(
            success=False,
            output="",
            error="API Error"
        )

        with patch.object(analyzer, '_get_adapter', return_value=mock_adapter):
            result = await analyzer.analyze()

        assert isinstance(result, AnalysisResult)
        assert result.project_type == "unknown"

    @pytest.mark.asyncio
    async def test_analyze_respects_inherit_user_settings(self, tmp_path: Path):
        """analyze() passes inherit_user_settings to adapter configuration."""
        analyzer = AgentAnalyzer(tmp_path, inherit_user_settings=True)

        mock_adapter = AsyncMock()
        mock_adapter.aexecute.return_value = MagicMock(
            success=True,
            output=json.dumps({"project_type": "python"})
        )
        mock_adapter.configure = MagicMock()

        with patch.object(analyzer, '_get_adapter', return_value=mock_adapter):
            with patch.object(analyzer, '_configure_adapter') as mock_configure:
                await analyzer.analyze()

                # Verify configure was called with inherit_user_settings=True
                mock_configure.assert_called_once()


class TestGetAdapter:
    """Tests for getting/creating the Claude adapter."""

    def test_get_adapter_returns_claude_adapter(self, tmp_path: Path):
        """_get_adapter returns a ClaudeAdapter instance."""
        analyzer = AgentAnalyzer(tmp_path)

        with patch('ralph_orchestrator.onboarding.agent_analyzer.ClaudeAdapter') as MockAdapter:
            MockAdapter.return_value = MagicMock()
            adapter = analyzer._get_adapter()

            MockAdapter.assert_called_once()
            assert adapter is not None

    def test_get_adapter_configures_inherit_user_settings(self, tmp_path: Path):
        """_get_adapter configures adapter with inherit_user_settings."""
        analyzer = AgentAnalyzer(tmp_path, inherit_user_settings=True)

        with patch('ralph_orchestrator.onboarding.agent_analyzer.ClaudeAdapter') as MockAdapter:
            mock_instance = MagicMock()
            MockAdapter.return_value = mock_instance

            adapter = analyzer._get_adapter()

            # Should be called with inherit_user_settings=True
            MockAdapter.assert_called_once_with(inherit_user_settings=True)


class TestSyncAnalyze:
    """Tests for synchronous analyze method."""

    def test_sync_analyze_wraps_async_analyze(self, tmp_path: Path):
        """sync_analyze() provides synchronous wrapper for analyze()."""
        analyzer = AgentAnalyzer(tmp_path)

        mock_result = AnalysisResult(project_type="go", frameworks=["gin"])

        with patch.object(analyzer, 'analyze', new_callable=AsyncMock) as mock_analyze:
            mock_analyze.return_value = mock_result

            # Use the sync wrapper
            result = analyzer.sync_analyze()

            assert result.project_type == "go"
            assert "gin" in result.frameworks
