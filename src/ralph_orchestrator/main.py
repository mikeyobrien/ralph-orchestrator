#!/usr/bin/env python3
# ABOUTME: Ralph orchestrator main loop implementation with multi-agent support
# ABOUTME: Implements the core Ralph Wiggum technique with continuous iteration

import sys
import logging
import argparse
import threading
import yaml
from pathlib import Path
from typing import Optional, Dict, Any, List
from dataclasses import dataclass, field
from enum import Enum

from .orchestrator import RalphOrchestrator


# Configuration defaults
DEFAULT_MAX_ITERATIONS = 100
DEFAULT_MAX_RUNTIME = 14400  # 4 hours
DEFAULT_PROMPT_FILE = "PROMPT.md"
DEFAULT_CHECKPOINT_INTERVAL = 5
DEFAULT_RETRY_DELAY = 2
DEFAULT_MAX_TOKENS = 1000000  # 1M tokens total
DEFAULT_MAX_COST = 50.0  # $50 USD
DEFAULT_CONTEXT_WINDOW = 200000  # 200K token context window
DEFAULT_CONTEXT_THRESHOLD = 0.8  # Trigger summarization at 80% of context
DEFAULT_METRICS_INTERVAL = 10  # Log metrics every 10 iterations
DEFAULT_MAX_PROMPT_SIZE = 10485760  # 10MB max prompt file size

# Token costs per million (approximate)
TOKEN_COSTS = {
    "claude": {"input": 3.0, "output": 15.0},  # Claude 3.5 Sonnet
    "q": {"input": 0.5, "output": 1.5},  # Estimated
    "gemini": {"input": 0.5, "output": 1.5}  # Gemini Pro
}

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.StreamHandler()
    ]
)
logger = logging.getLogger('ralph-orchestrator')

class AgentType(Enum):
    """Supported AI agent types"""
    CLAUDE = "claude"
    Q = "q"
    GEMINI = "gemini"
    ACP = "acp"
    AUTO = "auto"

