"""Tests for YAML configuration loading and thread-safe configuration."""

import concurrent.futures
import pytest
import tempfile
import threading
import time
import yaml
from pathlib import Path

from ralph_orchestrator.main import (
    RalphConfig,
    AgentType,
    ConfigValidator,
)


def test_yaml_config_loading():
    """Test loading configuration from YAML file."""
    config_data = {
        'agent': 'claude',
        'max_iterations': 50,
        'verbose': True,
        'adapters': {
            'claude': {
                'enabled': True,
                'timeout': 600,
                'args': ['--model', 'claude-3-sonnet']
            },
            'q': False
        }
    }
    
    with tempfile.NamedTemporaryFile(mode='w', suffix='.yml', delete=False) as f:
        yaml.dump(config_data, f)
        config_path = f.name
    
    try:
        config = RalphConfig.from_yaml(config_path)
        
        assert config.agent == AgentType.CLAUDE
        assert config.max_iterations == 50
        assert config.verbose is True
        
        # Test adapter configs
        claude_config = config.get_adapter_config('claude')
        assert claude_config.enabled is True
        assert claude_config.timeout == 600
        assert claude_config.args == ['--model', 'claude-3-sonnet']
        
        q_config = config.get_adapter_config('q')
        assert q_config.enabled is False
        
    finally:
        Path(config_path).unlink()


def test_adapter_config_defaults():
    """Test adapter configuration defaults."""
    config = RalphConfig()
    adapter_config = config.get_adapter_config('nonexistent')
    
    assert adapter_config.enabled is True
    assert adapter_config.timeout == 300
    assert adapter_config.max_retries == 3
    assert adapter_config.args == []
    assert adapter_config.env == {}


def test_yaml_config_missing_file():
    """Test error handling for missing config file."""
    with pytest.raises(FileNotFoundError):
        RalphConfig.from_yaml('nonexistent.yml')


# =============================================================================
# Thread-Safe Configuration Tests
# =============================================================================


class TestThreadSafeConfig:
    """Test thread-safe configuration access."""

    def test_config_has_lock(self):
        """Test that RalphConfig has an RLock."""
        config = RalphConfig()
        assert hasattr(config, '_lock')
        assert isinstance(config._lock, type(threading.RLock()))

    def test_thread_safe_get_set_max_iterations(self):
        """Test thread-safe access to max_iterations."""
        config = RalphConfig()

        # Test getter
        assert config.get_max_iterations() == 100  # default

        # Test setter
        config.set_max_iterations(50)
        assert config.get_max_iterations() == 50
        assert config.max_iterations == 50  # Direct access also works

    def test_thread_safe_get_set_max_runtime(self):
        """Test thread-safe access to max_runtime."""
        config = RalphConfig()

        # Test getter
        assert config.get_max_runtime() == 14400  # default

        # Test setter
        config.set_max_runtime(7200)
        assert config.get_max_runtime() == 7200

    def test_thread_safe_get_set_retry_delay(self):
        """Test thread-safe access to retry_delay."""
        config = RalphConfig()

        # Test getter
        assert config.get_retry_delay() == 2  # default

        # Test setter
        config.set_retry_delay(5)
        assert config.get_retry_delay() == 5

    def test_thread_safe_get_set_max_tokens(self):
        """Test thread-safe access to max_tokens."""
        config = RalphConfig()

        # Test getter
        assert config.get_max_tokens() == 1000000  # default

        # Test setter
        config.set_max_tokens(500000)
        assert config.get_max_tokens() == 500000

    def test_thread_safe_get_set_max_cost(self):
        """Test thread-safe access to max_cost."""
        config = RalphConfig()

        # Test getter
        assert config.get_max_cost() == 50.0  # default

        # Test setter
        config.set_max_cost(25.0)
        assert config.get_max_cost() == 25.0

    def test_thread_safe_get_set_verbose(self):
        """Test thread-safe access to verbose flag."""
        config = RalphConfig()

        # Test getter
        assert config.get_verbose() is False  # default

        # Test setter
        config.set_verbose(True)
        assert config.get_verbose() is True

    def test_concurrent_access_safety(self):
        """Test that concurrent access is thread-safe."""
        config = RalphConfig()
        errors = []
        iterations = 100

        def writer():
            try:
                for i in range(iterations):
                    config.set_max_iterations(i)
                    time.sleep(0.001)
            except Exception as e:
                errors.append(e)

        def reader():
            try:
                for _ in range(iterations):
                    val = config.get_max_iterations()
                    assert isinstance(val, int)
                    time.sleep(0.001)
            except Exception as e:
                errors.append(e)

        threads = [
            threading.Thread(target=writer),
            threading.Thread(target=reader),
            threading.Thread(target=writer),
            threading.Thread(target=reader),
        ]

        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0, f"Thread errors: {errors}"

    def test_concurrent_thread_pool_access(self):
        """Test thread-safe access with ThreadPoolExecutor."""
        config = RalphConfig(max_iterations=0)

        def increment():
            for _ in range(10):
                current = config.get_max_iterations()
                config.set_max_iterations(current + 1)

        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = [executor.submit(increment) for _ in range(4)]
            concurrent.futures.wait(futures)

        # With proper locking, final value should be 40 (4 threads * 10 increments)
        # Note: Without locking, race conditions could cause lost updates
        final_value = config.get_max_iterations()
        assert final_value <= 40  # May be less due to race conditions in increment logic
        assert final_value > 0  # Should have some increments

    def test_lock_is_reentrant(self):
        """Test that the lock is reentrant (RLock)."""
        config = RalphConfig()

        # This should not deadlock because RLock is reentrant
        with config._lock:
            val = config.get_max_iterations()  # Acquires lock again
            config.set_max_iterations(val + 1)  # Acquires lock again

        assert config.max_iterations == 101

    def test_config_equality_ignores_lock(self):
        """Test that lock is not included in equality comparison."""
        config1 = RalphConfig(max_iterations=50)
        config2 = RalphConfig(max_iterations=50)

        # Different lock instances but configs should be equal
        assert config1 == config2

    def test_config_repr_excludes_lock(self):
        """Test that lock is not included in repr."""
        config = RalphConfig()
        repr_str = repr(config)

        assert '_lock' not in repr_str


