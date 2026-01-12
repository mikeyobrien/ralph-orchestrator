# ABOUTME: Claude SDK adapter implementation
# ABOUTME: Provides integration with Anthropic's Claude via Python SDK
# ABOUTME: Supports inheriting user's Claude Code settings (MCP servers, CLAUDE.md, etc.)

"""Claude SDK adapter for Ralph Orchestrator."""

import asyncio
import logging
import os
import signal
from typing import Optional
from .base import ToolAdapter, ToolResponse
from ..error_formatter import ClaudeErrorFormatter
from ..output.console import RalphConsole

# Setup logging
logger = logging.getLogger(__name__)

try:
    from claude_agent_sdk import ClaudeAgentOptions, query
    CLAUDE_SDK_AVAILABLE = True
except ImportError:
    # Fallback to old package name for backwards compatibility
    try:
        from claude_code_sdk import ClaudeCodeOptions as ClaudeAgentOptions, query
        CLAUDE_SDK_AVAILABLE = True
    except ImportError:
        CLAUDE_SDK_AVAILABLE = False
        query = None
        ClaudeAgentOptions = None


class ClaudeAdapter(ToolAdapter):
    """Adapter for Claude using the Python SDK."""

    # Default max buffer size: 10MB (handles large screenshots from chrome-devtools-mcp)
    DEFAULT_MAX_BUFFER_SIZE = 10 * 1024 * 1024

    # Default model: Claude Opus 4.5 (most intelligent model)
    DEFAULT_MODEL = "claude-opus-4-5-20251101"

    # Model pricing (per million tokens)
    MODEL_PRICING = {
        "claude-opus-4-5-20251101": {"input": 5.0, "output": 25.0},
        "claude-sonnet-4-5-20250929": {"input": 3.0, "output": 15.0},
        "claude-haiku-4-5-20251001": {"input": 1.0, "output": 5.0},
        # Legacy models
        "claude-3-opus": {"input": 15.0, "output": 75.0},
        "claude-3-sonnet": {"input": 3.0, "output": 15.0},
        "claude-3-haiku": {"input": 0.25, "output": 1.25},
    }

    def __init__(self, verbose: bool = False, max_buffer_size: int = None,
                 inherit_user_settings: bool = True, cli_path: str = None,
                 model: str = None):
        super().__init__("claude")
        self.sdk_available = CLAUDE_SDK_AVAILABLE
        self._system_prompt = None
        self._allowed_tools = None
        self._disallowed_tools = None
        self._enable_all_tools = False
        self._enable_web_search = True  # Enable WebSearch by default
        self._max_buffer_size = max_buffer_size or self.DEFAULT_MAX_BUFFER_SIZE
        self.verbose = verbose
        # Enable loading user's Claude Code settings (including MCP servers) by default
        self._inherit_user_settings = inherit_user_settings
        # Optional path to user's Claude Code CLI (uses bundled CLI if not specified)
        self._cli_path = cli_path
        self._model = model or self.DEFAULT_MODEL
        self._subprocess_pid: Optional[int] = None
        self._console = RalphConsole()
    
    def check_availability(self) -> bool:
        """Check if Claude SDK is available and properly configured."""
        # Claude Code SDK works without API key - it uses the local environment
        return CLAUDE_SDK_AVAILABLE
    
    def configure(self,
                  system_prompt: Optional[str] = None,
                  allowed_tools: Optional[list] = None,
                  disallowed_tools: Optional[list] = None,
                  enable_all_tools: bool = False,
                  enable_web_search: bool = True,
                  inherit_user_settings: Optional[bool] = None,
                  cli_path: Optional[str] = None,
                  model: Optional[str] = None):
        """Configure the Claude adapter with custom options.

        Args:
            system_prompt: Custom system prompt for Claude
            allowed_tools: List of allowed tools for Claude to use (if None and enable_all_tools=True, all tools are enabled)
            disallowed_tools: List of disallowed tools
            enable_all_tools: If True and allowed_tools is None, enables all native Claude tools
            enable_web_search: If True, explicitly enables WebSearch tool (default: True)
            inherit_user_settings: If True, load user's Claude Code settings including MCP servers (default: True)
            cli_path: Path to user's Claude Code CLI (uses bundled CLI if not specified)
            model: Model to use (default: claude-opus-4-5-20251101)
        """
        self._system_prompt = system_prompt
        self._allowed_tools = allowed_tools
        self._disallowed_tools = disallowed_tools
        self._enable_all_tools = enable_all_tools
        self._enable_web_search = enable_web_search

        # Update user settings inheritance if specified
        if inherit_user_settings is not None:
            self._inherit_user_settings = inherit_user_settings

        # Update CLI path if specified
        if cli_path is not None:
            self._cli_path = cli_path

        # Update model if specified
        if model is not None:
            self._model = model

        # If web search is enabled and we have an allowed tools list, add WebSearch to it
        if enable_web_search and allowed_tools is not None and 'WebSearch' not in allowed_tools:
            self._allowed_tools = allowed_tools + ['WebSearch']
    
    def execute(self, prompt: str, **kwargs) -> ToolResponse:
        """Execute Claude with the given prompt synchronously.

        This is a blocking wrapper around the async implementation.
        """
        try:
            # Create new event loop if needed
            try:
                loop = asyncio.get_running_loop()
            except RuntimeError:
                loop = asyncio.new_event_loop()
                asyncio.set_event_loop(loop)
                return loop.run_until_complete(self.aexecute(prompt, **kwargs))
            else:
                # If loop is already running, schedule as task
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(asyncio.run, self.aexecute(prompt, **kwargs))
                    return future.result()
        except Exception as e:
            # Use error formatter for user-friendly error messages
            error_msg = ClaudeErrorFormatter.format_error_from_exception(
                iteration=kwargs.get('iteration', 0),
                exception=e
            )
            return ToolResponse(
                success=False,
                output="",
                error=str(error_msg)
            )
    
    async def aexecute(self, prompt: str, **kwargs) -> ToolResponse:
        """Execute Claude with the given prompt asynchronously."""
        if not self.available:
            logger.debug("Claude SDK not available")
            return ToolResponse(
                success=False,
                output="",
                error="Claude SDK is not available"
            )
        
        try:
            # Get configuration from kwargs or use defaults
            prompt_file = kwargs.get('prompt_file', 'PROMPT.md')
            
            # Build options for Claude Code
            options_dict = {}
            
            # Set system prompt with orchestration context
            system_prompt = kwargs.get('system_prompt', self._system_prompt)
            if not system_prompt:
                # Create a default system prompt with orchestration context
                enhanced_prompt = self._enhance_prompt_with_instructions(prompt)
                system_prompt = (
                    f"You are helping complete a task. "
                    f"The task is described in the file '{prompt_file}'. "
                    f"Please edit this file directly to add your solution and progress updates."
                )
                # Use the enhanced prompt as the main prompt
                prompt = enhanced_prompt
            else:
                # If custom system prompt provided, still enhance the main prompt
                prompt = self._enhance_prompt_with_instructions(prompt)
            options_dict['system_prompt'] = system_prompt
            
            # Set tool restrictions if provided
            # If enable_all_tools is True and no allowed_tools specified, don't set any restrictions
            enable_all_tools = kwargs.get('enable_all_tools', self._enable_all_tools)
            enable_web_search = kwargs.get('enable_web_search', self._enable_web_search)
            allowed_tools = kwargs.get('allowed_tools', self._allowed_tools)
            disallowed_tools = kwargs.get('disallowed_tools', self._disallowed_tools)
            
            # Add WebSearch to allowed tools if web search is enabled
            if enable_web_search and allowed_tools is not None and 'WebSearch' not in allowed_tools:
                allowed_tools = allowed_tools + ['WebSearch']
            
            # Only set tool restrictions if we're not enabling all tools or if specific tools are provided
            if not enable_all_tools or allowed_tools:
                if allowed_tools:
                    options_dict['allowed_tools'] = allowed_tools
                
                if disallowed_tools:
                    options_dict['disallowed_tools'] = disallowed_tools
            
            # If enable_all_tools is True and no allowed_tools, Claude will have access to all native tools
            if enable_all_tools and not allowed_tools:
                if self.verbose:
                    logger.debug("Enabling all native Claude tools (including WebSearch)")
            
            # Set permission mode - default to bypassPermissions for smoother operation
            permission_mode = kwargs.get('permission_mode', 'bypassPermissions')
            options_dict['permission_mode'] = permission_mode
            if self.verbose:
                logger.debug(f"Permission mode: {permission_mode}")
            
            # Set current working directory to ensure files are created in the right place
            import os
            cwd = kwargs.get('cwd', os.getcwd())
            options_dict['cwd'] = cwd
            if self.verbose:
                logger.debug(f"Working directory: {cwd}")

            # Set max buffer size for handling large responses (e.g., screenshots)
            max_buffer_size = kwargs.get('max_buffer_size', self._max_buffer_size)
            options_dict['max_buffer_size'] = max_buffer_size
            if self.verbose:
                logger.debug(f"Max buffer size: {max_buffer_size} bytes")

            # Configure setting sources to inherit user's Claude Code configuration
            # This enables MCP servers, CLAUDE.md files, and other user settings
            inherit_user_settings = kwargs.get('inherit_user_settings', self._inherit_user_settings)
            if inherit_user_settings:
                # Load user, project, and local settings (includes MCP servers)
                options_dict['setting_sources'] = ['user', 'project', 'local']
                if self.verbose:
                    logger.debug("Inheriting user's Claude Code settings (MCP servers, CLAUDE.md, etc.)")

            # Optional: use user's installed Claude Code CLI instead of bundled
            cli_path = kwargs.get('cli_path', self._cli_path)
            if cli_path:
                options_dict['cli_path'] = cli_path
                if self.verbose:
                    logger.debug(f"Using custom Claude CLI: {cli_path}")

            # Set model - defaults to Opus 4.5
            model = kwargs.get('model', self._model)
            options_dict['model'] = model
            if self.verbose:
                logger.debug(f"Using model: {model}")

            # Create options
            options = ClaudeAgentOptions(**options_dict)
            
            # Log request details if verbose
            if self.verbose:
                logger.debug("Claude SDK Request:")
                logger.debug(f"  Prompt length: {len(prompt)} characters")
                logger.debug(f"  System prompt: {system_prompt}")
                if allowed_tools:
                    logger.debug(f"  Allowed tools: {allowed_tools}")
                if disallowed_tools:
                    logger.debug(f"  Disallowed tools: {disallowed_tools}")
            
            # Collect all response chunks
            output_chunks = []
            tokens_used = 0
            chunk_count = 0
            
            # Use one-shot query for simpler execution
            if self.verbose:
                logger.debug("Starting Claude SDK query...")
                self._console.print_header("CLAUDE PROCESSING")
            
            async for message in query(prompt=prompt, options=options):
                chunk_count += 1
                msg_type = type(message).__name__
                
                if self.verbose:
                    print(f"\n[DEBUG: Received {msg_type}]", flush=True)
                    logger.debug(f"Received message type: {msg_type}")
                
                # Handle different message types
                if msg_type == 'AssistantMessage':
                    # Extract content from AssistantMessage
                    if hasattr(message, 'content') and message.content:
                        for content_block in message.content:
                            block_type = type(content_block).__name__
                            
                            if hasattr(content_block, 'text'):
                                # TextBlock
                                text = content_block.text
                                output_chunks.append(text)

                                if self.verbose and text:
                                    self._console.print_message(text)
                                    logger.debug(f"Received assistant text: {len(text)} characters")
                            
                            elif block_type == 'ToolUseBlock':
                                if self.verbose:
                                    tool_name = getattr(content_block, 'name', 'unknown')
                                    tool_id = getattr(content_block, 'id', 'unknown')
                                    tool_input = getattr(content_block, 'input', {})

                                    self._console.print_separator()
                                    self._console.print_status(f"TOOL USE: {tool_name}", style="cyan bold")
                                    self._console.print_info(f"ID: {tool_id[:12]}...")

                                    if tool_input:
                                        self._console.print_info("Input Parameters:")
                                        for key, value in tool_input.items():
                                            value_str = str(value)
                                            if len(value_str) > 100:
                                                value_str = value_str[:97] + "..."
                                            self._console.print_info(f"  - {key}: {value_str}")

                                    logger.debug(f"Tool use detected: {tool_name} (id: {tool_id[:8]}...)")
                                    if hasattr(content_block, 'input'):
                                        logger.debug(f"  Tool input: {content_block.input}")
                            
                            else:
                                if self.verbose:
                                    logger.debug(f"Unknown content block type: {block_type}")
                
                elif msg_type == 'ResultMessage':
                    # ResultMessage contains final result and usage stats
                    if hasattr(message, 'result'):
                        # Don't append result - it's usually a duplicate of assistant message
                        if self.verbose:
                            logger.debug(f"Result message received: {len(str(message.result))} characters")
                    
                    # Extract token usage from ResultMessage
                    if hasattr(message, 'usage'):
                        usage = message.usage
                        if isinstance(usage, dict):
                            tokens_used = usage.get('input_tokens', 0) + usage.get('output_tokens', 0)
                        else:
                            tokens_used = getattr(usage, 'total_tokens', 0)
                        if self.verbose:
                            logger.debug(f"Token usage: {tokens_used} tokens")
                
                elif msg_type == 'SystemMessage':
                    # SystemMessage is initialization data, skip it
                    if self.verbose:
                        logger.debug("System initialization message received")
                
                elif msg_type == 'UserMessage':
                    if self.verbose:
                        logger.debug("User message (tool result) received")

                        if hasattr(message, 'content'):
                            content = message.content
                            if isinstance(content, list):
                                for content_item in content:
                                    if hasattr(content_item, '__class__'):
                                        item_type = content_item.__class__.__name__
                                        if item_type == 'ToolResultBlock':
                                            tool_use_id = getattr(content_item, 'tool_use_id', 'unknown')
                                            result_content = getattr(content_item, 'content', None)
                                            is_error = getattr(content_item, 'is_error', False)

                                            self._console.print_separator()
                                            self._console.print_status("TOOL RESULT", style="yellow bold")
                                            self._console.print_info(f"For Tool ID: {tool_use_id[:12]}...")

                                            if is_error:
                                                self._console.print_error("Status: ERROR")
                                            else:
                                                self._console.print_success("Status: Success")

                                            if result_content:
                                                self._console.print_info("Output:")
                                                if isinstance(result_content, str):
                                                    if len(result_content) > 500:
                                                        self._console.print_message(f"  {result_content[:497]}...")
                                                    else:
                                                        self._console.print_message(f"  {result_content}")
                                                elif isinstance(result_content, list):
                                                    for item in result_content[:3]:
                                                        self._console.print_info(f"  - {item}")
                                                    if len(result_content) > 3:
                                                        self._console.print_info(f"  ... and {len(result_content) - 3} more items")
                
                elif msg_type == 'ToolResultMessage':
                    if self.verbose:
                        logger.debug("Tool result message received")

                        self._console.print_separator()
                        self._console.print_status("TOOL RESULT MESSAGE", style="yellow bold")

                        if hasattr(message, 'tool_use_id'):
                            self._console.print_info(f"Tool ID: {message.tool_use_id[:12]}...")

                        if hasattr(message, 'content'):
                            content = message.content
                            if content:
                                self._console.print_info("Content:")
                                if isinstance(content, str):
                                    if len(content) > 500:
                                        self._console.print_message(f"  {content[:497]}...")
                                    else:
                                        self._console.print_message(f"  {content}")
                                elif isinstance(content, list):
                                    for item in content[:3]:
                                        self._console.print_info(f"  - {item}")
                                    if len(content) > 3:
                                        self._console.print_info(f"  ... and {len(content) - 3} more items")

                        if hasattr(message, 'is_error') and message.is_error:
                            self._console.print_error("Error: True")
                
                elif hasattr(message, 'text'):
                    chunk_text = message.text
                    output_chunks.append(chunk_text)
                    if self.verbose:
                        self._console.print_message(chunk_text)
                        logger.debug(f"Received text chunk {chunk_count}: {len(chunk_text)} characters")

                elif isinstance(message, str):
                    output_chunks.append(message)
                    if self.verbose:
                        self._console.print_message(message)
                        logger.debug(f"Received string chunk {chunk_count}: {len(message)} characters")
                
                else:
                    if self.verbose:
                        logger.debug(f"Unknown message type {msg_type}: {message}")
            
            # Combine output
            output = ''.join(output_chunks)

            # End streaming section if verbose
            if self.verbose:
                self._console.print_separator()
            
            # Always log the output we're about to return
            logger.debug(f"Claude adapter returning {len(output)} characters of output")
            if output:
                logger.debug(f"Output preview: {output[:200]}...")
            
            # Calculate cost if we have token count (using model-specific pricing)
            cost = self._calculate_cost(tokens_used, model) if tokens_used > 0 else None
            
            # Log response details if verbose
            if self.verbose:
                logger.debug("Claude SDK Response:")
                logger.debug(f"  Output length: {len(output)} characters")
                logger.debug(f"  Chunks received: {chunk_count}")
                if tokens_used > 0:
                    logger.debug(f"  Tokens used: {tokens_used}")
                    if cost:
                        logger.debug(f"  Estimated cost: ${cost:.4f}")
                logger.debug(f"Response preview: {output[:500]}..." if len(output) > 500 else f"Response: {output}")
            
            return ToolResponse(
                success=True,
                output=output,
                tokens_used=tokens_used if tokens_used > 0 else None,
                cost=cost,
                metadata={"model": model}
            )
            
        except asyncio.TimeoutError as e:
            # Use error formatter for user-friendly timeout message
            error_msg = ClaudeErrorFormatter.format_error_from_exception(
                iteration=kwargs.get('iteration', 0),
                exception=e
            )
            logger.warning(f"Claude SDK request timed out: {error_msg.message}")
            return ToolResponse(
                success=False,
                output="",
                error=str(error_msg)
            )
        except Exception as e:
            # Check if this is a user-initiated cancellation (SIGINT = exit code -2)
            error_str = str(e)
            if "exit code -2" in error_str or "exit code: -2" in error_str:
                logger.debug("Claude execution cancelled by user")
                return ToolResponse(
                    success=False,
                    output="",
                    error="Execution cancelled by user"
                )

            # Use error formatter for user-friendly error messages
            error_msg = ClaudeErrorFormatter.format_error_from_exception(
                iteration=kwargs.get('iteration', 0),
                exception=e
            )
            
            # Log additional debugging information for "Command failed" errors
            if "Command failed with exit code 1" in error_str:
                logger.error(f"Claude CLI command failed - this may indicate:")
                logger.error("  1. Claude CLI not properly installed or configured")
                logger.error("  2. Missing API key or authentication issues")
                logger.error("  3. Network connectivity problems")
                logger.error("  4. Insufficient permissions")
                logger.error(f"  Full error: {error_str}")
                
                # Try to provide more specific guidance
                try:
                    import subprocess
                    result = subprocess.run(['claude', '--version'], capture_output=True, text=True, timeout=5)
                    if result.returncode == 0:
                        logger.error(f"  Claude CLI version: {result.stdout.strip()}")
                    else:
                        logger.error(f"  Claude CLI check failed: {result.stderr}")
                except Exception as check_e:
                    logger.error(f"  Could not check Claude CLI: {check_e}")
            
            logger.error(f"Claude SDK error: {error_msg.message}", exc_info=True)
            return ToolResponse(
                success=False,
                output="",
                error=str(error_msg)
            )
    
    def _calculate_cost(self, tokens: Optional[int], model: str = None) -> Optional[float]:
        """Calculate estimated cost based on tokens and model.

        Args:
            tokens: Total tokens used (input + output combined)
            model: Model ID used for the request

        Returns:
            Estimated cost in USD, or None if tokens is None/0
        """
        if not tokens:
            return None

        model = model or self._model

        # Get model pricing or use default
        if model in self.MODEL_PRICING:
            pricing = self.MODEL_PRICING[model]
        else:
            # Fallback to Opus 4.5 pricing for unknown models
            pricing = self.MODEL_PRICING[self.DEFAULT_MODEL]

        # Estimate input/output split (typically ~30% input, ~70% output for agent work)
        # This is an approximation since we don't always get separate counts
        input_tokens = int(tokens * 0.3)
        output_tokens = int(tokens * 0.7)

        input_cost = (input_tokens / 1_000_000) * pricing["input"]
        output_cost = (output_tokens / 1_000_000) * pricing["output"]

        return input_cost + output_cost
    
    def estimate_cost(self, prompt: str, model: str = None) -> float:
        """Estimate cost for the prompt.

        Args:
            prompt: The prompt text to estimate cost for
            model: Model ID to use for pricing (defaults to configured model)

        Returns:
            Estimated cost in USD
        """
        # Rough estimation: 1 token ≈ 4 characters
        estimated_tokens = len(prompt) / 4
        return self._calculate_cost(int(estimated_tokens), model) or 0.0

    def kill_subprocess_sync(self) -> None:
        """
        Kill subprocess synchronously (safe to call from signal handler).

        This method uses os.kill() which is signal-safe and can be called
        from the signal handler context. It immediately terminates the subprocess,
        which unblocks any I/O operations waiting on it.
        """
        if self._subprocess_pid:
            try:
                # Try SIGTERM first for graceful shutdown
                os.kill(self._subprocess_pid, signal.SIGTERM)
                # Small delay to allow graceful shutdown - keep minimal for signal handler
                import time
                try:
                    time.sleep(0.01)
                except Exception:
                    pass  # Ignore errors during sleep in signal handler
                # Then SIGKILL if still alive (more forceful)
                try:
                    os.kill(self._subprocess_pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass  # Already dead from SIGTERM
            except ProcessLookupError:
                pass  # Already dead
            except (PermissionError, OSError):
                pass  # Best effort - process might be owned by another user
            finally:
                self._subprocess_pid = None

    async def _cleanup_transport(self) -> None:
        """Clean up transport and kill subprocess with timeout protection."""
        # Kill subprocess first (if not already killed by signal handler)
        if self._subprocess_pid:
            try:
                # Try SIGTERM first
                os.kill(self._subprocess_pid, signal.SIGTERM)
                # Wait with timeout to avoid hanging
                try:
                    await asyncio.wait_for(asyncio.sleep(0.01), timeout=0.05)
                except asyncio.TimeoutError:
                    pass  # Continue even if sleep times out
                # Force kill if still alive
                try:
                    os.kill(self._subprocess_pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass  # Already dead
            except ProcessLookupError:
                pass  # Already terminated
            except (PermissionError, OSError):
                pass  # Best effort cleanup
            finally:
                self._subprocess_pid = None
