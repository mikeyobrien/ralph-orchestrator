# Security API Reference

This page documents the security utilities available in `ralph_orchestrator.security`.

## SecurityValidator

The `SecurityValidator` class provides static methods for input validation, path sanitization, and sensitive data protection.

### Methods

#### `sanitize_path`

Sanitizes a file path to prevent directory traversal attacks.

```python
@classmethod
def sanitize_path(cls, path: str, base_dir: Optional[Path] = None) -> Path
```

**Arguments:**

- `path` (str): Input path to sanitize.
- `base_dir` (Optional[Path]): Base directory to resolve relative paths against. Defaults to current working directory.

**Returns:**

- `Path`: Sanitized absolute Path.

**Raises:**

- `ValueError`: If the path contains dangerous patterns (e.g., `../`, null bytes) or resolves to a location outside the base directory/allowed system paths.

**Example:**

```python
from ralph_orchestrator.security import SecurityValidator

# Safe usage
path = SecurityValidator.sanitize_path("data/output.json")

# Dangerous usage (will raise ValueError)
# path = SecurityValidator.sanitize_path("../../etc/passwd")
```

#### `mask_sensitive_data`

Masks sensitive data in text for logging purposes. Supports detection of API keys, bearer tokens, passwords, and other secrets.

```python
@classmethod
def mask_sensitive_data(cls, text: str) -> str
```

**Arguments:**

- `text` (str): Text to mask sensitive data in.

**Returns:**

- `str`: Text with sensitive data replaced by masked values (e.g., `sk-***********`).

**Example:**

```python
log_message = "Using API key sk-1234567890abcdef"
safe_log = SecurityValidator.mask_sensitive_data(log_message)
print(safe_log)
# Output: Using API key sk-***********
```

#### `validate_filename`

Validates a filename for security, checking for forbidden characters, reserved names, and length limits.

```python
@classmethod
def validate_filename(cls, filename: str) -> str
```

**Arguments:**

- `filename` (str): Filename to validate.

**Returns:**

- `str`: Sanitized filename.

**Raises:**

- `ValueError`: If filename is empty, contains path separators, invalid characters, or is a reserved name (e.g., `CON` on Windows).

#### `validate_config_value`

Validates and sanitizes configuration values based on their key.

```python
@classmethod
def validate_config_value(cls, key: str, value: Any) -> Any
```

**Arguments:**

- `key` (str): Configuration key (e.g., "max_iterations", "log_file").
- `value` (Any): Configuration value.

**Returns:**

- `Any`: Sanitized value.

#### `create_secure_logger`

Creates a logger instance that automatically masks sensitive data in log records.

```python
@classmethod
def create_secure_logger(cls, name: str, log_file: Optional[str] = None) -> logging.Logger
```

**Arguments:**

- `name` (str): Logger name.
- `log_file` (Optional[str]): Path to log file. If None, logs to console.

**Returns:**

- `logging.Logger`: Configured logger instance.

## PathTraversalProtection

Utilities specifically for preventing path traversal in file operations.

### Methods

#### `safe_file_read`

Safely reads a file with path traversal protection.

```python
@staticmethod
def safe_file_read(file_path: str, base_dir: Optional[Path] = None) -> str
```

**Arguments:**

- `file_path` (str): Path to file.
- `base_dir` (Optional[Path]): Base directory.

**Returns:**

- `str`: File content (utf-8).

#### `safe_file_write`

Safely writes to a file with path traversal protection. Creates parent directories if needed.

```python
@staticmethod
def safe_file_write(file_path: str, content: str, base_dir: Optional[Path] = None) -> None
```

**Arguments:**

- `file_path` (str): Path to file.
- `content` (str): Content to write.
- `base_dir` (Optional[Path]): Base directory.

## Decorators

### `secure_file_operation`

Decorator to secure functions that handle file paths as arguments. Automatically sanitizes any string argument containing path separators.

```python
@secure_file_operation(base_dir=None)
def my_file_func(path, content):
    ...
```
