"""ConfigGenerator - Creates RALPH configuration from analysis.

This module generates RALPH configuration files (ralph.yml, RALPH_INSTRUCTIONS.md,
PROMPT.md template) from project analysis results including patterns, workflows,
tool usage statistics, and MCP server configurations.
"""

import logging
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

import yaml

from ralph_orchestrator.onboarding.pattern_extractor import PatternExtractor, ProjectPatterns
from ralph_orchestrator.onboarding.scanner import ProjectScanner, ProjectType
from ralph_orchestrator.onboarding.settings_loader import SettingsLoader

logger = logging.getLogger(__name__)


class ConfigGenerator:
    """Generates RALPH configuration from project analysis.

    Creates ralph.yml, RALPH_INSTRUCTIONS.md, and PROMPT.md template based on:
    - Project type detection (via ProjectScanner)
    - Pattern extraction (via PatternExtractor)
    - MCP server and permission settings (via SettingsLoader)

    Attributes:
        scanner: ProjectScanner for detecting project type and structure
        extractor: PatternExtractor for workflow and tool patterns
        settings: SettingsLoader for MCP and permission settings
    """

    def __init__(
        self,
        scanner: ProjectScanner,
        extractor: PatternExtractor,
        settings: SettingsLoader,
    ):
        """Initialize ConfigGenerator with analysis components.

        Args:
            scanner: ProjectScanner for project detection
            extractor: PatternExtractor for pattern analysis
            settings: SettingsLoader for configuration settings
        """
        self.scanner = scanner
        self.extractor = extractor
        self.settings = settings

    def _get_project_name(self) -> str:
        """Get the project name from the project path.

        Returns:
            Project directory name as the project name.
        """
        return self.scanner.project_path.name

    def _get_project_type_string(self) -> str:
        """Get human-readable project type string.

        Returns:
            Project type as a string (e.g., 'Python', 'Node.js', 'Expo').
        """
        project_type = self.scanner.detect_project_type()
        type_names = {
            ProjectType.PYTHON: "Python",
            ProjectType.NODEJS: "Node.js",
            ProjectType.EXPO: "Expo/React Native",
            ProjectType.REACT: "React",
            ProjectType.RUST: "Rust",
            ProjectType.GO: "Go",
            ProjectType.FLUTTER: "Flutter/Dart",
            ProjectType.UNKNOWN: "Unknown",
        }
        return type_names.get(project_type, "Unknown")

    def _get_mcp_servers_for_config(self) -> Dict[str, Dict[str, Any]]:
        """Get MCP servers to include in configuration.

        Returns:
            Dictionary mapping server names to their configurations.
        """
        return self.settings.get_mcp_servers()

    def _get_tool_permissions(self) -> Dict[str, Any]:
        """Get tool permissions from settings.

        Returns:
            Dictionary with 'allow' and 'deny' lists for tool permissions.
        """
        return self.settings.get_permissions()

    def _get_patterns(self) -> ProjectPatterns:
        """Get extracted project patterns.

        Returns:
            ProjectPatterns containing workflows, tools, and MCP servers.
        """
        return self.extractor.identify_project_patterns()

    def _build_config_dict(self) -> Dict[str, Any]:
        """Build the configuration dictionary for ralph.yml.

        Returns:
            Dictionary representing the ralph.yml configuration.
        """
        patterns = self._get_patterns()
        analysis_result = self.extractor.analysis_result

        # Start with defaults
        config: Dict[str, Any] = {
            "agent": "claude",
            "prompt_file": "PROMPT.md",
            "max_iterations": 50,
        }

        # Add recommended config from analysis if available
        if analysis_result and analysis_result.recommended_config:
            rec = analysis_result.recommended_config
            if "max_iterations" in rec:
                config["max_iterations"] = rec["max_iterations"]
            if "max_tokens" in rec:
                config["max_tokens"] = rec["max_tokens"]
            if "max_cost" in rec:
                config["max_cost"] = rec["max_cost"]

        # Add standard features
        config["archive_prompts"] = True
        config["git_checkpoint"] = True
        config["verbose"] = False

        return config

    def generate_ralph_yml(self) -> str:
        """Generate optimized ralph.yml content.

        Creates a YAML configuration file with:
        - Auto-generated header with metadata
        - Agent configuration
        - Resource limits (from analysis or defaults)
        - Feature flags
        - MCP server and tool comments

        Returns:
            String content of ralph.yml.
        """
        config = self._build_config_dict()
        patterns = self._get_patterns()
        mcp_servers = self._get_mcp_servers_for_config()
        project_type = self._get_project_type_string()
        project_name = self._get_project_name()

        # Build header comments
        lines = [
            "# Auto-generated by: ralph onboard",
            f"# Project: {project_name}",
            f"# Type: {project_type}",
            f"# Generated: {datetime.now().strftime('%Y-%m-%d')}",
            "#",
        ]

        # Add MCP server info as comments
        if mcp_servers:
            lines.append("# MCP Servers configured:")
            for server_name in mcp_servers:
                lines.append(f"#   - {server_name}")
            lines.append("#")

        # Add tool success rates as comments
        if patterns.tool_success_rates:
            lines.append("# Tool success rates (from history):")
            sorted_tools = sorted(
                patterns.tool_success_rates.items(),
                key=lambda x: x[1],
                reverse=True,
            )[:5]  # Top 5 tools
            for tool_name, rate in sorted_tools:
                lines.append(f"#   - {tool_name}: {rate:.0%}")
            lines.append("#")

        lines.append("")

        # Serialize config to YAML
        yaml_content = yaml.dump(config, default_flow_style=False, sort_keys=False)

        return "\n".join(lines) + yaml_content

    def generate_instructions(self) -> str:
        """Generate RALPH_INSTRUCTIONS.md from learned patterns.

        Creates markdown documentation with:
        - Project context (type, frameworks)
        - Proven workflows from history
        - Common tools and their success rates
        - MCP server recommendations
        - Content from CLAUDE.md if present

        Returns:
            String content of RALPH_INSTRUCTIONS.md.
        """
        lines: List[str] = []
        patterns = self._get_patterns()
        analysis_result = self.extractor.analysis_result
        project_name = self._get_project_name()
        project_type = self._get_project_type_string()
        mcp_servers = self._get_mcp_servers_for_config()

        # Header
        lines.append(f"# RALPH Instructions for {project_name}")
        lines.append("")

        # Project Context
        lines.append("## Project Context")
        lines.append(f"This is a {project_type} project.")
        lines.append("")

        # Add frameworks if detected
        if analysis_result and analysis_result.frameworks:
            lines.append("### Frameworks & Technologies")
            for fw in analysis_result.frameworks:
                lines.append(f"- {fw}")
            lines.append("")

        # Proven Workflows
        if patterns.workflows:
            lines.append("## Proven Workflows")
            lines.append("")
            for workflow in patterns.workflows[:5]:  # Limit to top 5
                lines.append(f"### {workflow.name}")
                if workflow.description:
                    lines.append(workflow.description)
                lines.append(f"Steps: {' -> '.join(workflow.steps[:5])}")
                if workflow.count > 1:
                    lines.append(f"(Observed {workflow.count} times)")
                lines.append("")

        # Common Tools
        if patterns.successful_tools:
            lines.append("## Common Tools")
            lines.append("These tools have high success rates for this project:")
            lines.append("")
            for tool in patterns.successful_tools[:10]:
                rate = patterns.tool_success_rates.get(tool, 0.0)
                if rate > 0:
                    lines.append(f"- **{tool}** ({rate:.0%} success rate)")
                else:
                    lines.append(f"- **{tool}**")
            lines.append("")

        # MCP Servers
        if mcp_servers:
            lines.append("## MCP Servers")
            lines.append("These MCP servers are configured:")
            lines.append("")
            for server_name, config in mcp_servers.items():
                command = config.get("command", "unknown")
                lines.append(f"- **{server_name}**: `{command}`")
            lines.append("")

        # Include CLAUDE.md content if present
        claude_md_files = self.scanner.find_claude_md_files()
        if claude_md_files:
            lines.append("## Project Guidelines")
            lines.append("*(From CLAUDE.md)*")
            lines.append("")
            for claude_file in claude_md_files[:1]:  # Just first file
                try:
                    content = claude_file.read_text()
                    # Extract key points (simplified)
                    for line in content.split("\n")[:20]:  # First 20 lines
                        if line.strip() and not line.startswith("#"):
                            lines.append(line)
                except Exception as e:
                    logger.debug(f"Failed to read {claude_file}: {e}")
            lines.append("")

        # Default content if nothing was generated
        if len(lines) <= 5:
            lines.append("## Getting Started")
            lines.append("No patterns extracted yet. Build up history by using Claude Code.")
            lines.append("")

        return "\n".join(lines)

    def generate_prompt_md(self) -> str:
        """Generate initial PROMPT.md template with context.

        Creates a template with:
        - Task placeholder
        - Project context (auto-detected)
        - Available tools section
        - Requirements placeholders
        - Success criteria

        Returns:
            String content of PROMPT.md template.
        """
        lines: List[str] = []
        patterns = self._get_patterns()
        analysis_result = self.extractor.analysis_result
        project_type = self._get_project_type_string()

        # Task section
        lines.append("# Task: [Your task description]")
        lines.append("")

        # Project Context
        lines.append("## Project Context")
        lines.append("<!-- Auto-detected by onboarding -->")
        lines.append(f"- **Type**: {project_type}")

        if analysis_result and analysis_result.frameworks:
            frameworks_str = ", ".join(analysis_result.frameworks[:5])
            lines.append(f"- **Key Frameworks**: {frameworks_str}")
        lines.append("")

        # Available Tools
        lines.append("## Available Tools")
        if patterns.successful_tools:
            lines.append("Based on your project's history, these tools are most effective:")
            tools_list = ", ".join(patterns.successful_tools[:5])
            lines.append(f"- File operations: {tools_list}")
        else:
            lines.append("Standard Claude Code tools are available:")
            lines.append("- File editing: Edit, Write, Read")
            lines.append("- Commands: Bash, Glob, Grep")

        mcp_servers = self._get_mcp_servers_for_config()
        if mcp_servers:
            server_list = ", ".join(mcp_servers.keys())
            lines.append(f"- MCP: {server_list}")
        lines.append("")

        # Requirements
        lines.append("## Requirements")
        lines.append("- [ ] Requirement 1")
        lines.append("- [ ] Requirement 2")
        lines.append("- [ ] Requirement 3")
        lines.append("")

        # Success Criteria
        lines.append("## Success Criteria")
        lines.append("- [ ] All requirements implemented")
        lines.append("- [ ] Tests pass")
        lines.append("- [ ] TASK_COMPLETE when all requirements met")
        lines.append("")

        return "\n".join(lines)

    def write_ralph_yml(self, output_path: Path) -> None:
        """Write ralph.yml to disk.

        Args:
            output_path: Path to write the file to.
        """
        content = self.generate_ralph_yml()
        output_path.write_text(content)
        logger.info(f"Wrote ralph.yml to {output_path}")

    def write_instructions(self, output_path: Path) -> None:
        """Write RALPH_INSTRUCTIONS.md to disk.

        Args:
            output_path: Path to write the file to.
        """
        content = self.generate_instructions()
        output_path.write_text(content)
        logger.info(f"Wrote RALPH_INSTRUCTIONS.md to {output_path}")

    def write_prompt_md(self, output_path: Path) -> None:
        """Write PROMPT.md to disk.

        Args:
            output_path: Path to write the file to.
        """
        content = self.generate_prompt_md()
        output_path.write_text(content)
        logger.info(f"Wrote PROMPT.md to {output_path}")

    def write_all(self, output_dir: Path) -> None:
        """Write all configuration files to a directory.

        Args:
            output_dir: Directory to write files to.
        """
        output_dir = Path(output_dir)
        output_dir.mkdir(parents=True, exist_ok=True)

        self.write_ralph_yml(output_dir / "ralph.yml")
        self.write_instructions(output_dir / "RALPH_INSTRUCTIONS.md")
        self.write_prompt_md(output_dir / "PROMPT.md")

        logger.info(f"Wrote all config files to {output_dir}")
