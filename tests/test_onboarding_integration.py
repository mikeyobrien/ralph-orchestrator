#!/usr/bin/env python3
"""Integration tests for onboarding with mock MCP servers.

These tests verify the full onboarding workflow using mock MCP servers
to simulate various real-world scenarios without requiring actual
API calls or installed MCP servers.
"""

import json
import tempfile
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest


class TestMockMCPServerFixture:
    """Tests for the mock MCP server fixture infrastructure."""

    def test_mock_mcp_servers_fixture_provides_memory_server(self, mock_mcp_servers):
        """Test that the mock MCP servers fixture provides a memory server."""
        assert "memory" in mock_mcp_servers
        assert "command" in mock_mcp_servers["memory"]

    def test_mock_mcp_servers_fixture_provides_filesystem_server(self, mock_mcp_servers):
        """Test that the mock MCP servers fixture provides a filesystem server."""
        assert "filesystem" in mock_mcp_servers
        assert "command" in mock_mcp_servers["filesystem"]

    def test_mock_mcp_servers_fixture_provides_github_server(self, mock_mcp_servers):
        """Test that the mock MCP servers fixture provides a github server."""
        assert "github" in mock_mcp_servers
        assert "command" in mock_mcp_servers["github"]

    def test_mock_settings_file_creates_valid_json(self, mock_claude_settings_dir):
        """Test that mock settings file creates valid JSON."""
        settings_path = mock_claude_settings_dir / "settings.json"
        assert settings_path.exists()

        content = json.loads(settings_path.read_text())
        assert "mcpServers" in content

    def test_mock_project_with_mcp_config(self, mock_project_with_mcp):
        """Test that mock project fixture creates .mcp.json."""
        mcp_path = mock_project_with_mcp / ".mcp.json"
        assert mcp_path.exists()

        content = json.loads(mcp_path.read_text())
        assert "mcpServers" in content


class TestStaticModeIntegration:
    """Integration tests for static analysis mode (--static)."""

    def test_static_mode_full_workflow_with_python_project(self, tmp_path):
        """Test complete static mode workflow with a Python project."""
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader
        from ralph_orchestrator.onboarding.scanner import ProjectScanner
        from ralph_orchestrator.onboarding.history_analyzer import HistoryAnalyzer
        from ralph_orchestrator.onboarding.pattern_extractor import PatternExtractor
        from ralph_orchestrator.onboarding.config_generator import ConfigGenerator
        import json

        # Create project structure
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"\n')

        # Create mock settings dir
        mock_claude_dir = tmp_path / "mock_home" / ".claude"
        mock_claude_dir.mkdir(parents=True)
        (mock_claude_dir / "settings.json").write_text(json.dumps({
            "mcpServers": {"memory": {"command": "mcp-memory", "args": []}}
        }))

        # Create mock history
        history_dir = tmp_path / ".claude" / "history"
        history_dir.mkdir(parents=True)
        history_file = history_dir / "conversation_001.jsonl"
        messages = [
            {"type": "user", "content": [{"type": "text", "text": "test"}]},
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Read", "id": "1", "input": {}}]},
        ]
        with open(history_file, "w") as f:
            for msg in messages:
                f.write(json.dumps(msg) + "\n")

        # 1. Load settings
        settings = SettingsLoader(tmp_path)

        # 2. Scan project
        scanner = ProjectScanner(tmp_path, settings)
        project_type = scanner.detect_project_type()

        # 3. Analyze history (static mode)
        history_files = [history_file]
        analyzer = HistoryAnalyzer(history_files)
        tool_usage = analyzer.extract_tool_usage()

        # 4. Extract patterns
        extractor = PatternExtractor(history=analyzer)
        patterns = extractor.identify_project_patterns()

        # 5. Generate config
        generator = ConfigGenerator(
            scanner=scanner,
            extractor=extractor,
            settings=settings
        )
        ralph_yml = generator.generate_ralph_yml()

        # Verify the config is valid
        assert "agent: claude" in ralph_yml
        assert "prompt_file:" in ralph_yml

    def test_static_mode_with_expo_project(self, tmp_path, mock_claude_settings_dir):
        """Test static mode correctly identifies Expo project type."""
        from ralph_orchestrator.onboarding.scanner import ProjectScanner, ProjectType
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader

        # Create Expo project
        (tmp_path / "package.json").write_text(json.dumps({
            "name": "my-expo-app",
            "version": "1.0.0",
            "dependencies": {
                "expo": "^51.0.0",
                "react-native": "0.74.0"
            }
        }))

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        project_type = scanner.detect_project_type()
        assert project_type == ProjectType.EXPO

    def test_static_mode_without_history_uses_defaults(self, tmp_path):
        """Test static mode works gracefully without Claude history."""
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader
        from ralph_orchestrator.onboarding.scanner import ProjectScanner
        from ralph_orchestrator.onboarding.history_analyzer import HistoryAnalyzer
        from ralph_orchestrator.onboarding.pattern_extractor import PatternExtractor
        from ralph_orchestrator.onboarding.config_generator import ConfigGenerator

        # Create minimal project with no history
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"\n')

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        # No history files
        history_files = []
        analyzer = HistoryAnalyzer(history_files)

        # Should still work
        tool_usage = analyzer.extract_tool_usage()
        assert tool_usage == {}

        extractor = PatternExtractor(history=analyzer)
        patterns = extractor.identify_project_patterns()

        generator = ConfigGenerator(
            scanner=scanner,
            extractor=extractor,
            settings=settings
        )
        ralph_yml = generator.generate_ralph_yml()

        # Should generate valid config with defaults
        assert "agent: claude" in ralph_yml