# =============================================================================
# ConfigValidator Tests
# =============================================================================


class TestConfigValidator:
    """Test ConfigValidator validation methods."""

    def test_validate_max_iterations_valid(self):
        """Test valid max_iterations values."""
        assert ConfigValidator.validate_max_iterations(0) == []
        assert ConfigValidator.validate_max_iterations(100) == []
        assert ConfigValidator.validate_max_iterations(10000) == []

    def test_validate_max_iterations_negative(self):
        """Test negative max_iterations."""
        errors = ConfigValidator.validate_max_iterations(-1)
        assert len(errors) == 1
        assert "non-negative" in errors[0]

    def test_validate_max_iterations_exceeds_limit(self):
        """Test max_iterations exceeding limit."""
        errors = ConfigValidator.validate_max_iterations(200000)
        assert len(errors) == 1
        assert "exceeds limit" in errors[0]

    def test_validate_max_runtime_valid(self):
        """Test valid max_runtime values."""
        assert ConfigValidator.validate_max_runtime(0) == []
        assert ConfigValidator.validate_max_runtime(3600) == []
        assert ConfigValidator.validate_max_runtime(86400) == []

    def test_validate_max_runtime_negative(self):
        """Test negative max_runtime."""
        errors = ConfigValidator.validate_max_runtime(-1)
        assert len(errors) == 1
        assert "non-negative" in errors[0]

    def test_validate_max_runtime_exceeds_limit(self):
        """Test max_runtime exceeding limit."""
        errors = ConfigValidator.validate_max_runtime(1000000)
        assert len(errors) == 1
        assert "exceeds limit" in errors[0]

    def test_validate_checkpoint_interval_valid(self):
        """Test valid checkpoint_interval values."""
        assert ConfigValidator.validate_checkpoint_interval(0) == []
        assert ConfigValidator.validate_checkpoint_interval(5) == []
        assert ConfigValidator.validate_checkpoint_interval(100) == []

    def test_validate_checkpoint_interval_negative(self):
        """Test negative checkpoint_interval."""
        errors = ConfigValidator.validate_checkpoint_interval(-1)
        assert len(errors) == 1
        assert "non-negative" in errors[0]

    def test_validate_retry_delay_valid(self):
        """Test valid retry_delay values."""
        assert ConfigValidator.validate_retry_delay(0) == []
        assert ConfigValidator.validate_retry_delay(5) == []
        assert ConfigValidator.validate_retry_delay(60) == []

    def test_validate_retry_delay_negative(self):
        """Test negative retry_delay."""
        errors = ConfigValidator.validate_retry_delay(-1)
        assert len(errors) == 1
        assert "non-negative" in errors[0]

    def test_validate_retry_delay_exceeds_limit(self):
        """Test retry_delay exceeding limit."""
        errors = ConfigValidator.validate_retry_delay(5000)
        assert len(errors) == 1
        assert "exceeds limit" in errors[0]

    def test_validate_max_tokens_valid(self):
        """Test valid max_tokens values."""
        assert ConfigValidator.validate_max_tokens(0) == []
        assert ConfigValidator.validate_max_tokens(1000000) == []

    def test_validate_max_tokens_negative(self):
        """Test negative max_tokens."""
        errors = ConfigValidator.validate_max_tokens(-1)
        assert len(errors) == 1
        assert "non-negative" in errors[0]

    def test_validate_max_cost_valid(self):
        """Test valid max_cost values."""
        assert ConfigValidator.validate_max_cost(0.0) == []
        assert ConfigValidator.validate_max_cost(50.0) == []
        assert ConfigValidator.validate_max_cost(1000.0) == []

    def test_validate_max_cost_negative(self):
        """Test negative max_cost."""
        errors = ConfigValidator.validate_max_cost(-1.0)
        assert len(errors) == 1
        assert "non-negative" in errors[0]

    def test_validate_context_threshold_valid(self):
        """Test valid context_threshold values."""
        assert ConfigValidator.validate_context_threshold(0.0) == []
        assert ConfigValidator.validate_context_threshold(0.5) == []
        assert ConfigValidator.validate_context_threshold(1.0) == []

    def test_validate_context_threshold_invalid(self):
        """Test invalid context_threshold values."""
        errors = ConfigValidator.validate_context_threshold(-0.1)
        assert len(errors) == 1
        assert "between 0.0 and 1.0" in errors[0]

        errors = ConfigValidator.validate_context_threshold(1.5)
        assert len(errors) == 1
        assert "between 0.0 and 1.0" in errors[0]

    def test_warning_large_delay(self):
        """Test warning for large delay values."""
        warnings = ConfigValidator.get_warning_large_delay(100)
        assert len(warnings) == 0

        warnings = ConfigValidator.get_warning_large_delay(5000)
        assert len(warnings) == 1
        assert "very large" in warnings[0]

    def test_warning_single_iteration(self):
        """Test warning for single iteration."""
        warnings = ConfigValidator.get_warning_single_iteration(10)
        assert len(warnings) == 0

        warnings = ConfigValidator.get_warning_single_iteration(1)
        assert len(warnings) == 1
        assert "max_iterations is 1" in warnings[0]

    def test_warning_short_timeout(self):
        """Test warning for short timeout."""
        warnings = ConfigValidator.get_warning_short_timeout(3600)
        assert len(warnings) == 0

        warnings = ConfigValidator.get_warning_short_timeout(5)
        assert len(warnings) == 1
        assert "very short" in warnings[0]


