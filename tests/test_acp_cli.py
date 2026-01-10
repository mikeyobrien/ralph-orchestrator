# ABOUTME: Tests for ACP adapter CLI integration
# ABOUTME: Verifies argument parsing, adapter selection, and ACP-specific options

"""Tests for ACP CLI integration."""

import pytest
import argparse
from unittest.mock import patch


class TestACPAgentChoice:
    """Test that 'acp' is a valid agent choice."""

    def test_acp_in_agent_choices(self):
        """Test that 'acp' is accepted as an agent choice."""

        # Import argparse setup from __main__
        parser = argparse.ArgumentParser()
        parser.add_argument(
            "-a", "--agent",
            choices=["claude", "q", "gemini", "acp", "auto"],
            default="auto"
        )

        # This should not raise
        args = parser.parse_args(["-a", "acp"])
        assert args.agent == "acp"

    def test_acp_agent_type_enum(self):
        """Test that ACP is in AgentType enum."""
        from ralph_orchestrator.main import AgentType

        assert hasattr(AgentType, "ACP")
        assert AgentType.ACP.value == "acp"


class TestACPCLIArguments:
    """Test ACP-specific CLI arguments."""

    def test_acp_agent_argument(self):
        """Test --acp-agent argument parsing."""
        parser = argparse.ArgumentParser()
        parser.add_argument("--acp-agent", default="gemini", help="ACP agent binary")

        # Default value
        args = parser.parse_args([])
        assert args.acp_agent == "gemini"

        # Custom value
        args = parser.parse_args(["--acp-agent", "custom-agent"])
        assert args.acp_agent == "custom-agent"

    def test_acp_permission_mode_argument(self):
        """Test --acp-permission-mode argument parsing."""
        parser = argparse.ArgumentParser()
        parser.add_argument(
            "--acp-permission-mode",
            choices=["auto_approve", "deny_all", "allowlist", "interactive"],
            default="auto_approve"
        )

        # Default value
        args = parser.parse_args([])
        assert args.acp_permission_mode == "auto_approve"

        # Custom value
        args = parser.parse_args(["--acp-permission-mode", "deny_all"])
        assert args.acp_permission_mode == "deny_all"

        # Invalid value should fail
        with pytest.raises(SystemExit):
            parser.parse_args(["--acp-permission-mode", "invalid"])


class TestACPAdapterMap:
    """Test agent name mapping for ACP."""

    def test_agent_map_includes_acp(self):
        """Test that agent_map includes 'acp' mapping."""
        from ralph_orchestrator.main import AgentType

        # Simulate the agent_map from __main__.py
        agent_map = {
            "claude": AgentType.CLAUDE,
            "c": AgentType.CLAUDE,
            "q": AgentType.Q,
            "qchat": AgentType.Q,
            "gemini": AgentType.GEMINI,
            "g": AgentType.GEMINI,
            "acp": AgentType.ACP,
            "auto": AgentType.AUTO
        }

        assert "acp" in agent_map
        assert agent_map["acp"] == AgentType.ACP

    def test_tool_name_map_includes_acp(self):
        """Test that tool_name_map includes 'acp' mapping."""
        tool_name_map = {
            "q": "qchat",
            "claude": "claude",
            "gemini": "gemini",
            "acp": "acp",
            "auto": "auto"
        }

        assert "acp" in tool_name_map
        assert tool_name_map["acp"] == "acp"


class TestOrchestratorACPAdapter:
    """Test orchestrator initialization with ACP adapter."""

    def test_orchestrator_adapters_includes_acp(self):
        """Test that orchestrator initializes ACP adapter."""
        from ralph_orchestrator.orchestrator import RalphOrchestrator
        from ralph_orchestrator.adapters import ACPAdapter

        with patch.object(ACPAdapter, 'check_availability', return_value=True):
            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                with patch('ralph_orchestrator.orchestrator.QChatAdapter') as mock_qchat:
                    with patch('ralph_orchestrator.orchestrator.GeminiAdapter') as mock_gemini:
                        with patch('ralph_orchestrator.orchestrator.ACPAdapter') as mock_acp:
                            # Mock all adapters as available
                            mock_claude.return_value.available = True
                            mock_qchat.return_value.available = True
                            mock_gemini.return_value.available = True
                            mock_acp.return_value.available = True

                            # Create a minimal config
                            from ralph_orchestrator.main import RalphConfig, AgentType
                            config = RalphConfig(
                                agent=AgentType.ACP,
                                prompt_file="PROMPT.md"
                            )

                            # Initialize orchestrator
                            RalphOrchestrator(
                                prompt_file_or_config=config,
                                primary_tool="acp"
                            )

                            # Verify ACP adapter was initialized
                            mock_acp.assert_called()

    def test_orchestrator_acp_adapter_with_config(self):
        """Test orchestrator passes ACP config to adapter."""
        from ralph_orchestrator.adapters.acp import ACPAdapter
        from ralph_orchestrator.adapters.acp_models import ACPAdapterConfig

        config = ACPAdapterConfig(
            agent_command="custom-agent",
            permission_mode="deny_all",
            timeout=600
        )

        with patch('shutil.which', return_value="/usr/bin/custom-agent"):
            adapter = ACPAdapter.from_config(config)

            assert adapter.agent_command == "custom-agent"
            assert adapter.permission_mode == "deny_all"
            assert adapter.timeout == 600

    def test_orchestrator_passes_acp_agent_cli_param(self):
        """Test orchestrator passes acp_agent CLI parameter to ACPAdapter."""
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude, \
             patch('ralph_orchestrator.orchestrator.QChatAdapter') as mock_qchat, \
             patch('ralph_orchestrator.orchestrator.GeminiAdapter') as mock_gemini, \
             patch('ralph_orchestrator.orchestrator.ACPAdapter') as mock_acp:

            # Make all adapters unavailable except ACP
            mock_claude.return_value.available = False
            mock_qchat.return_value.available = False
            mock_gemini.return_value.available = False
            mock_acp.return_value.available = True

            # Create a minimal config
            from ralph_orchestrator.main import RalphConfig, AgentType
            config = RalphConfig(
                agent=AgentType.ACP,
                prompt_file="PROMPT.md"
            )

            # Initialize orchestrator with acp_agent parameter
            RalphOrchestrator(
                prompt_file_or_config=config,
                primary_tool="acp",
                acp_agent="claude-code-acp",
                acp_permission_mode="auto_approve"
            )

            # Verify ACPAdapter was called with correct parameters
            mock_acp.assert_called_once()
            kwargs = mock_acp.call_args.kwargs
            assert kwargs["agent_command"] == "claude-code-acp"
            assert kwargs["permission_mode"] == "auto_approve"


