# ABOUTME: Tests for the rate limiting functionality in the web module
# ABOUTME: Verifies token bucket algorithm and endpoint-specific limits

"""Tests for the rate limiting module."""

import asyncio
import time
from unittest.mock import AsyncMock, MagicMock, patch

import pytest
from fastapi import Request

from ralph_orchestrator.web.rate_limit import (
    RateLimiter,
    RateLimitConfig,
    rate_limit,
    rate_limit_middleware,
    setup_rate_limit_cleanup,
)


class TestRateLimiter:
    """Tests for the RateLimiter class."""
    
    @pytest.mark.asyncio
    async def test_initialization(self):
        """Test that rate limiter initializes correctly."""
        limiter = RateLimiter(capacity=10, refill_rate=2.0, refill_period=1.0)
        
        assert limiter.capacity == 10
        assert limiter.refill_rate == 2.0
        assert limiter.refill_period == 1.0
        assert len(limiter.buckets) == 0
        assert len(limiter.blocked_ips) == 0
    
    @pytest.mark.asyncio
    async def test_basic_rate_limiting(self):
        """Test basic rate limiting functionality."""
        limiter = RateLimiter(capacity=3, refill_rate=1.0, refill_period=1.0)
        
        # First 3 requests should succeed
        for _ in range(3):
            allowed, retry_after = await limiter.check_rate_limit("127.0.0.1")
            assert allowed is True
            assert retry_after is None
        
        # 4th request should fail
        allowed, retry_after = await limiter.check_rate_limit("127.0.0.1")
        assert allowed is False
        assert retry_after is not None
    
    @pytest.mark.asyncio
    async def test_token_refill(self):
        """Test that tokens refill over time."""
        limiter = RateLimiter(capacity=2, refill_rate=2.0, refill_period=0.1)
        
        # Use all tokens
        await limiter.check_rate_limit("127.0.0.1")
        await limiter.check_rate_limit("127.0.0.1")
        
        # Should be rate limited
        allowed, _ = await limiter.check_rate_limit("127.0.0.1")
        assert allowed is False
        
        # Wait for refill
        await asyncio.sleep(0.15)
        
        # Should have tokens again
        allowed, _ = await limiter.check_rate_limit("127.0.0.1")
        assert allowed is True
    
    @pytest.mark.asyncio
    async def test_multiple_clients(self):
        """Test rate limiting for multiple clients."""
        limiter = RateLimiter(capacity=2, refill_rate=1.0, refill_period=1.0)
        
        # Client 1 uses tokens
        allowed1, _ = await limiter.check_rate_limit("192.168.1.1")
        allowed2, _ = await limiter.check_rate_limit("192.168.1.1")
        allowed3, _ = await limiter.check_rate_limit("192.168.1.1")
        
        assert allowed1 is True
        assert allowed2 is True
        assert allowed3 is False
        
        # Client 2 should have separate bucket
        allowed1, _ = await limiter.check_rate_limit("192.168.1.2")
        allowed2, _ = await limiter.check_rate_limit("192.168.1.2")
        
        assert allowed1 is True
        assert allowed2 is True
    
    @pytest.mark.asyncio
    async def test_ip_blocking(self):
        """Test that IPs get blocked after multiple violations."""
        limiter = RateLimiter(
            capacity=1,
            refill_rate=0.1,
            refill_period=10.0,
            block_duration=1.0
        )
        
        # Use the token
        await limiter.check_rate_limit("10.0.0.1")
        
        # Trigger violations - need to get 5 consecutive violations
        violations = 0
        for _ in range(10):  # Try more times to ensure we hit the violation limit
            allowed, retry_after = await limiter.check_rate_limit("10.0.0.1")
            if not allowed:
                violations += 1
            if "10.0.0.1" in limiter.blocked_ips:
                break
        
        # IP should now be blocked after 5 consecutive violations
        assert "10.0.0.1" in limiter.blocked_ips
        
        # Wait for block to expire
        await asyncio.sleep(1.1)
        
        # Should be unblocked after checking
        allowed, _ = await limiter.check_rate_limit("10.0.0.1")
        assert "10.0.0.1" not in limiter.blocked_ips
    
    @pytest.mark.asyncio
    async def test_cleanup_old_buckets(self):
        """Test cleanup of old inactive buckets."""
        limiter = RateLimiter(capacity=5)
        
        # Create some buckets
        await limiter.check_rate_limit("old_ip")
        await limiter.check_rate_limit("new_ip")
        
        # Modify the old bucket's timestamp
        tokens, _, violations = limiter.buckets["old_ip"]
        limiter.buckets["old_ip"] = (tokens, time.time() - 7200, violations)
        
        # Run cleanup
        await limiter.cleanup_old_buckets(max_age=3600)
        
        # Old bucket should be removed
        assert "old_ip" not in limiter.buckets
        assert "new_ip" in limiter.buckets


class TestRateLimitConfig:
    """Tests for RateLimitConfig class."""
    
    def test_get_limiter(self):
        """Test getting limiters for different categories."""
        auth_limiter = RateLimitConfig.get_limiter("auth")
        api_limiter = RateLimitConfig.get_limiter("api")
        
        assert auth_limiter.capacity == 10
        assert api_limiter.capacity == 100
        
        # Same category should return same instance
        auth_limiter2 = RateLimitConfig.get_limiter("auth")
        assert auth_limiter is auth_limiter2
    
    def test_unknown_category(self):
        """Test that unknown categories use default API limits."""
        unknown_limiter = RateLimitConfig.get_limiter("unknown")
        api_config = RateLimitConfig.LIMITS["api"]
        
        assert unknown_limiter.capacity == api_config["capacity"]
        assert unknown_limiter.refill_rate == api_config["refill_rate"]


