# ABOUTME: Unit tests for ACPHandlers class
# ABOUTME: Tests permission modes (auto_approve, deny_all, allowlist, interactive)

"""Tests for ACPHandlers - permission handling for ACP adapter."""

from unittest.mock import patch, MagicMock
import pytest

from ralph_orchestrator.adapters.acp_handlers import (
    ACPHandlers,
    PermissionRequest,
    PermissionResult,
)


class TestPermissionRequest:
    """Tests for PermissionRequest dataclass."""

    def test_from_params_basic(self):
        """Test creating PermissionRequest from params."""
        params = {"operation": "fs/read_text_file", "path": "/test/file.txt"}

        request = PermissionRequest.from_params(params)

        assert request.operation == "fs/read_text_file"
        assert request.path == "/test/file.txt"
        assert request.command is None
        assert request.arguments == params

    def test_from_params_with_command(self):
        """Test creating PermissionRequest with command."""
        params = {"operation": "terminal/execute", "command": "ls -la"}

        request = PermissionRequest.from_params(params)

        assert request.operation == "terminal/execute"
        assert request.command == "ls -la"

    def test_from_params_empty(self):
        """Test creating PermissionRequest from empty params."""
        params = {}

        request = PermissionRequest.from_params(params)

        assert request.operation == ""
        assert request.path is None
        assert request.command is None


class TestPermissionResult:
    """Tests for PermissionResult dataclass."""

    def test_to_dict_approved(self):
        """Test to_dict for approved result."""
        result = PermissionResult(approved=True, reason="test", mode="auto_approve")

        assert result.to_dict() == {"approved": True}

    def test_to_dict_denied(self):
        """Test to_dict for denied result."""
        result = PermissionResult(approved=False, reason="test", mode="deny_all")

        assert result.to_dict() == {"approved": False}


class TestACPHandlersInitialization:
    """Tests for ACPHandlers initialization."""

    def test_init_default(self):
        """Test initialization with default values."""
        handlers = ACPHandlers()

        assert handlers.permission_mode == "auto_approve"
        assert handlers.allowlist == []
        assert handlers.on_permission_log is None

    def test_init_with_mode(self):
        """Test initialization with custom mode."""
        handlers = ACPHandlers(permission_mode="deny_all")

        assert handlers.permission_mode == "deny_all"

    def test_init_with_allowlist(self):
        """Test initialization with allowlist."""
        allowlist = ["fs/*", "terminal/execute"]
        handlers = ACPHandlers(
            permission_mode="allowlist", permission_allowlist=allowlist
        )

        assert handlers.allowlist == allowlist

    def test_init_with_log_callback(self):
        """Test initialization with logging callback."""
        log_fn = MagicMock()
        handlers = ACPHandlers(on_permission_log=log_fn)

        assert handlers.on_permission_log == log_fn

    def test_init_invalid_mode(self):
        """Test initialization with invalid mode raises error."""
        with pytest.raises(ValueError, match="Invalid permission_mode"):
            ACPHandlers(permission_mode="invalid_mode")

    def test_valid_modes(self):
        """Test all valid modes can be set."""
        for mode in ("auto_approve", "deny_all", "allowlist", "interactive"):
            handlers = ACPHandlers(permission_mode=mode)
            assert handlers.permission_mode == mode


class TestACPHandlersAutoApprove:
    """Tests for auto_approve permission mode."""

    def test_auto_approve_simple_request(self):
        """Test auto_approve mode approves any request."""
        handlers = ACPHandlers(permission_mode="auto_approve")

        result = handlers.handle_request_permission(
            {"operation": "fs/read_text_file", "path": "/etc/passwd"}
        )

        assert result == {"approved": True}

    def test_auto_approve_any_operation(self):
        """Test auto_approve mode approves any operation."""
        handlers = ACPHandlers(permission_mode="auto_approve")

        operations = [
            "fs/read_text_file",
            "fs/write_text_file",
            "terminal/execute",
            "dangerous/operation",
        ]

        for op in operations:
            result = handlers.handle_request_permission({"operation": op})
            assert result == {"approved": True}


