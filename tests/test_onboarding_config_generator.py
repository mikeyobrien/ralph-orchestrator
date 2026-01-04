"""Tests for ConfigGenerator.

ConfigGenerator creates RALPH configuration files (ralph.yml, RALPH_INSTRUCTIONS.md,
PROMPT.md template) from project analysis results including patterns, workflows,
tool usage, and MCP server configurations.
"""

import pytest
from pathlib import Path
from typing import Dict, List
from datetime import datetime

from ralph_orchestrator.onboarding.settings_loader import SettingsLoader
from ralph_orchestrator.onboarding.scanner import ProjectScanner, ProjectType
from ralph_orchestrator.onboarding.pattern_extractor import (
    PatternExtractor,
    ProjectPatterns,
    Workflow,
)
from ralph_orchestrator.onboarding.agent_analyzer import AnalysisResult
from ralph_orchestrator.onboarding.history_analyzer import HistoryAnalyzer
from ralph_orchestrator.onboarding.config_generator import ConfigGenerator


# =============================================================================
# ConfigGenerator Initialization Tests
# =============================================================================


class TestConfigGeneratorInit:
    """Tests for ConfigGenerator initialization."""

    def test_init_with_all_components(self, tmp_path: Path) -> None:
        """ConfigGenerator initializes with scanner, extractor, and settings."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(
            scanner=scanner,
            extractor=extractor,
            settings=settings,
        )

        assert generator.scanner == scanner
        assert generator.extractor == extractor
        assert generator.settings == settings

    def test_init_with_minimal_components(self, tmp_path: Path) -> None:
        """ConfigGenerator can be created with minimal configuration."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(
            scanner=scanner,
            extractor=extractor,
            settings=settings,
        )

        assert generator is not None


# =============================================================================
# ralph.yml Generation Tests
# =============================================================================


class TestGenerateRalphYml:
    """Tests for generate_ralph_yml method."""

    def test_generate_ralph_yml_basic(self, tmp_path: Path) -> None:
        """Generates valid ralph.yml content."""
        # Create package.json to detect nodejs project
        (tmp_path / "package.json").write_text('{"name": "test-app"}')

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        assert isinstance(yaml_content, str)
        assert "agent:" in yaml_content
        assert "prompt_file:" in yaml_content
        assert "max_iterations:" in yaml_content

    def test_generate_ralph_yml_includes_project_type(self, tmp_path: Path) -> None:
        """Includes detected project type as comment."""
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "test"')

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        # Should mention project type
        assert "python" in yaml_content.lower() or "Project:" in yaml_content

    def test_generate_ralph_yml_includes_mcp_servers(self, tmp_path: Path) -> None:
        """Includes MCP server configurations from settings."""
        # Create .mcp.json with servers
        mcp_config = tmp_path / ".mcp.json"
        mcp_config.write_text('{"mcpServers": {"github": {"command": "github-mcp"}}}')

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        # Should include MCP server info in comments or config
        assert "github" in yaml_content.lower() or "MCP" in yaml_content

    def test_generate_ralph_yml_includes_tool_stats(self, tmp_path: Path) -> None:
        """Includes tool success rates from patterns."""
        analysis = AnalysisResult(
            common_tools={"Edit": 0.95, "Read": 0.98, "Bash": 0.85},
        )
        extractor = PatternExtractor(history=None, analysis_result=analysis)
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        # Should include tool information
        assert "Edit" in yaml_content or "Read" in yaml_content or "tool" in yaml_content.lower()

    def test_generate_ralph_yml_has_sensible_defaults(self, tmp_path: Path) -> None:
        """Has sensible default values for unknown projects."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        # Should have default max_iterations
        assert "max_iterations:" in yaml_content
        # Should have prompt_file
        assert "prompt_file:" in yaml_content or "PROMPT.md" in yaml_content

    def test_generate_ralph_yml_auto_generated_header(self, tmp_path: Path) -> None:
        """Includes auto-generated header with metadata."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        # Should have auto-generated comment
        assert "Auto-generated" in yaml_content or "Generated" in yaml_content


