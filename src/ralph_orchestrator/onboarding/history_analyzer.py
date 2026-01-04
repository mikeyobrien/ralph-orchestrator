"""HistoryAnalyzer - Parses Claude Code JSONL conversation history.

This module provides static analysis of Claude Code conversation history by
parsing JSONL files directly. This is the offline fallback when MCP servers
are not available (--static mode).

The architecture is SDK-ready: when Anthropic releases proper APIs for
conversation history access, this class can be updated to use them while
maintaining the same interface.
"""

import json
import logging
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Set

logger = logging.getLogger(__name__)


@dataclass
class ToolUsageStats:
    """Statistics for a single tool's usage.

    Attributes:
        name: Tool name (e.g., "Read", "Edit", "Bash")
        count: Total number of times the tool was used
        success_count: Number of successful uses
        failure_count: Number of failed uses
    """

    name: str
    count: int = 0
    success_count: int = 0
    failure_count: int = 0

    @property
    def success_rate(self) -> float:
        """Calculate success rate as a float between 0 and 1."""
        if self.count == 0:
            return 0.0
        return self.success_count / self.count


@dataclass
class MCPServerStats:
    """Statistics for MCP server usage.

    Attributes:
        server_name: Name of the MCP server (e.g., "github", "memory")
        tool_count: Total number of tool calls to this server
        tools_used: Set of unique tool names called on this server
    """

    server_name: str
    tool_count: int = 0
    tools_used: Set[str] = field(default_factory=set)


@dataclass
class ToolChain:
    """A sequence of tools commonly used together.

    Attributes:
        tools: List of tool names in order
        count: Number of times this sequence was observed
    """

    tools: List[str]
    count: int = 1


@dataclass
class Conversation:
    """Represents a parsed conversation from a JSONL file.

    Attributes:
        source_file: Path to the source JSONL file
        messages: List of message dictionaries from the conversation
    """

    source_file: Path
    messages: List[Dict[str, Any]] = field(default_factory=list)


