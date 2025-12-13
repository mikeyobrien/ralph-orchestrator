# ABOUTME: Tests for ACP adapter configuration support in ralph.yml
# ABOUTME: Verifies YAML parsing, environment variable overrides, and defaults

"""Tests for ACP configuration in ralph.yml."""

import os
import pytest
import tempfile
from pathlib import Path
from unittest.mock import patch

from ralph_orchestrator.main import RalphConfig, AdapterConfig
from ralph_orchestrator.adapters.acp_models import ACPAdapterConfig


class TestACPAdapterConfigParsing:
    """Test parsing ACP adapter config from YAML."""

    def test_parse_acp_config_from_yaml_basic(self, tmp_path: Path):
        """Test basic ACP config parsing from YAML file."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp:
    enabled: true
    timeout: 300
    tool_permissions:
      agent_command: gemini
""")
        config = RalphConfig.from_yaml(str(config_file))

        assert "acp" in config.adapters
        acp_config = config.adapters["acp"]
        assert acp_config.enabled is True
        assert acp_config.timeout == 300

        # Check ACP-specific settings via tool_permissions
        acp_adapter_config = ACPAdapterConfig.from_adapter_config(acp_config)
        assert acp_adapter_config.agent_command == "gemini"

    def test_parse_acp_config_full_options(self, tmp_path: Path):
        """Test full ACP config with all options."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp:
    enabled: true
    timeout: 600
    tool_permissions:
      agent_command: gemini
      agent_args:
        - --verbose
        - --no-color
      permission_mode: allowlist
      permission_allowlist:
        - "fs/read_text_file"
        - "fs/*"
        - "/^terminal\\/.*$/"
""")
        config = RalphConfig.from_yaml(str(config_file))

        assert "acp" in config.adapters
        acp_config = config.adapters["acp"]
        assert acp_config.enabled is True
        assert acp_config.timeout == 600

        # Check ACP-specific settings via tool_permissions
        acp_adapter_config = ACPAdapterConfig.from_adapter_config(acp_config)
        assert acp_adapter_config.agent_command == "gemini"
        assert acp_adapter_config.agent_args == ["--verbose", "--no-color"]
        assert acp_adapter_config.permission_mode == "allowlist"
        assert "fs/read_text_file" in acp_adapter_config.permission_allowlist

    def test_parse_acp_config_disabled(self, tmp_path: Path):
        """Test ACP config when disabled."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp:
    enabled: false
""")
        config = RalphConfig.from_yaml(str(config_file))

        assert "acp" in config.adapters
        acp_config = config.adapters["acp"]
        assert acp_config.enabled is False

    def test_parse_acp_config_simple_boolean(self, tmp_path: Path):
        """Test ACP config with simple boolean enable/disable."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp: true
""")
        config = RalphConfig.from_yaml(str(config_file))

        assert "acp" in config.adapters
        acp_config = config.adapters["acp"]
        assert acp_config.enabled is True

    def test_parse_acp_config_missing_uses_defaults(self, tmp_path: Path):
        """Test that missing ACP config returns defaults."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  claude:
    enabled: true
""")
        config = RalphConfig.from_yaml(str(config_file))

        # ACP not configured, but get_adapter_config should return defaults
        acp_config = config.get_adapter_config("acp")
        assert acp_config.enabled is True  # Default is enabled
        assert acp_config.timeout == 300  # Default timeout

    def test_parse_acp_config_with_other_adapters(self, tmp_path: Path):
        """Test ACP config alongside other adapters."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  claude:
    enabled: true
    timeout: 300
  acp:
    enabled: true
    timeout: 600
    tool_permissions:
      agent_command: gemini
  gemini:
    enabled: false
""")
        config = RalphConfig.from_yaml(str(config_file))

        assert "claude" in config.adapters
        assert "acp" in config.adapters
        assert "gemini" in config.adapters

        assert config.adapters["claude"].enabled is True
        assert config.adapters["acp"].enabled is True
        assert config.adapters["acp"].timeout == 600
        assert config.adapters["gemini"].enabled is False


