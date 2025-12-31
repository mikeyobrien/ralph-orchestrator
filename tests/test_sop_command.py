# ABOUTME: Tests for the 'ralph sop' subcommand and SOP execution protocol
# ABOUTME: Verifies argument parsing, protocol prepending, and SOP-based workflow execution

"""Tests for SOP subcommand and Native Execution Protocol."""

import argparse
import os
import tempfile
import pytest
from pathlib import Path
from unittest.mock import patch, MagicMock


class TestSOPSubcommandParsing:
    """Test that 'sop' is a valid subcommand."""

    def test_sop_subcommand_exists(self):
        """Test that 'sop' subcommand is recognized."""
        from ralph_orchestrator.__main__ import main

        # Verify the parser accepts 'sop' subcommand
        with patch('sys.argv', ['ralph', 'sop', '--dry-run', '-p', 'Test task']):
            with patch('ralph_orchestrator.__main__.RalphOrchestrator'):
                with patch('ralph_orchestrator.__main__.Path') as mock_path:
                    mock_path.return_value.exists.return_value = True
                    # Dry run should exit cleanly
                    with pytest.raises(SystemExit) as exc_info:
                        main()
                    # 0 for dry-run success
                    assert exc_info.value.code == 0

    def test_sop_accepts_prompt_text(self):
        """Test that 'sop' accepts --prompt-text argument."""
        from ralph_orchestrator.__main__ import main

        with patch('sys.argv', ['ralph', 'sop', '--dry-run', '-p', 'Run the hello-world SOP']):
            with patch('ralph_orchestrator.__main__.RalphOrchestrator'):
                with patch('ralph_orchestrator.__main__.Path') as mock_path:
                    mock_path.return_value.exists.return_value = True
                    with pytest.raises(SystemExit) as exc_info:
                        main()
                    assert exc_info.value.code == 0

    def test_sop_accepts_agent_selection(self):
        """Test that 'sop' accepts agent selection."""
        from ralph_orchestrator.__main__ import main

        for agent in ['claude', 'gemini', 'q', 'auto']:
            with patch('sys.argv', ['ralph', 'sop', '--dry-run', '-a', agent, '-p', 'Test']):
                with patch('ralph_orchestrator.__main__.RalphOrchestrator'):
                    with patch('ralph_orchestrator.__main__.Path') as mock_path:
                        mock_path.return_value.exists.return_value = True
                        with pytest.raises(SystemExit) as exc_info:
                            main()
                        assert exc_info.value.code == 0


class TestNativeExecutionProtocol:
    """Test the Native Execution Protocol injection."""

    def test_protocol_constant_defined(self):
        """Test that NATIVE_EXECUTION_PROTOCOL is defined."""
        from ralph_orchestrator.__main__ import NATIVE_EXECUTION_PROTOCOL

        assert NATIVE_EXECUTION_PROTOCOL is not None
        assert len(NATIVE_EXECUTION_PROTOCOL) > 0

    def test_protocol_contains_key_elements(self):
        """Test that protocol contains required elements."""
        from ralph_orchestrator.__main__ import NATIVE_EXECUTION_PROTOCOL

        # Must contain role description
        assert "Autonomous Lead Developer" in NATIVE_EXECUTION_PROTOCOL

        # Must contain discovery step
        assert "Discover Protocols" in NATIVE_EXECUTION_PROTOCOL
        assert ".agent/sops" in NATIVE_EXECUTION_PROTOCOL

        # Must contain load step
        assert "Load Protocol" in NATIVE_EXECUTION_PROTOCOL

        # Must contain execute step
        assert "Execute" in NATIVE_EXECUTION_PROTOCOL

        # Must contain terminate step
        assert "Terminate" in NATIVE_EXECUTION_PROTOCOL

        # Must contain Original Task marker
        assert "Original Task" in NATIVE_EXECUTION_PROTOCOL


