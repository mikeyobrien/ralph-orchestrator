# Ralph Orchestrator Adapter Architecture

## Base Adapter Interface

**Location:** `src/ralph_orchestrator/adapters/base.py`

### ToolAdapter ABC

```python
class ToolAdapter(ABC):
    def __init__(self, name: str, config=None)

    # Required abstract methods
    @abstractmethod
    def check_availability(self) -> bool
    @abstractmethod
    def execute(self, prompt: str, **kwargs) -> ToolResponse

    # Optional (default implementation available)
    async def aexecute(self, prompt: str, **kwargs) -> ToolResponse
    def execute_with_file(self, prompt_file: Path, **kwargs) -> ToolResponse
    def estimate_cost(self, prompt: str) -> float
    def _enhance_prompt_with_instructions(self, prompt: str) -> str
```

### ToolResponse Dataclass

```python
@dataclass
class ToolResponse:
    success: bool
    output: str
    error: Optional[str] = None
    tokens_used: Optional[int] = None
    cost: Optional[float] = None
    metadata: Dict[str, Any] = None
```

## Adapter Registration

**Location:** `src/ralph_orchestrator/adapters/__init__.py`

```python
from .base import ToolAdapter, ToolResponse
from .claude import ClaudeAdapter
from .qchat import QChatAdapter
from .gemini import GeminiAdapter

__all__ = ["ToolAdapter", "ToolResponse", "ClaudeAdapter", "QChatAdapter", "GeminiAdapter"]
```

## Adapter Selection (orchestrator.py)

```python
def _initialize_adapters(self) -> Dict[str, ToolAdapter]:
    adapters = {}
    try:
        adapter = ClaudeAdapter(verbose=self.verbose)
        if adapter.available:
            adapters['claude'] = adapter
    except Exception as e:
        logger.warning(f"Claude adapter error: {e}")
    # ... repeat for other adapters
    return adapters
```

## Implementation Patterns

### Pattern A: CLI-Based (QChat)
- Spawns external process via `subprocess.Popen`
- Real-time stdout/stderr streaming
- Signal handlers for graceful shutdown

### Pattern B: SDK-Based (Claude)
- Direct SDK integration
- Async-first with sync wrapper
- Complex message type handling
- Token counting and cost calculation

### Pattern C: REST API (Gemini)
- Simple command-line wrapper
- Basic token extraction

## Key Integration Points

### Orchestrator Loop (orchestrator.py:300+)
```python
response = await self.current_adapter.aexecute(
    prompt=enhanced_prompt,
    prompt_file=str(self.prompt_file),
    iteration=iteration,
    system_prompt=system_prompt,
)
```

### CLI Choices (__main__.py:445, 462)
```python
choices=['claude', 'q', 'gemini', 'auto']
```

### Configuration (ralph.yml)
```yaml
adapters:
  claude:
    enabled: true
    timeout: 300
```

## New Adapter Checklist

1. Create `src/ralph_orchestrator/adapters/acp.py`
2. Extend `ToolAdapter` base class
3. Implement `check_availability() -> bool`
4. Implement `execute(prompt, **kwargs) -> ToolResponse`
5. Optionally implement `aexecute()` for async
6. Add to `__init__.py` imports and `__all__`
7. Add 'acp' to CLI choices in `__main__.py`
8. Add configuration section in ralph.yml template