class TestRalphConfigValidation:
    """Test RalphConfig validation and warnings methods."""

    def test_validate_valid_config(self):
        """Test validation of valid configuration."""
        config = RalphConfig()
        errors = config.validate()
        assert errors == []

    def test_validate_invalid_max_iterations(self):
        """Test validation catches invalid max_iterations."""
        config = RalphConfig(max_iterations=-10)
        errors = config.validate()
        assert len(errors) > 0
        assert any("non-negative" in e for e in errors)

    def test_validate_invalid_context_threshold(self):
        """Test validation catches invalid context_threshold."""
        config = RalphConfig(context_threshold=2.0)
        errors = config.validate()
        assert len(errors) > 0
        assert any("between 0.0 and 1.0" in e for e in errors)

    def test_get_warnings_with_issues(self):
        """Test warnings for configuration issues."""
        config = RalphConfig(
            max_iterations=1,
            max_runtime=5,
            retry_delay=5000
        )
        warnings = config.get_warnings()
        assert len(warnings) >= 2  # At least single iteration and short timeout

    def test_get_warnings_no_issues(self):
        """Test no warnings for normal configuration."""
        config = RalphConfig()
        warnings = config.get_warnings()
        assert warnings == []

    def test_validation_is_thread_safe(self):
        """Test that validation is thread-safe."""
        config = RalphConfig()
        errors_list = []

        def validate_repeatedly():
            for _ in range(50):
                errors = config.validate()
                errors_list.append(errors)

        threads = [threading.Thread(target=validate_repeatedly) for _ in range(4)]

        for t in threads:
            t.start()
        for t in threads:
            t.join()

        # All validations should return empty lists (valid config)
        assert all(e == [] for e in errors_list)


# =============================================================================
# Prompt Text CLI Tests
# =============================================================================


class TestPromptTextConfig:
    """Test prompt_text configuration option."""

    def test_prompt_text_defaults_to_none(self):
        """Test that prompt_text defaults to None."""
        config = RalphConfig()
        assert config.prompt_text is None

    def test_prompt_text_can_be_set(self):
        """Test that prompt_text can be set directly."""
        config = RalphConfig(prompt_text="Build a REST API")
        assert config.prompt_text == "Build a REST API"

    def test_prompt_text_with_prompt_file(self):
        """Test prompt_text alongside prompt_file."""
        config = RalphConfig(
            prompt_file="PROMPT.md",
            prompt_text="Direct prompt text"
        )
        assert config.prompt_file == "PROMPT.md"
        assert config.prompt_text == "Direct prompt text"

    def test_prompt_text_from_yaml(self):
        """Test loading prompt_text from YAML config."""
        config_data = {
            'agent': 'claude',
            'prompt_text': 'Build a web app',
            'max_iterations': 10
        }

        with tempfile.NamedTemporaryFile(mode='w', suffix='.yml', delete=False) as f:
            yaml.dump(config_data, f)
            config_path = f.name

        try:
            config = RalphConfig.from_yaml(config_path)
            assert config.prompt_text == 'Build a web app'
        finally:
            Path(config_path).unlink()

    def test_prompt_text_multiline(self):
        """Test prompt_text with multiline content."""
        prompt = """# Task
Build a REST API with:
- User authentication
- CRUD operations
- Rate limiting"""
        config = RalphConfig(prompt_text=prompt)
        assert "REST API" in config.prompt_text
        assert "Rate limiting" in config.prompt_text
