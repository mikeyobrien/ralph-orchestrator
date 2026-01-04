"""Tests for HistoryAnalyzer - parses Claude Code JSONL conversation history.

This module tests the HistoryAnalyzer class which parses JSONL conversation files
directly without needing API calls (static analysis mode / offline fallback).
"""

import json
import pytest
from pathlib import Path
from dataclasses import dataclass
from typing import Dict, List

from ralph_orchestrator.onboarding.history_analyzer import (
    HistoryAnalyzer,
    ToolUsageStats,
    MCPServerStats,
    ToolChain,
    Conversation,
)


class TestHistoryAnalyzerInit:
    """Tests for HistoryAnalyzer initialization."""

    def test_init_with_empty_file_list(self):
        """HistoryAnalyzer initializes with empty file list."""
        analyzer = HistoryAnalyzer([])
        assert analyzer.files == []

    def test_init_with_file_list(self, tmp_path: Path):
        """HistoryAnalyzer initializes with file paths."""
        file1 = tmp_path / "conversation1.jsonl"
        file2 = tmp_path / "conversation2.jsonl"
        file1.touch()
        file2.touch()

        analyzer = HistoryAnalyzer([file1, file2])
        assert len(analyzer.files) == 2
        assert file1 in analyzer.files
        assert file2 in analyzer.files


class TestExtractToolUsage:
    """Tests for extracting tool usage statistics from conversations."""

    def test_extracts_tool_usage_from_single_message(self, tmp_path: Path):
        """Extracts tool usage from a single tool_use message."""
        jsonl_file = tmp_path / "conv.jsonl"
        messages = [
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Read", "input": {"path": "/foo"}, "id": "1"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "1", "content": "file contents", "is_error": False}
                ],
            },
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        stats = analyzer.extract_tool_usage()

        assert "Read" in stats
        assert stats["Read"].count == 1
        assert stats["Read"].success_count == 1
        assert stats["Read"].success_rate == 1.0

    def test_extracts_multiple_tools_from_conversation(self, tmp_path: Path):
        """Extracts multiple tool usages from a conversation."""
        jsonl_file = tmp_path / "conv.jsonl"
        messages = [
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Read", "input": {"path": "/foo"}, "id": "1"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "1", "content": "contents", "is_error": False}
                ],
            },
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Edit", "input": {"path": "/foo"}, "id": "2"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "2", "content": "edited", "is_error": False}
                ],
            },
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Read", "input": {"path": "/bar"}, "id": "3"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "3", "content": "more", "is_error": False}
                ],
            },
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        stats = analyzer.extract_tool_usage()

        assert stats["Read"].count == 2
        assert stats["Edit"].count == 1

    def test_tracks_tool_errors_separately(self, tmp_path: Path):
        """Tracks tool errors as failures."""
        jsonl_file = tmp_path / "conv.jsonl"
        messages = [
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Write", "input": {"path": "/readonly"}, "id": "1"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "1", "content": "Permission denied", "is_error": True}
                ],
            },
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Write", "input": {"path": "/writable"}, "id": "2"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "2", "content": "success", "is_error": False}
                ],
            },
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        stats = analyzer.extract_tool_usage()

        assert stats["Write"].count == 2
        assert stats["Write"].success_count == 1
        assert stats["Write"].failure_count == 1
        assert stats["Write"].success_rate == 0.5

    def test_returns_empty_dict_for_empty_file_list(self):
        """Returns empty dict when no files provided."""
        analyzer = HistoryAnalyzer([])
        stats = analyzer.extract_tool_usage()
        assert stats == {}

    def test_handles_malformed_jsonl_gracefully(self, tmp_path: Path):
        """Skips malformed lines in JSONL file."""
        jsonl_file = tmp_path / "conv.jsonl"
        content = '{"type": "user", "content": "hello"}\nnot valid json\n{"type": "assistant", "content": []}'
        jsonl_file.write_text(content)

        analyzer = HistoryAnalyzer([jsonl_file])
        # Should not raise, just skip bad lines
        stats = analyzer.extract_tool_usage()
        assert isinstance(stats, dict)

    def test_handles_missing_file_gracefully(self, tmp_path: Path):
        """Handles missing file without crashing."""
        missing_file = tmp_path / "nonexistent.jsonl"
        analyzer = HistoryAnalyzer([missing_file])

        # Should not raise
        stats = analyzer.extract_tool_usage()
        assert stats == {}