class TestAgentModeIntegration:
    """Integration tests for agent-assisted analysis mode (--agent)."""

    def test_agent_mode_uses_mcp_memory_when_available(
        self, mock_project_with_mcp, mock_claude_settings_dir
    ):
        """Test that agent mode detects and uses memory MCP when available."""
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader
        from ralph_orchestrator.onboarding.agent_analyzer import AgentAnalyzer

        # Create settings with memory MCP
        settings_content = {
            "mcpServers": {
                "memory": {"command": "mcp-memory", "args": []},
                "filesystem": {"command": "mcp-fs", "args": ["/home"]}
            }
        }
        settings_path = mock_claude_settings_dir / "settings.json"
        settings_path.write_text(json.dumps(settings_content))

        with patch.object(Path, 'home', return_value=mock_claude_settings_dir.parent):
            settings = SettingsLoader(mock_project_with_mcp)

            # Verify memory plugin detected
            assert settings.has_memory_plugin()

            # Create analyzer
            analyzer = AgentAnalyzer(mock_project_with_mcp, settings)

            # Build prompt should include memory instructions
            prompt = analyzer._build_analysis_prompt()
            assert "memory" in prompt.lower()
            assert "episodic" in prompt.lower()

    def test_agent_mode_falls_back_without_memory(
        self, mock_project_with_mcp, mock_claude_settings_dir
    ):
        """Test that agent mode works without memory MCP."""
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader
        from ralph_orchestrator.onboarding.agent_analyzer import AgentAnalyzer

        # Create settings without memory MCP
        settings_content = {
            "mcpServers": {
                "filesystem": {"command": "mcp-fs", "args": ["/home"]}
            }
        }
        settings_path = mock_claude_settings_dir / "settings.json"
        settings_path.write_text(json.dumps(settings_content))

        with patch.object(Path, 'home', return_value=mock_claude_settings_dir.parent):
            settings = SettingsLoader(mock_project_with_mcp)

            # Verify no memory plugin
            assert not settings.has_memory_plugin()

            # Create analyzer
            analyzer = AgentAnalyzer(mock_project_with_mcp, settings)

            # Build prompt should NOT include memory instructions
            prompt = analyzer._build_analysis_prompt()
            # Still works, just different prompt
            assert "project" in prompt.lower()

    def test_agent_mode_with_mocked_claude_response(self, mock_project_with_mcp):
        """Test agent mode with mocked Claude response."""
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader
        from ralph_orchestrator.onboarding.agent_analyzer import AgentAnalyzer, AnalysisResult
        from ralph_orchestrator.onboarding.pattern_extractor import PatternExtractor
        from ralph_orchestrator.onboarding.config_generator import ConfigGenerator
        from ralph_orchestrator.onboarding.scanner import ProjectScanner

        settings = SettingsLoader(mock_project_with_mcp)

        # Mock the agent analysis result
        mock_result = AnalysisResult(
            project_type="python",
            frameworks=["fastapi", "pytest"],
            common_tools={"Edit": 0.95, "Bash": 0.90, "Read": 0.85},
            workflows=[["Read", "Edit", "Bash"]],
            recommended_config={"max_iterations": 75}
        )

        # Create extractor with analysis result
        extractor = PatternExtractor(analysis_result=mock_result)
        patterns = extractor.identify_project_patterns()

        # Verify the analysis result is correctly stored
        assert mock_result.project_type == "python"
        assert "fastapi" in mock_result.frameworks

        # Verify patterns extracted tools with high success rates
        assert "Edit" in patterns.successful_tools
        assert "Bash" in patterns.successful_tools
        assert "Read" in patterns.successful_tools

        # Generate config
        scanner = ProjectScanner(mock_project_with_mcp, settings)
        generator = ConfigGenerator(
            scanner=scanner,
            extractor=extractor,
            settings=settings
        )
        ralph_yml = generator.generate_ralph_yml()

        # Should include analysis recommendations
        assert "agent: claude" in ralph_yml
        # Should contain tool info from the patterns
        assert "prompt_file:" in ralph_yml


