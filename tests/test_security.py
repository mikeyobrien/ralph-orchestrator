# ABOUTME: Unit tests for SecurityValidator system
# ABOUTME: Tests path traversal protection, sensitive data masking, and filename validation

"""Tests for security.py module."""

import pytest
from pathlib import Path
import tempfile

from ralph_orchestrator.security import (
    SecurityValidator,
    PathTraversalProtection,
    secure_file_operation,
)


class TestSecurityValidatorPaths:
    """Tests for path sanitization and validation."""

    def test_sanitize_path_normal_path(self):
        """Normal paths should pass validation."""
        base_dir = Path("/tmp/test_base")
        base_dir.mkdir(parents=True, exist_ok=True)

        # Create a test file
        test_file = base_dir / "test.txt"
        test_file.touch()

        result = SecurityValidator.sanitize_path("test.txt", base_dir)
        assert result == test_file

        # Cleanup
        test_file.unlink()
        base_dir.rmdir()

    def test_sanitize_path_blocks_parent_traversal(self):
        """Should block paths with '..' traversal."""
        with pytest.raises(ValueError, match="dangerous pattern"):
            SecurityValidator.sanitize_path("../etc/passwd")

    def test_sanitize_path_blocks_double_traversal(self):
        """Should block double directory traversal."""
        with pytest.raises(ValueError, match="dangerous pattern"):
            SecurityValidator.sanitize_path("../../root")

    def test_sanitize_path_blocks_etc(self):
        """Should block access to /etc."""
        with pytest.raises(ValueError, match="dangerous system location"):
            SecurityValidator.sanitize_path("/etc/passwd")

    def test_sanitize_path_blocks_usr_bin(self):
        """Should block access to /usr/bin."""
        with pytest.raises(ValueError, match="dangerous system location"):
            SecurityValidator.sanitize_path("/usr/bin/python")

    def test_sanitize_path_blocks_root(self):
        """Should block access to /root."""
        with pytest.raises(ValueError, match="dangerous system location"):
            SecurityValidator.sanitize_path("/root/.bashrc")

    def test_sanitize_path_blocks_proc(self):
        """Should block access to /proc."""
        with pytest.raises(ValueError, match="dangerous system location"):
            SecurityValidator.sanitize_path("/proc/self/environ")

    def test_sanitize_path_blocks_sys(self):
        """Should block access to /sys."""
        with pytest.raises(ValueError, match="dangerous system location"):
            SecurityValidator.sanitize_path("/sys/kernel")

    def test_sanitize_path_blocks_control_chars(self):
        """Should block paths with control characters."""
        with pytest.raises(ValueError, match="dangerous pattern"):
            SecurityValidator.sanitize_path("test\x00file")


class TestSecurityValidatorSensitiveData:
    """Tests for sensitive data masking."""

    def test_mask_openai_api_key(self):
        """Should mask OpenAI API keys."""
        text = "My key is sk-abc123def456ghi789jkl"
        result = SecurityValidator.mask_sensitive_data(text)
        assert "sk-abc123" not in result
        assert "sk-***" in result

    def test_mask_bearer_token(self):
        """Should mask Bearer tokens."""
        text = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"
        result = SecurityValidator.mask_sensitive_data(text)
        assert "eyJhbGciOiJ" not in result
        assert "Bearer ***" in result

    def test_mask_password_in_json(self):
        """Should mask passwords in JSON format."""
        text = '{"password": "mysecretpassword123"}'
        result = SecurityValidator.mask_sensitive_data(text)
        assert "mysecretpassword123" not in result

    def test_mask_password_key_value(self):
        """Should mask passwords in key=value format."""
        text = "password=mysecretpassword123"
        result = SecurityValidator.mask_sensitive_data(text)
        assert "mysecretpassword123" not in result

    def test_mask_api_key(self):
        """Should mask API keys."""
        text = 'api_key="abcd1234efgh5678ijkl9012"'
        result = SecurityValidator.mask_sensitive_data(text)
        assert "abcd1234efgh5678" not in result

    def test_mask_ssh_path(self):
        """Should mask SSH key paths."""
        text = "Reading from /home/user/.ssh/id_rsa"
        result = SecurityValidator.mask_sensitive_data(text)
        assert "/home/user/.ssh/id_rsa" not in result
        assert "[REDACTED" in result

    def test_mask_aws_credentials_path(self):
        """Should mask AWS credentials path."""
        text = "Using /home/user/.aws/credentials"
        result = SecurityValidator.mask_sensitive_data(text)
        assert "/home/user/.aws/credentials" not in result
        assert "[REDACTED" in result

    def test_mask_token_in_config(self):
        """Should mask tokens in configuration."""
        text = 'token: "ghp_123456789abcdefghijklmnop"'
        result = SecurityValidator.mask_sensitive_data(text)
        assert "ghp_123456789" not in result

    def test_mask_secret_in_env(self):
        """Should mask secrets."""
        text = "secret=my_super_secret_value_12345"
        result = SecurityValidator.mask_sensitive_data(text)
        assert "my_super_secret_value" not in result