# =============================================================================
# RALPH_INSTRUCTIONS.md Generation Tests
# =============================================================================


class TestGenerateInstructions:
    """Tests for generate_instructions method."""

    def test_generate_instructions_basic(self, tmp_path: Path) -> None:
        """Generates valid RALPH_INSTRUCTIONS.md content."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        instructions = generator.generate_instructions()

        assert isinstance(instructions, str)
        assert "RALPH" in instructions or "Instructions" in instructions

    def test_generate_instructions_includes_project_context(self, tmp_path: Path) -> None:
        """Includes project context from analysis."""
        (tmp_path / "package.json").write_text(
            '{"name": "my-expo-app", "dependencies": {"expo": "^50.0.0"}}'
        )

        analysis = AnalysisResult(
            project_type="expo",
            frameworks=["expo", "react-native"],
        )
        extractor = PatternExtractor(history=None, analysis_result=analysis)
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        generator = ConfigGenerator(scanner, extractor, settings)
        instructions = generator.generate_instructions()

        assert "expo" in instructions.lower() or "Expo" in instructions

    def test_generate_instructions_includes_workflows(self, tmp_path: Path) -> None:
        """Includes identified workflow patterns."""
        analysis = AnalysisResult(
            workflows=[["Read", "Edit", "Bash"], ["Grep", "Read", "Edit"]],
        )
        extractor = PatternExtractor(history=None, analysis_result=analysis)
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        generator = ConfigGenerator(scanner, extractor, settings)
        instructions = generator.generate_instructions()

        # Should mention workflows
        assert "workflow" in instructions.lower() or "pattern" in instructions.lower()

    def test_generate_instructions_includes_common_tools(self, tmp_path: Path) -> None:
        """Includes common tools and their success rates."""
        analysis = AnalysisResult(
            common_tools={"Edit": 0.95, "Read": 0.98},
        )
        extractor = PatternExtractor(history=None, analysis_result=analysis)
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        generator = ConfigGenerator(scanner, extractor, settings)
        instructions = generator.generate_instructions()

        assert "Edit" in instructions or "Read" in instructions or "tool" in instructions.lower()

    def test_generate_instructions_includes_mcp_servers(self, tmp_path: Path) -> None:
        """Includes used MCP servers."""
        mcp_config = tmp_path / ".mcp.json"
        mcp_config.write_text('{"mcpServers": {"memory": {"command": "mcp-memory"}}}')

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        instructions = generator.generate_instructions()

        # Should mention MCP or servers
        assert "MCP" in instructions or "server" in instructions.lower() or "memory" in instructions.lower()

    def test_generate_instructions_respects_claude_md(self, tmp_path: Path) -> None:
        """Incorporates content from CLAUDE.md if present."""
        (tmp_path / "CLAUDE.md").write_text(
            "# Project Guidelines\n\n- Use TypeScript strict mode\n- Test with Jest"
        )

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        instructions = generator.generate_instructions()

        # Should reference or incorporate CLAUDE.md content
        assert "TypeScript" in instructions or "Jest" in instructions or "CLAUDE.md" in instructions


# =============================================================================
# PROMPT.md Template Generation Tests
# =============================================================================


class TestGeneratePromptMd:
    """Tests for generate_prompt_md method."""

    def test_generate_prompt_md_basic(self, tmp_path: Path) -> None:
        """Generates valid PROMPT.md template."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        prompt = generator.generate_prompt_md()

        assert isinstance(prompt, str)
        assert "Task:" in prompt or "task" in prompt.lower()

    def test_generate_prompt_md_includes_project_context(self, tmp_path: Path) -> None:
        """Includes project context section."""
        (tmp_path / "pyproject.toml").write_text('[project]\nname = "my-api"')

        analysis = AnalysisResult(
            project_type="python",
            frameworks=["fastapi", "pytest"],
        )
        extractor = PatternExtractor(history=None, analysis_result=analysis)
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        generator = ConfigGenerator(scanner, extractor, settings)
        prompt = generator.generate_prompt_md()

        assert "python" in prompt.lower() or "fastapi" in prompt.lower()

    def test_generate_prompt_md_includes_available_tools(self, tmp_path: Path) -> None:
        """Includes available tools section."""
        analysis = AnalysisResult(
            common_tools={"Edit": 0.95, "Bash": 0.90},
        )
        extractor = PatternExtractor(history=None, analysis_result=analysis)
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        generator = ConfigGenerator(scanner, extractor, settings)
        prompt = generator.generate_prompt_md()

        assert "tool" in prompt.lower() or "Edit" in prompt or "Bash" in prompt

    def test_generate_prompt_md_has_requirements_section(self, tmp_path: Path) -> None:
        """Has placeholder for requirements."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        prompt = generator.generate_prompt_md()

        assert "Requirement" in prompt or "requirement" in prompt.lower() or "- [ ]" in prompt

    def test_generate_prompt_md_has_success_criteria(self, tmp_path: Path) -> None:
        """Has success criteria section."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        prompt = generator.generate_prompt_md()

        assert "Success" in prompt or "Complete" in prompt or "TASK_COMPLETE" in prompt


