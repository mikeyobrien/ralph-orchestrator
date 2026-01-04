"""AgentAnalyzer - Uses Claude with user's MCP servers for intelligent analysis.

This module provides intelligent project analysis by running Claude with the user's
inherited MCP servers (including memory plugins like mcp-memory, fogmap, etc.)
to extract patterns, workflows, and configuration recommendations.
"""

import asyncio
import json
import logging
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

from ralph_orchestrator.adapters.claude import ClaudeAdapter
from ralph_orchestrator.onboarding.settings_loader import SettingsLoader

logger = logging.getLogger(__name__)


@dataclass
class AnalysisResult:
    """Result of project analysis containing extracted patterns and recommendations.

    Attributes:
        project_type: Detected project type (python, nodejs, expo, etc.)
        frameworks: List of detected frameworks
        common_tools: Dict mapping tool names to their success rates
        workflows: List of workflow sequences (tool chains)
        recommended_config: Dict of recommended ralph.yml settings
        raw_response: The raw response from Claude for debugging
    """

    project_type: str = "unknown"
    frameworks: List[str] = field(default_factory=list)
    common_tools: Dict[str, float] = field(default_factory=dict)
    workflows: List[List[str]] = field(default_factory=list)
    recommended_config: Dict[str, Any] = field(default_factory=dict)
    raw_response: str = ""


class AgentAnalyzer:
    """Uses Claude with user's MCP servers for intelligent project analysis.

    This analyzer runs Claude with the user's inherited MCP servers to perform
    intelligent analysis of the project. When memory plugins (mcp-memory, fogmap,
    etc.) are available, Claude can use episodic memory to recall past patterns.

    Attributes:
        project_path: Path to the project directory
        settings: SettingsLoader instance for loading settings
        inherit_user_settings: Whether to inherit user's Claude Code settings
    """

    def __init__(
        self,
        project_path: Path | str,
        settings: Optional[SettingsLoader] = None,
        inherit_user_settings: bool = True,
    ):
        """Initialize AgentAnalyzer with a project path.

        Args:
            project_path: Path to the project directory
            settings: Optional SettingsLoader instance. Created if not provided.
            inherit_user_settings: Whether to inherit user's Claude Code settings
        """
        self.project_path = Path(project_path).resolve()
        self.settings = settings or SettingsLoader(self.project_path)
        self.inherit_user_settings = inherit_user_settings

    def _build_analysis_prompt(self) -> str:
        """Build the prompt for Claude to analyze the project.

        Returns:
            String prompt requesting structured analysis.
        """
        has_memory = self.settings.has_memory_plugin()

        memory_instructions = ""
        if has_memory:
            memory_instructions = """
You have access to memory plugins. Use your episodic memory to recall:
- Past conversations and patterns from work on this project
- Successful workflows and tool combinations
- User preferences and coding style
"""

        prompt = f"""Analyze this project to generate RALPH Orchestrator configuration.

Project path: {self.project_path}
{memory_instructions}
Please analyze:
1. Project structure and type (framework, language)
2. Common tools and commands used
3. Successful workflow patterns
4. MCP servers that are most useful

Return your analysis as a JSON object with this structure:
```json
{{
    "project_type": "string (python, nodejs, expo, rust, go, etc.)",
    "frameworks": ["list", "of", "frameworks"],
    "common_tools": {{"tool_name": success_rate_0_to_1}},
    "workflows": [["step1", "step2", "step3"]],
    "recommended_config": {{
        "max_iterations": 50,
        "max_tokens": 500000
    }}
}}
```

Analyze the project structure and provide your structured JSON response."""

        return prompt

    def _parse_analysis_result(self, response: str) -> AnalysisResult:
        """Parse Claude's response into an AnalysisResult.

        Handles both raw JSON and JSON embedded in markdown code blocks.

        Args:
            response: Raw response string from Claude

        Returns:
            AnalysisResult with parsed data or defaults if parsing fails.
        """
        result = AnalysisResult(raw_response=response)

        # Try to extract JSON from markdown code block
        json_match = re.search(r"```(?:json)?\s*\n?(.*?)\n?```", response, re.DOTALL)
        if json_match:
            json_str = json_match.group(1).strip()
        else:
            # Try the whole response as JSON
            json_str = response.strip()

        try:
            data = json.loads(json_str)

            result.project_type = data.get("project_type", "unknown")
            result.frameworks = data.get("frameworks", [])
            result.common_tools = data.get("common_tools", {})
            result.workflows = data.get("workflows", [])
            result.recommended_config = data.get("recommended_config", {})

        except json.JSONDecodeError as e:
            logger.warning(f"Failed to parse JSON from response: {e}")
            # Return result with defaults but raw_response preserved

        return result

    def _get_adapter(self) -> ClaudeAdapter:
        """Get a configured ClaudeAdapter instance.

        Returns:
            ClaudeAdapter configured with inherit_user_settings.
        """
        return ClaudeAdapter(inherit_user_settings=self.inherit_user_settings)

    def _configure_adapter(self, adapter: ClaudeAdapter) -> None:
        """Configure the adapter for analysis.

        Args:
            adapter: ClaudeAdapter instance to configure
        """
        adapter.configure(
            enable_all_tools=True,
            inherit_user_settings=self.inherit_user_settings,
        )

    async def analyze(self) -> AnalysisResult:
        """Run Claude to analyze the project using available tools.

        Returns:
            AnalysisResult containing the analysis data.
        """
        adapter = self._get_adapter()
        self._configure_adapter(adapter)

        prompt = self._build_analysis_prompt()

        try:
            response = await adapter.aexecute(prompt)

            if not response.success:
                logger.warning(f"Claude execution failed: {response.error}")
                return AnalysisResult()

            return self._parse_analysis_result(response.output)

        except Exception as e:
            logger.error(f"Analysis failed: {e}")
            return AnalysisResult()

    def sync_analyze(self) -> AnalysisResult:
        """Synchronous wrapper for analyze().

        Returns:
            AnalysisResult containing the analysis data.
        """
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            return loop.run_until_complete(self.analyze())
        else:
            # If loop is already running, create task
            import concurrent.futures

            with concurrent.futures.ThreadPoolExecutor() as executor:
                future = executor.submit(asyncio.run, self.analyze())
                return future.result()
