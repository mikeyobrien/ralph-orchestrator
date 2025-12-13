# ABOUTME: Implements rate limiting for API endpoints to prevent abuse
# ABOUTME: Uses token bucket algorithm with configurable limits per endpoint

"""Rate limiting implementation for the Ralph web server."""

import asyncio
import time
from collections import defaultdict
from typing import Dict, Optional, Tuple
from functools import wraps
import logging

from fastapi import Request, status
from fastapi.responses import JSONResponse

logger = logging.getLogger(__name__)


class RateLimiter:
    """Token bucket rate limiter implementation.
    
    Uses a token bucket algorithm to limit requests per IP address.
    Tokens are replenished at a fixed rate up to a maximum capacity.
    """
    
    def __init__(
        self,
        capacity: int = 100,
        refill_rate: float = 10.0,
        refill_period: float = 1.0,
        block_duration: float = 60.0
    ):
        """Initialize the rate limiter.
        
        Args:
            capacity: Maximum number of tokens in the bucket
            refill_rate: Number of tokens to add per refill period
            refill_period: Time in seconds between refills
            block_duration: Time in seconds to block an IP after exhausting tokens
        """
        self.capacity = capacity
        self.refill_rate = refill_rate
        self.refill_period = refill_period
        self.block_duration = block_duration
        
        # Store buckets per IP address
        self.buckets: Dict[str, Tuple[float, float, float]] = defaultdict(
            lambda: (float(capacity), time.time(), 0.0)
        )
        self.blocked_ips: Dict[str, float] = {}
        
        # Lock for thread-safe access
        self._lock = asyncio.Lock()
    
    async def check_rate_limit(self, identifier: str) -> Tuple[bool, Optional[int]]:
        """Check if a request is allowed under the rate limit.
        
        Args:
            identifier: Unique identifier for the client (e.g., IP address)
            
        Returns:
            Tuple of (allowed, retry_after_seconds)
        """
        async with self._lock:
            current_time = time.time()
            
            # Check if IP is blocked
            if identifier in self.blocked_ips:
                block_end = self.blocked_ips[identifier]
                if current_time < block_end:
                    retry_after = int(block_end - current_time)
                    return False, retry_after
                else:
                    # Unblock the IP
                    del self.blocked_ips[identifier]
            
            # Get or create bucket for this identifier
            tokens, last_refill, consecutive_violations = self.buckets[identifier]
            
            # Calculate tokens to add based on time elapsed
            time_elapsed = current_time - last_refill
            tokens_to_add = (time_elapsed / self.refill_period) * self.refill_rate
            tokens = min(self.capacity, tokens + tokens_to_add)
            
            if tokens >= 1:
                # Consume a token
                tokens -= 1
                consecutive_violations = 0
                self.buckets[identifier] = (tokens, current_time, consecutive_violations)
                return True, None
            else:
                # No tokens available
                consecutive_violations += 1
                
                # Block IP if too many consecutive violations
                if consecutive_violations >= 5:
                    block_end = current_time + self.block_duration
                    self.blocked_ips[identifier] = block_end
                    del self.buckets[identifier]
                    return False, int(self.block_duration)
                
                self.buckets[identifier] = (tokens, current_time, consecutive_violations)
                retry_after = int(self.refill_period / self.refill_rate)
                return False, retry_after
    
    async def cleanup_old_buckets(self, max_age: float = 3600.0):
        """Remove old inactive buckets to prevent memory growth.
        
        Args:
            max_age: Maximum age in seconds for inactive buckets
        """
        async with self._lock:
            current_time = time.time()
            to_remove = []
            
            for identifier, (tokens, last_refill, _) in self.buckets.items():
                if current_time - last_refill > max_age:
                    to_remove.append(identifier)
            
            for identifier in to_remove:
                del self.buckets[identifier]
            
            # Clean up expired blocks
            to_remove = []
            for identifier, block_end in self.blocked_ips.items():
                if current_time >= block_end:
                    to_remove.append(identifier)
            
            for identifier in to_remove:
                del self.blocked_ips[identifier]
            
            if to_remove:
                logger.info(f"Cleaned up {len(to_remove)} expired rate limit entries")


