"""Connection layer for TUI to orchestrator communication."""

import asyncio
from dataclasses import dataclass, field
from typing import AsyncIterator, Optional, Any, Callable
from enum import Enum
from datetime import datetime
import json


class EventType(str, Enum):
    """Types of events from orchestrator."""
    ITERATION_START = "iteration_start"
    ITERATION_END = "iteration_end"
    OUTPUT = "output"
    TOOL_CALL = "tool_call"
    TASK_UPDATE = "task_update"
    METRICS = "metrics"
    VALIDATION_GATE = "validation_gate"
    CHECKPOINT = "checkpoint"
    ERROR = "error"
    WARNING = "warning"
    COMPLETE = "complete"
    PAUSED = "paused"
    RESUMED = "resumed"


@dataclass
class TUIEvent:
    """Event from orchestrator to TUI."""
    type: EventType
    data: dict = field(default_factory=dict)
    timestamp: float = field(default_factory=lambda: datetime.now().timestamp())

    def to_json(self) -> str:
        """Serialize event to JSON."""
        return json.dumps({
            "type": self.type.value,
            "data": self.data,
            "timestamp": self.timestamp,
        })

    @classmethod
    def from_json(cls, json_str: str) -> "TUIEvent":
        """Deserialize event from JSON."""
        obj = json.loads(json_str)
        return cls(
            type=EventType(obj["type"]),
            data=obj.get("data", {}),
            timestamp=obj.get("timestamp", datetime.now().timestamp()),
        )


class OrchestratorConnection:
    """Base class for orchestrator connections.

    Provides:
    - Event streaming interface
    - Command sending interface
    - Connection lifecycle management
    """

    def __init__(self, mode: str = "attached"):
        self.mode = mode
        self._queue: asyncio.Queue[TUIEvent] = asyncio.Queue()
        self._connected: bool = False
        self._running: bool = False

    @property
    def is_connected(self) -> bool:
        """Check if connection is active."""
        return self._connected

    async def connect(self, target: Optional[str] = None) -> bool:
        """Connect to orchestrator.

        Args:
            target: URL for websocket, None for attached mode

        Returns:
            True if connection successful
        """
        raise NotImplementedError("Subclasses must implement connect()")

    async def disconnect(self) -> None:
        """Disconnect from orchestrator."""
        self._connected = False
        self._running = False

    async def events(self) -> AsyncIterator[TUIEvent]:
        """Stream events from orchestrator.

        Yields:
            TUIEvent objects as they arrive
        """
        self._running = True
        while self._running:
            try:
                event = await asyncio.wait_for(
                    self._queue.get(),
                    timeout=0.1
                )
                yield event
            except asyncio.TimeoutError:
                continue
            except asyncio.CancelledError:
                break

    async def send_command(self, command: str, **kwargs) -> bool:
        """Send command to orchestrator.

        Args:
            command: Command name (pause, resume, checkpoint, etc.)
            **kwargs: Command arguments

        Returns:
            True if command was accepted
        """
        raise NotImplementedError("Subclasses must implement send_command()")

    def _emit_event(self, event: TUIEvent) -> None:
        """Add event to queue (for subclasses to use)."""
        try:
            self._queue.put_nowait(event)
        except asyncio.QueueFull:
            # Drop oldest event if queue is full
            try:
                self._queue.get_nowait()
                self._queue.put_nowait(event)
            except asyncio.QueueEmpty:
                pass