class TestACPHandlersDenyAll:
    """Tests for deny_all permission mode."""

    def test_deny_all_simple_request(self):
        """Test deny_all mode denies any request."""
        handlers = ACPHandlers(permission_mode="deny_all")

        result = handlers.handle_request_permission(
            {"operation": "fs/read_text_file", "path": "/test/file.txt"}
        )

        assert result == {"approved": False}

    def test_deny_all_any_operation(self):
        """Test deny_all mode denies any operation."""
        handlers = ACPHandlers(permission_mode="deny_all")

        operations = [
            "fs/read_text_file",
            "fs/write_text_file",
            "terminal/execute",
            "safe/operation",
        ]

        for op in operations:
            result = handlers.handle_request_permission({"operation": op})
            assert result == {"approved": False}


class TestACPHandlersAllowlist:
    """Tests for allowlist permission mode."""

    def test_allowlist_exact_match(self):
        """Test allowlist with exact operation match."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["fs/read_text_file"],
        )

        result = handlers.handle_request_permission(
            {"operation": "fs/read_text_file"}
        )

        assert result == {"approved": True}

    def test_allowlist_no_match(self):
        """Test allowlist denies when no match."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["fs/read_text_file"],
        )

        result = handlers.handle_request_permission(
            {"operation": "fs/write_text_file"}
        )

        assert result == {"approved": False}

    def test_allowlist_glob_pattern(self):
        """Test allowlist with glob pattern."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["fs/*"],
        )

        # Should match
        assert handlers.handle_request_permission(
            {"operation": "fs/read_text_file"}
        ) == {"approved": True}

        assert handlers.handle_request_permission(
            {"operation": "fs/write_text_file"}
        ) == {"approved": True}

        # Should not match
        assert handlers.handle_request_permission(
            {"operation": "terminal/execute"}
        ) == {"approved": False}

    def test_allowlist_question_mark_pattern(self):
        """Test allowlist with question mark pattern."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["fs/?_text_file"],
        )

        # Should match single character
        assert handlers.handle_request_permission(
            {"operation": "fs/r_text_file"}
        ) == {"approved": True}

        # Should not match multiple characters
        assert handlers.handle_request_permission(
            {"operation": "fs/read_text_file"}
        ) == {"approved": False}

    def test_allowlist_regex_pattern(self):
        """Test allowlist with regex pattern."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["/^fs\\/.*$/"],
        )

        # Should match regex
        assert handlers.handle_request_permission(
            {"operation": "fs/read_text_file"}
        ) == {"approved": True}

        # Should not match
        assert handlers.handle_request_permission(
            {"operation": "terminal/execute"}
        ) == {"approved": False}

    def test_allowlist_multiple_patterns(self):
        """Test allowlist with multiple patterns."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["fs/read_text_file", "terminal/*"],
        )

        # Should match first pattern
        assert handlers.handle_request_permission(
            {"operation": "fs/read_text_file"}
        ) == {"approved": True}

        # Should match second pattern
        assert handlers.handle_request_permission(
            {"operation": "terminal/execute"}
        ) == {"approved": True}

        # Should not match any
        assert handlers.handle_request_permission(
            {"operation": "fs/write_text_file"}
        ) == {"approved": False}

    def test_allowlist_empty(self):
        """Test empty allowlist denies everything."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=[],
        )

        assert handlers.handle_request_permission(
            {"operation": "fs/read_text_file"}
        ) == {"approved": False}

    def test_allowlist_invalid_regex(self):
        """Test allowlist handles invalid regex gracefully."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["/[invalid/"],  # Invalid regex
        )

        # Should not match (invalid regex returns False)
        assert handlers.handle_request_permission(
            {"operation": "[invalid"}
        ) == {"approved": False}


class TestACPHandlersInteractive:
    """Tests for interactive permission mode."""

    def test_interactive_no_terminal(self):
        """Test interactive mode denies when no terminal."""
        handlers = ACPHandlers(permission_mode="interactive")

        with patch("sys.stdin.isatty", return_value=False):
            result = handlers.handle_request_permission(
                {"operation": "fs/read_text_file"}
            )

        assert result == {"approved": False}

    def test_interactive_user_approves(self):
        """Test interactive mode with user approval."""
        handlers = ACPHandlers(permission_mode="interactive")

        with patch("sys.stdin.isatty", return_value=True):
            with patch("builtins.input", return_value="y"):
                result = handlers.handle_request_permission(
                    {"operation": "fs/read_text_file"}
                )

        assert result == {"approved": True}

    def test_interactive_user_denies(self):
        """Test interactive mode with user denial."""
        handlers = ACPHandlers(permission_mode="interactive")

        with patch("sys.stdin.isatty", return_value=True):
            with patch("builtins.input", return_value="n"):
                result = handlers.handle_request_permission(
                    {"operation": "fs/read_text_file"}
                )

        assert result == {"approved": False}

    def test_interactive_empty_input_denies(self):
        """Test interactive mode denies on empty input."""
        handlers = ACPHandlers(permission_mode="interactive")

        with patch("sys.stdin.isatty", return_value=True):
            with patch("builtins.input", return_value=""):
                result = handlers.handle_request_permission(
                    {"operation": "fs/read_text_file"}
                )

        assert result == {"approved": False}

    def test_interactive_yes_variations(self):
        """Test interactive mode accepts various yes inputs."""
        handlers = ACPHandlers(permission_mode="interactive")

        for yes_input in ["y", "Y", "yes", "YES", "Yes"]:
            with patch("sys.stdin.isatty", return_value=True):
                with patch("builtins.input", return_value=yes_input):
                    result = handlers.handle_request_permission(
                        {"operation": "fs/read_text_file"}
                    )
                    assert result == {"approved": True}, f"Failed for input: {yes_input}"

    def test_interactive_keyboard_interrupt(self):
        """Test interactive mode handles keyboard interrupt."""
        handlers = ACPHandlers(permission_mode="interactive")

        with patch("sys.stdin.isatty", return_value=True):
            with patch("builtins.input", side_effect=KeyboardInterrupt):
                result = handlers.handle_request_permission(
                    {"operation": "fs/read_text_file"}
                )

        assert result == {"approved": False}

    def test_interactive_eof_error(self):
        """Test interactive mode handles EOF error."""
        handlers = ACPHandlers(permission_mode="interactive")

        with patch("sys.stdin.isatty", return_value=True):
            with patch("builtins.input", side_effect=EOFError):
                result = handlers.handle_request_permission(
                    {"operation": "fs/read_text_file"}
                )

        assert result == {"approved": False}


class TestACPHandlersHistory:
    """Tests for permission history tracking."""

    def test_history_starts_empty(self):
        """Test history starts empty."""
        handlers = ACPHandlers()

        assert handlers.get_history() == []

    def test_history_tracks_decisions(self):
        """Test history tracks permission decisions."""
        handlers = ACPHandlers(permission_mode="auto_approve")

        handlers.handle_request_permission({"operation": "op1"})
        handlers.handle_request_permission({"operation": "op2"})

        history = handlers.get_history()
        assert len(history) == 2
        assert history[0][0].operation == "op1"
        assert history[1][0].operation == "op2"

    def test_history_clear(self):
        """Test clearing history."""
        handlers = ACPHandlers(permission_mode="auto_approve")

        handlers.handle_request_permission({"operation": "op1"})
        handlers.clear_history()

        assert handlers.get_history() == []

    def test_get_approved_count(self):
        """Test getting approved count."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["allowed"],
        )

        handlers.handle_request_permission({"operation": "allowed"})
        handlers.handle_request_permission({"operation": "allowed"})
        handlers.handle_request_permission({"operation": "denied"})

        assert handlers.get_approved_count() == 2

    def test_get_denied_count(self):
        """Test getting denied count."""
        handlers = ACPHandlers(
            permission_mode="allowlist",
            permission_allowlist=["allowed"],
        )

        handlers.handle_request_permission({"operation": "allowed"})
        handlers.handle_request_permission({"operation": "denied"})
        handlers.handle_request_permission({"operation": "denied"})

        assert handlers.get_denied_count() == 2

    def test_history_is_copy(self):
        """Test get_history returns a copy."""
        handlers = ACPHandlers(permission_mode="auto_approve")

        handlers.handle_request_permission({"operation": "op1"})
        history = handlers.get_history()
        history.clear()

        # Original history should be unchanged
        assert len(handlers.get_history()) == 1


class TestACPHandlersLogging:
    """Tests for permission decision logging."""

    def test_logging_callback_called(self):
        """Test logging callback is called on decisions."""
        log_fn = MagicMock()
        handlers = ACPHandlers(
            permission_mode="auto_approve",
            on_permission_log=log_fn,
        )

        handlers.handle_request_permission({"operation": "test_op"})

        log_fn.assert_called_once()
        call_arg = log_fn.call_args[0][0]
        assert "APPROVED" in call_arg
        assert "test_op" in call_arg

    def test_logging_shows_denied(self):
        """Test logging shows denied status."""
        log_fn = MagicMock()
        handlers = ACPHandlers(
            permission_mode="deny_all",
            on_permission_log=log_fn,
        )

        handlers.handle_request_permission({"operation": "test_op"})

        call_arg = log_fn.call_args[0][0]
        assert "DENIED" in call_arg

    def test_no_logging_without_callback(self):
        """Test no error when no logging callback."""
        handlers = ACPHandlers(permission_mode="auto_approve")

        # Should not raise
        handlers.handle_request_permission({"operation": "test_op"})


class TestACPHandlersIntegration:
    """Integration tests for ACPHandlers with ACPAdapter."""

    def test_adapter_uses_handlers(self):
        """Test ACPAdapter uses ACPHandlers for permissions."""
        from ralph_orchestrator.adapters.acp import ACPAdapter

        adapter = ACPAdapter(
            permission_mode="allowlist",
            permission_allowlist=["fs/read_text_file"],
        )

        # Test via internal handler
        result = adapter._handle_permission_request(
            {"operation": "fs/read_text_file"}
        )
        assert result == {"approved": True}

        result = adapter._handle_permission_request(
            {"operation": "fs/write_text_file"}
        )
        assert result == {"approved": False}

    def test_adapter_permission_stats(self):
        """Test ACPAdapter provides permission statistics."""
        from ralph_orchestrator.adapters.acp import ACPAdapter

        adapter = ACPAdapter(permission_mode="auto_approve")

        adapter._handle_permission_request({"operation": "op1"})
        adapter._handle_permission_request({"operation": "op2"})

        stats = adapter.get_permission_stats()
        assert stats["approved_count"] == 2
        assert stats["denied_count"] == 0

    def test_adapter_permission_history(self):
        """Test ACPAdapter provides permission history."""
        from ralph_orchestrator.adapters.acp import ACPAdapter

        adapter = ACPAdapter(permission_mode="deny_all")

        adapter._handle_permission_request({"operation": "op1"})

        history = adapter.get_permission_history()
        assert len(history) == 1
        assert history[0][0].operation == "op1"
        assert history[0][1].approved is False

    def test_adapter_from_config_with_allowlist(self):
        """Test ACPAdapter.from_config with allowlist."""
        from ralph_orchestrator.adapters.acp import ACPAdapter
        from ralph_orchestrator.adapters.acp_models import ACPAdapterConfig

        config = ACPAdapterConfig(
            permission_mode="allowlist",
            permission_allowlist=["fs/*"],
        )

        adapter = ACPAdapter.from_config(config)

        result = adapter._handle_permission_request(
            {"operation": "fs/read_text_file"}
        )
        assert result == {"approved": True}


class TestACPHandlersReadFile:
    """Tests for handle_read_file method."""

    def test_read_file_success(self, tmp_path):
        """Test successful file read."""
        handlers = ACPHandlers()

        # Create a test file
        test_file = tmp_path / "test.txt"
        test_file.write_text("Hello, World!")

        result = handlers.handle_read_file({"path": str(test_file)})

        assert "content" in result
        assert result["content"] == "Hello, World!"

    def test_read_file_missing_path(self):
        """Test read file with missing path parameter."""
        handlers = ACPHandlers()

        result = handlers.handle_read_file({})

        assert "error" in result
        assert result["error"]["code"] == -32602
        assert "Missing required parameter: path" in result["error"]["message"]

    def test_read_file_not_found(self, tmp_path):
        """Test read file that doesn't exist."""
        handlers = ACPHandlers()

        result = handlers.handle_read_file({"path": str(tmp_path / "nonexistent.txt")})

        assert "error" in result
        assert result["error"]["code"] == -32001
        assert "File not found" in result["error"]["message"]

    def test_read_file_is_directory(self, tmp_path):
        """Test read file when path is a directory."""
        handlers = ACPHandlers()

        result = handlers.handle_read_file({"path": str(tmp_path)})

        assert "error" in result
        assert result["error"]["code"] == -32002
        assert "Path is not a file" in result["error"]["message"]

    def test_read_file_relative_path_rejected(self, tmp_path):
        """Test that relative paths are rejected."""
        handlers = ACPHandlers()

        result = handlers.handle_read_file({"path": "relative/path.txt"})

        assert "error" in result
        assert result["error"]["code"] == -32602
        assert "Path must be absolute" in result["error"]["message"]

    def test_read_file_multiline_content(self, tmp_path):
        """Test reading file with multiple lines."""
        handlers = ACPHandlers()

        test_file = tmp_path / "multiline.txt"
        content = "Line 1\nLine 2\nLine 3"
        test_file.write_text(content)

        result = handlers.handle_read_file({"path": str(test_file)})

        assert result["content"] == content

    def test_read_file_empty_file(self, tmp_path):
        """Test reading empty file."""
        handlers = ACPHandlers()

        test_file = tmp_path / "empty.txt"
        test_file.write_text("")

        result = handlers.handle_read_file({"path": str(test_file)})

        assert result["content"] == ""

    def test_read_file_unicode_content(self, tmp_path):
        """Test reading file with unicode content."""
        handlers = ACPHandlers()

        test_file = tmp_path / "unicode.txt"
        content = "Hello, 世界! 🌍 Привет"
        test_file.write_text(content, encoding="utf-8")

        result = handlers.handle_read_file({"path": str(test_file)})

        assert result["content"] == content


class TestACPHandlersWriteFile:
    """Tests for handle_write_file method."""

    def test_write_file_success(self, tmp_path):
        """Test successful file write."""
        handlers = ACPHandlers()

        test_file = tmp_path / "output.txt"

        result = handlers.handle_write_file({
            "path": str(test_file),
            "content": "Hello, World!"
        })

        assert result == {"success": True}
        assert test_file.read_text() == "Hello, World!"

    def test_write_file_missing_path(self):
        """Test write file with missing path parameter."""
        handlers = ACPHandlers()

        result = handlers.handle_write_file({"content": "test"})

        assert "error" in result
        assert result["error"]["code"] == -32602
        assert "Missing required parameter: path" in result["error"]["message"]

    def test_write_file_missing_content(self, tmp_path):
        """Test write file with missing content parameter."""
        handlers = ACPHandlers()

        result = handlers.handle_write_file({"path": str(tmp_path / "test.txt")})

        assert "error" in result
        assert result["error"]["code"] == -32602
        assert "Missing required parameter: content" in result["error"]["message"]

    def test_write_file_empty_content(self, tmp_path):
        """Test write file with empty content."""
        handlers = ACPHandlers()

        test_file = tmp_path / "empty.txt"

        result = handlers.handle_write_file({
            "path": str(test_file),
            "content": ""
        })

        assert result == {"success": True}
        assert test_file.read_text() == ""

    def test_write_file_overwrites_existing(self, tmp_path):
        """Test write file overwrites existing file."""
        handlers = ACPHandlers()

        test_file = tmp_path / "existing.txt"
        test_file.write_text("Old content")

        result = handlers.handle_write_file({
            "path": str(test_file),
            "content": "New content"
        })

        assert result == {"success": True}
        assert test_file.read_text() == "New content"

    def test_write_file_creates_parent_dirs(self, tmp_path):
        """Test write file creates parent directories."""
        handlers = ACPHandlers()

        test_file = tmp_path / "nested" / "path" / "file.txt"

        result = handlers.handle_write_file({
            "path": str(test_file),
            "content": "Nested content"
        })

        assert result == {"success": True}
        assert test_file.read_text() == "Nested content"

    def test_write_file_relative_path_rejected(self, tmp_path):
        """Test that relative paths are rejected."""
        handlers = ACPHandlers()

        result = handlers.handle_write_file({
            "path": "relative/path.txt",
            "content": "test"
        })

        assert "error" in result
        assert result["error"]["code"] == -32602
        assert "Path must be absolute" in result["error"]["message"]

    def test_write_file_to_directory_rejected(self, tmp_path):
        """Test write file to directory path rejected."""
        handlers = ACPHandlers()

        result = handlers.handle_write_file({
            "path": str(tmp_path),
            "content": "test"
        })

        assert "error" in result
        assert result["error"]["code"] == -32002
        assert "Path is a directory" in result["error"]["message"]

    def test_write_file_unicode_content(self, tmp_path):
        """Test writing file with unicode content."""
        handlers = ACPHandlers()

        test_file = tmp_path / "unicode.txt"
        content = "Hello, 世界! 🌍 Привет"

        result = handlers.handle_write_file({
            "path": str(test_file),
            "content": content
        })

        assert result == {"success": True}
        assert test_file.read_text(encoding="utf-8") == content

    def test_write_file_multiline_content(self, tmp_path):
        """Test writing file with multiple lines."""
        handlers = ACPHandlers()

        test_file = tmp_path / "multiline.txt"
        content = "Line 1\nLine 2\nLine 3"

        result = handlers.handle_write_file({
            "path": str(test_file),
            "content": content
        })

        assert result == {"success": True}
        assert test_file.read_text() == content


class TestACPHandlersFileIntegration:
    """Integration tests for file operations."""

    def test_read_write_roundtrip(self, tmp_path):
        """Test write then read returns same content."""
        handlers = ACPHandlers()

        test_file = tmp_path / "roundtrip.txt"
        original = "Test content for roundtrip"

        # Write
        write_result = handlers.handle_write_file({
            "path": str(test_file),
            "content": original
        })
        assert write_result == {"success": True}

        # Read
        read_result = handlers.handle_read_file({"path": str(test_file)})
        assert read_result["content"] == original

    def test_read_write_large_file(self, tmp_path):
        """Test read/write with large file."""
        handlers = ACPHandlers()

        test_file = tmp_path / "large.txt"
        # Create ~1MB content
        original = "x" * (1024 * 1024)

        # Write
        write_result = handlers.handle_write_file({
            "path": str(test_file),
            "content": original
        })
        assert write_result == {"success": True}

        # Read
        read_result = handlers.handle_read_file({"path": str(test_file)})
        assert read_result["content"] == original
