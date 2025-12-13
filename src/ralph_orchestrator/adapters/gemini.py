# ABOUTME: Gemini CLI adapter implementation 
# ABOUTME: Provides fallback integration with Google's Gemini AI

"""Gemini CLI adapter for Ralph Orchestrator."""

import subprocess
from typing import Optional
from .base import ToolAdapter, ToolResponse


class GeminiAdapter(ToolAdapter):
    """Adapter for Gemini CLI tool."""
    
    def __init__(self):
        self.command = "gemini"
        super().__init__("gemini")
    
    def check_availability(self) -> bool:
        """Check if Gemini CLI is available."""
        try:
            result = subprocess.run(
                [self.command, "--version"],
                capture_output=True,
                timeout=5,
                text=True
            )
            return result.returncode == 0
        except (subprocess.TimeoutExpired, FileNotFoundError):
            return False
    
    def execute(self, prompt: str, **kwargs) -> ToolResponse:
        """Execute Gemini with the given prompt."""
        if not self.available:
            return ToolResponse(
                success=False,
                output="",
                error="Gemini CLI is not available"
            )
        
        try:
            # Enhance prompt with orchestration instructions
            enhanced_prompt = self._enhance_prompt_with_instructions(prompt)
            
            # Build command
            cmd = [self.command]
            
            # Add model if specified
            if kwargs.get("model"):
                cmd.extend(["--model", kwargs["model"]])
            
            # Add the enhanced prompt
            cmd.extend(["-p", enhanced_prompt])
            
            # Add output format if specified
            if kwargs.get("output_format"):
                cmd.extend(["--output", kwargs["output_format"]])
            
            # Execute command
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=kwargs.get("timeout", 300)  # 5 minute default
            )
            
            if result.returncode == 0:
                # Extract token count if available
                tokens = self._extract_token_count(result.stderr)
                
                return ToolResponse(
                    success=True,
                    output=result.stdout,
                    tokens_used=tokens,
                    cost=self._calculate_cost(tokens),
                    metadata={"model": kwargs.get("model", "gemini-2.5-pro")}
                )
            else:
                return ToolResponse(
                    success=False,
                    output=result.stdout,
                    error=result.stderr or "Gemini command failed"
                )
                
        except subprocess.TimeoutExpired:
            return ToolResponse(
                success=False,
                output="",
                error="Gemini command timed out"
            )
        except Exception as e:
            return ToolResponse(
                success=False,
                output="",
                error=str(e)
            )
    
    def _extract_token_count(self, stderr: str) -> Optional[int]:
        """Extract token count from Gemini output."""
        # Implementation depends on Gemini's output format
        return None
    
    def _calculate_cost(self, tokens: Optional[int]) -> Optional[float]:
        """Calculate estimated cost based on tokens."""
        if not tokens:
            return None
        
        # Gemini has free tier up to 1M tokens
        if tokens < 1_000_000:
            return 0.0
        
        # After free tier: $0.001 per 1K tokens (approximate)
        excess_tokens = tokens - 1_000_000
        cost_per_1k = 0.001
        return (excess_tokens / 1000) * cost_per_1k
    
    def estimate_cost(self, prompt: str) -> float:
        """Estimate cost for the prompt."""
        # Rough estimation: 1 token ≈ 4 characters
        estimated_tokens = len(prompt) / 4
        return self._calculate_cost(estimated_tokens) or 0.0