class AttachedConnection(OrchestratorConnection):
    """Direct connection when running TUI with orchestrator in same process.

    Hooks into orchestrator's internal events for real-time updates.
    """

    def __init__(self, orchestrator: Any):
        super().__init__(mode="attached")
        self.orchestrator = orchestrator
        self._original_callbacks: dict = {}

    async def connect(self, target: Optional[str] = None) -> bool:
        """Set up hooks into orchestrator."""
        try:
            self._setup_hooks()
            self._connected = True
            return True
        except Exception as e:
            self._emit_event(TUIEvent(
                type=EventType.ERROR,
                data={"message": f"Failed to attach: {e}"}
            ))
            return False

    def _setup_hooks(self) -> None:
        """Hook into orchestrator callbacks."""
        # Store original callbacks
        if hasattr(self.orchestrator, "on_iteration_start"):
            self._original_callbacks["on_iteration_start"] = getattr(
                self.orchestrator, "on_iteration_start", None
            )

        # Set up new callbacks that emit TUI events
        def on_iteration_start(iteration: int, task: Optional[str] = None):
            self._emit_event(TUIEvent(
                type=EventType.ITERATION_START,
                data={
                    "iteration": iteration,
                    "max_iterations": getattr(self.orchestrator, "max_iterations", 100),
                    "task": task or "",
                }
            ))
            # Call original if exists
            if self._original_callbacks.get("on_iteration_start"):
                self._original_callbacks["on_iteration_start"](iteration, task)

        def on_output(text: str):
            self._emit_event(TUIEvent(
                type=EventType.OUTPUT,
                data={"text": text}
            ))

        def on_tool_call(name: str, input_data: dict, result: str, status: str = "success"):
            self._emit_event(TUIEvent(
                type=EventType.TOOL_CALL,
                data={
                    "name": name,
                    "input": input_data,
                    "result": result,
                    "status": status,
                }
            ))

        def on_metrics(cpu: float, memory: float, tokens: int, cost: float):
            self._emit_event(TUIEvent(
                type=EventType.METRICS,
                data={
                    "cpu": cpu,
                    "memory": memory,
                    "tokens": tokens,
                    "cost": cost,
                }
            ))

        def on_validation_gate(name: str, description: str, evidence: list):
            self._emit_event(TUIEvent(
                type=EventType.VALIDATION_GATE,
                data={
                    "name": name,
                    "description": description,
                    "evidence": evidence,
                }
            ))

        def on_complete():
            self._emit_event(TUIEvent(type=EventType.COMPLETE))

        # Attach callbacks to orchestrator
        self.orchestrator.on_iteration_start = on_iteration_start
        self.orchestrator.on_output = on_output
        self.orchestrator.on_tool_call = on_tool_call
        self.orchestrator.on_metrics = on_metrics
        self.orchestrator.on_validation_gate = on_validation_gate
        self.orchestrator.on_complete = on_complete

    async def send_command(self, command: str, **kwargs) -> bool:
        """Send command to attached orchestrator."""
        try:
            match command:
                case "pause":
                    if hasattr(self.orchestrator, "pause"):
                        self.orchestrator.pause()
                        self._emit_event(TUIEvent(type=EventType.PAUSED))
                        return True
                case "resume":
                    if hasattr(self.orchestrator, "resume"):
                        self.orchestrator.resume()
                        self._emit_event(TUIEvent(type=EventType.RESUMED))
                        return True
                case "checkpoint":
                    if hasattr(self.orchestrator, "checkpoint"):
                        self.orchestrator.checkpoint()
                        self._emit_event(TUIEvent(
                            type=EventType.CHECKPOINT,
                            data={"manual": True}
                        ))
                        return True
                case "validation_response":
                    if hasattr(self.orchestrator, "set_validation_response"):
                        self.orchestrator.set_validation_response(
                            approved=kwargs.get("approved", False),
                            skipped=kwargs.get("skipped", False),
                        )
                        return True
            return False
        except Exception as e:
            self._emit_event(TUIEvent(
                type=EventType.ERROR,
                data={"message": f"Command failed: {e}"}
            ))
            return False

    async def disconnect(self) -> None:
        """Restore original callbacks and disconnect."""
        # Restore original callbacks
        for name, callback in self._original_callbacks.items():
            if callback:
                setattr(self.orchestrator, name, callback)
        await super().disconnect()


class WebSocketConnection(OrchestratorConnection):
    """WebSocket connection to remote orchestrator.

    Used for `ralph watch` mode to connect to running orchestrator.
    """

    def __init__(self):
        super().__init__(mode="websocket")
        self._ws = None
        self._url: Optional[str] = None

    async def connect(self, target: Optional[str] = None) -> bool:
        """Connect to orchestrator via WebSocket.

        Args:
            target: WebSocket URL (e.g., ws://localhost:8080/ws)
        """
        if not target:
            target = "ws://localhost:8080/ws"

        self._url = target

        try:
            import websockets
            self._ws = await websockets.connect(target)
            self._connected = True

            # Start receiving messages
            asyncio.create_task(self._receive_messages())

            return True
        except ImportError:
            self._emit_event(TUIEvent(
                type=EventType.ERROR,
                data={"message": "websockets package not installed"}
            ))
            return False
        except Exception as e:
            self._emit_event(TUIEvent(
                type=EventType.ERROR,
                data={"message": f"WebSocket connection failed: {e}"}
            ))
            return False

    async def _receive_messages(self) -> None:
        """Receive and process WebSocket messages."""
        try:
            async for message in self._ws:
                try:
                    event = TUIEvent.from_json(message)
                    self._emit_event(event)
                except json.JSONDecodeError:
                    pass
        except Exception as e:
            self._connected = False
            self._emit_event(TUIEvent(
                type=EventType.ERROR,
                data={"message": f"WebSocket error: {e}"}
            ))

    async def send_command(self, command: str, **kwargs) -> bool:
        """Send command via WebSocket."""
        if not self._ws or not self._connected:
            return False

        try:
            message = json.dumps({
                "type": "command",
                "command": command,
                "args": kwargs,
            })
            await self._ws.send(message)
            return True
        except Exception as e:
            self._emit_event(TUIEvent(
                type=EventType.ERROR,
                data={"message": f"Failed to send command: {e}"}
            ))
            return False

    async def disconnect(self) -> None:
        """Close WebSocket connection."""
        if self._ws:
            await self._ws.close()
            self._ws = None
        await super().disconnect()