# =============================================================================
# MCP Server Inclusion Tests
# =============================================================================


class TestIncludeMcpServers:
    """Tests for MCP server configuration inclusion."""

    def test_include_mcp_servers_from_user_settings(self, tmp_path: Path, monkeypatch) -> None:
        """Includes MCP servers from user settings."""
        # Create fake home with settings
        fake_home = tmp_path / "home"
        fake_home.mkdir()
        claude_dir = fake_home / ".claude"
        claude_dir.mkdir()
        (claude_dir / "settings.json").write_text(
            '{"mcpServers": {"github": {"command": "gh-mcp"}, "filesystem": {"command": "fs-mcp"}}}'
        )
        monkeypatch.setattr(Path, "home", lambda: fake_home)

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        mcp_servers = generator._get_mcp_servers_for_config()

        assert isinstance(mcp_servers, dict)
        assert "github" in mcp_servers or "filesystem" in mcp_servers

    def test_include_mcp_servers_from_project_config(self, tmp_path: Path) -> None:
        """Includes MCP servers from project .mcp.json."""
        mcp_config = tmp_path / ".mcp.json"
        mcp_config.write_text(
            '{"mcpServers": {"project-server": {"command": "custom-mcp"}}}'
        )

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        mcp_servers = generator._get_mcp_servers_for_config()

        assert "project-server" in mcp_servers

    def test_project_mcp_overrides_user_mcp(self, tmp_path: Path, monkeypatch) -> None:
        """Project MCP config overrides user MCP config."""
        # User settings with github server
        fake_home = tmp_path / "home"
        fake_home.mkdir()
        claude_dir = fake_home / ".claude"
        claude_dir.mkdir()
        (claude_dir / "settings.json").write_text(
            '{"mcpServers": {"github": {"command": "user-gh-mcp"}}}'
        )
        monkeypatch.setattr(Path, "home", lambda: fake_home)

        # Project config overrides
        mcp_config = tmp_path / ".mcp.json"
        mcp_config.write_text(
            '{"mcpServers": {"github": {"command": "project-gh-mcp"}}}'
        )

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        mcp_servers = generator._get_mcp_servers_for_config()

        assert mcp_servers.get("github", {}).get("command") == "project-gh-mcp"


# =============================================================================
# Tool Permissions Inclusion Tests
# =============================================================================


class TestToolPermissions:
    """Tests for tool permission configuration."""

    def test_include_tool_permissions(self, tmp_path: Path, monkeypatch) -> None:
        """Includes tool permissions from settings."""
        fake_home = tmp_path / "home"
        fake_home.mkdir()
        claude_dir = fake_home / ".claude"
        claude_dir.mkdir()
        (claude_dir / "settings.json").write_text(
            '{"permissions": {"allow": ["Read", "Write", "Edit"], "deny": ["Bash"]}}'
        )
        monkeypatch.setattr(Path, "home", lambda: fake_home)

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        permissions = generator._get_tool_permissions()

        assert "allow" in permissions or "deny" in permissions