class ConfigValidator:
    """Validates Ralph configuration settings.

    Provides validation methods for configuration parameters with security
    checks and warnings for unusual values.
    """

    # Validation thresholds
    LARGE_DELAY_THRESHOLD_SECONDS = 3600  # 1 hour
    SHORT_TIMEOUT_THRESHOLD_SECONDS = 10  # Very short timeout
    TYPICAL_AI_ITERATION_MIN_SECONDS = 30  # Typical minimum time for AI iteration
    TYPICAL_AI_ITERATION_MAX_SECONDS = 300  # Typical maximum time for AI iteration

    # Reasonable limits to prevent resource exhaustion
    MAX_ITERATIONS_LIMIT = 100000
    MAX_RUNTIME_LIMIT = 604800  # 1 week in seconds
    MAX_TOKENS_LIMIT = 100000000  # 100M tokens
    MAX_COST_LIMIT = 10000.0  # $10K USD

    @staticmethod
    def validate_max_iterations(max_iterations: int) -> List[str]:
        """Validate max iterations parameter."""
        errors = []
        if max_iterations < 0:
            errors.append("Max iterations must be non-negative")
        elif max_iterations > ConfigValidator.MAX_ITERATIONS_LIMIT:
            errors.append(f"Max iterations exceeds limit ({ConfigValidator.MAX_ITERATIONS_LIMIT})")
        return errors

    @staticmethod
    def validate_max_runtime(max_runtime: int) -> List[str]:
        """Validate max runtime parameter."""
        errors = []
        if max_runtime < 0:
            errors.append("Max runtime must be non-negative")
        elif max_runtime > ConfigValidator.MAX_RUNTIME_LIMIT:
            errors.append(f"Max runtime exceeds limit ({ConfigValidator.MAX_RUNTIME_LIMIT}s)")
        return errors

    @staticmethod
    def validate_checkpoint_interval(checkpoint_interval: int) -> List[str]:
        """Validate checkpoint interval parameter."""
        errors = []
        if checkpoint_interval < 0:
            errors.append("Checkpoint interval must be non-negative")
        return errors

    @staticmethod
    def validate_retry_delay(retry_delay: int) -> List[str]:
        """Validate retry delay parameter."""
        errors = []
        if retry_delay < 0:
            errors.append("Retry delay must be non-negative")
        elif retry_delay > ConfigValidator.LARGE_DELAY_THRESHOLD_SECONDS:
            errors.append(f"Retry delay exceeds limit ({ConfigValidator.LARGE_DELAY_THRESHOLD_SECONDS}s)")
        return errors

    @staticmethod
    def validate_max_tokens(max_tokens: int) -> List[str]:
        """Validate max tokens parameter."""
        errors = []
        if max_tokens < 0:
            errors.append("Max tokens must be non-negative")
        elif max_tokens > ConfigValidator.MAX_TOKENS_LIMIT:
            errors.append(f"Max tokens exceeds limit ({ConfigValidator.MAX_TOKENS_LIMIT})")
        return errors

    @staticmethod
    def validate_max_cost(max_cost: float) -> List[str]:
        """Validate max cost parameter."""
        errors = []
        if max_cost < 0:
            errors.append("Max cost must be non-negative")
        elif max_cost > ConfigValidator.MAX_COST_LIMIT:
            errors.append(f"Max cost exceeds limit (${ConfigValidator.MAX_COST_LIMIT})")
        return errors

    @staticmethod
    def validate_context_threshold(context_threshold: float) -> List[str]:
        """Validate context threshold parameter."""
        errors = []
        if not 0.0 <= context_threshold <= 1.0:
            errors.append("Context threshold must be between 0.0 and 1.0")
        return errors

    @staticmethod
    def validate_prompt_file(prompt_file: str) -> List[str]:
        """Validate prompt file exists and is readable."""
        errors = []
        path = Path(prompt_file)
        if not path.exists():
            errors.append(f"Prompt file not found: {prompt_file}")
        elif not path.is_file():
            errors.append(f"Prompt file is not a regular file: {prompt_file}")
        return errors

    @staticmethod
    def get_warning_large_delay(retry_delay: int) -> List[str]:
        """Check for unusually large delay values."""
        if retry_delay > ConfigValidator.LARGE_DELAY_THRESHOLD_SECONDS:
            return [
                f"Warning: Retry delay is very large ({retry_delay}s = {retry_delay/60:.1f}m). "
                f"Did you mean to use minutes instead of seconds?"
            ]
        return []

    @staticmethod
    def get_warning_single_iteration(max_iterations: int) -> List[str]:
        """Check for max_iterations=1."""
        if max_iterations == 1:
            return [
                "Warning: max_iterations is 1. "
                "Ralph is designed for continuous loops. Did you mean 0 (infinite)?"
            ]
        return []

    @staticmethod
    def get_warning_short_timeout(max_runtime: int) -> List[str]:
        """Check for very short runtime limits."""
        if 0 < max_runtime < ConfigValidator.SHORT_TIMEOUT_THRESHOLD_SECONDS:
            return [
                f"Warning: Max runtime is very short ({max_runtime}s). "
                f"AI iterations typically take {ConfigValidator.TYPICAL_AI_ITERATION_MIN_SECONDS}-"
                f"{ConfigValidator.TYPICAL_AI_ITERATION_MAX_SECONDS} seconds."
            ]
        return []


@dataclass
class AdapterConfig:
    """Configuration for individual adapters"""
    enabled: bool = True
    args: List[str] = field(default_factory=list)
    env: Dict[str, str] = field(default_factory=dict)
    timeout: int = 300
    max_retries: int = 3
    tool_permissions: Dict[str, Any] = field(default_factory=dict)

