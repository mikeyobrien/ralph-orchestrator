# Orchestrator API Reference

Complete API documentation for the Hats core module.

## Module: `hats_orchestrator`

The main orchestration module that coordinates AI agent execution.

## Classes

### `HatsOrchestrator`

Main orchestrator class managing the execution loop.

```python
class HatsOrchestrator:
    def __init__(
        self,
        prompt_file_or_config = None,
        primary_tool: str = "claude",
        max_iterations: int = 100,
        max_runtime: int = 14400,
        track_costs: bool = False,
        max_cost: float = 10.0,
        checkpoint_interval: int = 5,
        archive_dir: str = "./prompts/archive",
        verbose: bool = False
    ):
        """Initialize the orchestrator with configuration or individual parameters."""
```

#### Methods

##### `run()`

```python
def run(self) -> None:
    """Run the orchestration loop until completion or limits reached."""
```

##### `arun()`

```python
async def arun(self) -> None:
    """Run the orchestration loop asynchronously."""
```

### `HatsConfig`

Configuration dataclass for the orchestrator.

```python
@dataclass
class HatsConfig:
    agent: AgentType = AgentType.AUTO
    prompt_file: str = "PROMPT.md"
    max_iterations: int = 100
    max_runtime: int = 14400
    checkpoint_interval: int = 5
    retry_delay: int = 2
    archive_prompts: bool = True
    git_checkpoint: bool = True
    verbose: bool = False
    dry_run: bool = False
    max_tokens: int = 1000000
    max_cost: float = 50.0
    context_window: int = 200000
    context_threshold: float = 0.8
    metrics_interval: int = 10
    enable_metrics: bool = True
    max_prompt_size: int = 10485760
    allow_unsafe_paths: bool = False
    agent_args: List[str] = field(default_factory=list)
    adapters: Dict[str, AdapterConfig] = field(default_factory=dict)
```

### `AgentType`

```python
class AgentType(Enum):
    CLAUDE = "claude"
    Q = "q"
    GEMINI = "gemini"
    AUTO = "auto"
```

## Functions

### `main()`

Entry point for CLI execution.

```python
def main() -> int:
    """Main entry point for CLI execution."""
```

## Usage Examples

```python
from hats_orchestrator import HatsOrchestrator, HatsConfig

# Using config object
config = HatsConfig(agent=AgentType.CLAUDE)
orchestrator = HatsOrchestrator(config)
orchestrator.run()

# Using individual parameters
orchestrator = HatsOrchestrator(
    prompt_file_or_config="PROMPT.md",
    primary_tool="claude",
    max_iterations=50
)
orchestrator.run()
```

The main orchestration module that implements the Hats Wiggum technique.

### Classes

#### `HatsOrchestrator`

The main orchestrator class that manages the iteration loop.

```python
class HatsOrchestrator:
    """
    Orchestrates AI agent iterations for autonomous task completion.
    
    Attributes:
        config (HatsConfig): Configuration object
        agent (Agent): Active AI agent instance
        metrics (MetricsCollector): Metrics tracking
        state (OrchestratorState): Current state
    """
```

##### Constructor

```python
def __init__(self, config: HatsConfig) -> None:
    """
    Initialize the orchestrator with configuration.
    
    Args:
        config: HatsConfig object with settings
        
    Raises:
        ValueError: If configuration is invalid
        RuntimeError: If no agents are available
    """
```

##### Methods

###### `run()`

```python
def run(self) -> int:
    """
    Execute the main orchestration loop.
    
    Returns:
        int: Exit code (0 for success, non-zero for failure)
        
    Raises:
        SecurityError: If security validation fails
        RuntimeError: If unrecoverable error occurs
    """
```

###### `iterate()`

```python
def iterate(self) -> bool:
    """
    Execute a single iteration.
    
    Returns:
        bool: True if task is complete, False otherwise
        
    Raises:
        AgentError: If agent execution fails
        TokenLimitError: If token limit exceeded
        CostLimitError: If cost limit exceeded
    """
```

###### `checkpoint()`

```python
def checkpoint(self) -> None:
    """
    Create a Git checkpoint of current state.
    
    Raises:
        GitError: If Git operations fail
    """
```