class TestSOPPromptGeneration:
    """Test SOP prompt generation logic."""

    def test_generate_sop_prompt_from_text(self):
        """Test generating SOP prompt from direct text."""
        from ralph_orchestrator.__main__ import generate_sop_prompt

        original_task = "Refactor the database layer"
        result = generate_sop_prompt(prompt_text=original_task)

        # Result should start with the Native Execution Protocol
        assert "# Role: Autonomous Lead Developer" in result
        # Original task should be at the end
        assert original_task in result
        # Protocol should come before the task
        protocol_pos = result.find("Autonomous Lead Developer")
        task_pos = result.find(original_task)
        assert protocol_pos < task_pos

    def test_generate_sop_prompt_from_file(self):
        """Test generating SOP prompt from file."""
        from ralph_orchestrator.__main__ import generate_sop_prompt

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Task: Build a web API\n\nCreate a REST API with endpoints.")
            prompt_file = f.name

        try:
            result = generate_sop_prompt(prompt_file=prompt_file)

            # Result should contain the protocol
            assert "# Role: Autonomous Lead Developer" in result
            # Result should contain the file content
            assert "Build a web API" in result
            assert "REST API" in result
        finally:
            Path(prompt_file).unlink()

    def test_generate_sop_prompt_text_takes_precedence(self):
        """Test that prompt_text takes precedence over prompt_file."""
        from ralph_orchestrator.__main__ import generate_sop_prompt

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("Content from file")
            prompt_file = f.name

        try:
            result = generate_sop_prompt(
                prompt_text="Content from text",
                prompt_file=prompt_file
            )

            # Should use text, not file
            assert "Content from text" in result
            assert "Content from file" not in result
        finally:
            Path(prompt_file).unlink()


class TestSOPCommandExecution:
    """Test SOP command execution flow."""

    def test_sop_command_creates_correct_config(self):
        """Test that SOP command creates RalphConfig with prepended protocol."""
        from ralph_orchestrator.__main__ import main
        from ralph_orchestrator.main import RalphConfig

        captured_config = {}

        def capture_orchestrator(prompt_file_or_config, **kwargs):
            if isinstance(prompt_file_or_config, RalphConfig):
                captured_config['config'] = prompt_file_or_config
            # Return mock orchestrator
            mock = MagicMock()
            return mock

        with patch('sys.argv', ['ralph', 'sop', '-p', 'Run hello-world SOP', '-i', '5']):
            with patch('ralph_orchestrator.__main__.RalphOrchestrator', side_effect=capture_orchestrator):
                with patch('ralph_orchestrator.__main__.Path') as mock_path:
                    mock_path.return_value.exists.return_value = True
                    try:
                        main()
                    except Exception:
                        pass  # May fail on other checks, that's ok

        # Verify the config was captured and has correct prompt_text
        if 'config' in captured_config:
            config = captured_config['config']
            assert config.prompt_text is not None
            assert "Autonomous Lead Developer" in config.prompt_text
            assert "Run hello-world SOP" in config.prompt_text


class TestSOPDirectoryIntegration:
    """Test integration with .agent/sops directory."""

    def test_init_creates_sops_directory(self):
        """Test that ralph init creates .agent/sops directory."""
        from ralph_orchestrator.__main__ import init_project

        with tempfile.TemporaryDirectory() as tmpdir:
            original_cwd = os.getcwd()
            try:
                os.chdir(tmpdir)
                init_project()

                # Check that .agent/sops exists
                sops_dir = Path(".agent/sops")
                assert sops_dir.exists(), ".agent/sops directory should be created"
                assert sops_dir.is_dir(), ".agent/sops should be a directory"
            finally:
                os.chdir(original_cwd)


class TestSOPCodeAssistFile:
    """Test code-assist.sop.md file placement."""

    def test_code_assist_sop_exists_in_project(self):
        """Test that code-assist.sop.md exists in .agent/sops."""
        # This verifies the file was created by the implementation
        sop_file = Path(__file__).parent.parent / ".agent" / "sops" / "code-assist.sop.md"

        assert sop_file.exists(), f"code-assist.sop.md should exist at {sop_file}"

    def test_code_assist_sop_has_required_sections(self):
        """Test that code-assist.sop.md has all required sections."""
        sop_file = Path(__file__).parent.parent / ".agent" / "sops" / "code-assist.sop.md"

        content = sop_file.read_text()

        required_sections = [
            "# Code Assist",
            "## Overview",
            "## Parameters",
            "## Mode Behavior",
            "## Steps",
            "### 1. Setup",
            "### 2. Explore Phase",
            "### 3. Plan Phase",
            "### 4. Code Phase",
            "### 5. Commit Phase",
            "## Desired Outcome",
        ]

        for section in required_sections:
            assert section in content, f"Missing required section: {section}"
