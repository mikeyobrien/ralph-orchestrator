# ABOUTME: Security utilities for Ralph Orchestrator
# ABOUTME: Provides input validation, path sanitization, and sensitive data protection

"""
Security utilities for Ralph Orchestrator.

This module provides security hardening functions including input validation,
path sanitization, and sensitive data protection.
"""

import re
import logging
from pathlib import Path
from typing import Any, Optional

logger = logging.getLogger("ralph-orchestrator.security")


class SecurityValidator:
    """Security validation utilities for Ralph Orchestrator."""

    # Patterns for dangerous path components
    DANGEROUS_PATH_PATTERNS = [
        r"\.\.\/.*",  # Directory traversal (Unix)
        r"\.\.\\.*",  # Windows directory traversal
        r"^\.\.[\/\\]",  # Starts with parent directory
        r"[\/\\]\.\.[\/\\]",  # Contains parent directory
        r"[<>:\"|?*]",  # Invalid filename characters (Windows)
        r"[\x00-\x1f]",  # Control characters
        r"[\/\\]\.\.[\/\\]\.\.[\/\\]",  # Double traversal
    ]

    # Sensitive data patterns that should be masked (16+ patterns)
    SENSITIVE_PATTERNS = [
        # API Keys
        (r"(sk-[a-zA-Z0-9]{10,})", r"sk-***********"),  # OpenAI API keys
        (r"(xai-[a-zA-Z0-9]{10,})", r"xai-***********"),  # xAI API keys
        (r"(AIza[a-zA-Z0-9_-]{35})", r"AIza***********"),  # Google API keys
        # Bearer tokens
        (r"(Bearer [a-zA-Z0-9\-_\.]{20,})", r"Bearer ***********"),
        # Passwords in various formats
        (
            r'(["\']?password["\']?\s*[:=]\s*["\']?)([^"\'\s]{3,})(["\']?)',
            r"\1*********\3",
        ),
        (r"(password\s*=\s*)([^\"'\s]{3,})", r"\1*********"),
        # Tokens in various formats
        (
            r'(token["\']?\s*[:=]\s*["\']?)([a-zA-Z0-9\-_\.]{10,})(["\']?)',
            r"\1*********\3",
        ),
        (r"(token\s*=\s*)([a-zA-Z0-9\-_\.]{10,})", r"\1*********"),
        # Secrets
        (
            r'(secret["\']?\s*[:=]\s*["\']?)([a-zA-Z0-9\-_\.]{10,})(["\']?)',
            r"\1*********\3",
        ),
        (r"(secret\s*=\s*)([a-zA-Z0-9\-_\.]{10,})", r"\1*********"),
        # Generic keys
        (
            r'(key["\']?\s*[:=]\s*["\']?)([a-zA-Z0-9\-_\.]{10,})(["\']?)',
            r"\1*********\3",
        ),
        # API keys in various formats
        (
            r'(api[_-]?key["\']?\s*[:=]\s*["\']?)([a-zA-Z0-9\-_\.]{10,})(["\']?)',
            r"\1*********\3",
        ),
        (r"(api[_-]?key\s*=\s*)([a-zA-Z0-9\-_\.]{10,})", r"\1*********"),
        # Sensitive file paths
        (
            r"(/[a-zA-Z0-9_\-\./]*\.ssh/[a-zA-Z0-9_\-\./]*)",
            r"[REDACTED_SSH_PATH]",
        ),  # SSH paths
        (
            r"(/[a-zA-Z0-9_\-\./]*\.ssh/id_[a-zA-Z0-9]*)",
            r"[REDACTED_SSH_KEY]",
        ),  # SSH private keys
        (
            r"(/[a-zA-Z0-9_\-\./]*\.config/[a-zA-Z0-9_\-\./]*)",
            r"[REDACTED_CONFIG_PATH]",
        ),  # Config files
        (
            r"(/[a-zA-Z0-9_\-\./]*\.aws/[a-zA-Z0-9_\-\./]*)",
            r"[REDACTED_AWS_PATH]",
        ),  # AWS credentials
        (
            r"(/[a-zA-Z0-9_\-\./]*(passwd|shadow|group|hosts))",
            r"[REDACTED_SYSTEM_FILE]",
        ),  # System files
        (
            r"(C:\\\\[a-zA-Z0-9_\-\./]*\\\\System32\\\\[a-zA-Z0-9_\-\./]*)",
            r"[REDACTED_SYSTEM_PATH]",
        ),  # Windows system files
        (
            r"(/[a-zA-Z0-9_\-\./]*(id_rsa|id_dsa|id_ecdsa|id_ed25519))",
            r"[REDACTED_PRIVATE_KEY]",
        ),  # Private key files
    ]

    # Dangerous absolute path prefixes
    DANGEROUS_ABS_PATHS = [
        "/etc",
        "/usr/bin",
        "/bin",
        "/sbin",
        "/root",
        "/var",
        "/opt",
        "/sys",
        "/proc",
        "/dev",
    ]

    @classmethod
    def sanitize_path(cls, path: str, base_dir: Optional[Path] = None) -> Path:
        """
        Sanitize a file path to prevent directory traversal attacks.

        Args:
            path: Input path to sanitize
            base_dir: Base directory to resolve relative paths against

        Returns:
            Sanitized absolute Path

        Raises:
            ValueError: If path contains dangerous patterns
        """
        if base_dir is None:
            base_dir = Path.cwd()

        # Convert to Path object
        try:
            input_path = Path(path)
        except (ValueError, OSError) as e:
            raise ValueError(f"Invalid path: {path}") from e

        # Check for dangerous patterns
        path_str = str(input_path)
        for pattern in cls.DANGEROUS_PATH_PATTERNS:
            if re.search(pattern, path_str, re.IGNORECASE):
                raise ValueError(f"Path contains dangerous pattern: {path}")

        # Check for dangerous absolute paths
        if input_path.is_absolute():
            for dangerous in cls.DANGEROUS_ABS_PATHS:
                if path_str.startswith(dangerous):
                    raise ValueError(
                        f"Path resolves to dangerous system location: {path_str}"
                    )

        # Resolve the path
        if input_path.is_absolute():
            resolved_path = input_path.resolve()
        else:
            resolved_path = (base_dir / input_path).resolve()

        # Ensure resolved path is within base directory or a safe location
        try:
            resolved_path.relative_to(base_dir.resolve())
        except ValueError:
            # Check if this is an absolute path that might be dangerous
            if input_path.is_absolute():
                # Check dangerous absolute paths
                dangerous_paths = cls.DANGEROUS_ABS_PATHS + ["/home"]
                for dangerous in dangerous_paths:
                    try:
                        resolved_path.relative_to(dangerous)
                        raise ValueError(
                            f"Path resolves to dangerous system location: {resolved_path}"
                        )
                    except ValueError:
                        continue
            else:
                # Relative path that goes outside base directory
                raise ValueError(
                    f"Path traversal detected: {path} -> {resolved_path}"
                )

        return resolved_path

    @classmethod
    def validate_config_value(cls, key: str, value: Any) -> Any:
        """
        Validate and sanitize configuration values.

        Args:
            key: Configuration key
            value: Configuration value

        Returns:
            Sanitized value

        Raises:
            ValueError: If value is invalid or dangerous
        """
        if value is None:
            return value

        # Type-specific validation
        if key in ["delay", "stats_interval", "max_iterations", "iteration_timeout"]:
            if isinstance(value, str):
                try:
                    value = int(value)
                except ValueError as e:
                    raise ValueError(f"Invalid integer value for {key}: {value}") from e

            # Validate ranges
            if value < 0:
                raise ValueError(f"{key} must be non-negative, got: {value}")
            if key == "delay" and value > 86400:  # 24 hours
                raise ValueError(f"{key} too large (>24 hours): {value}")
            if key == "max_iterations" and value > 10000:
                raise ValueError(f"{key} too large (>10000): {value}")
            if key == "stats_interval" and value > 3600:  # 1 hour
                raise ValueError(f"{key} too large (>1 hour): {value}")
            if key == "iteration_timeout" and value > 7200:  # 2 hours
                raise ValueError(f"{key} too large (>2 hours): {value}")

        elif key in ["log_file", "pid_file", "prompt_file", "system_prompt_file"]:
            if isinstance(value, str):
                # Sanitize file paths for non-prompt files
                if key not in ["prompt_file", "system_prompt_file"]:
                    cls.sanitize_path(value)

        elif key in [
            "verbose",
            "dry_run",
            "clear_screen",
            "show_countdown",
            "inject_best_practices",
        ]:
            # Boolean validation
            if isinstance(value, str):
                value = cls._parse_bool_safe(value)
            elif not isinstance(value, bool):
                raise ValueError(f"Invalid boolean value for {key}: {value}")

        elif key == "focus":
            if isinstance(value, str):
                # Sanitize focus text - remove potential command injection
                value = re.sub(r"[;&|`$()]", "", value)
                if len(value) > 200:
                    value = value[:200]

        return value

    @classmethod
    def _parse_bool_safe(cls, value: str) -> bool:
        """
        Safely parse boolean values from strings.

        Args:
            value: String value to parse

        Returns:
            Boolean value
        """
        if not value or not value.strip():
            return False

        value_lower = value.lower().strip()

        # Remove any dangerous characters
        value_clean = re.sub(r"[;&|`$()]", "", value_lower)

        true_values = ("true", "1", "yes", "on")
        false_values = ("false", "0", "no", "off")

        if value_clean in true_values:
            return True
        elif value_clean in false_values:
            return False
        else:
            # Default to False for ambiguous values
            return False

    @classmethod
    def mask_sensitive_data(cls, text: str) -> str:
        """
        Mask sensitive data in text for logging.

        Args:
            text: Text to mask sensitive data in

        Returns:
            Text with sensitive data masked
        """
        masked_text = text
        for pattern, replacement in cls.SENSITIVE_PATTERNS:
            masked_text = re.sub(pattern, replacement, masked_text, flags=re.IGNORECASE)
        return masked_text

    @classmethod
    def validate_filename(cls, filename: str) -> str:
        """
        Validate a filename for security.

        Args:
            filename: Filename to validate

        Returns:
            Sanitized filename

        Raises:
            ValueError: If filename is invalid or dangerous
        """
        if not filename or not filename.strip():
            raise ValueError("Filename cannot be empty")

        # Check for path traversal attempts in filename
        if ".." in filename or "/" in filename or "\\" in filename:
            raise ValueError(f"Filename contains path traversal: {filename}")

        # Remove dangerous characters
        sanitized = re.sub(r'[<>:"|?*\x00-\x1f]', "", filename.strip())

        if not sanitized:
            raise ValueError("Filename contains only invalid characters")

        # Prevent reserved names (Windows)
        reserved_names = {
            "CON",
            "PRN",
            "AUX",
            "NUL",
            "COM1",
            "COM2",
            "COM3",
            "COM4",
            "COM5",
            "COM6",
            "COM7",
            "COM8",
            "COM9",
            "LPT1",
            "LPT2",
            "LPT3",
            "LPT4",
            "LPT5",
            "LPT6",
            "LPT7",
            "LPT8",
            "LPT9",
        }

        name_without_ext = sanitized.split(".")[0].upper()
        if name_without_ext in reserved_names:
            raise ValueError(f"Filename uses reserved name: {filename}")

        # Check for control characters
        if any(ord(char) < 32 for char in filename):
            raise ValueError(f"Filename contains control characters: {filename}")

        # Limit length
        if len(sanitized) > 255:
            sanitized = sanitized[:255]

        return sanitized

    @classmethod
    def create_secure_logger(
        cls, name: str, log_file: Optional[str] = None
    ) -> logging.Logger:
        """
        Create a logger with security features enabled.

        Args:
            name: Logger name
            log_file: Optional log file path

        Returns:
            Secure logger instance
        """
        secure_logger = logging.getLogger(name)

        # Create custom formatter that masks sensitive data
        class SecureFormatter(logging.Formatter):
            def format(self, record):
                formatted = super().format(record)
                return cls.mask_sensitive_data(formatted)

        # Set up secure formatter
        if log_file:
            handler = logging.FileHandler(log_file)
        else:
            handler = logging.StreamHandler()

        handler.setFormatter(
            SecureFormatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
        )

        secure_logger.addHandler(handler)
        secure_logger.setLevel(logging.INFO)

        return secure_logger


