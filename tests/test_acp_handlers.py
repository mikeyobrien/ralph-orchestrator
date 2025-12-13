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
