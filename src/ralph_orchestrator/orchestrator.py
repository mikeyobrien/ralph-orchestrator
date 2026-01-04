# ABOUTME: Core orchestration loop implementing the Ralph Wiggum technique
# ABOUTME: Manages AI agent execution with safety, metrics, and recovery

"""Core orchestration loop for Ralph Orchestrator."""

import time
import signal
import logging
import asyncio
from pathlib import Path
from typing import Dict, Any
import json
from datetime import datetime

from .adapters.base import ToolAdapter
from .adapters.claude import ClaudeAdapter
from .adapters.qchat import QChatAdapter
from .adapters.gemini import GeminiAdapter
from .adapters.acp import ACPAdapter
from .adapters.acp_models import ACPAdapterConfig
from .metrics import Metrics, CostTracker, IterationStats, TriggerReason
from .safety import SafetyGuard
from .context import ContextManager
from .output import RalphConsole

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger('ralph-orchestrator')


class RalphOrchestrator:
    """Main orchestration loop for AI agents."""

    _KNOWN_ADAPTERS = ("acp", "claude", "gemini", "qchat")

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
        verbose: bool = False,
        acp_agent: str = None,
        acp_permission_mode: str = None,
        acp_agent_args: list[str] | None = None,
        iteration_telemetry: bool = True,
        output_preview_length: int = 500
    ):
        """Initialize the orchestrator.

        Args:
            prompt_file_or_config: Path to prompt file or RalphConfig object
            primary_tool: Primary AI tool to use (claude, qchat, gemini)
            max_iterations: Maximum number of iterations
            max_runtime: Maximum runtime in seconds
            track_costs: Whether to track costs
            max_cost: Maximum allowed cost
            checkpoint_interval: Git checkpoint frequency
            archive_dir: Directory for prompt archives
            verbose: Enable verbose logging output
            acp_agent: ACP agent command (e.g., claude-code-acp, gemini)
            acp_permission_mode: ACP permission handling mode
            iteration_telemetry: Enable per-iteration telemetry capture
            output_preview_length: Max chars for output preview in telemetry
        """
        # Store ACP-specific settings
        self.acp_agent = acp_agent
        self.acp_permission_mode = acp_permission_mode
        self.acp_agent_args = acp_agent_args or []
        self._config = None
        self.agent_priority: list[str] = []
        # Handle both config object and individual parameters
        if hasattr(prompt_file_or_config, 'prompt_file'):
            # It's a config object
            config = prompt_file_or_config
            self._config = config
            self.prompt_file = Path(config.prompt_file)
            self.prompt_text = getattr(config, 'prompt_text', None)
            self.primary_tool = config.agent.value if hasattr(config.agent, 'value') else str(config.agent)
            self.agent_priority = list(getattr(config, "agent_priority", []) or [])
            self.max_iterations = config.max_iterations
            self.max_runtime = config.max_runtime
            self.track_costs = hasattr(config, 'max_cost') and config.max_cost > 0
            self.max_cost = config.max_cost if hasattr(config, 'max_cost') else max_cost
            self.checkpoint_interval = config.checkpoint_interval
            self.archive_dir = Path(config.archive_dir if hasattr(config, 'archive_dir') else archive_dir)
            self.verbose = config.verbose if hasattr(config, 'verbose') else False
            self.iteration_telemetry = getattr(config, 'iteration_telemetry', True)
            self.output_preview_length = getattr(config, 'output_preview_length', 500)
        else:
            # Individual parameters
            self.prompt_file = Path(prompt_file_or_config if prompt_file_or_config else "PROMPT.md")
            self.prompt_text = None
            self.primary_tool = primary_tool
            self.agent_priority = []
            self.max_iterations = max_iterations
            self.max_runtime = max_runtime
            self.track_costs = track_costs
            self.max_cost = max_cost
            self.checkpoint_interval = checkpoint_interval
            self.archive_dir = Path(archive_dir)
            self.verbose = verbose
            self.iteration_telemetry = iteration_telemetry
            self.output_preview_length = output_preview_length

        # Initialize components
        self.metrics = Metrics()
        self.iteration_stats = IterationStats(
            max_preview_length=self.output_preview_length
        ) if self.iteration_telemetry else None
        self.cost_tracker = CostTracker() if track_costs else None
        self.safety_guard = SafetyGuard(max_iterations, max_runtime, max_cost)
        self.context_manager = ContextManager(self.prompt_file, prompt_text=self.prompt_text)
        self.console = RalphConsole()  # Enhanced console output
        
        # Initialize adapters
        self.adapters = self._initialize_adapters()
        if self.primary_tool == "auto":
            self.current_adapter = self._select_auto_adapter()
            # Record resolved tool name for telemetry/cost tracking
            self.primary_tool = self.current_adapter.name
        else:
            self.current_adapter = self.adapters.get(self.primary_tool)
        
        if not self.current_adapter:
            logger.error(f"DEBUG: primary_tool={self.primary_tool}, adapters={list(self.adapters.keys())}")
            raise ValueError(f"Unknown tool: {self.primary_tool}")
        
        # Signal handling - use basic signal registration here
        # The async handlers will be set up when arun() is called
        self.stop_requested = False
        self._running_task = None  # Track the current async task for cancellation
        self._async_logger = None  # Will hold optional AsyncFileLogger for emergency shutdown
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)

        # Task queue tracking
        self.task_queue = []  # List of pending tasks extracted from prompt
        self.current_task = None  # Currently executing task
        self.completed_tasks = []  # List of completed tasks with results
        self.task_start_time = None  # Start time of current task
        self.last_response_output = None  # Final agent output from last iteration
        
        # Create directories
        self.archive_dir.mkdir(parents=True, exist_ok=True)
        Path(".agent").mkdir(exist_ok=True)
        
        logger.info(f"Ralph Orchestrator initialized with {primary_tool}")
    
    def _initialize_adapters(self) -> Dict[str, ToolAdapter]:
        """Initialize available adapters."""
        adapters: Dict[str, ToolAdapter] = {}

        # Resolve configured adapter order (used for both selection and fallback ordering).
        desired_order = self._resolve_agent_priority()

        # Try to initialize each adapter (in priority order).
        for name in desired_order:
            if not self._adapter_enabled(name):
                logger.info("Adapter disabled via config: %s", name)
                continue

            if name == "claude":
                try:
                    adapter = ClaudeAdapter(verbose=self.verbose)
                    if adapter.available:
                        adapters["claude"] = adapter
                        logger.info("Claude adapter initialized")
                    else:
                        logger.warning("Claude SDK not available")
                except Exception as e:
                    logger.warning(f"Claude adapter error: {e}")
                continue

            if name == "qchat":
                try:
                    adapter = QChatAdapter()
                    if adapter.available:
                        adapters["qchat"] = adapter
                        logger.info("Q Chat adapter initialized")
                    else:
                        logger.warning("Q Chat CLI not available")
                except Exception as e:
                    logger.warning(f"Q Chat adapter error: {e}")
                continue

            if name == "gemini":
                try:
                    adapter = GeminiAdapter()
                    if adapter.available:
                        adapters["gemini"] = adapter
                        logger.info("Gemini adapter initialized")
                    else:
                        logger.warning("Gemini CLI not available")
                except Exception as e:
                    logger.warning(f"Gemini adapter error: {e}")
                continue

            if name == "acp":
                try:
                    adapter = self._init_acp_adapter()
                    if adapter and adapter.available:
                        adapters["acp"] = adapter
                        logger.info("ACP adapter initialized (agent: %s)", getattr(adapter, "agent_command", "unknown"))
                    else:
                        logger.warning("ACP agent not available")
                except Exception as e:
                    logger.warning(f"ACP adapter error: {e}")
                continue

        return adapters

    def _normalize_agent_name(self, name: str) -> str:
        """Normalize user-facing agent names to internal adapter keys."""
        name = (name or "").strip().lower()
        mapping = {
            "codex": "acp",
            "acp": "acp",
            "claude": "claude",
            "gemini": "gemini",
            "q": "qchat",
            "qchat": "qchat",
        }
        return mapping.get(name, name)

    def _resolve_agent_priority(self) -> list[str]:
        """Compute adapter initialization/fallback order."""
        raw = self.agent_priority or []
        normalized: list[str] = []
        seen: set[str] = set()

        for item in raw:
            n = self._normalize_agent_name(str(item))
            if n in self._KNOWN_ADAPTERS and n not in seen:
                normalized.append(n)
                seen.add(n)

        # Always include any remaining known adapters (stable default ordering)
        for n in ("claude", "qchat", "gemini", "acp"):
            if n not in seen:
                normalized.append(n)
                seen.add(n)

        return normalized

    def _adapter_enabled(self, name: str) -> bool:
        """Return whether an adapter is enabled in config (defaults to True)."""
        if not self._config or not hasattr(self._config, "get_adapter_config"):
            return True

        # Map internal key -> config key for adapter section
        if name == "qchat":
            # Support both "q" and "qchat" config keys
            for key in ("qchat", "q"):
                cfg = self._config.get_adapter_config(key)
                if cfg and getattr(cfg, "enabled", True) is False:
                    return False
            return True

        cfg = self._config.get_adapter_config(name)
        return getattr(cfg, "enabled", True)

    def _init_acp_adapter(self) -> ACPAdapter | None:
        """Initialize ACP adapter using config defaults + CLI overrides."""
        kwargs: dict[str, Any] = {}

        # Pull defaults from config.adapters.acp.tool_permissions when available.
        if self._config and hasattr(self._config, "get_adapter_config"):
            adapter_cfg = self._config.get_adapter_config("acp")
            try:
                acp_cfg = ACPAdapterConfig.from_adapter_config(adapter_cfg)
                kwargs.update(
                    agent_command=acp_cfg.agent_command,
                    agent_args=acp_cfg.agent_args,
                    timeout=acp_cfg.timeout,
                    permission_mode=acp_cfg.permission_mode,
                    permission_allowlist=acp_cfg.permission_allowlist,
                )
            except Exception as e:
                logger.debug("Failed to parse ACP adapter config from ralph.yml: %s", e)

        # CLI overrides take precedence.
        if self.acp_agent:
            kwargs["agent_command"] = self.acp_agent
        if self.acp_permission_mode:
            kwargs["permission_mode"] = self.acp_permission_mode
        if self.acp_agent_args:
            kwargs["agent_args"] = self.acp_agent_args

        # Propagate verbose mode to ACP adapter for streaming output.
        kwargs["verbose"] = bool(self.verbose)

        return ACPAdapter(**kwargs)

    def _select_auto_adapter(self) -> ToolAdapter:
        """Select an adapter when primary_tool is 'auto'."""
        desired_order = self._resolve_agent_priority()
        for name in desired_order:
            adapter = self.adapters.get(name)
            if adapter is not None:
                return adapter

        raise ValueError("No available adapters found for auto mode")
    
    def _signal_handler(self, signum, frame):
        """Handle shutdown signals with subprocess-first cleanup.

        This handler follows a critical shutdown sequence:
        1. Kill subprocess FIRST (synchronous, signal-safe) - unblocks I/O
        2. Set emergency shutdown on logger (prevents blocking log writes)
        3. Set stop flag and cancel async task
        4. Schedule async emergency cleanup
        """
        logger.info(f"Received signal {signum}, initiating graceful shutdown...")

        # CRITICAL: Kill subprocess FIRST (synchronous, signal-safe)
        # This unblocks any I/O operations waiting on subprocess
        if hasattr(self.current_adapter, 'kill_subprocess_sync'):
            self.current_adapter.kill_subprocess_sync()

        # Force emergency shutdown on async logger if present
        if self._async_logger is not None:
            self._async_logger.emergency_shutdown()

        # Set stop flag
        self.stop_requested = True

        # Cancel running async task if present
        if self._running_task and not self._running_task.done():
            self._running_task.cancel()

        # Schedule emergency cleanup on the event loop (if available)
        try:
            asyncio.get_running_loop()
            asyncio.create_task(self._emergency_cleanup())
        except RuntimeError:
            # No running event loop - sync cleanup handled by finally blocks
            pass

    async def _emergency_cleanup(self) -> None:
        """Emergency cleanup scheduled from signal handler.

        This method handles any remaining async cleanup that needs to happen
        after the signal handler has done its synchronous cleanup.
        """
        try:
            # Clean up adapter transport if available
            if hasattr(self.current_adapter, '_cleanup_transport'):
                try:
                    await asyncio.wait_for(
                        self.current_adapter._cleanup_transport(),
                        timeout=0.5
                    )
                except asyncio.TimeoutError:
                    logger.debug("Cleanup transport timed out during emergency shutdown")
        except Exception as e:
            logger.debug(f"Error during emergency cleanup (ignored): {type(e).__name__}: {e}")
    
    def run(self) -> None:
        """Run the main orchestration loop."""
        # Create event loop if needed and run async version
        try:
            asyncio.run(self.arun())
        except RuntimeError:
            # If loop already exists, use it
            loop = asyncio.get_event_loop()
            loop.run_until_complete(self.arun())
    
    def set_async_logger(self, async_logger) -> None:
        """Set the AsyncFileLogger for emergency shutdown during signal handling.

        Args:
            async_logger: An AsyncFileLogger instance with emergency_shutdown() method
        """
        self._async_logger = async_logger

    def _setup_async_signal_handlers(self) -> None:
        """Set up async signal handlers for graceful shutdown in event loop context."""
        try:
            loop = asyncio.get_running_loop()

            def async_signal_handler(signum: int) -> None:
                """Handle shutdown signals in async context."""
                logger.info(f"Received signal {signum}, initiating graceful shutdown...")

                # CRITICAL: Kill subprocess FIRST (synchronous, signal-safe)
                if hasattr(self.current_adapter, 'kill_subprocess_sync'):
                    self.current_adapter.kill_subprocess_sync()

                # Force emergency shutdown on async logger if present
                if self._async_logger is not None:
                    self._async_logger.emergency_shutdown()

                # Set stop flag and cancel running task
                self.stop_requested = True
                if self._running_task and not self._running_task.done():
                    self._running_task.cancel()

                # Schedule emergency cleanup
                asyncio.create_task(self._emergency_cleanup())

            # Register handlers with event loop for proper async handling
            for sig in (signal.SIGINT, signal.SIGTERM):
                loop.add_signal_handler(sig, lambda s=sig: async_signal_handler(s))
        except NotImplementedError:
            # Windows doesn't support add_signal_handler, fall back to basic handling
            pass

    async def arun(self) -> None:
        """Run the main orchestration loop asynchronously."""
        logger.info("Starting Ralph orchestration loop")

        # Set up async signal handlers now that we have a running loop
        self._setup_async_signal_handlers()

        start_time = time.time()
        self._start_time = start_time  # Store for state retrieval

        while not self.stop_requested:
            # Check safety limits
            safety_check = self.safety_guard.check(
                self.metrics.iterations,
                time.time() - start_time,
                self.cost_tracker.total_cost if self.cost_tracker else 0
            )
            
            if not safety_check.passed:
                logger.info(f"Safety limit reached: {safety_check.reason}")
                break

            # Check for explicit completion marker in prompt
            if self._check_completion_marker():
                logger.info("Completion marker found - task marked complete")
                self.console.print_success("Task completion marker detected - stopping orchestration")
                break
            
            # Determine trigger reason BEFORE incrementing iteration
            trigger_reason = self._determine_trigger_reason()

            # Execute iteration
            self.metrics.iterations += 1
            self.console.print_iteration_header(self.metrics.iterations)
            logger.info(f"Starting iteration {self.metrics.iterations}")

            # Record iteration timing
            iteration_start = time.time()
            iteration_success = False
            iteration_error = ""
            loop_detected = False

            try:
                success = await self._aexecute_iteration()

                if success:
                    iteration_success = True
                    self.metrics.successful_iterations += 1
                    self.console.print_success(
                        f"Iteration {self.metrics.iterations} completed successfully"
                    )
                    # Show agent output for this iteration
                    if self.last_response_output:
                        self.console.print_header(f"Agent Output (Iteration {self.metrics.iterations})")
                        self.console.print_message(self.last_response_output)

                        # Check for loop (repeated similar outputs)
                        if self.safety_guard.detect_loop(self.last_response_output):
                            loop_detected = True
                            self.console.print_warning(
                                "Loop detected - agent producing repetitive outputs"
                            )
                            logger.warning("Breaking loop due to repetitive agent outputs")
                else:
                    self.metrics.failed_iterations += 1
                    iteration_error = "Iteration failed"
                    self.console.print_warning(
                        f"Iteration {self.metrics.iterations} failed"
                    )
                    await self._handle_failure()

                # Checkpoint if needed
                if self.metrics.iterations % self.checkpoint_interval == 0:
                    await self._create_checkpoint()
                    self.console.print_info(
                        f"Checkpoint {self.metrics.checkpoints} created"
                    )

            except Exception as e:
                logger.warning(f"Error in iteration: {e}")
                self.metrics.errors += 1
                iteration_error = str(e)
                self.console.print_error(f"Error in iteration: {e}")
                self._handle_error(e)

            # Record per-iteration telemetry
            iteration_duration = time.time() - iteration_start

            # Extract cost/tokens from the latest usage if available
            iteration_tokens = 0
            iteration_cost = 0.0
            if self.cost_tracker and self.cost_tracker.usage_history:
                latest_usage = self.cost_tracker.usage_history[-1]
                # Only use if this usage is from this iteration (recent timestamp)
                if latest_usage.get("timestamp", 0) >= iteration_start:
                    iteration_tokens = latest_usage.get("input_tokens", 0) + latest_usage.get("output_tokens", 0)
                    iteration_cost = latest_usage.get("cost", 0.0)

            # Record per-iteration telemetry if enabled
            if self.iteration_stats:
                # Get output preview (truncated to configured length)
                output_preview = ""
                if self.last_response_output:
                    preview_len = self.output_preview_length
                    output_preview = self.last_response_output[:preview_len] if len(self.last_response_output) > preview_len else self.last_response_output

                self.iteration_stats.record_iteration(
                    iteration=self.metrics.iterations,
                    duration=iteration_duration,
                    success=iteration_success,
                    error=iteration_error,
                    trigger_reason=trigger_reason,
                    output_preview=output_preview,
                    tokens_used=iteration_tokens,
                    cost=iteration_cost,
                )

            # Break loop if detected (after recording telemetry)
            if loop_detected:
                break
            
            # Brief pause between iterations
            await asyncio.sleep(2)
        
        # Final summary
        self._print_summary()
    
    
    def _execute_iteration(self) -> bool:
        """Execute a single iteration (sync wrapper)."""
        try:
            loop = asyncio.get_event_loop()
            return loop.run_until_complete(self._aexecute_iteration())
        except RuntimeError:
            # Create new event loop if needed
            return asyncio.run(self._aexecute_iteration())
    
    async def _aexecute_iteration(self) -> bool:
        """Execute a single iteration asynchronously."""
        # Get the current prompt
        prompt = self.context_manager.get_prompt()
        
        # Extract tasks from prompt if task queue is empty
        if not self.task_queue and not self.current_task:
            self._extract_tasks_from_prompt(prompt)
        
        # Update current task status
        self._update_current_task('in_progress')
        
        # Try primary adapter with prompt file path
        response = await self.current_adapter.aexecute(
            prompt, 
            prompt_file=str(self.prompt_file),
            verbose=self.verbose
        )
        
        if not response.success and len(self.adapters) > 1 and not self.stop_requested:
            # Try fallback adapters (skip if shutdown requested)
            for name, adapter in self.adapters.items():
                if self.stop_requested:
                    break
                if adapter != self.current_adapter:
                    logger.info(f"Falling back to {name}")
                    response = await adapter.aexecute(
                        prompt,
                        prompt_file=str(self.prompt_file),
                        verbose=self.verbose
                    )
                    if response.success:
                        break
        
        # Store and log the response output (already streamed to console if verbose)
        if response.success and response.output:
            self.last_response_output = response.output
            # Log a preview for the logs
            output_preview = response.output[:500] if len(response.output) > 500 else response.output
            logger.debug(f"Agent response preview: {output_preview}")
            if len(response.output) > 500:
                logger.debug(f"... (total {len(response.output)} characters)")
        
        # Track costs if enabled
        if self.cost_tracker and response.success:
            if response.tokens_used:
                tokens = response.tokens_used
            else:
                tokens = self._estimate_tokens(response.output)
            
            cost = self.cost_tracker.add_usage(
                self.current_adapter.name,
                tokens,
                tokens // 4  # Rough output estimate
            )
            logger.info(f"Estimated cost: ${cost:.4f} (total: ${self.cost_tracker.total_cost:.4f})")
        
        # Update context if needed
        if response.success and len(response.output) > 1000:
            self.context_manager.update_context(response.output)
        
        # Update task status based on response
        if response.success and self.current_task:
            # Check if response indicates task completion
            output_lower = response.output.lower() if response.output else ""
            if any(word in output_lower for word in ['completed', 'finished', 'done', 'committed']):
                self._update_current_task('completed')
        
        return response.success
    
    def _estimate_tokens(self, text: str) -> int:
        """Estimate token count from text."""
        # Rough estimate: 1 token per 4 characters
        return len(text) // 4
    
    async def _handle_failure(self):
        """Handle iteration failure asynchronously."""
        logger.warning("Iteration failed, attempting recovery")

        # Simple exponential backoff (non-blocking)
        backoff = min(2 ** self.metrics.failed_iterations, 60)
        logger.debug(f"Backing off for {backoff} seconds")
        await asyncio.sleep(backoff)

        # Consider rollback after multiple failures
        if self.metrics.failed_iterations > 3:
            await self._rollback_checkpoint()
    
    def _handle_error(self, error: Exception):
        """Handle iteration error."""
        logger.warning(f"Handling error: {error}")
        
        # Archive current prompt
        self._archive_prompt()
        
        # Reset if too many errors
        if self.metrics.errors > 5:
            logger.info("Too many errors, resetting state")
            self._reset_state()
    
    async def _create_checkpoint(self):
        """Create a git checkpoint asynchronously."""
        try:
            # Stage all changes
            proc = await asyncio.create_subprocess_exec(
                "git", "add", "-A",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            _, stderr = await proc.communicate()
            if proc.returncode != 0:
                logger.warning(f"Failed to stage changes: {stderr.decode()}")
                return

            # Commit
            proc = await asyncio.create_subprocess_exec(
                "git", "commit", "-m", f"Ralph checkpoint {self.metrics.iterations}",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            _, stderr = await proc.communicate()
            if proc.returncode != 0:
                logger.warning(f"Failed to create checkpoint: {stderr.decode()}")
                return

            self.metrics.checkpoints += 1
            logger.debug(f"Created checkpoint {self.metrics.checkpoints}")
        except Exception as e:
            logger.warning(f"Failed to create checkpoint: {e}")
    
    async def _rollback_checkpoint(self):
        """Rollback to previous checkpoint asynchronously."""
        try:
            proc = await asyncio.create_subprocess_exec(
                "git", "reset", "--hard", "HEAD~1",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            _, stderr = await proc.communicate()
            if proc.returncode != 0:
                logger.error(f"Failed to rollback: {stderr.decode()}")
                return

            logger.debug("Rolled back to previous checkpoint")
            self.metrics.rollbacks += 1
        except Exception as e:
            logger.error(f"Failed to rollback: {e}")
    
    def _archive_prompt(self):
        """Archive the current prompt."""
        if not self.prompt_file.exists():
            return
        
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        archive_path = self.archive_dir / f"prompt_{timestamp}.md"
        
        try:
            archive_path.write_text(self.prompt_file.read_text())
            logger.info(f"Archived prompt to {archive_path}")
        except Exception as e:
            logger.error(f"Failed to archive prompt: {e}")
    
    def _reset_state(self):
        """Reset the orchestrator state."""
        logger.info("Resetting orchestrator state")
        self.metrics = Metrics()
        if self.iteration_telemetry:
            self.iteration_stats = IterationStats(
                max_preview_length=self.output_preview_length
            )
        if self.cost_tracker:
            self.cost_tracker = CostTracker()
        self.context_manager.reset()
    
    def _print_summary(self):
        """Print execution summary with enhanced console output."""
        # Use RalphConsole for enhanced summary display
        self.console.print_header("Ralph Orchestration Summary")

        # Display final agent output if available
        if self.last_response_output:
            self.console.print_header("Final Agent Output")
            self.console.print_message(self.last_response_output)

        # Print stats using RalphConsole
        self.console.print_stats(
            iteration=self.metrics.iterations,
            success_count=self.metrics.successful_iterations,
            error_count=self.metrics.failed_iterations,
            start_time=datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            prompt_file=str(self.prompt_file),
            recent_lines=[
                f"Checkpoints: {self.metrics.checkpoints}",
                f"Rollbacks: {self.metrics.rollbacks}",
                f"Errors: {self.metrics.errors}",
            ],
        )

        if self.cost_tracker:
            self.console.print_info(f"Total cost: ${self.cost_tracker.total_cost:.4f}")
            self.console.print_info("Cost breakdown:")
            for tool, cost in self.cost_tracker.costs_by_tool.items():
                self.console.print_info(f"  {tool}: ${cost:.4f}")

        # Save metrics to file with enhanced per-iteration telemetry
        metrics_dir = Path(".agent") / "metrics"
        metrics_dir.mkdir(parents=True, exist_ok=True)
        metrics_file = metrics_dir / f"metrics_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"

        # Build enhanced metrics data structure
        metrics_data = {
            # Summary section (backward compatible)
            "summary": {
                "iterations": self.metrics.iterations,
                "successful": self.metrics.successful_iterations,
                "failed": self.metrics.failed_iterations,
                "errors": self.metrics.errors,
                "checkpoints": self.metrics.checkpoints,
                "rollbacks": self.metrics.rollbacks,
            },
            # Per-iteration details (if telemetry enabled)
            "iterations": self.iteration_stats.iterations if self.iteration_stats else [],
            # Cost tracking
            "cost": {
                "total": self.cost_tracker.total_cost if self.cost_tracker else 0,
                "by_tool": self.cost_tracker.costs_by_tool if self.cost_tracker else {},
                "history": self.cost_tracker.usage_history if self.cost_tracker else [],
            },
            # Analysis metrics (if telemetry enabled)
            "analysis": {
                "avg_iteration_duration": self.iteration_stats.get_average_duration() if self.iteration_stats else 0,
                "success_rate": self.iteration_stats.get_success_rate() if self.iteration_stats else 0,
            }
        }

        metrics_file.write_text(json.dumps(metrics_data, indent=2))
        self.console.print_success(f"Metrics saved to {metrics_file}")
    
    def _extract_tasks_from_prompt(self, prompt: str):
        """Extract tasks from the prompt text."""
        import re
        
        # Look for task patterns in the prompt
        # Common patterns: "- [ ] task", "1. task", "Task: description"
        task_patterns = [
            r'^\s*-\s*\[\s*\]\s*(.+)$',  # Checkbox tasks
            r'^\s*\d+\.\s*(.+)$',  # Numbered tasks
            r'^Task:\s*(.+)$',  # Task: format
            r'^TODO:\s*(.+)$',  # TODO: format
        ]
        
        lines = prompt.split('\n')
        for line in lines:
            for pattern in task_patterns:
                match = re.match(pattern, line, re.MULTILINE)
                if match:
                    task = {
                        'id': len(self.task_queue) + len(self.completed_tasks) + 1,
                        'description': match.group(1).strip(),
                        'status': 'pending',
                        'created_at': datetime.now().isoformat(),
                        'completed_at': None,
                        'iteration': None
                    }
                    self.task_queue.append(task)
                    break
        
        # If no tasks found, create a general task
        if not self.task_queue and not self.completed_tasks:
            self.task_queue.append({
                'id': 1,
                'description': 'Execute orchestrator instructions',
                'status': 'pending',
                'created_at': datetime.now().isoformat(),
                'completed_at': None,
                'iteration': None
            })
    
    def _update_current_task(self, status: str = 'in_progress'):
        """Update the current task status."""
        if not self.current_task and self.task_queue:
            self.current_task = self.task_queue.pop(0)
            self.current_task['status'] = 'in_progress'
            self.current_task['iteration'] = self.metrics.iterations
            self.task_start_time = time.time()
        elif self.current_task:
            self.current_task['status'] = status
            if status == 'completed':
                self.current_task['completed_at'] = datetime.now().isoformat()
                self.completed_tasks.append(self.current_task)
                self.current_task = None
                self.task_start_time = None

    def _check_completion_marker(self) -> bool:
        """Check if prompt contains TASK_COMPLETE marker (checkbox style).

        Supports the following marker formats:
        - `- [x] TASK_COMPLETE` (checkbox style, recommended)
        - `[x] TASK_COMPLETE` (checkbox without dash)

        Returns:
            True if completion marker found, False otherwise.
        """
        if not self.prompt_file.exists():
            return False

        try:
            content = self.prompt_file.read_text()
            for line in content.split('\n'):
                line_stripped = line.strip()
                if line_stripped in ('- [x] TASK_COMPLETE', '[x] TASK_COMPLETE'):
                    return True
            return False
        except Exception as e:
            logger.warning(f"Error checking completion marker: {e}")
            return False

    def _determine_trigger_reason(self) -> str:
        """Determine why this iteration is being triggered.

        Analyzes the current orchestrator state to determine the reason
        for triggering a new iteration. This is used for per-iteration
        telemetry to understand orchestration patterns.

        Returns:
            str: The trigger reason value from TriggerReason enum.
        """
        # First iteration is always INITIAL
        if self.metrics.iterations == 0:
            return TriggerReason.INITIAL.value

        # Check if we're in recovery mode (recent failures)
        # Recovery if the last iteration failed and we've had multiple failures
        if self.metrics.failed_iterations > 0:
            # If failures are increasing relative to successes, we're recovering
            recent_failure_rate = self.metrics.failed_iterations / max(1, self.metrics.iterations)
            if recent_failure_rate > 0.5:
                return TriggerReason.RECOVERY.value

        # Check if previous iteration was successful
        # The iteration counter has already been incremented by the time we check
        # So we compare successful iterations to iterations - 1 (previous)
        if self.metrics.successful_iterations == self.metrics.iterations - 1:
            return TriggerReason.PREVIOUS_SUCCESS.value

        # Default: task is incomplete and we're continuing
        return TriggerReason.TASK_INCOMPLETE.value

    def _reload_prompt(self):
        """Reload the prompt file to pick up any changes.
        
        Note:
            Used by external web monitor to reload prompt state on user request.
        """
        logger.info("Reloading prompt file due to external update")
        # The context manager will automatically reload on next get_prompt() call
        # Clear the context manager's cache to force reload
        if hasattr(self.context_manager, '_load_initial_prompt'):
            self.context_manager._load_initial_prompt()
        
        # Extract new tasks if the prompt has changed significantly
        prompt = self.context_manager.get_prompt()
        
        # Only re-extract tasks if we don't have a current task or queue
        if not self.current_task and not self.task_queue:
            self._extract_tasks_from_prompt(prompt)
    
    def get_task_status(self) -> Dict[str, Any]:
        """Get current task queue status."""
        return {
            'current_task': self.current_task,
            'task_queue': self.task_queue,
            'completed_tasks': self.completed_tasks[-10:],  # Last 10 completed
            'queue_length': len(self.task_queue),
            'completed_count': len(self.completed_tasks),
            'current_iteration': self.metrics.iterations,
            'task_duration': (time.time() - self.task_start_time) if self.task_start_time else None
        }
    
    def get_orchestrator_state(self) -> Dict[str, Any]:
        """Get comprehensive orchestrator state."""
        return {
            'id': id(self),  # Unique instance ID
            'status': 'paused' if self.stop_requested else 'running',
            'primary_tool': self.primary_tool,
            'prompt_file': str(self.prompt_file),
            'iteration': self.metrics.iterations,
            'max_iterations': self.max_iterations,
            'runtime': time.time() - getattr(self, '_start_time', time.time()),
            'max_runtime': self.max_runtime,
            'tasks': self.get_task_status(),
            'metrics': {
                'successful': self.metrics.successful_iterations,
                'failed': self.metrics.failed_iterations,
                'errors': self.metrics.errors,
                'checkpoints': self.metrics.checkpoints,
                'rollbacks': self.metrics.rollbacks
            },
            'cost': {
                'total': self.cost_tracker.total_cost if self.cost_tracker else 0,
                'limit': self.max_cost if self.track_costs else None
            }
        }