class PathTraversalProtection:
    """Protection against path traversal attacks."""

    @staticmethod
    def safe_file_read(file_path: str, base_dir: Optional[Path] = None) -> str:
        """
        Safely read a file with path traversal protection.

        Args:
            file_path: Path to file to read
            base_dir: Base directory for relative paths

        Returns:
            File content

        Raises:
            ValueError: If path is dangerous
            FileNotFoundError: If file doesn't exist
            PermissionError: If file cannot be read
        """
        safe_path = SecurityValidator.sanitize_path(file_path, base_dir)

        if not safe_path.exists():
            raise FileNotFoundError(f"File not found: {safe_path}")

        if not safe_path.is_file():
            raise ValueError(f"Path is not a file: {safe_path}")

        try:
            return safe_path.read_text(encoding="utf-8")
        except PermissionError as e:
            raise PermissionError(f"Cannot read file: {safe_path}") from e

    @staticmethod
    def safe_file_write(
        file_path: str, content: str, base_dir: Optional[Path] = None
    ) -> None:
        """
        Safely write to a file with path traversal protection.

        Args:
            file_path: Path to file to write
            content: Content to write
            base_dir: Base directory for relative paths

        Raises:
            ValueError: If path is dangerous
            PermissionError: If file cannot be written
        """
        safe_path = SecurityValidator.sanitize_path(file_path, base_dir)

        # Create parent directories if needed
        safe_path.parent.mkdir(parents=True, exist_ok=True)

        try:
            safe_path.write_text(content, encoding="utf-8")
        except PermissionError as e:
            raise PermissionError(f"Cannot write file: {safe_path}") from e


# Security decorator for functions that handle file paths
def secure_file_operation(base_dir: Optional[Path] = None):
    """
    Decorator to secure file operations against path traversal.

    Args:
        base_dir: Base directory for relative paths
    """

    def decorator(func):
        def wrapper(*args, **kwargs):
            # Find path arguments and sanitize them
            new_args = []
            for arg in args:
                if isinstance(arg, str) and ("/" in arg or "\\" in arg):
                    arg = str(SecurityValidator.sanitize_path(arg, base_dir))
                new_args.append(arg)

            new_kwargs = {}
            for key, value in kwargs.items():
                if isinstance(value, str) and ("/" in value or "\\" in value):
                    value = str(SecurityValidator.sanitize_path(value, base_dir))
                new_kwargs[key] = value

            return func(*new_args, **new_kwargs)

        return wrapper

    return decorator
