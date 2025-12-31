# ABOUTME: Test suite for validating Q chat adapter message queue processing
# ABOUTME: Ensures delivery guarantees and proper message handling under various conditions

"""Test suite for Q chat adapter message queue processing and delivery guarantees.

NOTE: These tests require q CLI to be available. They are marked with skipif
to skip when q CLI is not installed.
"""

import asyncio
import concurrent.futures
import pytest
import threading
import random
from unittest.mock import Mock, patch
import subprocess
import shutil

from src.ralph_orchestrator.adapters.qchat import QChatAdapter


# Check if q CLI is available
Q_CLI_AVAILABLE = shutil.which("q") is not None


@pytest.mark.skipif(not Q_CLI_AVAILABLE, reason="q CLI not available - these are integration tests")
class TestMessageQueueProcessing:
    """Test message queue processing and delivery guarantees."""
    
    def test_message_order_preservation(self):
        """Test that messages are processed in the order they are submitted."""
        adapter = QChatAdapter()

        # Track message processing order
        processed_messages = []

        def mock_popen_factory(*args, **kwargs):
            """Create a mock process for each command."""
            cmd = args[0] if args else kwargs.get('args', [])

            if len(cmd) > 0:
                prompt = cmd[-1]
                import re
                match = re.search(r'Message (\d+)', prompt)
                if match:
                    processed_messages.append(int(match.group(1)))

            mock_process = Mock()
            # Use return_value instead of side_effect to avoid iterator exhaustion
            mock_process.poll.return_value = 0
            mock_process.stdout = Mock()
            mock_process.stderr = Mock()
            mock_process.stdout.read = Mock(return_value="Output")
            mock_process.stderr.read = Mock(return_value="")
            mock_process.stdout.fileno = Mock(return_value=3)
            mock_process.stderr.fileno = Mock(return_value=4)

            return mock_process

        with patch('src.ralph_orchestrator.adapters.qchat.subprocess.Popen', side_effect=mock_popen_factory):
            messages = ["Message 1", "Message 2", "Message 3"]
            for i, msg in enumerate(messages, 1):
                response = adapter.execute(msg, verbose=False, timeout=5)
                assert response.success
                assert i in processed_messages
    
    def test_concurrent_message_processing(self):
        """Test that concurrent messages are handled without loss."""
        adapter = QChatAdapter()
        
        # Track all processed messages
        processed_messages = []
        processing_lock = threading.Lock()
        
        def mock_process_factory(*args, **kwargs):
            """Create a mock process that records the message."""
            cmd = args[0] if args else kwargs.get('args', [])
            mock_process = Mock()
            mock_process.poll.side_effect = [None] * 10 + [0] * 5
            mock_process.stdout = Mock()
            mock_process.stderr = Mock()
            mock_process.stdout.read.return_value = "Processed"
            mock_process.stderr.read.return_value = ""
            mock_process.stdout.fileno.return_value = 3
            mock_process.stderr.fileno.return_value = 4
            
            # Extract and record the message
            if len(cmd) > 0:
                prompt = cmd[-1]  # Last argument is the prompt
                with processing_lock:
                    processed_messages.append(prompt)
            
            return mock_process
        
        with patch('subprocess.Popen') as mock_popen:
            mock_popen.side_effect = mock_process_factory
            
            # Process messages concurrently
            messages = [f"Concurrent message {i}" for i in range(5)]
            with concurrent.futures.ThreadPoolExecutor(max_workers=3) as executor:
                futures = []
                for msg in messages:
                    future = executor.submit(adapter.execute, msg, verbose=False, timeout=5)
                    futures.append(future)
                
                # Wait for all to complete
                results = [f.result() for f in futures]
            
            # Verify all messages were processed
            assert len(processed_messages) == len(messages)
            # All results should be successful
            assert all(r.success for r in results)
    
    @pytest.mark.asyncio
    async def test_async_message_queue_processing(self):
        """Test async message processing and delivery."""
        adapter = QChatAdapter()
        
        # Track processed messages
        processed_messages = []
        
        async def mock_subprocess(*args, **kwargs):
            """Mock async subprocess that records messages."""
            mock_process = Mock()
            mock_process.returncode = 0
            
            # Extract message from command
            if len(args) > 0:
                prompt = args[-1]  # Last argument is the prompt
                processed_messages.append(prompt)
            
            # Mock communicate method
            async def mock_communicate():
                await asyncio.sleep(0.01)  # Simulate some processing time
                return b"Processed", b""
            
            mock_process.communicate = mock_communicate
            return mock_process
        
        with patch('asyncio.create_subprocess_exec', side_effect=mock_subprocess):
            # Process multiple messages asynchronously
            messages = [f"Async message {i}" for i in range(5)]
            tasks = []
            for msg in messages:
                task = adapter.aexecute(msg, verbose=False, timeout=5)
                tasks.append(task)
            
            # Wait for all tasks to complete
            results = await asyncio.gather(*tasks)
            
            # Verify all messages were processed
            assert len(processed_messages) == len(messages)
            assert all(r.success for r in results)
    
    def test_message_delivery_on_process_failure(self):
        """Test that messages are properly handled when process fails."""
        adapter = QChatAdapter()
        
        with patch('subprocess.Popen') as mock_popen:
            # Simulate process failure
            mock_process = Mock()
            mock_process.poll.side_effect = [None] * 10 + [1] * 5  # Running, running, failed
            mock_process.stdout = Mock()
            mock_process.stderr = Mock()
            mock_process.stdout.read.return_value = "Partial output"
            mock_process.stderr.read.return_value = "Error occurred"
            mock_process.stdout.fileno.return_value = 3
            mock_process.stderr.fileno.return_value = 4
            mock_popen.return_value = mock_process
            
            response = adapter.execute("Test message", verbose=False, timeout=5)
            
            # Should return failure but preserve output
            assert not response.success
            assert "Partial output" in response.output
            assert "Error occurred" in response.error
    
    def test_message_delivery_on_timeout(self):
        """Test that messages are handled properly on timeout."""
        adapter = QChatAdapter()
        
        with patch('subprocess.Popen') as mock_popen:
            # Simulate timeout scenario
            mock_process = Mock()
            mock_process.poll.return_value = None  # Always running
            mock_process.stdout = Mock()
            mock_process.stderr = Mock()
            mock_process.stdout.read.return_value = "Partial output before timeout"
            mock_process.stderr.read.return_value = ""
            mock_process.stdout.fileno.return_value = 3
            mock_process.stderr.fileno.return_value = 4
            mock_process.terminate = Mock()
            mock_process.kill = Mock()
            mock_process.wait = Mock(side_effect=subprocess.TimeoutExpired('cmd', 1))
            mock_popen.return_value = mock_process
            
            # Use very short timeout to trigger timeout condition
            response = adapter.execute("Test message", verbose=False, timeout=0.1)
            
            # Should return failure with timeout error
            assert not response.success
            assert "timed out" in response.error.lower()
            # Should attempt to terminate the process
            mock_process.terminate.assert_called()
    
    def test_message_buffering_and_streaming(self):
        """Test that message output is properly buffered and streamed."""
        adapter = QChatAdapter()
        
        output_chunks = ["Chunk 1\n", "Chunk 2\n", "Chunk 3\n"]
        chunk_index = [0]
        
        def mock_read(size=None):
            """Simulate reading chunks of output."""
            if chunk_index[0] < len(output_chunks):
                chunk = output_chunks[chunk_index[0]]
                chunk_index[0] += 1
                return chunk
            return ""
        
        with patch('subprocess.Popen') as mock_popen:
            mock_process = Mock()
            # Simulate process that outputs data over time
            mock_process.poll.side_effect = [None] * (len(output_chunks) + 10) + [0] * 5
            mock_process.stdout = Mock()
            mock_process.stderr = Mock()
            mock_process.stdout.read = mock_read
            mock_process.stderr.read.return_value = ""
            mock_process.stdout.fileno.return_value = 3
            mock_process.stderr.fileno.return_value = 4
            mock_popen.return_value = mock_process
            
            response = adapter.execute("Test message", verbose=False, timeout=5)
            
            # Should capture all output chunks
            assert response.success
            for chunk in output_chunks:
                assert chunk in response.output
    
    def test_signal_handler_message_preservation(self):
        """Test that messages are preserved when signal handlers are triggered."""
        adapter = QChatAdapter()
        
        with patch('subprocess.Popen') as mock_popen:
            mock_process = Mock()
            # Define side effect to trigger shutdown during execution
            def mock_poll_side_effect():
                adapter.shutdown_requested = True
                return None
            
            mock_process.poll.side_effect = mock_poll_side_effect
            # mock_process.poll.return_value = None  # Replaced by side_effect
            mock_process.stdout = Mock()
            mock_process.stderr = Mock()
            mock_process.stdout.read.return_value = "Output before signal"
            mock_process.stderr.read.return_value = ""
            mock_process.stdout.fileno.return_value = 3
            mock_process.stderr.fileno.return_value = 4
            mock_process.terminate = Mock()
            mock_process.wait = Mock()
            mock_popen.return_value = mock_process
            
            # Set process as current
            adapter.current_process = mock_process
            
            # Trigger shutdown is now handled by side_effect
            # adapter.shutdown_requested = True
            
            # Execute should handle shutdown gracefully
            response = adapter.execute("Test message", verbose=False, timeout=5)
            
            # Should return with shutdown error but preserve output
            assert not response.success
            assert "shutdown signal" in response.error.lower()
            assert "Output before signal" in response.output
    
    def test_message_queue_stress_test(self):
        """Stress test message queue with rapid message submission."""
        adapter = QChatAdapter()
        
        success_count = 0
        failure_count = 0
        
        def mock_process_factory(*args, **kwargs):
            """Create mock processes with random success/failure."""
            mock_process = Mock()
            success = random.random() > 0.2  # 80% success rate
            
            if success:
                mock_process.poll.side_effect = [None] * 5 + [0] * 5
                mock_process.stdout = Mock()
                mock_process.stderr = Mock()
                mock_process.stdout.read.return_value = "Success"
                mock_process.stderr.read.return_value = ""
            else:
                mock_process.poll.side_effect = [None] * 5 + [1] * 5
                mock_process.stdout = Mock()
                mock_process.stderr = Mock()
                mock_process.stdout.read.return_value = ""
                mock_process.stderr.read.return_value = "Random failure"
            
            mock_process.stdout.fileno.return_value = 3
            mock_process.stderr.fileno.return_value = 4
            return mock_process
        
        with patch('subprocess.Popen', side_effect=mock_process_factory):
            # Submit many messages rapidly
            num_messages = 20
            with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
                futures = []
                for i in range(num_messages):
                    future = executor.submit(
                        adapter.execute,
                        f"Stress test message {i}",
                        verbose=False,
                        timeout=5
                    )
                    futures.append(future)
                
                # Collect results
                for future in futures:
                    result = future.result()
                    if result.success:
                        success_count += 1
                    else:
                        failure_count += 1
            
            # Verify all messages were processed
            assert success_count + failure_count == num_messages
            # Most should succeed given 80% success rate
            assert success_count > failure_count
    
    @pytest.mark.asyncio
    async def test_async_message_ordering(self):
        """Test that async execution preserves message ordering semantics."""
        adapter = QChatAdapter()
        
        processed_order = []
        
        async def mock_subprocess(*args, **kwargs):
            """Mock async subprocess that tracks order."""
            mock_process = Mock()
            mock_process.returncode = 0
            
            # Extract message number from command
            if len(args) > 0:
                prompt = args[-1]
                # Extract number from "Message N" format
                import re
                match = re.search(r'Message (\d+)', prompt)
                if match:
                    processed_order.append(int(match.group(1)))
            
            async def mock_communicate():
                # Add variable delay to simulate real processing
                await asyncio.sleep(random.uniform(0.01, 0.05))
                return b"Processed", b""
            
            mock_process.communicate = mock_communicate
            return mock_process
        
        with patch('asyncio.create_subprocess_exec', side_effect=mock_subprocess):
            # Submit messages in order
            messages = [f"Message {i}" for i in range(10)]
            
            # Process sequentially to verify ordering
            for msg in messages:
                result = await adapter.aexecute(msg, verbose=False, timeout=5)
                assert result.success
            
            # Verify messages were processed in order
            assert processed_order == list(range(10))
    
    def test_message_integrity_under_concurrent_load(self):
        """Test that message content integrity is maintained under concurrent load."""
        adapter = QChatAdapter()
        
        # Use a dictionary to track message integrity
        message_integrity = {}
        integrity_lock = threading.Lock()
        
        def mock_process_factory(*args, **kwargs):
            """Create mock process that validates message integrity."""
            cmd = args[0] if args else kwargs.get('args', [])
            mock_process = Mock()
            mock_process.poll.side_effect = [None] * 10 + [0] * 5
            mock_process.stdout = Mock()
            mock_process.stderr = Mock()
            
            # Extract and validate message
            if len(cmd) > 0:
                prompt = cmd[-1]
                # Extract ID from message
                import re
                match = re.search(r'ID:(\d+)', prompt)
                if match:
                    msg_id = int(match.group(1))
                    with integrity_lock:
                        if msg_id in message_integrity:
                            # Duplicate!
                            message_integrity[msg_id] = "DUPLICATE"
                        else:
                            message_integrity[msg_id] = "OK"
                    mock_process.stdout.read.return_value = f"Processed {msg_id}"
                else:
                    mock_process.stdout.read.return_value = "Processed"
            else:
                mock_process.stdout.read.return_value = "No message"
            
            mock_process.stderr.read.return_value = ""
            mock_process.stdout.fileno.return_value = 3
            mock_process.stderr.fileno.return_value = 4
            return mock_process
        
        with patch('subprocess.Popen', side_effect=mock_process_factory):
            # Submit unique messages concurrently
            num_messages = 50
            messages = [f"Message ID:{i} with unique content" for i in range(num_messages)]
            
            with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
                futures = [
                    executor.submit(adapter.execute, msg, verbose=False, timeout=5)
                    for msg in messages
                ]
                results = [f.result() for f in futures]
            
            # Verify message integrity
            assert all(r.success for r in results)
            assert len(message_integrity) == num_messages
            assert all(v == "OK" for v in message_integrity.values())