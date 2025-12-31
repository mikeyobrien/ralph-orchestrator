#!/usr/bin/env python3
"""
Simple performance test to verify Q adapter functionality.
"""

import time
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from ralph_orchestrator.adapters.qchat import QChatAdapter

def main():
    adapter = QChatAdapter()
    
    print("Checking Q adapter availability...")
    if not adapter.available:
        print("❌ Q adapter is not available (qchat command not found)")
        print("Performance tests cannot be run without qchat installed.")
        return
    
    print("✅ Q adapter is available")
    
    # Run a simple performance test
    print("\nRunning simple performance test...")
    
    # Test 1: Single request timing
    print("\n1. Single request latency:")
    start = time.perf_counter()
    try:
        result = adapter.execute("echo test", timeout=5)
        elapsed = time.perf_counter() - start
        if result.success:
            print(f"   Response time: {elapsed:.3f}s")
            print("   Success: ✅")
            print(f"   Output: {result.output[:50]}...")
        else:
            print(f"   Error: {result.error}")
    except Exception as e:
        print(f"   Error: {e}")
    
    # Test 2: Multiple sequential requests
    print("\n2. Sequential requests (5 iterations):")
    times = []
    for i in range(5):
        start = time.perf_counter()
        try:
            result = adapter.execute(f"echo iteration {i}", timeout=5)
            if result.success:
                elapsed = time.perf_counter() - start
                times.append(elapsed)
                print(f"   Request {i+1}: {elapsed:.3f}s - Success")
            else:
                print(f"   Request {i+1}: Error - {result.error}")
        except Exception as e:
            print(f"   Request {i+1}: Error - {e}")
    
    if times:
        avg_time = sum(times) / len(times)
        print(f"\n   Average response time: {avg_time:.3f}s")
        print(f"   Min time: {min(times):.3f}s")
        print(f"   Max time: {max(times):.3f}s")
    
    # Test 3: Async performance
    print("\n3. Async execution test:")
    import asyncio
    
    async def test_async():
        start = time.perf_counter()
        try:
            result = await adapter.aexecute("echo async test", timeout=5)
            elapsed = time.perf_counter() - start
            if result.success:
                print(f"   Async response time: {elapsed:.3f}s")
                print("   Success: ✅")
            else:
                print(f"   Error: {result.error}")
        except Exception as e:
            print(f"   Error: {e}")
    
    asyncio.run(test_async())
    
    print("\n✅ Performance test completed")

if __name__ == "__main__":
    main()