class TestACPAdapterConfigEnvironmentOverrides:
    """Test environment variable overrides for ACP config."""

    def test_env_override_agent_command(self, tmp_path: Path):
        """Test RALPH_ACP_AGENT environment variable override."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp:
    enabled: true
    tool_permissions:
      agent_command: gemini
""")
        with patch.dict(os.environ, {"RALPH_ACP_AGENT": "custom-agent"}):
            config = RalphConfig.from_yaml(str(config_file))
            acp_adapter_config = ACPAdapterConfig.from_adapter_config(
                config.adapters.get("acp", AdapterConfig())
            )

            # Environment variable should override
            assert acp_adapter_config.agent_command == "custom-agent"

    def test_env_override_permission_mode(self, tmp_path: Path):
        """Test RALPH_ACP_PERMISSION_MODE environment variable override."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp:
    enabled: true
    tool_permissions:
      permission_mode: auto_approve
""")
        with patch.dict(os.environ, {"RALPH_ACP_PERMISSION_MODE": "deny_all"}):
            config = RalphConfig.from_yaml(str(config_file))
            acp_adapter_config = ACPAdapterConfig.from_adapter_config(
                config.adapters.get("acp", AdapterConfig())
            )

            # Environment variable should override
            assert acp_adapter_config.permission_mode == "deny_all"

    def test_env_override_timeout(self, tmp_path: Path):
        """Test RALPH_ACP_TIMEOUT environment variable override."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp:
    enabled: true
    timeout: 300
""")
        with patch.dict(os.environ, {"RALPH_ACP_TIMEOUT": "900"}):
            config = RalphConfig.from_yaml(str(config_file))
            acp_adapter_config = ACPAdapterConfig.from_adapter_config(
                config.adapters.get("acp", AdapterConfig())
            )

            # Environment variable should override
            assert acp_adapter_config.timeout == 900

    def test_env_override_invalid_timeout_uses_default(self, tmp_path: Path):
        """Test invalid RALPH_ACP_TIMEOUT falls back to config value."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp:
    enabled: true
    timeout: 300
""")
        with patch.dict(os.environ, {"RALPH_ACP_TIMEOUT": "not-a-number"}):
            config = RalphConfig.from_yaml(str(config_file))
            acp_adapter_config = ACPAdapterConfig.from_adapter_config(
                config.adapters.get("acp", AdapterConfig())
            )

            # Should fall back to config value
            assert acp_adapter_config.timeout == 300

    def test_env_no_override_without_env_var(self, tmp_path: Path):
        """Test config values are used when no env vars set."""
        config_file = tmp_path / "ralph.yml"
        config_file.write_text("""
agent: auto
adapters:
  acp:
    enabled: true
    timeout: 600
    tool_permissions:
      agent_command: gemini
      permission_mode: allowlist
""")
        # Clear any existing env vars
        with patch.dict(os.environ, {}, clear=True):
            # Remove ACP-related env vars
            env_copy = {k: v for k, v in os.environ.items()
                       if not k.startswith("RALPH_ACP_")}
            with patch.dict(os.environ, env_copy, clear=True):
                config = RalphConfig.from_yaml(str(config_file))
                acp_adapter_config = ACPAdapterConfig.from_adapter_config(
                    config.adapters.get("acp", AdapterConfig())
                )

                # Config values should be used
                assert acp_adapter_config.agent_command == "gemini"
                assert acp_adapter_config.timeout == 600
                assert acp_adapter_config.permission_mode == "allowlist"


class TestACPAdapterConfigDefaults:
    """Test default values for ACP config."""

    def test_acp_adapter_config_defaults(self):
        """Test ACPAdapterConfig has correct defaults."""
        config = ACPAdapterConfig()

        assert config.agent_command == "gemini"
        assert config.agent_args == []
        assert config.timeout == 300
        assert config.permission_mode == "auto_approve"
        assert config.permission_allowlist == []

    def test_acp_adapter_config_from_empty_dict(self):
        """Test ACPAdapterConfig.from_dict with empty dict uses defaults."""
        config = ACPAdapterConfig.from_dict({})

        assert config.agent_command == "gemini"
        assert config.agent_args == []
        assert config.timeout == 300
        assert config.permission_mode == "auto_approve"
        assert config.permission_allowlist == []

    def test_acp_adapter_config_from_partial_dict(self):
        """Test ACPAdapterConfig.from_dict with partial dict fills defaults."""
        config = ACPAdapterConfig.from_dict({
            "agent_command": "custom-agent",
            "timeout": 600
        })

        assert config.agent_command == "custom-agent"
        assert config.agent_args == []  # Default
        assert config.timeout == 600
        assert config.permission_mode == "auto_approve"  # Default
        assert config.permission_allowlist == []  # Default

    def test_acp_adapter_config_from_adapter_config_empty(self):
        """Test ACPAdapterConfig.from_adapter_config with empty AdapterConfig."""
        adapter_config = AdapterConfig()
        acp_config = ACPAdapterConfig.from_adapter_config(adapter_config)

        assert acp_config.agent_command == "gemini"
        assert acp_config.timeout == 300  # Uses AdapterConfig default timeout