###### `save_state()`

```python
def save_state(self) -> None:
    """
    Persist current state to disk.
    
    The state includes:
    - Current iteration number
    - Token usage
    - Cost accumulation
    - Timestamps
    - Agent information
    """
```

###### `load_state()`

```python
def load_state(self) -> Optional[OrchestratorState]:
    """
    Load previous state from disk.
    
    Returns:
        OrchestratorState or None if no state exists
    """
```

#### `HatsConfig`

Configuration dataclass for the orchestrator.

```python
@dataclass
class HatsConfig:
    """
    Configuration for Hats orchestrator.
    
    All parameters can be set via:
    - Command-line arguments
    - Environment variables (HATS_*)
    - Configuration file (.hats.conf)
    - Default values
    """
    
    # Agent configuration
    agent: AgentType = AgentType.AUTO
    agent_args: List[str] = field(default_factory=list)
    
    # File paths
    prompt_file: str = "PROMPT.md"
    
    # Iteration limits
    max_iterations: int = 100
    max_runtime: int = 14400  # 4 hours
    
    # Token and cost limits
    max_tokens: int = 1000000  # 1M tokens
    max_cost: float = 50.0  # $50 USD
    
    # Context management
    context_window: int = 200000  # 200K tokens
    context_threshold: float = 0.8  # 80% trigger
    
    # Checkpointing
    checkpoint_interval: int = 5
    git_checkpoint: bool = True
    archive_prompts: bool = True
    
    # Retry configuration
    retry_delay: int = 2
    max_retries: int = 3
    
    # Monitoring
    metrics_interval: int = 10
    enable_metrics: bool = True
    
    # Security
    max_prompt_size: int = 10485760  # 10MB
    allow_unsafe_paths: bool = False
    
    # Output
    verbose: bool = False
    dry_run: bool = False
```

#### `OrchestratorState`

State tracking for the orchestrator.

```python
@dataclass
class OrchestratorState:
    """
    Orchestrator state for persistence and recovery.
    """
    
    # Iteration tracking
    current_iteration: int = 0
    total_iterations: int = 0
    
    # Time tracking
    start_time: datetime = field(default_factory=datetime.now)
    last_iteration_time: Optional[datetime] = None
    total_runtime: float = 0.0
    
    # Token tracking
    total_input_tokens: int = 0
    total_output_tokens: int = 0
    
    # Cost tracking
    total_cost: float = 0.0
    
    # Agent information
    agent_type: str = ""
    agent_version: Optional[str] = None
    
    # Completion status
    is_complete: bool = False
    completion_reason: Optional[str] = None
```

### Functions

#### `detect_agents()`

```python
def detect_agents() -> List[AgentType]:
    """
    Detect available AI agents on the system.
    
    Returns:
        List of available AgentType enums
        
    Example:
        >>> detect_agents()
        [AgentType.CLAUDE, AgentType.GEMINI]
    """
```

#### `validate_prompt_file()`

```python
def validate_prompt_file(
    file_path: str, 
    max_size: int = DEFAULT_MAX_PROMPT_SIZE
) -> None:
    """
    Validate prompt file for security and size.
    
    Args:
        file_path: Path to prompt file
        max_size: Maximum allowed file size in bytes
        
    Raises:
        FileNotFoundError: If file doesn't exist
        SecurityError: If file contains dangerous patterns
        ValueError: If file exceeds size limit
    """
```

#### `sanitize_input()`

```python
def sanitize_input(text: str) -> str:
    """
    Sanitize input text for security.
    
    Args:
        text: Input text to sanitize
        
    Returns:
        Sanitized text safe for processing
        
    Example:
        >>> sanitize_input("rm -rf /; echo 'done'")
        "rm -rf _; echo 'done'"
    """
```

#### `calculate_cost()`

```python
def calculate_cost(
    input_tokens: int,
    output_tokens: int,
    agent_type: AgentType
) -> float:
    """
    Calculate cost based on token usage.
    
    Args:
        input_tokens: Number of input tokens
        output_tokens: Number of output tokens
        agent_type: Type of agent used
        
    Returns:
        Cost in USD
        
    Example:
        >>> calculate_cost(1000, 500, AgentType.CLAUDE)
        0.0105  # $0.0105
    """
```