class RateLimitConfig:
    """Configuration for different rate limit tiers."""
    
    # Default limits for different endpoint categories
    LIMITS = {
        "auth": {"capacity": 10, "refill_rate": 1.0, "refill_period": 60.0},  # 10 requests/minute
        "api": {"capacity": 100, "refill_rate": 10.0, "refill_period": 1.0},   # 100 requests/10 seconds
        "websocket": {"capacity": 10, "refill_rate": 1.0, "refill_period": 10.0},  # 10 connections/10 seconds
        "static": {"capacity": 200, "refill_rate": 20.0, "refill_period": 1.0},    # 200 requests/20 seconds
        "admin": {"capacity": 50, "refill_rate": 5.0, "refill_period": 1.0},       # 50 requests/5 seconds
    }
    
    @classmethod
    def get_limiter(cls, category: str) -> RateLimiter:
        """Get or create a rate limiter for a specific category.
        
        Args:
            category: The category of endpoints to limit
            
        Returns:
            A configured RateLimiter instance
        """
        if not hasattr(cls, "_limiters"):
            cls._limiters = {}
        
        if category not in cls._limiters:
            config = cls.LIMITS.get(category, cls.LIMITS["api"])
            cls._limiters[category] = RateLimiter(**config)
        
        return cls._limiters[category]


def rate_limit(category: str = "api"):
    """Decorator to apply rate limiting to FastAPI endpoints.
    
    Args:
        category: The rate limit category to apply
    """
    def decorator(func):
        @wraps(func)
        async def wrapper(request: Request, *args, **kwargs):
            # Get client IP address
            client_ip = request.client.host if request.client else "unknown"
            
            # Check for X-Forwarded-For header (for proxies)
            forwarded_for = request.headers.get("X-Forwarded-For")
            if forwarded_for:
                client_ip = forwarded_for.split(",")[0].strip()
            
            # Get the appropriate rate limiter
            limiter = RateLimitConfig.get_limiter(category)
            
            # Check rate limit
            allowed, retry_after = await limiter.check_rate_limit(client_ip)
            
            if not allowed:
                logger.warning(f"Rate limit exceeded for {client_ip} on {category} endpoint")
                response = JSONResponse(
                    status_code=status.HTTP_429_TOO_MANY_REQUESTS,
                    content={
                        "detail": "Rate limit exceeded",
                        "retry_after": retry_after
                    }
                )
                if retry_after:
                    response.headers["Retry-After"] = str(retry_after)
                return response
            
            # Call the original function
            return await func(request, *args, **kwargs)
        
        return wrapper
    return decorator


async def setup_rate_limit_cleanup():
    """Set up periodic cleanup of old rate limit buckets.
    
    Returns:
        The cleanup task
    """
    async def cleanup_task():
        while True:
            try:
                await asyncio.sleep(300)  # Run every 5 minutes
                for category in RateLimitConfig.LIMITS:
                    limiter = RateLimitConfig.get_limiter(category)
                    await limiter.cleanup_old_buckets()
            except Exception as e:
                logger.error(f"Error in rate limit cleanup: {e}")
    
    return asyncio.create_task(cleanup_task())


# Middleware for global rate limiting
async def rate_limit_middleware(request: Request, call_next):
    """Global rate limiting middleware for all requests.
    
    Args:
        request: The incoming request
        call_next: The next middleware or endpoint
        
    Returns:
        The response
    """
    # Determine the category based on the path
    path = request.url.path
    
    if path.startswith("/api/auth"):
        category = "auth"
    elif path.startswith("/api/admin"):
        category = "admin"
    elif path.startswith("/ws"):
        category = "websocket"
    elif path.startswith("/static"):
        category = "static"
    else:
        category = "api"
    
    # Get client IP
    client_ip = request.client.host if request.client else "unknown"
    forwarded_for = request.headers.get("X-Forwarded-For")
    if forwarded_for:
        client_ip = forwarded_for.split(",")[0].strip()
    
    # Check rate limit
    limiter = RateLimitConfig.get_limiter(category)
    allowed, retry_after = await limiter.check_rate_limit(client_ip)
    
    if not allowed:
        logger.warning(f"Rate limit exceeded for {client_ip} on {path}")
        response = JSONResponse(
            status_code=status.HTTP_429_TOO_MANY_REQUESTS,
            content={
                "detail": "Rate limit exceeded",
                "retry_after": retry_after
            }
        )
        if retry_after:
            response.headers["Retry-After"] = str(retry_after)
        return response
    
    # Continue with the request
    response = await call_next(request)
    return response