class HistoryAnalyzer:
    """Parses Claude Code conversation history from JSONL files.

    This is the static analysis backend for onboarding, used when MCP servers
    are not available or when --static mode is requested. It parses JSONL
    conversation files to extract:
    - Tool usage frequency and success rates
    - MCP server invocations
    - Tool chains (sequences of tools used together)
    - Full conversation data for deeper analysis

    Attributes:
        files: List of paths to JSONL conversation files
    """

    def __init__(self, jsonl_files: List[Path]):
        """Initialize HistoryAnalyzer with JSONL file paths.

        Args:
            jsonl_files: List of paths to JSONL conversation files
        """
        self.files = jsonl_files

    def _parse_jsonl_file(self, file_path: Path) -> List[Dict[str, Any]]:
        """Parse a JSONL file into a list of messages.

        Args:
            file_path: Path to the JSONL file

        Returns:
            List of message dictionaries
        """
        messages = []

        if not file_path.exists():
            logger.debug(f"File does not exist: {file_path}")
            return messages

        try:
            with open(file_path, "r", encoding="utf-8") as f:
                for line_num, line in enumerate(f, 1):
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        msg = json.loads(line)
                        messages.append(msg)
                    except json.JSONDecodeError as e:
                        logger.debug(f"Skipping malformed JSON at {file_path}:{line_num}: {e}")
                        continue
        except Exception as e:
            logger.warning(f"Error reading file {file_path}: {e}")

        return messages

    def _extract_tool_uses_and_results(
        self, messages: List[Dict[str, Any]]
    ) -> List[Dict[str, Any]]:
        """Extract tool_use and tool_result pairs from messages.

        Returns a list of dicts with keys:
        - tool_name: Name of the tool
        - tool_use_id: ID linking use to result
        - is_error: Whether the result was an error
        """
        tool_uses = {}  # tool_use_id -> tool_name
        tool_results = []

        for msg in messages:
            msg_type = msg.get("type")
            content = msg.get("content", [])

            # Handle content that may be a string or list
            if isinstance(content, str):
                continue

            for item in content:
                if not isinstance(item, dict):
                    continue

                item_type = item.get("type")

                if item_type == "tool_use":
                    tool_name = item.get("name", "unknown")
                    tool_id = item.get("id")
                    if tool_id:
                        tool_uses[tool_id] = tool_name

                elif item_type == "tool_result":
                    tool_id = item.get("tool_use_id")
                    is_error = item.get("is_error", False)
                    if tool_id and tool_id in tool_uses:
                        tool_results.append({
                            "tool_name": tool_uses[tool_id],
                            "tool_use_id": tool_id,
                            "is_error": is_error,
                        })

        return tool_results

    def extract_tool_usage(self) -> Dict[str, ToolUsageStats]:
        """Extract tool usage frequency and success rates.

        Returns:
            Dictionary mapping tool names to their usage statistics.
        """
        if not self.files:
            return {}

        stats: Dict[str, ToolUsageStats] = {}

        for file_path in self.files:
            messages = self._parse_jsonl_file(file_path)
            tool_results = self._extract_tool_uses_and_results(messages)

            for result in tool_results:
                tool_name = result["tool_name"]

                if tool_name not in stats:
                    stats[tool_name] = ToolUsageStats(name=tool_name)

                stats[tool_name].count += 1
                if result["is_error"]:
                    stats[tool_name].failure_count += 1
                else:
                    stats[tool_name].success_count += 1

        return stats

    def extract_mcp_usage(self) -> Dict[str, MCPServerStats]:
        """Extract MCP server usage patterns.

        Identifies tools that are MCP server calls (prefixed with mcp_) and
        aggregates usage by server name.

        Returns:
            Dictionary mapping server names to their usage statistics.
        """
        if not self.files:
            return {}

        stats: Dict[str, MCPServerStats] = {}

        for file_path in self.files:
            messages = self._parse_jsonl_file(file_path)
            tool_results = self._extract_tool_uses_and_results(messages)

            for result in tool_results:
                tool_name = result["tool_name"]

                # Only process MCP tools (prefixed with mcp_)
                if not tool_name.startswith("mcp_"):
                    continue

                # Extract server name: mcp_<server>_<action> -> server
                parts = tool_name.split("_")
                if len(parts) >= 2:
                    server_name = parts[1]
                else:
                    server_name = "unknown"

                if server_name not in stats:
                    stats[server_name] = MCPServerStats(server_name=server_name)

                stats[server_name].tool_count += 1
                stats[server_name].tools_used.add(tool_name)

        return stats

    def extract_tool_chains(self) -> List[ToolChain]:
        """Identify sequences of tools commonly used together.

        Analyzes conversations to find patterns of tools used in sequence.

        Returns:
            List of ToolChain objects representing common sequences.
        """
        if not self.files:
            return []

        chains: List[ToolChain] = []

        for file_path in self.files:
            messages = self._parse_jsonl_file(file_path)

            # Extract sequence of tools from this conversation
            tool_sequence = []
            for msg in messages:
                content = msg.get("content", [])
                if isinstance(content, str):
                    continue

                for item in content:
                    if isinstance(item, dict) and item.get("type") == "tool_use":
                        tool_name = item.get("name")
                        if tool_name:
                            tool_sequence.append(tool_name)

            # If we have a sequence, add it as a chain
            if tool_sequence:
                chains.append(ToolChain(tools=tool_sequence, count=1))

        return chains

    def extract_conversations(self) -> List[Conversation]:
        """Parse full conversations for deeper analysis.

        Returns:
            List of Conversation objects, one per JSONL file.
        """
        conversations = []

        for file_path in self.files:
            messages = self._parse_jsonl_file(file_path)
            if messages:  # Only include non-empty conversations
                conversations.append(Conversation(
                    source_file=file_path,
                    messages=messages,
                ))

        return conversations