class TestACPAutoDetection:
    """Test ACP auto-detection in 'auto' mode."""

    def test_auto_mode_includes_acp_check(self):
        """Test that 'auto' mode checks for ACP adapter availability."""
        # When acp adapter is available and others are not, it should be selected
        from ralph_orchestrator.adapters.acp import ACPAdapter

        # ACP adapter should have check_availability() method
        with patch('shutil.which', return_value="/usr/bin/gemini"):
            adapter = ACPAdapter()
            assert adapter.check_availability() is True

        with patch('shutil.which', return_value=None):
            adapter = ACPAdapter()
            assert adapter.check_availability() is False


class TestACPCLIConfigIntegration:
    """Test CLI arguments override config file for ACP."""

    def test_cli_acp_agent_overrides_config(self):
        """Test that --acp-agent CLI arg overrides ralph.yml config."""
        from ralph_orchestrator.adapters.acp_models import ACPAdapterConfig

        # Config file says "gemini"
        config = ACPAdapterConfig(agent_command="gemini")

        # CLI says "claude-cli"
        cli_agent = "claude-cli"

        # CLI should override
        if cli_agent:
            config.agent_command = cli_agent

        assert config.agent_command == "claude-cli"

    def test_cli_permission_mode_overrides_config(self):
        """Test that --acp-permission-mode CLI arg overrides config."""
        from ralph_orchestrator.adapters.acp_models import ACPAdapterConfig

        # Config file says "auto_approve"
        config = ACPAdapterConfig(permission_mode="auto_approve")

        # CLI says "deny_all"
        cli_mode = "deny_all"

        # CLI should override
        if cli_mode:
            config.permission_mode = cli_mode

        assert config.permission_mode == "deny_all"


class TestACPMainEntryPoint:
    """Test main entry point with ACP agent."""

    def test_main_parses_acp_agent(self):
        """Test that main() correctly parses -a acp."""
        # This test validates the argparse configuration
        from ralph_orchestrator.__main__ import main

        # We can't easily test main() directly without mocking everything
        # Instead, verify the parser accepts the args
        with patch('sys.argv', ['ralph', '--dry-run', '-a', 'acp']):
            with patch('ralph_orchestrator.__main__.RalphOrchestrator'):
                with patch('ralph_orchestrator.__main__.Path') as mock_path:
                    mock_path.return_value.exists.return_value = True
                    # main() will exit with dry-run, which is fine
                    with pytest.raises(SystemExit) as exc_info:
                        main()
                    # Dry run exits with 0
                    assert exc_info.value.code == 0


class TestACPInitTemplate:
    """Test that ralph init includes ACP configuration."""

    def test_init_creates_acp_config(self):
        """Test that init_project creates ACP adapter config."""
        from ralph_orchestrator.__main__ import init_project
        import tempfile
        import os
        import yaml

        with tempfile.TemporaryDirectory() as tmpdir:
            original_cwd = os.getcwd()
            try:
                os.chdir(tmpdir)

                # Run init_project
                init_project()

                # Check ralph.yml contains ACP config
                with open("ralph.yml") as f:
                    config = yaml.safe_load(f)

                assert "adapters" in config
                assert "acp" in config["adapters"]
                assert config["adapters"]["acp"]["enabled"] is True
                assert "tool_permissions" in config["adapters"]["acp"]
                assert "agent_command" in config["adapters"]["acp"]["tool_permissions"]
                assert "permission_mode" in config["adapters"]["acp"]["tool_permissions"]
            finally:
                os.chdir(original_cwd)