@dataclass
class RalphConfig:
    """Configuration for Ralph orchestrator.

    Thread-safe configuration class with RLock protection for mutable fields.
    Provides both direct attribute access (backwards compatible) and thread-safe
    getter/setter methods for concurrent access scenarios.
    """

    # Core configuration fields
    agent: AgentType = AgentType.AUTO
    # Agent selection and fallback priority (used when agent=auto, and for fallback ordering)
    # Valid values: "acp", "claude", "gemini", "qchat" (also accepts aliases: "codex"->"acp", "q"->"qchat")
    agent_priority: List[str] = field(default_factory=lambda: ["claude", "qchat", "gemini", "acp"])
    prompt_file: str = DEFAULT_PROMPT_FILE
    prompt_text: Optional[str] = None  # Direct prompt text (overrides prompt_file)
    max_iterations: int = DEFAULT_MAX_ITERATIONS
    max_runtime: int = DEFAULT_MAX_RUNTIME
    checkpoint_interval: int = DEFAULT_CHECKPOINT_INTERVAL
    retry_delay: int = DEFAULT_RETRY_DELAY
    archive_prompts: bool = True
    git_checkpoint: bool = True
    verbose: bool = False
    dry_run: bool = False
    max_tokens: int = DEFAULT_MAX_TOKENS
    max_cost: float = DEFAULT_MAX_COST
    context_window: int = DEFAULT_CONTEXT_WINDOW
    context_threshold: float = DEFAULT_CONTEXT_THRESHOLD
    metrics_interval: int = DEFAULT_METRICS_INTERVAL
    enable_metrics: bool = True
    max_prompt_size: int = DEFAULT_MAX_PROMPT_SIZE
    allow_unsafe_paths: bool = False
    agent_args: List[str] = field(default_factory=list)
    adapters: Dict[str, AdapterConfig] = field(default_factory=dict)

    # Output formatting configuration
    output_format: str = "rich"  # "plain", "rich", or "json"
    output_verbosity: str = "normal"  # "quiet", "normal", "verbose", "debug"
    show_token_usage: bool = True  # Display token usage after iterations
    show_timestamps: bool = True  # Include timestamps in output

    # Thread safety lock - not included in initialization/equals
    _lock: threading.RLock = field(
        default_factory=threading.RLock, init=False, repr=False, compare=False
    )

    # Thread-safe property access methods for mutable fields
    def get_max_iterations(self) -> int:
        """Thread-safe access to max_iterations property."""
        with self._lock:
            return self.max_iterations

    def set_max_iterations(self, value: int) -> None:
        """Thread-safe setting of max_iterations property."""
        with self._lock:
            object.__setattr__(self, 'max_iterations', value)

    def get_max_runtime(self) -> int:
        """Thread-safe access to max_runtime property."""
        with self._lock:
            return self.max_runtime

    def set_max_runtime(self, value: int) -> None:
        """Thread-safe setting of max_runtime property."""
        with self._lock:
            object.__setattr__(self, 'max_runtime', value)

    def get_checkpoint_interval(self) -> int:
        """Thread-safe access to checkpoint_interval property."""
        with self._lock:
            return self.checkpoint_interval

    def set_checkpoint_interval(self, value: int) -> None:
        """Thread-safe setting of checkpoint_interval property."""
        with self._lock:
            object.__setattr__(self, 'checkpoint_interval', value)

    def get_retry_delay(self) -> int:
        """Thread-safe access to retry_delay property."""
        with self._lock:
            return self.retry_delay

    def set_retry_delay(self, value: int) -> None:
        """Thread-safe setting of retry_delay property."""
        with self._lock:
            object.__setattr__(self, 'retry_delay', value)

    def get_max_tokens(self) -> int:
        """Thread-safe access to max_tokens property."""
        with self._lock:
            return self.max_tokens

    def set_max_tokens(self, value: int) -> None:
        """Thread-safe setting of max_tokens property."""
        with self._lock:
            object.__setattr__(self, 'max_tokens', value)

    def get_max_cost(self) -> float:
        """Thread-safe access to max_cost property."""
        with self._lock:
            return self.max_cost

    def set_max_cost(self, value: float) -> None:
        """Thread-safe setting of max_cost property."""
        with self._lock:
            object.__setattr__(self, 'max_cost', value)

    def get_verbose(self) -> bool:
        """Thread-safe access to verbose property."""
        with self._lock:
            return self.verbose

    def set_verbose(self, value: bool) -> None:
        """Thread-safe setting of verbose property."""
        with self._lock:
            object.__setattr__(self, 'verbose', value)

    @classmethod
    def from_yaml(cls, config_path: str) -> 'RalphConfig':
        """Load configuration from YAML file."""
        config_file = Path(config_path)
        if not config_file.exists():
            raise FileNotFoundError(f"Configuration file not found: {config_path}")

        with open(config_file, 'r') as f:
            config_data = yaml.safe_load(f)

        # Convert agent string to AgentType enum
        if 'agent' in config_data:
            config_data['agent'] = AgentType(config_data['agent'])

        # Process adapter configurations
        if 'adapters' in config_data:
            adapter_configs = {}
            for name, adapter_data in config_data['adapters'].items():
                if isinstance(adapter_data, dict):
                    adapter_configs[name] = AdapterConfig(**adapter_data)
                else:
                    # Simple boolean enable/disable
                    adapter_configs[name] = AdapterConfig(enabled=bool(adapter_data))
            config_data['adapters'] = adapter_configs

        # Filter out unknown keys
        valid_keys = {f.name for f in cls.__dataclass_fields__.values()}
        filtered_data = {k: v for k, v in config_data.items() if k in valid_keys}

        return cls(**filtered_data)

    def get_adapter_config(self, adapter_name: str) -> AdapterConfig:
        """Get configuration for a specific adapter."""
        with self._lock:
            return self.adapters.get(adapter_name, AdapterConfig())

    def validate(self) -> List[str]:
        """Validate configuration settings.

        Returns:
            List of validation errors (empty if valid).
        """
        errors = []

        with self._lock:
            errors.extend(ConfigValidator.validate_max_iterations(self.max_iterations))
            errors.extend(ConfigValidator.validate_max_runtime(self.max_runtime))
            errors.extend(ConfigValidator.validate_checkpoint_interval(self.checkpoint_interval))
            errors.extend(ConfigValidator.validate_retry_delay(self.retry_delay))
            errors.extend(ConfigValidator.validate_max_tokens(self.max_tokens))
            errors.extend(ConfigValidator.validate_max_cost(self.max_cost))
            errors.extend(ConfigValidator.validate_context_threshold(self.context_threshold))

        return errors

    def get_warnings(self) -> List[str]:
        """Get configuration warnings (non-blocking issues).

        Returns:
            List of warning messages.
        """
        warnings = []

        with self._lock:
            warnings.extend(ConfigValidator.get_warning_large_delay(self.retry_delay))
            warnings.extend(ConfigValidator.get_warning_single_iteration(self.max_iterations))
            warnings.extend(ConfigValidator.get_warning_short_timeout(self.max_runtime))

        return warnings

    def create_output_formatter(self):
        """Create an output formatter based on configuration settings.

        Returns:
            OutputFormatter instance configured according to settings.
        """
        from ralph_orchestrator.output import VerbosityLevel, create_formatter

        # Map verbosity string to enum
        verbosity_map = {
            "quiet": VerbosityLevel.QUIET,
            "normal": VerbosityLevel.NORMAL,
            "verbose": VerbosityLevel.VERBOSE,
            "debug": VerbosityLevel.DEBUG,
        }

        with self._lock:
            verbosity = verbosity_map.get(self.output_verbosity.lower(), VerbosityLevel.NORMAL)
            return create_formatter(
                format_type=self.output_format,
                verbosity=verbosity,
            )