class TestRateLimitDecorator:
    """Tests for the rate_limit decorator."""
    
    @pytest.mark.asyncio
    async def test_decorator_allows_requests(self):
        """Test that decorator allows requests within limit."""
        
        @rate_limit(category="api")
        async def test_endpoint(request: Request):
            return {"status": "ok"}
        
        # Create mock request
        request = MagicMock(spec=Request)
        request.client.host = "127.0.0.1"
        request.headers = {}
        
        # Should allow request
        result = await test_endpoint(request)
        assert result == {"status": "ok"}
    
    @pytest.mark.asyncio
    async def test_decorator_blocks_excessive_requests(self):
        """Test that decorator blocks excessive requests."""
        # Clear any existing limiters to avoid test interference
        if hasattr(RateLimitConfig, '_limiters'):
            delattr(RateLimitConfig, '_limiters')
        
        # Create a fresh limiter with very low capacity for this test
        test_limiter = RateLimiter(capacity=2, refill_rate=0.1, refill_period=10.0)
        
        with patch.object(RateLimitConfig, 'get_limiter', return_value=test_limiter):
            @rate_limit(category="test")
            async def test_endpoint(request: Request):
                return {"status": "ok"}
            
            # Use unique IPs to avoid interference
            request1 = MagicMock(spec=Request)
            request1.client.host = "192.168.1.100"
            request1.headers = {}
            
            # First request should succeed
            result = await test_endpoint(request1)
            assert result == {"status": "ok"}
            
            # Second request should succeed (we have capacity of 2)
            result = await test_endpoint(request1)
            assert result == {"status": "ok"}
            
            # Third request should be blocked
            from fastapi.responses import JSONResponse
            result = await test_endpoint(request1)
            assert isinstance(result, JSONResponse)
            assert result.status_code == 429
    
    @pytest.mark.asyncio
    async def test_decorator_uses_forwarded_ip(self):
        """Test that decorator uses X-Forwarded-For header."""
        
        @rate_limit(category="api")
        async def test_endpoint(request: Request):
            return {"status": "ok"}
        
        request = MagicMock(spec=Request)
        request.client.host = "127.0.0.1"
        request.headers = {"X-Forwarded-For": "10.0.0.1, proxy1, proxy2"}
        
        # Mock the limiter to verify the correct IP is used
        with patch.object(RateLimitConfig, 'get_limiter') as mock_get_limiter:
            mock_limiter = AsyncMock()
            mock_limiter.check_rate_limit = AsyncMock(return_value=(True, None))
            mock_get_limiter.return_value = mock_limiter
            
            await test_endpoint(request)
            
            # Verify that the forwarded IP was used
            mock_limiter.check_rate_limit.assert_called_with("10.0.0.1")


class TestRateLimitMiddleware:
    """Tests for the rate limit middleware."""
    
    @pytest.mark.asyncio
    async def test_middleware_categorizes_paths(self):
        """Test that middleware correctly categorizes different paths."""
        
        async def mock_call_next(request):
            return MagicMock(status_code=200)
        
        # Test auth path
        request = MagicMock(spec=Request)
        request.url.path = "/api/auth/login"
        request.client.host = "127.0.0.1"
        request.headers = {}
        
        with patch.object(RateLimitConfig, 'get_limiter') as mock_get_limiter:
            mock_limiter = AsyncMock()
            mock_limiter.check_rate_limit = AsyncMock(return_value=(True, None))
            mock_get_limiter.return_value = mock_limiter
            
            await rate_limit_middleware(request, mock_call_next)
            mock_get_limiter.assert_called_with("auth")
        
        # Test admin path
        request.url.path = "/api/admin/users"
        with patch.object(RateLimitConfig, 'get_limiter') as mock_get_limiter:
            mock_limiter = AsyncMock()
            mock_limiter.check_rate_limit = AsyncMock(return_value=(True, None))
            mock_get_limiter.return_value = mock_limiter
            
            await rate_limit_middleware(request, mock_call_next)
            mock_get_limiter.assert_called_with("admin")
    
    @pytest.mark.asyncio
    async def test_middleware_blocks_requests(self):
        """Test that middleware blocks requests when rate limited."""
        
        async def mock_call_next(request):
            return MagicMock(status_code=200)
        
        request = MagicMock(spec=Request)
        request.url.path = "/api/test"
        request.client.host = "127.0.0.1"
        request.headers = {}
        
        with patch.object(RateLimitConfig, 'get_limiter') as mock_get_limiter:
            mock_limiter = AsyncMock()
            mock_limiter.check_rate_limit = AsyncMock(return_value=(False, 60))
            mock_get_limiter.return_value = mock_limiter
            
            response = await rate_limit_middleware(request, mock_call_next)
            
            assert response.status_code == 429
            assert response.headers.get("Retry-After") == "60"


class TestCleanupTask:
    """Tests for the cleanup task."""
    
    @pytest.mark.asyncio
    async def test_setup_cleanup_task(self):
        """Test that cleanup task is set up correctly."""
        with patch('asyncio.create_task') as mock_create_task:
            await setup_rate_limit_cleanup()
            mock_create_task.assert_called_once()
    
    @pytest.mark.asyncio
    async def test_cleanup_runs_periodically(self):
        """Test that cleanup runs periodically."""
        # Test that the setup function returns a task
        task = await setup_rate_limit_cleanup()
        assert isinstance(task, asyncio.Task)
        
        # Cancel the task to clean up
        task.cancel()
        try:
            await task
        except asyncio.CancelledError:
            pass