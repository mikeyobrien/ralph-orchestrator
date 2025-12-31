# ABOUTME: Context management and optimization for Ralph Orchestrator
# ABOUTME: Handles prompt caching, summarization, and context window management

"""Context management for Ralph Orchestrator."""

from pathlib import Path
from typing import List, Optional, Dict
import hashlib
import logging

logger = logging.getLogger('ralph-orchestrator.context')


class ContextManager:
    """Manage prompt context and optimization."""

    def __init__(
        self,
        prompt_file: Path,
        max_context_size: int = 8000,
        cache_dir: Path = Path(".agent/cache"),
        prompt_text: Optional[str] = None
    ):
        """Initialize context manager.

        Args:
            prompt_file: Path to the main prompt file
            max_context_size: Maximum context size in characters
            cache_dir: Directory for caching context
            prompt_text: Direct prompt text (overrides prompt_file if provided)
        """
        self.prompt_file = prompt_file
        self.max_context_size = max_context_size
        self.cache_dir = cache_dir
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        self.prompt_text = prompt_text  # Direct prompt text override

        # Context components
        self.stable_prefix: Optional[str] = None
        self.dynamic_context: List[str] = []
        self.error_history: List[str] = []
        self.success_patterns: List[str] = []

        # Load initial prompt
        self._load_initial_prompt()
    
    def _load_initial_prompt(self):
        """Load and analyze the initial prompt."""
        # Use direct prompt text if provided
        if self.prompt_text:
            logger.info("Using direct prompt_text input")
            content = self.prompt_text
        elif self.prompt_file.exists():
            try:
                content = self.prompt_file.read_text()
            except UnicodeDecodeError as e:
                logger.warning(f"Encoding error reading {self.prompt_file}: {e}")
                return
            except PermissionError as e:
                logger.warning(f"Permission denied reading {self.prompt_file}: {e}")
                return
            except OSError as e:
                logger.warning(f"OS error reading {self.prompt_file}: {e}")
                return
        else:
            logger.info(f"Prompt file {self.prompt_file} not found")
            return
        
        # Extract stable prefix (instructions that don't change)
        lines = content.split('\n')
        stable_lines = []
        
        for line in lines:
            if line.startswith('#') or line.startswith('##'):
                stable_lines.append(line)
            # No longer breaking on completion markers
            elif len(stable_lines) > 0 and line.strip() == '':
                stable_lines.append(line)
            elif len(stable_lines) > 0:
                break
        
        self.stable_prefix = '\n'.join(stable_lines)
        logger.info(f"Extracted stable prefix: {len(self.stable_prefix)} chars")
    
    def get_prompt(self) -> str:
        """Get the current prompt with optimizations."""
        # Use direct prompt text if provided
        if self.prompt_text:
            base_content = self.prompt_text
        elif self.prompt_file.exists():
            try:
                base_content = self.prompt_file.read_text()
            except UnicodeDecodeError as e:
                logger.warning(f"Encoding error reading {self.prompt_file}: {e}")
                return ""
            except PermissionError as e:
                logger.warning(f"Permission denied reading {self.prompt_file}: {e}")
                return ""
            except OSError as e:
                logger.warning(f"OS error reading {self.prompt_file}: {e}")
                return ""
        else:
            logger.warning(f"No prompt available: prompt_text={self.prompt_text is not None}, prompt_file={self.prompt_file}")
            return ""
        
        # Check if we need to optimize
        if len(base_content) > self.max_context_size:
            return self._optimize_prompt(base_content)
        
        # Add dynamic context if there's room
        if self.dynamic_context:
            context_addition = "\n\n## Previous Context\n" + "\n".join(self.dynamic_context[-3:])
            if len(base_content) + len(context_addition) < self.max_context_size:
                base_content += context_addition
        
        # Add error history if relevant
        if self.error_history:
            error_addition = "\n\n## Recent Errors to Avoid\n" + "\n".join(self.error_history[-2:])
            if len(base_content) + len(error_addition) < self.max_context_size:
                base_content += error_addition
        
        return base_content
    
    def _optimize_prompt(self, content: str) -> str:
        """Optimize a prompt that's too large."""
        logger.info("Optimizing large prompt")
        
        # Strategy 1: Use stable prefix caching
        if self.stable_prefix:
            # Cache the stable prefix
            prefix_hash = hashlib.sha256(self.stable_prefix.encode()).hexdigest()[:8]
            cache_file = self.cache_dir / f"prefix_{prefix_hash}.txt"
            
            if not cache_file.exists():
                cache_file.write_text(self.stable_prefix)
            
            # Reference the cached prefix instead of including it
            optimized = f"<!-- Using cached prefix {prefix_hash} -->\n"
            
            # Add the dynamic part
            dynamic_part = content[len(self.stable_prefix):]
            
            # Truncate if still too large
            if len(dynamic_part) > self.max_context_size - 100:
                dynamic_part = self._summarize_content(dynamic_part)
            
            optimized += dynamic_part
            return optimized
        
        # Strategy 2: Summarize the content
        return self._summarize_content(content)
    
    def _summarize_content(self, content: str) -> str:
        """Summarize content to fit within limits."""
        lines = content.split('\n')
        
        # Keep headers and key instructions
        important_lines = []
        for line in lines:
            if any([
                line.startswith('#'),
                # 'TODO' in line,
                'IMPORTANT' in line,
                'ERROR' in line,
                line.startswith('- [ ]'),  # Unchecked tasks
            ]):
                important_lines.append(line)
        
        summary = '\n'.join(important_lines)
        
        # If still too long, truncate
        if len(summary) > self.max_context_size:
            summary = summary[:self.max_context_size - 100] + "\n<!-- Content truncated -->"
        
        return summary
    
    def update_context(self, output: str):
        """Update dynamic context based on agent output."""
        # Extract key information from output
        if "error" in output.lower():
            # Track errors for learning
            error_lines = [line for line in output.split('\n') if 'error' in line.lower()]
            self.error_history.extend(error_lines[:2])
            
            # Keep only recent errors
            self.error_history = self.error_history[-5:]
        
        if "success" in output.lower() or "complete" in output.lower():
            # Track successful patterns
            success_lines = [line for line in output.split('\n') 
                           if any(word in line.lower() for word in ['success', 'complete', 'done'])]
            self.success_patterns.extend(success_lines[:1])
            self.success_patterns = self.success_patterns[-3:]
        
        # Add to dynamic context (summarized)
        if len(output) > 500:
            summary = output[:200] + "..." + output[-200:]
            self.dynamic_context.append(summary)
        else:
            self.dynamic_context.append(output)
        
        # Keep dynamic context limited
        self.dynamic_context = self.dynamic_context[-5:]
    
    def add_error_feedback(self, error: str):
        """Add error feedback to context."""
        self.error_history.append(f"Error: {error}")
        self.error_history = self.error_history[-5:]
    
    def reset(self):
        """Reset dynamic context."""
        self.dynamic_context = []
        self.error_history = []
        self.success_patterns = []
        logger.info("Context reset")
    
    def get_stats(self) -> Dict:
        """Get context statistics."""
        return {
            "stable_prefix_size": len(self.stable_prefix) if self.stable_prefix else 0,
            "dynamic_context_items": len(self.dynamic_context),
            "error_history_items": len(self.error_history),
            "success_patterns": len(self.success_patterns),
            "cache_files": len(list(self.cache_dir.glob("*.txt")))
        }