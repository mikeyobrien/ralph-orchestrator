# ABOUTME: Q Chat adapter implementation for q CLI tool
# ABOUTME: Provides integration with q chat command for AI interactions

"""Q Chat adapter for Ralph Orchestrator."""

import subprocess
import os
import sys
import signal
import threading
import asyncio
import time
try:
    import fcntl  # Unix-only
except ModuleNotFoundError:
    fcntl = None
from .base import ToolAdapter, ToolResponse
from ..logging_config import RalphLogger

# Get logger for this module
logger = RalphLogger.get_logger(RalphLogger.ADAPTER_QCHAT)


class QChatAdapter(ToolAdapter):
    """Adapter for Q Chat CLI tool."""
    
    def __init__(self):
        # Get configuration from environment variables
        self.command = os.getenv("RALPH_QCHAT_COMMAND", "q")
        self.default_timeout = int(os.getenv("RALPH_QCHAT_TIMEOUT", "600"))
        self.default_prompt_file = os.getenv("RALPH_QCHAT_PROMPT_FILE", "PROMPT.md")
        self.trust_all_tools = os.getenv("RALPH_QCHAT_TRUST_TOOLS", "true").lower() == "true"
        self.no_interactive = os.getenv("RALPH_QCHAT_NO_INTERACTIVE", "true").lower() == "true"
        
        # Initialize signal handler attributes before calling super()
        self._original_sigint = None
        self._original_sigterm = None
        
        super().__init__("qchat")
        self.current_process = None
        self.shutdown_requested = False
        
        # Thread synchronization
        self._lock = threading.Lock()
        
        # Register signal handlers to propagate shutdown to subprocess
        self._register_signal_handlers()
        
        logger.info(f"Q Chat adapter initialized - Command: {self.command}, "
                   f"Default timeout: {self.default_timeout}s, "
                   f"Trust tools: {self.trust_all_tools}")
    
    def _register_signal_handlers(self):
        """Register signal handlers and store originals."""
        self._original_sigint = signal.signal(signal.SIGINT, self._signal_handler)
        self._original_sigterm = signal.signal(signal.SIGTERM, self._signal_handler)
        logger.debug("Signal handlers registered for SIGINT and SIGTERM")
    
    def _restore_signal_handlers(self):
        """Restore original signal handlers."""
        if hasattr(self, '_original_sigint') and self._original_sigint is not None:
            signal.signal(signal.SIGINT, self._original_sigint)
        if hasattr(self, '_original_sigterm') and self._original_sigterm is not None:
            signal.signal(signal.SIGTERM, self._original_sigterm)
    
    def _signal_handler(self, signum, frame):
        """Handle shutdown signals and terminate running subprocess."""
        with self._lock:
            self.shutdown_requested = True
            process = self.current_process
        
        if process and process.poll() is None:
            logger.warning(f"Received signal {signum}, terminating q chat process...")
            try:
                process.terminate()
                process.wait(timeout=3)
                logger.debug("Process terminated gracefully")
            except subprocess.TimeoutExpired:
                logger.warning("Force killing q chat process...")
                process.kill()
                try:
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    logger.warning("Process may still be running after force kill")
    
    def check_availability(self) -> bool:
        """Check if q CLI is available."""
        try:
            # Try to check if q command exists
            result = subprocess.run(
                ["which", self.command],
                capture_output=True,
                timeout=5,
                text=True
            )
            available = result.returncode == 0
            logger.debug(f"Q command '{self.command}' availability check: {available}")
            return available
        except (subprocess.TimeoutExpired, FileNotFoundError) as e:
            logger.warning(f"Q command availability check failed: {e}")
            return False
    
    def execute(self, prompt: str, **kwargs) -> ToolResponse:
        """Execute q chat with the given prompt."""
        if not self.available:
            return ToolResponse(
                success=False,
                output="",
                error="q CLI is not available"
            )
        
        try:
            # Get verbose flag from kwargs
            verbose = kwargs.get('verbose', True)
            
            # Get the prompt file path from kwargs if available
            prompt_file = kwargs.get('prompt_file', self.default_prompt_file)
            
            logger.info(f"Executing Q chat - Prompt file: {prompt_file}, Verbose: {verbose}")
            
            # Enhance prompt with orchestration instructions
            enhanced_prompt = self._enhance_prompt_with_instructions(prompt)
            
            # Construct a more effective prompt for q chat
            # Tell it explicitly to edit the prompt file
            effective_prompt = (
                f"Please read and complete the task described in the file '{prompt_file}'. "
                f"The current content is:\n\n{enhanced_prompt}\n\n"
                f"Edit the file '{prompt_file}' directly to add your solution and progress updates."
            )
            
            # Build command - q chat works with files by adding them to context
            # We pass the prompt through stdin and tell it to trust file operations
            cmd = [self.command, "chat"]
            
            if self.no_interactive:
                cmd.append("--no-interactive")
            
            if self.trust_all_tools:
                cmd.append("--trust-all-tools")
            
            cmd.append(effective_prompt)
            
            logger.debug(f"Command constructed: {' '.join(cmd)}")
            
            timeout = kwargs.get("timeout", self.default_timeout)
            
            if verbose:
                logger.info("Starting q chat command...")
                logger.info(f"Command: {' '.join(cmd)}")
                logger.info(f"Working directory: {os.getcwd()}")
                logger.info(f"Timeout: {timeout} seconds")
                print("-" * 60, file=sys.stderr)
            
            # Use Popen for real-time output streaming
            process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                cwd=os.getcwd(),
                bufsize=0,  # Unbuffered to prevent deadlock
                universal_newlines=True
            )
            
            # Set process reference with lock
            with self._lock:
                self.current_process = process
            
            # Make pipes non-blocking to prevent deadlock
            self._make_non_blocking(process.stdout)
            self._make_non_blocking(process.stderr)
            
            # Collect output while streaming
            stdout_lines = []
            stderr_lines = []
            
            start_time = time.time()
            last_output_time = start_time
            
            while True:
                # Check for shutdown signal first with lock
                with self._lock:
                    shutdown = self.shutdown_requested
                
                if shutdown:
                    if verbose:
                        print("Shutdown requested, terminating q chat process...", file=sys.stderr)
                    process.terminate()
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=2)
                    
                    # Clean up process reference with lock
                    with self._lock:
                        self.current_process = None
                    
                    return ToolResponse(
                        success=False,
                        output="".join(stdout_lines),
                        error="Process terminated due to shutdown signal"
                    )
                
                # Check for timeout
                elapsed_time = time.time() - start_time
                
                # Log progress every 30 seconds
                if int(elapsed_time) % 30 == 0 and int(elapsed_time) > 0:
                    logger.debug(f"Q chat still running... elapsed: {elapsed_time:.1f}s / {timeout}s")
                    
                    # Check if the process seems stuck (no output for a while)
                    time_since_output = time.time() - last_output_time
                    if time_since_output > 60:
                        logger.info(f"No output received for {time_since_output:.1f}s, Q might be stuck")
                    
                    if verbose:
                        print(f"Q chat still running... elapsed: {elapsed_time:.1f}s / {timeout}s", file=sys.stderr)
                
                if elapsed_time > timeout:
                    logger.warning(f"Command timed out after {elapsed_time:.2f} seconds")
                    if verbose:
                        print(f"Command timed out after {elapsed_time:.2f} seconds", file=sys.stderr)
                    
                    # Try to terminate gracefully first
                    process.terminate()
                    try:
                        # Wait a bit for graceful termination
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        logger.warning("Graceful termination failed, force killing process")
                        if verbose:
                            print("Graceful termination failed, force killing process", file=sys.stderr)
                        process.kill()
                        # Wait for force kill to complete
                        try:
                            process.wait(timeout=2)
                        except subprocess.TimeoutExpired:
                            logger.warning("Process may still be running after kill")
                            if verbose:
                                print("Warning: Process may still be running after kill", file=sys.stderr)
                    
                    # Try to capture any remaining output after termination
                    try:
                        remaining_stdout = process.stdout.read()
                        remaining_stderr = process.stderr.read()
                        if remaining_stdout:
                            stdout_lines.append(remaining_stdout)
                        if remaining_stderr:
                            stderr_lines.append(remaining_stderr)
                    except Exception as e:
                        logger.warning(f"Could not read remaining output after timeout: {e}")
                        if verbose:
                            print(f"Warning: Could not read remaining output after timeout: {e}", file=sys.stderr)
                    
                    # Clean up process reference with lock
                    with self._lock:
                        self.current_process = None
                    
                    return ToolResponse(
                        success=False,
                        output="".join(stdout_lines),
                        error=f"q chat command timed out after {elapsed_time:.2f} seconds"
                    )
                
                # Check if process is still running
                if process.poll() is not None:
                    # Process finished, read remaining output
                    remaining_stdout = process.stdout.read()
                    remaining_stderr = process.stderr.read()
                    
                    if remaining_stdout:
                        stdout_lines.append(remaining_stdout)
                        if verbose:
                            print(f"{remaining_stdout}", end='', file=sys.stderr)
                    
                    if remaining_stderr:
                        stderr_lines.append(remaining_stderr)
                        if verbose:
                            print(f"{remaining_stderr}", end='', file=sys.stderr)
                    
                    break
                
                # Read available data without blocking
                try:
                    # Read stdout
                    stdout_data = self._read_available(process.stdout)
                    if stdout_data:
                        stdout_lines.append(stdout_data)
                        last_output_time = time.time()
                        if verbose:
                            print(stdout_data, end='', file=sys.stderr)
                    
                    # Read stderr
                    stderr_data = self._read_available(process.stderr)
                    if stderr_data:
                        stderr_lines.append(stderr_data)
                        last_output_time = time.time()
                        if verbose:
                            print(stderr_data, end='', file=sys.stderr)
                    
                    # Small sleep to prevent busy waiting
                    time.sleep(0.01)
                    
                except BlockingIOError:
                    # Non-blocking read returns BlockingIOError when no data available
                    pass
            
            # Get final return code
            returncode = process.poll()
            
            execution_time = time.time() - start_time
            logger.info(f"Process completed - Return code: {returncode}, Execution time: {execution_time:.2f}s")
            
            if verbose:
                print("-" * 60, file=sys.stderr)
                print(f"Process completed with return code: {returncode}", file=sys.stderr)
                print(f"Total execution time: {execution_time:.2f} seconds", file=sys.stderr)
            
            # Clean up process reference with lock
            with self._lock:
                self.current_process = None
            
            # Combine output
            full_stdout = "".join(stdout_lines)
            full_stderr = "".join(stderr_lines)
            
            if returncode == 0:
                logger.debug(f"Q chat succeeded - Output length: {len(full_stdout)} chars")
                return ToolResponse(
                    success=True,
                    output=full_stdout,
                    metadata={
                        "tool": "q chat",
                        "execution_time": execution_time,
                        "verbose": verbose,
                        "return_code": returncode
                    }
                )
            else:
                logger.warning(f"Q chat failed - Return code: {returncode}, Error: {full_stderr[:200]}")
                return ToolResponse(
                    success=False,
                    output=full_stdout,
                    error=full_stderr or f"q chat command failed with code {returncode}"
                )
                
        except Exception as e:
            logger.exception(f"Exception during Q chat execution: {str(e)}")
            if verbose:
                print(f"Exception occurred: {str(e)}", file=sys.stderr)
            # Clean up process reference on exception (matches async version's finally block)
            with self._lock:
                self.current_process = None
            return ToolResponse(
                success=False,
                output="",
                error=str(e)
            )

    def _make_non_blocking(self, pipe):
        """Make a pipe non-blocking to prevent deadlock."""
        if not pipe or fcntl is None:
            return

        try:
            fd = pipe.fileno()
            if isinstance(fd, int) and fd >= 0:
                flags = fcntl.fcntl(fd, fcntl.F_GETFL)
                fcntl.fcntl(fd, fcntl.F_SETFL, flags | os.O_NONBLOCK)
        except (AttributeError, ValueError, OSError):
            # In tests or when pipe doesn't support fileno()
            pass

    
    def _read_available(self, pipe):
        """Read available data from a non-blocking pipe."""
        if not pipe:
            return ""
        
        try:
            # Try to read up to 4KB at a time
            data = pipe.read(4096)
            # Ensure we always return a string, not None
            if data is None:
                return ""
            return data if data else ""
        except (IOError, OSError):
            # Would block or no data available
            return ""
    
    async def aexecute(self, prompt: str, **kwargs) -> ToolResponse:
        """Native async execution using asyncio subprocess."""
        if not self.available:
            return ToolResponse(
                success=False,
                output="",
                error="q CLI is not available"
            )
        
        try:
            verbose = kwargs.get('verbose', True)
            prompt_file = kwargs.get('prompt_file', self.default_prompt_file)
            timeout = kwargs.get('timeout', self.default_timeout)
            
            logger.info(f"Executing Q chat async - Prompt file: {prompt_file}, Timeout: {timeout}s")
            
            # Enhance prompt with orchestration instructions
            enhanced_prompt = self._enhance_prompt_with_instructions(prompt)
            
            # Construct effective prompt
            effective_prompt = (
                f"Please read and complete the task described in the file '{prompt_file}'. "
                f"The current content is:\n\n{enhanced_prompt}\n\n"
                f"Edit the file '{prompt_file}' directly to add your solution and progress updates."
            )
            
            # Build command
            cmd = [
                self.command,
                "chat",
                "--no-interactive",
                "--trust-all-tools",
                effective_prompt
            ]
            
            logger.debug(f"Starting async Q chat command: {' '.join(cmd)}")
            if verbose:
                print("Starting q chat command (async)...", file=sys.stderr)
                print(f"Command: {' '.join(cmd)}", file=sys.stderr)
                print("-" * 60, file=sys.stderr)
            
            # Create async subprocess
            process = await asyncio.create_subprocess_exec(
                *cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                cwd=os.getcwd()
            )
            
            # Set process reference with lock
            with self._lock:
                self.current_process = process
            
            try:
                # Wait for completion with timeout
                stdout_data, stderr_data = await asyncio.wait_for(
                    process.communicate(),
                    timeout=timeout
                )
                
                # Decode output
                stdout = stdout_data.decode('utf-8') if stdout_data else ""
                stderr = stderr_data.decode('utf-8') if stderr_data else ""
                
                if verbose and stdout:
                    print(stdout, file=sys.stderr)
                if verbose and stderr:
                    print(stderr, file=sys.stderr)
                
                # Check return code
                if process.returncode == 0:
                    logger.debug(f"Async Q chat succeeded - Output length: {len(stdout)} chars")
                    return ToolResponse(
                        success=True,
                        output=stdout,
                        metadata={
                            "tool": "q chat",
                            "verbose": verbose,
                            "async": True,
                            "return_code": process.returncode
                        }
                    )
                else:
                    logger.warning(f"Async Q chat failed - Return code: {process.returncode}")
                    return ToolResponse(
                        success=False,
                        output=stdout,
                        error=stderr or f"q chat failed with code {process.returncode}"
                    )
                
            except asyncio.TimeoutError:
                # Timeout occurred
                logger.warning(f"Async q chat timed out after {timeout} seconds")
                if verbose:
                    print(f"Async q chat timed out after {timeout} seconds", file=sys.stderr)
                
                # Try to terminate process
                try:
                    process.terminate()
                    await asyncio.wait_for(process.wait(), timeout=3)
                except (asyncio.TimeoutError, ProcessLookupError):
                    try:
                        process.kill()
                        await process.wait()
                    except ProcessLookupError:
                        pass
                
                return ToolResponse(
                    success=False,
                    output="",
                    error=f"q chat command timed out after {timeout} seconds"
                )
            
            finally:
                # Clean up process reference
                with self._lock:
                    self.current_process = None
                    
        except Exception as e:
            logger.exception(f"Async execution error: {str(e)}")
            if kwargs.get('verbose'):
                print(f"Async execution error: {str(e)}", file=sys.stderr)
            return ToolResponse(
                success=False,
                output="",
                error=str(e)
            )
    
    def estimate_cost(self, prompt: str) -> float:
        """Q chat cost estimation (if applicable)."""
        # Q chat might be free or have different pricing
        # Return 0 for now, can be updated based on actual pricing
        return 0.0
    
    def __del__(self):
        """Cleanup on deletion."""
        # Restore original signal handlers
        self._restore_signal_handlers()
        
        # Ensure any running process is terminated
        if hasattr(self, '_lock'):
            with self._lock:
                process = self.current_process if hasattr(self, 'current_process') else None
        else:
            process = getattr(self, 'current_process', None)
        
        if process:
            try:
                if hasattr(process, 'poll'):
                    # Sync process
                    if process.poll() is None:
                        process.terminate()
                        process.wait(timeout=1)
                else:
                    # Async process - can't do much in __del__
                    pass
            except Exception as e:
                # Best-effort cleanup during interpreter shutdown
                # Log at debug level since __del__ is unreliable
                logger.debug(f"Cleanup warning in __del__: {type(e).__name__}: {e}")