def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(
        description="Ralph Wiggum Orchestrator - Put AI in a loop until done"
    )
    
    parser.add_argument(
        "--agent", "-a",
        type=str,
        choices=["claude", "q", "gemini", "acp", "auto"],
        default="auto",
        help="AI agent to use (default: auto-detect)"
    )
    
    parser.add_argument(
        "--prompt-file", "-P",
        type=str,
        default=DEFAULT_PROMPT_FILE,
        dest="prompt",
        help="Prompt file path (default: PROMPT.md)"
    )

    parser.add_argument(
        "--prompt-text", "-p",
        type=str,
        default=None,
        help="Direct prompt text (overrides --prompt-file)"
    )
    
    parser.add_argument(
        "--max-iterations", "-i",
        type=int,
        default=DEFAULT_MAX_ITERATIONS,
        help=f"Maximum iterations (default: {DEFAULT_MAX_ITERATIONS})"
    )
    
    parser.add_argument(
        "--max-runtime", "-t",
        type=int,
        default=DEFAULT_MAX_RUNTIME,
        help=f"Maximum runtime in seconds (default: {DEFAULT_MAX_RUNTIME})"
    )
    
    parser.add_argument(
        "--checkpoint-interval", "-c",
        type=int,
        default=DEFAULT_CHECKPOINT_INTERVAL,
        help=f"Checkpoint interval (default: {DEFAULT_CHECKPOINT_INTERVAL})"
    )
    
    parser.add_argument(
        "--retry-delay", "-r",
        type=int,
        default=DEFAULT_RETRY_DELAY,
        help=f"Retry delay in seconds (default: {DEFAULT_RETRY_DELAY})"
    )
    
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=DEFAULT_MAX_TOKENS,
        help=f"Maximum total tokens (default: {DEFAULT_MAX_TOKENS:,})"
    )
    
    parser.add_argument(
        "--max-cost",
        type=float,
        default=DEFAULT_MAX_COST,
        help=f"Maximum cost in USD (default: ${DEFAULT_MAX_COST:.2f})"
    )
    
    parser.add_argument(
        "--context-window",
        type=int,
        default=DEFAULT_CONTEXT_WINDOW,
        help=f"Context window size in tokens (default: {DEFAULT_CONTEXT_WINDOW:,})"
    )
    
    parser.add_argument(
        "--context-threshold",
        type=float,
        default=DEFAULT_CONTEXT_THRESHOLD,
        help=f"Context summarization threshold (default: {DEFAULT_CONTEXT_THRESHOLD:.1f} = {DEFAULT_CONTEXT_THRESHOLD*100:.0f}%%)"
    )
    
    parser.add_argument(
        "--metrics-interval",
        type=int,
        default=DEFAULT_METRICS_INTERVAL,
        help=f"Metrics logging interval (default: {DEFAULT_METRICS_INTERVAL})"
    )
    
    parser.add_argument(
        "--no-metrics",
        action="store_true",
        help="Disable metrics collection"
    )
    
    parser.add_argument(
        "--max-prompt-size",
        type=int,
        default=DEFAULT_MAX_PROMPT_SIZE,
        help=f"Maximum prompt file size in bytes (default: {DEFAULT_MAX_PROMPT_SIZE})"
    )
    
    parser.add_argument(
        "--allow-unsafe-paths",
        action="store_true",
        help="Allow potentially unsafe prompt paths (use with caution)"
    )
    
    parser.add_argument(
        "--no-git",
        action="store_true",
        help="Disable git checkpointing"
    )
    
    parser.add_argument(
        "--no-archive",
        action="store_true",
        help="Disable prompt archiving"
    )
    
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Enable verbose output"
    )

    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Dry run mode (don't execute agents)"
    )

    # Output formatting options
    parser.add_argument(
        "--output-format",
        type=str,
        choices=["plain", "rich", "json"],
        default="rich",
        help="Output format (default: rich)"
    )

    parser.add_argument(
        "--output-verbosity",
        type=str,
        choices=["quiet", "normal", "verbose", "debug"],
        default="normal",
        help="Output verbosity level (default: normal)"
    )

    parser.add_argument(
        "--no-token-usage",
        action="store_true",
        help="Disable token usage display"
    )

    parser.add_argument(
        "--no-timestamps",
        action="store_true",
        help="Disable timestamps in output"
    )

    parser.add_argument(
        "agent_args",
        nargs="*",
        help="Additional arguments to pass to the AI agent"
    )
    
    args = parser.parse_args()
    
    # Configure logging level
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Create config
    config = RalphConfig(
        agent=AgentType(args.agent),
        prompt_file=args.prompt,
        prompt_text=args.prompt_text,
        max_iterations=args.max_iterations,
        max_runtime=args.max_runtime,
        checkpoint_interval=args.checkpoint_interval,
        retry_delay=args.retry_delay,
        archive_prompts=not args.no_archive,
        git_checkpoint=not args.no_git,
        verbose=args.verbose,
        dry_run=args.dry_run,
        max_tokens=args.max_tokens,
        max_cost=args.max_cost,
        context_window=args.context_window,
        context_threshold=args.context_threshold,
        metrics_interval=args.metrics_interval,
        enable_metrics=not args.no_metrics,
        max_prompt_size=args.max_prompt_size,
        allow_unsafe_paths=args.allow_unsafe_paths,
        agent_args=args.agent_args,
        # Output formatting options
        output_format=args.output_format,
        output_verbosity=args.output_verbosity,
        show_token_usage=not args.no_token_usage,
        show_timestamps=not args.no_timestamps,
    )
    
    # Run orchestrator
    orchestrator = RalphOrchestrator(config)
    return orchestrator.run()

if __name__ == "__main__":
    sys.exit(main())