# =============================================================================
# Edge Cases and Error Handling
# =============================================================================


class TestConfigGeneratorEdgeCases:
    """Tests for edge cases and error handling."""

    def test_handles_empty_project(self, tmp_path: Path) -> None:
        """Handles empty project directory gracefully."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)

        # Should not raise, should generate defaults
        yaml_content = generator.generate_ralph_yml()
        instructions = generator.generate_instructions()
        prompt = generator.generate_prompt_md()

        assert isinstance(yaml_content, str)
        assert isinstance(instructions, str)
        assert isinstance(prompt, str)

    def test_handles_missing_settings(self, tmp_path: Path) -> None:
        """Handles missing settings files gracefully."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        assert "agent:" in yaml_content

    def test_handles_analysis_result_with_patterns(self, tmp_path: Path) -> None:
        """Works with full AnalysisResult from agent analysis."""
        analysis = AnalysisResult(
            project_type="expo",
            frameworks=["expo", "react-native", "typescript"],
            common_tools={"Edit": 0.95, "Read": 0.98, "Bash": 0.85},
            workflows=[["Read", "Edit", "Bash", "Bash"]],
            recommended_config={"max_iterations": 75, "max_tokens": 500000},
        )
        extractor = PatternExtractor(history=None, analysis_result=analysis)
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        # Should include recommended config values
        assert "max_iterations" in yaml_content

    def test_preserves_special_characters_in_yaml(self, tmp_path: Path) -> None:
        """Properly handles special characters in YAML output."""
        # Create project with special characters in name
        (tmp_path / "package.json").write_text(
            '{"name": "my-app: test & example", "version": "1.0.0"}'
        )

        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        yaml_content = generator.generate_ralph_yml()

        # Should be valid YAML (no unescaped special chars)
        assert isinstance(yaml_content, str)
        # The content should not break YAML parsing
        import yaml
        try:
            parsed = yaml.safe_load(yaml_content)
            assert parsed is not None
        except yaml.YAMLError:
            pytest.fail("Generated YAML is not valid")


# =============================================================================
# Full Output File Tests
# =============================================================================


class TestWriteConfigFiles:
    """Tests for writing configuration files to disk."""

    def test_write_ralph_yml(self, tmp_path: Path) -> None:
        """Can write ralph.yml to disk."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        output_path = tmp_path / "ralph.yml"
        generator.write_ralph_yml(output_path)

        assert output_path.exists()
        content = output_path.read_text()
        assert "agent:" in content

    def test_write_instructions(self, tmp_path: Path) -> None:
        """Can write RALPH_INSTRUCTIONS.md to disk."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        output_path = tmp_path / "RALPH_INSTRUCTIONS.md"
        generator.write_instructions(output_path)

        assert output_path.exists()
        content = output_path.read_text()
        assert len(content) > 0

    def test_write_prompt_md(self, tmp_path: Path) -> None:
        """Can write PROMPT.md to disk."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        output_path = tmp_path / "PROMPT.md"
        generator.write_prompt_md(output_path)

        assert output_path.exists()
        content = output_path.read_text()
        assert "Task" in content or "task" in content.lower()

    def test_write_all_files(self, tmp_path: Path) -> None:
        """Can write all config files at once."""
        settings = SettingsLoader(tmp_path)
        scanner = ProjectScanner(tmp_path, settings)
        extractor = PatternExtractor(history=None)

        generator = ConfigGenerator(scanner, extractor, settings)
        generator.write_all(output_dir=tmp_path)

        assert (tmp_path / "ralph.yml").exists()
        assert (tmp_path / "RALPH_INSTRUCTIONS.md").exists()
        assert (tmp_path / "PROMPT.md").exists()