class TestExtractMCPUsage:
    """Tests for extracting MCP server usage patterns."""

    def test_identifies_mcp_tools(self, tmp_path: Path):
        """Identifies tools that are MCP server calls (prefixed with mcp_)."""
        jsonl_file = tmp_path / "conv.jsonl"
        messages = [
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "mcp_github_create_issue", "input": {}, "id": "1"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "1", "content": "created", "is_error": False}
                ],
            },
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "mcp_github_list_prs", "input": {}, "id": "2"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "2", "content": "prs", "is_error": False}
                ],
            },
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        mcp_stats = analyzer.extract_mcp_usage()

        assert "github" in mcp_stats
        assert mcp_stats["github"].tool_count == 2
        assert mcp_stats["github"].tools_used == {"mcp_github_create_issue", "mcp_github_list_prs"}

    def test_separates_multiple_mcp_servers(self, tmp_path: Path):
        """Separates usage stats for different MCP servers."""
        jsonl_file = tmp_path / "conv.jsonl"
        messages = [
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "mcp_github_create_issue", "input": {}, "id": "1"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "1", "content": "ok", "is_error": False}
                ],
            },
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "mcp_memory_store", "input": {}, "id": "2"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "2", "content": "ok", "is_error": False}
                ],
            },
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        mcp_stats = analyzer.extract_mcp_usage()

        assert "github" in mcp_stats
        assert "memory" in mcp_stats
        assert mcp_stats["github"].tool_count == 1
        assert mcp_stats["memory"].tool_count == 1

    def test_excludes_non_mcp_tools(self, tmp_path: Path):
        """Does not include regular tools in MCP stats."""
        jsonl_file = tmp_path / "conv.jsonl"
        messages = [
            {
                "type": "assistant",
                "content": [
                    {"type": "tool_use", "name": "Read", "input": {}, "id": "1"}
                ],
            },
            {
                "type": "user",
                "content": [
                    {"type": "tool_result", "tool_use_id": "1", "content": "ok", "is_error": False}
                ],
            },
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        mcp_stats = analyzer.extract_mcp_usage()

        assert mcp_stats == {}


class TestExtractToolChains:
    """Tests for identifying sequences of tools used together."""

    def test_identifies_tool_sequence(self, tmp_path: Path):
        """Identifies a sequence of tools used in a conversation."""
        jsonl_file = tmp_path / "conv.jsonl"
        # Simulate Read -> Edit -> Read sequence
        messages = [
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Read", "input": {}, "id": "1"}]},
            {"type": "user", "content": [{"type": "tool_result", "tool_use_id": "1", "content": "", "is_error": False}]},
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Edit", "input": {}, "id": "2"}]},
            {"type": "user", "content": [{"type": "tool_result", "tool_use_id": "2", "content": "", "is_error": False}]},
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Read", "input": {}, "id": "3"}]},
            {"type": "user", "content": [{"type": "tool_result", "tool_use_id": "3", "content": "", "is_error": False}]},
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        chains = analyzer.extract_tool_chains()

        assert len(chains) > 0
        # Should find ["Read", "Edit", "Read"] as one of the chains
        found_chain = any(chain.tools == ["Read", "Edit", "Read"] for chain in chains)
        assert found_chain

    def test_handles_conversations_without_tools(self, tmp_path: Path):
        """Handles conversations with no tool usage."""
        jsonl_file = tmp_path / "conv.jsonl"
        messages = [
            {"type": "user", "content": "Hello"},
            {"type": "assistant", "content": [{"type": "text", "text": "Hi there!"}]},
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        chains = analyzer.extract_tool_chains()

        assert chains == []


class TestExtractConversations:
    """Tests for parsing full conversations."""

    def test_parses_conversation_messages(self, tmp_path: Path):
        """Parses a full conversation from JSONL."""
        jsonl_file = tmp_path / "conv.jsonl"
        messages = [
            {"type": "user", "content": [{"type": "text", "text": "Hello"}]},
            {"type": "assistant", "content": [{"type": "text", "text": "Hi there!"}]},
            {"type": "user", "content": [{"type": "text", "text": "Can you help?"}]},
        ]
        jsonl_file.write_text("\n".join(json.dumps(m) for m in messages))

        analyzer = HistoryAnalyzer([jsonl_file])
        conversations = analyzer.extract_conversations()

        assert len(conversations) == 1
        assert len(conversations[0].messages) == 3

    def test_creates_separate_conversations_for_separate_files(self, tmp_path: Path):
        """Creates separate Conversation objects for each file."""
        file1 = tmp_path / "conv1.jsonl"
        file2 = tmp_path / "conv2.jsonl"

        msg1 = [{"type": "user", "content": "Hello from conv1"}]
        msg2 = [{"type": "user", "content": "Hello from conv2"}]

        file1.write_text(json.dumps(msg1[0]))
        file2.write_text(json.dumps(msg2[0]))

        analyzer = HistoryAnalyzer([file1, file2])
        conversations = analyzer.extract_conversations()

        assert len(conversations) == 2

    def test_includes_file_path_in_conversation(self, tmp_path: Path):
        """Includes source file path in Conversation object."""
        jsonl_file = tmp_path / "myconv.jsonl"
        jsonl_file.write_text('{"type": "user", "content": "test"}')

        analyzer = HistoryAnalyzer([jsonl_file])
        conversations = analyzer.extract_conversations()

        assert conversations[0].source_file == jsonl_file


class TestDataModels:
    """Tests for the data model classes."""

    def test_tool_usage_stats_defaults(self):
        """ToolUsageStats has correct defaults."""
        stats = ToolUsageStats(name="Read")
        assert stats.name == "Read"
        assert stats.count == 0
        assert stats.success_count == 0
        assert stats.failure_count == 0
        assert stats.success_rate == 0.0

    def test_tool_usage_stats_success_rate_calculation(self):
        """ToolUsageStats calculates success rate correctly."""
        stats = ToolUsageStats(name="Write", count=10, success_count=8, failure_count=2)
        assert stats.success_rate == 0.8

    def test_mcp_server_stats_defaults(self):
        """MCPServerStats has correct defaults."""
        stats = MCPServerStats(server_name="github")
        assert stats.server_name == "github"
        assert stats.tool_count == 0
        assert stats.tools_used == set()

    def test_tool_chain_stores_sequence(self):
        """ToolChain stores a sequence of tools."""
        chain = ToolChain(tools=["Read", "Edit", "Write"], count=5)
        assert chain.tools == ["Read", "Edit", "Write"]
        assert chain.count == 5

    def test_conversation_stores_messages(self):
        """Conversation stores messages and source file."""
        conv = Conversation(
            source_file=Path("/test/conv.jsonl"),
            messages=[{"type": "user", "content": "test"}],
        )
        assert conv.source_file == Path("/test/conv.jsonl")
        assert len(conv.messages) == 1


class TestMultipleFiles:
    """Tests for analyzing multiple JSONL files."""

    def test_aggregates_stats_across_files(self, tmp_path: Path):
        """Aggregates tool usage stats across multiple files."""
        file1 = tmp_path / "conv1.jsonl"
        file2 = tmp_path / "conv2.jsonl"

        # File 1: 2 Read calls
        messages1 = [
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Read", "input": {}, "id": "1"}]},
            {"type": "user", "content": [{"type": "tool_result", "tool_use_id": "1", "content": "", "is_error": False}]},
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Read", "input": {}, "id": "2"}]},
            {"type": "user", "content": [{"type": "tool_result", "tool_use_id": "2", "content": "", "is_error": False}]},
        ]
        file1.write_text("\n".join(json.dumps(m) for m in messages1))

        # File 2: 3 Read calls
        messages2 = [
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Read", "input": {}, "id": "1"}]},
            {"type": "user", "content": [{"type": "tool_result", "tool_use_id": "1", "content": "", "is_error": False}]},
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Read", "input": {}, "id": "2"}]},
            {"type": "user", "content": [{"type": "tool_result", "tool_use_id": "2", "content": "", "is_error": False}]},
            {"type": "assistant", "content": [{"type": "tool_use", "name": "Read", "input": {}, "id": "3"}]},
            {"type": "user", "content": [{"type": "tool_result", "tool_use_id": "3", "content": "", "is_error": False}]},
        ]
        file2.write_text("\n".join(json.dumps(m) for m in messages2))

        analyzer = HistoryAnalyzer([file1, file2])
        stats = analyzer.extract_tool_usage()

        assert stats["Read"].count == 5  # 2 + 3