class TestACPAdapterConfigFromAdapterConfig:
    """Test converting AdapterConfig to ACPAdapterConfig."""

    def test_from_adapter_config_basic(self):
        """Test basic conversion from AdapterConfig."""
        adapter_config = AdapterConfig(
            enabled=True,
            timeout=600,
        )
        acp_config = ACPAdapterConfig.from_adapter_config(adapter_config)

        assert acp_config.timeout == 600
        assert acp_config.agent_command == "gemini"  # Default

    def test_from_adapter_config_with_acp_fields(self):
        """Test conversion with ACP-specific fields in tool_permissions."""
        adapter_config = AdapterConfig(
            enabled=True,
            timeout=600,
            tool_permissions={
                "agent_command": "custom-agent",
                "agent_args": ["--verbose"],
                "permission_mode": "deny_all",
                "permission_allowlist": ["fs/*"],
            }
        )
        acp_config = ACPAdapterConfig.from_adapter_config(adapter_config)

        assert acp_config.agent_command == "custom-agent"
        assert acp_config.agent_args == ["--verbose"]
        assert acp_config.timeout == 600
        assert acp_config.permission_mode == "deny_all"
        assert acp_config.permission_allowlist == ["fs/*"]

    def test_from_adapter_config_env_override(self):
        """Test that env vars override AdapterConfig values."""
        adapter_config = AdapterConfig(
            enabled=True,
            timeout=600,
            tool_permissions={
                "agent_command": "gemini",
                "permission_mode": "auto_approve",
            }
        )

        with patch.dict(os.environ, {
            "RALPH_ACP_AGENT": "env-agent",
            "RALPH_ACP_PERMISSION_MODE": "interactive",
        }):
            acp_config = ACPAdapterConfig.from_adapter_config(adapter_config)

            assert acp_config.agent_command == "env-agent"
            assert acp_config.permission_mode == "interactive"


class TestACPConfigInitTemplate:
    """Test that ralph init creates ACP config template."""

    def test_init_creates_acp_config(self, tmp_path: Path, monkeypatch):
        """Test that ralph init includes ACP adapter config."""
        # Change to temp directory
        monkeypatch.chdir(tmp_path)

        # Import and run init
        from ralph_orchestrator.__main__ import init_project

        # Run init (suppress output)
        with patch('builtins.input', return_value='n'):
            init_project()

        # Check ralph.yml was created with ACP config
        config_file = tmp_path / "ralph.yml"
        assert config_file.exists()

        config_content = config_file.read_text()
        assert "acp:" in config_content
        assert "agent_command:" in config_content or "enabled:" in config_content

    def test_init_acp_config_is_valid_yaml(self, tmp_path: Path, monkeypatch):
        """Test that the generated ACP config is valid YAML."""
        import yaml

        monkeypatch.chdir(tmp_path)

        from ralph_orchestrator.__main__ import init_project

        with patch('builtins.input', return_value='n'):
            init_project()

        config_file = tmp_path / "ralph.yml"
        config_content = config_file.read_text()

        # Should parse without errors
        parsed = yaml.safe_load(config_content)
        assert "adapters" in parsed
        assert "acp" in parsed["adapters"]


class TestACPConfigValidation:
    """Test validation of ACP config values."""

    def test_valid_permission_modes(self):
        """Test that all valid permission modes are accepted."""
        valid_modes = ["auto_approve", "deny_all", "allowlist", "interactive"]

        for mode in valid_modes:
            config = ACPAdapterConfig(permission_mode=mode)
            assert config.permission_mode == mode

    def test_invalid_permission_mode_not_validated_at_creation(self):
        """Test that invalid permission mode is accepted at creation (validated at use)."""
        # Note: validation happens in ACPHandlers, not at config creation
        config = ACPAdapterConfig(permission_mode="invalid")
        assert config.permission_mode == "invalid"

    def test_timeout_must_be_positive(self):
        """Test that timeout must be positive (validated at use)."""
        # Config creation doesn't validate - this is validated at adapter init
        config = ACPAdapterConfig(timeout=-1)
        assert config.timeout == -1

    def test_agent_command_can_be_path(self):
        """Test that agent_command can be a full path."""
        config = ACPAdapterConfig(agent_command="/usr/local/bin/gemini")
        assert config.agent_command == "/usr/local/bin/gemini"

    def test_agent_args_as_list(self):
        """Test that agent_args must be a list."""
        config = ACPAdapterConfig(agent_args=["--verbose", "--no-color"])
        assert config.agent_args == ["--verbose", "--no-color"]