### Exceptions

#### `OrchestratorError`

Base exception for orchestrator errors.

```python
class OrchestratorError(Exception):
    """Base exception for orchestrator errors."""
    pass
```

#### `SecurityError`

```python
class SecurityError(OrchestratorError):
    """Raised when security validation fails."""
    pass
```

#### `TokenLimitError`

```python
class TokenLimitError(OrchestratorError):
    """Raised when token limit is exceeded."""
    pass
```

#### `CostLimitError`

```python
class CostLimitError(OrchestratorError):
    """Raised when cost limit is exceeded."""
    pass
```

#### `AgentError`

```python
class AgentError(OrchestratorError):
    """Raised when agent execution fails."""
    pass
```

### Constants

```python
# Version
VERSION = "1.0.0"

# Default values
DEFAULT_MAX_ITERATIONS = 100
DEFAULT_MAX_RUNTIME = 14400  # 4 hours
DEFAULT_PROMPT_FILE = "PROMPT.md"
DEFAULT_CHECKPOINT_INTERVAL = 5
DEFAULT_RETRY_DELAY = 2
DEFAULT_MAX_TOKENS = 1000000  # 1M tokens
DEFAULT_MAX_COST = 50.0  # $50 USD
DEFAULT_CONTEXT_WINDOW = 200000  # 200K tokens
DEFAULT_CONTEXT_THRESHOLD = 0.8  # 80%
DEFAULT_METRICS_INTERVAL = 10
DEFAULT_MAX_PROMPT_SIZE = 10485760  # 10MB

# Token costs per million
TOKEN_COSTS = {
    "claude": {"input": 3.0, "output": 15.0},
    "q": {"input": 0.5, "output": 1.5},
    "gemini": {"input": 0.5, "output": 1.5}
}

# Legacy completion markers (deprecated - orchestrator now uses iteration/cost/time limits)
# COMPLETION_MARKERS = ["TASK_COMPLETE", "TASK_DONE", "COMPLETE"]

# Security patterns
DANGEROUS_PATTERNS = [
    r"rm\s+-rf\s+/",
    r":(){ :|:& };:",
    r"dd\s+if=/dev/zero",
    r"mkfs\.",
    r"format\s+[cC]:",
]
```

## Usage Examples

### Basic Usage

```python
from hats_orchestrator import HatsOrchestrator, HatsConfig

# Create configuration
config = HatsConfig(
    agent=AgentType.CLAUDE,
    prompt_file="task.md",
    max_iterations=50,
    max_cost=25.0
)

# Initialize orchestrator
orchestrator = HatsOrchestrator(config)

# Run orchestration
exit_code = orchestrator.run()
```

### Custom Configuration

```python
# Load from environment and add overrides
config = HatsConfig()
config.max_iterations = 100
config.checkpoint_interval = 10
config.verbose = True

# Initialize with custom config
orchestrator = HatsOrchestrator(config)
```

### State Management

```python
# Save state manually
orchestrator.save_state()

# Load previous state
state = orchestrator.load_state()
if state:
    print(f"Resuming from iteration {state.current_iteration}")
```

### Error Handling

```python
try:
    orchestrator = HatsOrchestrator(config)
    exit_code = orchestrator.run()
except SecurityError as e:
    print(f"Security violation: {e}")
except TokenLimitError as e:
    print(f"Token limit exceeded: {e}")
except CostLimitError as e:
    print(f"Cost limit exceeded: {e}")
except Exception as e:
    print(f"Unexpected error: {e}")
```

## Thread Safety

The orchestrator is **not thread-safe**. If you need concurrent execution:

1. Create separate orchestrator instances
2. Use different working directories
3. Implement external synchronization

## Performance Considerations

- **Memory usage**: ~50MB base + agent overhead
- **Disk I/O**: Checkpoints create Git commits
- **Network**: Agent API calls may have latency
- **CPU**: Minimal overhead (<1% between iterations)

## See Also

- [Configuration API](config.md)
- [Agent API](agents.md)
- [Metrics API](metrics.md)
- [CLI Reference](cli.md)

---

📚 Continue to [Configuration API](config.md) →