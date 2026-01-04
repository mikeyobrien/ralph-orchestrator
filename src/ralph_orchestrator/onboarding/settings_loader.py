"""SettingsLoader - Loads and merges Claude Code settings from all sources.

This module handles loading settings from:
- ~/.claude/settings.json (user settings)
- [project]/.mcp.json (project MCP config)
- Merges settings with proper precedence (project overrides user)
"""

import json
import logging
from pathlib import Path
from typing import Any, Dict, List

logger = logging.getLogger(__name__)

# Known analysis plugins that can be used for intelligent onboarding
ANALYSIS_PLUGINS = frozenset([
    "memory",      # mcp-memory / mem0 - episodic memory
    "fogmap",      # fogmap - semantic search
    "graphiti",    # graphiti-mcp - knowledge graph
    "mem0",        # mem0 - memory system
    "claude-mem",  # claude-mem - memory MCP
])


class SettingsLoader:
    """Loads and merges Claude Code settings from all sources.

    Handles loading:
    - User settings from ~/.claude/settings.json
    - Project MCP config from [project]/.mcp.json
    - Merges with project settings taking precedence

    Attributes:
        project_path: Path to the project directory
        user_home: Path to user's home directory
    """

    def __init__(self, project_path: Path | str):
        """Initialize SettingsLoader with a project path.

        Args:
            project_path: Path to the project directory
        """
        self.project_path = Path(project_path)
        self.user_home = Path.home()
        self._user_settings_cache: Dict[str, Any] | None = None
        self._project_mcp_cache: Dict[str, Any] | None = None

    def load_user_settings(self) -> Dict[str, Any]:
        """Load user settings from ~/.claude/settings.json.

        Returns:
            Dictionary of user settings, or empty dict if file doesn't exist
            or is invalid JSON.
        """
        settings_path = self.user_home / ".claude" / "settings.json"

        if not settings_path.exists():
            logger.debug(f"No user settings found at {settings_path}")
            return {}

        try:
            content = settings_path.read_text()
            settings = json.loads(content)
            logger.debug(f"Loaded user settings from {settings_path}")
            return settings
        except json.JSONDecodeError as e:
            logger.warning(f"Invalid JSON in {settings_path}: {e}")
            return {}
        except Exception as e:
            logger.warning(f"Failed to load settings from {settings_path}: {e}")
            return {}

    def load_project_mcp(self) -> Dict[str, Any]:
        """Load project MCP config from [project]/.mcp.json.

        Returns:
            Dictionary of MCP config, or empty dict if file doesn't exist
            or is invalid JSON.
        """
        mcp_path = self.project_path / ".mcp.json"

        if not mcp_path.exists():
            logger.debug(f"No project MCP config found at {mcp_path}")
            return {}

        try:
            content = mcp_path.read_text()
            config = json.loads(content)
            logger.debug(f"Loaded project MCP config from {mcp_path}")
            return config
        except json.JSONDecodeError as e:
            logger.warning(f"Invalid JSON in {mcp_path}: {e}")
            return {}
        except Exception as e:
            logger.warning(f"Failed to load MCP config from {mcp_path}: {e}")
            return {}

    def get_mcp_servers(self) -> Dict[str, Dict[str, Any]]:
        """Get merged MCP server configurations.

        Merges user and project MCP servers, with project taking precedence
        for servers with the same name.

        Returns:
            Dictionary mapping server names to their configurations.
        """
        user_settings = self.load_user_settings()
        project_mcp = self.load_project_mcp()

        # Start with user servers
        servers = dict(user_settings.get("mcpServers", {}))

        # Project servers override user servers
        project_servers = project_mcp.get("mcpServers", {})
        servers.update(project_servers)

        return servers

    def get_permissions(self) -> Dict[str, Any]:
        """Get tool permission settings from user settings.

        Returns:
            Dictionary with 'allow' and 'deny' lists for tool permissions.
        """
        user_settings = self.load_user_settings()
        return user_settings.get("permissions", {})

    def get_analysis_plugins(self) -> List[str]:
        """Detect installed analysis plugins.

        Checks configured MCP servers for known analysis plugins like
        mcp-memory, fogmap, graphiti, etc.

        Returns:
            List of analysis plugin names that are configured.
        """
        servers = self.get_mcp_servers()
        plugins = []

        for name in servers:
            # Check if server name matches known analysis plugins
            if name in ANALYSIS_PLUGINS:
                plugins.append(name)
            # Also check command for plugin identifiers
            elif server_config := servers.get(name):
                command = server_config.get("command", "")
                for plugin in ANALYSIS_PLUGINS:
                    if plugin in command:
                        plugins.append(name)
                        break

        return plugins

    def has_memory_plugin(self) -> bool:
        """Check if a memory plugin is available.

        Returns:
            True if mcp-memory, mem0, or similar is configured.
        """
        plugins = self.get_analysis_plugins()
        memory_plugins = {"memory", "mem0", "claude-mem"}
        return any(p in memory_plugins or "memory" in p.lower() for p in plugins)