class TestSecurityValidatorFilename:
    """Tests for filename validation."""

    def test_validate_filename_normal(self):
        """Normal filenames should pass."""
        result = SecurityValidator.validate_filename("test.txt")
        assert result == "test.txt"

    def test_validate_filename_empty_raises(self):
        """Empty filenames should raise."""
        with pytest.raises(ValueError, match="cannot be empty"):
            SecurityValidator.validate_filename("")

    def test_validate_filename_blocks_traversal(self):
        """Should block paths with directory traversal."""
        with pytest.raises(ValueError, match="path traversal"):
            SecurityValidator.validate_filename("../etc/passwd")

    def test_validate_filename_blocks_slash(self):
        """Should block paths with forward slash."""
        with pytest.raises(ValueError, match="path traversal"):
            SecurityValidator.validate_filename("path/to/file")

    def test_validate_filename_blocks_backslash(self):
        """Should block paths with backslash."""
        with pytest.raises(ValueError, match="path traversal"):
            SecurityValidator.validate_filename("path\\to\\file")

    def test_validate_filename_blocks_reserved_con(self):
        """Should block Windows reserved name CON."""
        with pytest.raises(ValueError, match="reserved name"):
            SecurityValidator.validate_filename("CON")

    def test_validate_filename_blocks_reserved_prn(self):
        """Should block Windows reserved name PRN."""
        with pytest.raises(ValueError, match="reserved name"):
            SecurityValidator.validate_filename("PRN.txt")

    def test_validate_filename_blocks_reserved_aux(self):
        """Should block Windows reserved name AUX."""
        with pytest.raises(ValueError, match="reserved name"):
            SecurityValidator.validate_filename("AUX")

    def test_validate_filename_blocks_reserved_nul(self):
        """Should block Windows reserved name NUL."""
        with pytest.raises(ValueError, match="reserved name"):
            SecurityValidator.validate_filename("NUL.txt")

    def test_validate_filename_blocks_control_chars(self):
        """Should block control characters."""
        with pytest.raises(ValueError, match="control characters"):
            SecurityValidator.validate_filename("test\x05file.txt")

    def test_validate_filename_strips_dangerous_chars(self):
        """Should strip dangerous characters."""
        result = SecurityValidator.validate_filename("test<>file.txt")
        assert "<" not in result
        assert ">" not in result

    def test_validate_filename_truncates_long_names(self):
        """Should truncate very long filenames."""
        long_name = "a" * 300 + ".txt"
        result = SecurityValidator.validate_filename(long_name)
        assert len(result) <= 255