class TestMixedModeIntegration:
    """Tests for scenarios mixing static and agent analysis."""

    def test_fallback_from_agent_to_static(self, mock_project_with_history):
        """Test fallback from agent mode to static when agent fails."""
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader
        from ralph_orchestrator.onboarding.scanner import ProjectScanner
        from ralph_orchestrator.onboarding.agent_analyzer import AgentAnalyzer
        from ralph_orchestrator.onboarding.history_analyzer import HistoryAnalyzer
        from ralph_orchestrator.onboarding.pattern_extractor import PatternExtractor
        from ralph_orchestrator.onboarding.config_generator import ConfigGenerator

        settings = SettingsLoader(mock_project_with_history)
        scanner = ProjectScanner(mock_project_with_history, settings)

        # Try agent mode first (will fail because no real API)
        agent = AgentAnalyzer(mock_project_with_history, settings)

        with patch.object(agent, 'sync_analyze', side_effect=Exception("API unavailable")):
            try:
                agent_result = agent.sync_analyze()
            except Exception:
                agent_result = None

        # Fallback to static analysis
        history_files = scanner.find_claude_history()
        static_analyzer = HistoryAnalyzer(history_files)

        # This should always work
        tool_usage = static_analyzer.extract_tool_usage()

        # Extract patterns from static analysis
        extractor = PatternExtractor(history=static_analyzer)
        patterns = extractor.identify_project_patterns()

        # Generate config
        generator = ConfigGenerator(
            scanner=scanner,
            extractor=extractor,
            settings=settings
        )
        ralph_yml = generator.generate_ralph_yml()

        # Should still produce valid config
        assert "agent: claude" in ralph_yml


class TestMCPServerDetection:
    """Tests for MCP server detection and usage."""

    def test_detects_all_analysis_plugins(self, mock_claude_settings_dir):
        """Test detection of various analysis plugins."""
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader, ANALYSIS_PLUGINS

        # Create settings with all analysis plugins
        settings_content = {
            "mcpServers": {
                "memory": {"command": "mcp-memory", "args": []},
                "fogmap": {"command": "fogmap-mcp", "args": []},
                "graphiti": {"command": "graphiti-mcp", "args": []},
            }
        }
        settings_path = mock_claude_settings_dir / "settings.json"
        settings_path.write_text(json.dumps(settings_content))

        with patch.object(Path, 'home', return_value=mock_claude_settings_dir.parent):
            settings = SettingsLoader(mock_claude_settings_dir.parent / "test-project")
            plugins = settings.get_analysis_plugins()

            assert "memory" in plugins
            assert "fogmap" in plugins
            assert "graphiti" in plugins

    def test_project_mcp_overrides_user_mcp(self, tmp_path, mock_claude_settings_dir):
        """Test that project MCP config overrides user MCP config."""
        from ralph_orchestrator.onboarding.settings_loader import SettingsLoader

        # User settings with github MCP
        user_settings = {
            "mcpServers": {
                "github": {"command": "github-mcp-v1", "args": ["--old"]}
            }
        }
        settings_path = mock_claude_settings_dir / "settings.json"
        settings_path.write_text(json.dumps(user_settings))

        # Project overrides with new github config
        project_mcp = {
            "mcpServers": {
                "github": {"command": "github-mcp-v2", "args": ["--new"]}
            }
        }
        (tmp_path / ".mcp.json").write_text(json.dumps(project_mcp))

        with patch.object(Path, 'home', return_value=mock_claude_settings_dir.parent):
            settings = SettingsLoader(tmp_path)
            servers = settings.get_mcp_servers()

            # Project should win
            assert servers["github"]["command"] == "github-mcp-v2"
            assert "--new" in servers["github"]["args"]


