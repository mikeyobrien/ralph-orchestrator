"""Tests for SettingsLoader - loads and merges Claude Code settings."""

import json
import pytest
import tempfile
from pathlib import Path
from unittest.mock import patch

from ralph_orchestrator.onboarding.settings_loader import SettingsLoader


class TestSettingsLoaderInit:
    """Tests for SettingsLoader initialization."""

    def test_init_with_project_path(self, tmp_path: Path):
        """SettingsLoader initializes with a project path."""
        loader = SettingsLoader(tmp_path)
        assert loader.project_path == tmp_path
        assert loader.user_home == Path.home()

    def test_init_stores_project_path_as_path(self, tmp_path: Path):
        """Project path is stored as Path object."""
        loader = SettingsLoader(str(tmp_path))
        assert isinstance(loader.project_path, Path)


class TestLoadUserSettings:
    """Tests for loading ~/.claude/settings.json."""

    def test_loads_user_settings_json(self, tmp_path: Path):
        """Loads settings from ~/.claude/settings.json."""
        # Create mock settings
        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        settings = {
            "mcpServers": {
                "github": {"command": "github-mcp-server", "args": ["stdio"]}
            },
            "permissions": {"allow": ["Read", "Write"]},
        }
        (claude_dir / "settings.json").write_text(json.dumps(settings))

        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            result = loader.load_user_settings()

        assert result["mcpServers"]["github"]["command"] == "github-mcp-server"
        assert result["permissions"]["allow"] == ["Read", "Write"]

    def test_returns_empty_dict_when_no_settings(self, tmp_path: Path):
        """Returns empty dict when settings.json doesn't exist."""
        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            result = loader.load_user_settings()

        assert result == {}

    def test_returns_empty_dict_on_invalid_json(self, tmp_path: Path):
        """Returns empty dict when settings.json is invalid."""
        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        (claude_dir / "settings.json").write_text("not valid json {{{")

        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            result = loader.load_user_settings()

        assert result == {}


class TestLoadProjectMCP:
    """Tests for loading [project]/.mcp.json."""

    def test_loads_project_mcp_json(self, tmp_path: Path):
        """Loads MCP config from project .mcp.json."""
        mcp_config = {
            "mcpServers": {
                "local-db": {"command": "db-mcp", "args": ["--port", "5432"]}
            }
        }
        (tmp_path / ".mcp.json").write_text(json.dumps(mcp_config))

        loader = SettingsLoader(tmp_path)
        result = loader.load_project_mcp()

        assert result["mcpServers"]["local-db"]["command"] == "db-mcp"

    def test_returns_empty_dict_when_no_mcp_json(self, tmp_path: Path):
        """Returns empty dict when .mcp.json doesn't exist."""
        loader = SettingsLoader(tmp_path)
        result = loader.load_project_mcp()

        assert result == {}


class TestGetMCPServers:
    """Tests for merging MCP server configurations."""

    def test_merges_user_and_project_servers(self, tmp_path: Path):
        """Project servers override user servers with same name."""
        # User settings
        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        user_settings = {
            "mcpServers": {
                "github": {"command": "old-github", "args": []},
                "memory": {"command": "mcp-memory", "args": []},
            }
        }
        (claude_dir / "settings.json").write_text(json.dumps(user_settings))

        # Project MCP
        project_mcp = {
            "mcpServers": {
                "github": {"command": "new-github", "args": ["--token"]},
                "local": {"command": "local-mcp", "args": []},
            }
        }
        (tmp_path / ".mcp.json").write_text(json.dumps(project_mcp))

        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            result = loader.get_mcp_servers()

        # Project overrides user for 'github'
        assert result["github"]["command"] == "new-github"
        # User's 'memory' preserved
        assert result["memory"]["command"] == "mcp-memory"
        # Project's 'local' added
        assert result["local"]["command"] == "local-mcp"

    def test_returns_only_user_servers_when_no_project(self, tmp_path: Path):
        """Returns user servers when no project .mcp.json."""
        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        user_settings = {
            "mcpServers": {"github": {"command": "github-mcp", "args": []}}
        }
        (claude_dir / "settings.json").write_text(json.dumps(user_settings))

        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            result = loader.get_mcp_servers()

        assert result["github"]["command"] == "github-mcp"


class TestGetAnalysisPlugins:
    """Tests for detecting installed analysis plugins."""

    def test_detects_memory_plugin(self, tmp_path: Path):
        """Detects mcp-memory plugin when configured."""
        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        settings = {"mcpServers": {"memory": {"command": "mcp-memory"}}}
        (claude_dir / "settings.json").write_text(json.dumps(settings))

        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            plugins = loader.get_analysis_plugins()

        assert "memory" in plugins

    def test_detects_multiple_plugins(self, tmp_path: Path):
        """Detects multiple analysis plugins."""
        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        settings = {
            "mcpServers": {
                "memory": {"command": "mcp-memory"},
                "fogmap": {"command": "fogmap-mcp"},
                "graphiti": {"command": "graphiti-mcp"},
                "github": {"command": "github-mcp"},  # Not an analysis plugin
            }
        }
        (claude_dir / "settings.json").write_text(json.dumps(settings))

        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            plugins = loader.get_analysis_plugins()

        assert "memory" in plugins
        assert "fogmap" in plugins
        assert "graphiti" in plugins
        assert "github" not in plugins  # Not an analysis plugin


class TestHasMemoryPlugin:
    """Tests for checking if memory plugin is available."""

    def test_returns_true_when_memory_configured(self, tmp_path: Path):
        """Returns True when mcp-memory is configured."""
        claude_dir = tmp_path / ".claude"
        claude_dir.mkdir()
        settings = {"mcpServers": {"memory": {"command": "mcp-memory"}}}
        (claude_dir / "settings.json").write_text(json.dumps(settings))

        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            assert loader.has_memory_plugin() is True

    def test_returns_false_when_no_memory(self, tmp_path: Path):
        """Returns False when no memory plugin configured."""
        loader = SettingsLoader(tmp_path)
        with patch.object(loader, "user_home", tmp_path):
            assert loader.has_memory_plugin() is False