class TestSecurityValidatorConfigValues:
    """Tests for configuration value validation."""

    def test_validate_delay_valid(self):
        """Valid delay should pass."""
        result = SecurityValidator.validate_config_value("delay", 5)
        assert result == 5

    def test_validate_delay_negative_raises(self):
        """Negative delay should raise."""
        with pytest.raises(ValueError, match="must be non-negative"):
            SecurityValidator.validate_config_value("delay", -1)

    def test_validate_delay_too_large_raises(self):
        """Delay > 24 hours should raise."""
        with pytest.raises(ValueError, match="too large"):
            SecurityValidator.validate_config_value("delay", 100000)

    def test_validate_max_iterations_valid(self):
        """Valid max_iterations should pass."""
        result = SecurityValidator.validate_config_value("max_iterations", 100)
        assert result == 100

    def test_validate_max_iterations_too_large_raises(self):
        """max_iterations > 10000 should raise."""
        with pytest.raises(ValueError, match="too large"):
            SecurityValidator.validate_config_value("max_iterations", 20000)

    def test_validate_boolean_true_string(self):
        """String 'true' should parse to True."""
        result = SecurityValidator.validate_config_value("verbose", "true")
        assert result is True

    def test_validate_boolean_false_string(self):
        """String 'false' should parse to False."""
        result = SecurityValidator.validate_config_value("verbose", "false")
        assert result is False

    def test_validate_boolean_yes_string(self):
        """String 'yes' should parse to True."""
        result = SecurityValidator.validate_config_value("dry_run", "yes")
        assert result is True

    def test_validate_focus_sanitizes_injection(self):
        """Focus should remove command injection characters."""
        result = SecurityValidator.validate_config_value("focus", "test; rm -rf /")
        assert ";" not in result
        assert "rm -rf" in result  # text is kept, just dangerous chars removed


class TestPathTraversalProtection:
    """Tests for PathTraversalProtection class."""

    def test_safe_file_read_success(self):
        """Should successfully read safe files."""
        with tempfile.TemporaryDirectory() as tmpdir:
            base = Path(tmpdir)
            test_file = base / "test.txt"
            test_file.write_text("hello world")

            content = PathTraversalProtection.safe_file_read(
                "test.txt", base
            )
            assert content == "hello world"

    def test_safe_file_read_blocks_traversal(self):
        """Should block path traversal in reads."""
        with tempfile.TemporaryDirectory() as tmpdir:
            base = Path(tmpdir)

            with pytest.raises(ValueError, match="dangerous pattern"):
                PathTraversalProtection.safe_file_read(
                    "../../../etc/passwd", base
                )

    def test_safe_file_read_not_found(self):
        """Should raise FileNotFoundError for missing files."""
        with tempfile.TemporaryDirectory() as tmpdir:
            base = Path(tmpdir)

            with pytest.raises(FileNotFoundError):
                PathTraversalProtection.safe_file_read(
                    "nonexistent.txt", base
                )

    def test_safe_file_write_success(self):
        """Should successfully write to safe locations."""
        with tempfile.TemporaryDirectory() as tmpdir:
            base = Path(tmpdir)

            PathTraversalProtection.safe_file_write(
                "output.txt", "test content", base
            )

            result = (base / "output.txt").read_text()
            assert result == "test content"

    def test_safe_file_write_blocks_traversal(self):
        """Should block path traversal in writes."""
        with tempfile.TemporaryDirectory() as tmpdir:
            base = Path(tmpdir)

            with pytest.raises(ValueError, match="dangerous pattern"):
                PathTraversalProtection.safe_file_write(
                    "../../../tmp/evil.txt", "bad content", base
                )

    def test_safe_file_write_creates_parents(self):
        """Should create parent directories."""
        with tempfile.TemporaryDirectory() as tmpdir:
            base = Path(tmpdir)

            PathTraversalProtection.safe_file_write(
                "subdir/nested/file.txt", "nested content", base
            )

            result = (base / "subdir" / "nested" / "file.txt").read_text()
            assert result == "nested content"


class TestSecureFileOperationDecorator:
    """Tests for the secure_file_operation decorator."""

    def test_decorator_sanitizes_path_args(self):
        """Decorator should sanitize path arguments."""
        calls = []

        @secure_file_operation(Path("/tmp"))
        def record_call(*args, **kwargs):
            calls.append((args, kwargs))
            return True

        # This should work for safe paths
        result = record_call("/tmp/safe.txt")
        assert result is True

    def test_decorator_blocks_dangerous_paths(self):
        """Decorator should block dangerous paths."""
        @secure_file_operation(Path("/tmp"))
        def dangerous_call(path):
            return path

        with pytest.raises(ValueError):
            dangerous_call("../../../etc/passwd")
