# ABOUTME: Abstract base class for tool adapters
# ABOUTME: Defines the interface all tool adapters must implement

"""Base adapter interface for AI tools."""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Optional, Dict, Any
from pathlib import Path
import asyncio


@dataclass
class ToolResponse:
    """Response from a tool execution."""
    
    success: bool
    output: str
    error: Optional[str] = None
    tokens_used: Optional[int] = None
    cost: Optional[float] = None
    metadata: Dict[str, Any] = None
    
    def __post_init__(self):
        if self.metadata is None:
            self.metadata = {}


class ToolAdapter(ABC):
    """Abstract base class for tool adapters."""
    
    def __init__(self, name: str, config=None):
        self.name = name
        self.config = config or type('Config', (), {
            'enabled': True, 'timeout': 300, 'max_retries': 3, 
            'args': [], 'env': {}
        })()
        self.available = self.check_availability()
    
    @abstractmethod
    def check_availability(self) -> bool:
        """Check if the tool is available and properly configured."""
        pass
    
    @abstractmethod
    def execute(self, prompt: str, **kwargs) -> ToolResponse:
        """Execute the tool with the given prompt."""
        pass
    
    async def aexecute(self, prompt: str, **kwargs) -> ToolResponse:
        """Async execute the tool with the given prompt.
        
        Default implementation runs sync execute in thread pool.
        Subclasses can override for native async support.
        """
        loop = asyncio.get_event_loop()
        # Create a function that can be called with no arguments for run_in_executor
        def execute_with_args():
            return self.execute(prompt, **kwargs)
        return await loop.run_in_executor(None, execute_with_args)
    
    def execute_with_file(self, prompt_file: Path, **kwargs) -> ToolResponse:
        """Execute the tool with a prompt file."""
        if not prompt_file.exists():
            return ToolResponse(
                success=False,
                output="",
                error=f"Prompt file {prompt_file} not found"
            )
        
        with open(prompt_file, 'r') as f:
            prompt = f.read()
        
        return self.execute(prompt, **kwargs)
    
    async def aexecute_with_file(self, prompt_file: Path, **kwargs) -> ToolResponse:
        """Async execute the tool with a prompt file."""
        if not prompt_file.exists():
            return ToolResponse(
                success=False,
                output="",
                error=f"Prompt file {prompt_file} not found"
            )

        # Use asyncio.to_thread to avoid blocking the event loop with file I/O
        prompt = await asyncio.to_thread(prompt_file.read_text, encoding='utf-8')

        return await self.aexecute(prompt, **kwargs)
    
    def estimate_cost(self, prompt: str) -> float:
        """Estimate the cost of executing this prompt."""
        # Default implementation - subclasses can override
        return 0.0
    
    def _enhance_prompt_with_instructions(self, prompt: str) -> str:
        """Enhance prompt with orchestration context and instructions.
        
        Args:
            prompt: The original prompt
            
        Returns:
            Enhanced prompt with orchestration instructions
        """
        # Check if instructions already exist in the prompt
        instruction_markers = [
            "ORCHESTRATION CONTEXT:",
            "IMPORTANT INSTRUCTIONS:",
            "Implement only ONE small, focused task"
        ]
        
        # If any marker exists, assume instructions are already present
        for marker in instruction_markers:
            if marker in prompt:
                return prompt
        
        # Add orchestration context and instructions
        orchestration_instructions = """
ORCHESTRATION CONTEXT:
You are running within the Ralph Orchestrator loop. This system will call you repeatedly 
for multiple iterations until the overall task is complete. Each iteration is a separate 
execution where you should make incremental progress.

The final output must be well-tested, documented, and production ready.

IMPORTANT INSTRUCTIONS:
1. Implement only ONE small, focused task from this prompt per iteration.
   - Each iteration is independent - focus on a single atomic change
   - The orchestrator will handle calling you again for the next task
   - Mark subtasks complete as you finish them
   - You must commit your changes after each iteration, for checkpointing.
2. Use the .agent/workspace/ directory for any temporary files or workspaces if not already instructed in the prompt.
3. Follow this workflow for implementing features:
   - Explore: Research and understand the codebase
   - Plan: Design your implementation approach  
   - Implement: Use Test-Driven Development (TDD) - write tests first, then code
   - Commit: Commit your changes with clear messages
4. When you complete a subtask, document it in the prompt file so the next iteration knows what's done.
5. For maximum efficiency, whenever you need to perform multiple independent operations, invoke all relevant tools simultaneously rather than sequentially.
6. If you create any temporary new files, scripts, or helper files for iteration, clean up these files by removing them at the end of the task.

## Agent Scratchpad
Before starting your work, check if .agent/scratchpad.md exists in the current working directory.
If it does, read it to understand what was accomplished in previous iterations and continue from there.

At the end of your iteration, update .agent/scratchpad.md with:
- What you accomplished this iteration
- What remains to be done
- Any important context or decisions made
- Current blockers or issues (if any)

Do NOT restart from scratch if the scratchpad shows previous progress. Continue where the previous iteration left off.

Create the .agent/ directory if it doesn't exist.

---
ORIGINAL PROMPT:

"""
        
        return orchestration_instructions + prompt
    
    def __str__(self) -> str:
        return f"{self.name} (available: {self.available})"