# ============================================================================
# PYTEST FIXTURES FOR MOCK MCP INFRASTRUCTURE
# ============================================================================

@pytest.fixture
def mock_mcp_servers():
    """Provide mock MCP server configurations for testing."""
    return {
        "memory": {
            "command": "mcp-memory",
            "args": ["--project", "test"],
            "env": {}
        },
        "filesystem": {
            "command": "mcp-filesystem",
            "args": ["/home/user"],
            "env": {}
        },
        "github": {
            "command": "github-mcp-server",
            "args": ["stdio"],
            "env": {"GITHUB_TOKEN": "mock-token"}
        },
        "fogmap": {
            "command": "fogmap-mcp",
            "args": [],
            "env": {}
        }
    }


@pytest.fixture
def mock_claude_settings_dir(tmp_path, mock_mcp_servers):
    """Create a mock ~/.claude directory with settings.json."""
    claude_dir = tmp_path / ".claude"
    claude_dir.mkdir()

    settings = {
        "mcpServers": mock_mcp_servers,
        "permissions": {
            "allow": ["Read", "Write", "Edit", "Bash"],
            "deny": []
        }
    }
    (claude_dir / "settings.json").write_text(json.dumps(settings, indent=2))

    return claude_dir


@pytest.fixture
def mock_project_with_mcp(tmp_path):
    """Create a mock project with .mcp.json configuration."""
    # Create package.json for Node project
    (tmp_path / "package.json").write_text(json.dumps({
        "name": "test-project",
        "version": "1.0.0",
        "dependencies": {}
    }))

    # Create project MCP config
    mcp_config = {
        "mcpServers": {
            "project-db": {
                "command": "postgres-mcp",
                "args": ["--db", "test"]
            }
        }
    }
    (tmp_path / ".mcp.json").write_text(json.dumps(mcp_config, indent=2))

    return tmp_path


@pytest.fixture
def mock_project_with_history(tmp_path):
    """Create a mock project with Claude conversation history."""
    # Create Python project
    (tmp_path / "pyproject.toml").write_text("""
[project]
name = "test-project"
version = "0.1.0"
dependencies = ["fastapi", "pytest"]
""")

    # Create mock conversation history
    history_dir = tmp_path / ".claude" / "history"
    history_dir.mkdir(parents=True)

    # Write mock JSONL conversation
    history_file = history_dir / "conversation_001.jsonl"
    messages = [
        {"type": "user", "content": [{"type": "text", "text": "Fix the tests"}]},
        {"type": "assistant", "content": [
            {"type": "text", "text": "I'll fix the tests"},
            {"type": "tool_use", "name": "Read", "id": "1", "input": {"file": "test.py"}}
        ]},
        {"type": "user", "content": [
            {"type": "tool_result", "tool_use_id": "1", "content": "test content", "is_error": False}
        ]},
        {"type": "assistant", "content": [
            {"type": "tool_use", "name": "Edit", "id": "2", "input": {"file": "test.py"}}
        ]},
        {"type": "user", "content": [
            {"type": "tool_result", "tool_use_id": "2", "content": "edited", "is_error": False}
        ]},
        {"type": "assistant", "content": [
            {"type": "tool_use", "name": "Bash", "id": "3", "input": {"command": "pytest"}}
        ]},
        {"type": "user", "content": [
            {"type": "tool_result", "tool_use_id": "3", "content": "tests passed", "is_error": False}
        ]},
    ]

    with open(history_file, "w") as f:
        for msg in messages:
            f.write(json.dumps(msg) + "\n")

    return tmp_path
