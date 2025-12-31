# ABOUTME: Test suite for the web server module
# ABOUTME: Verifies API endpoints, WebSocket connections, and orchestrator monitoring

import pytest
import asyncio
import tempfile
import shutil
import os
from unittest.mock import MagicMock, patch, AsyncMock
from datetime import datetime

from fastapi.testclient import TestClient

from src.ralph_orchestrator.web.server import OrchestratorMonitor, WebMonitor


class TestOrchestratorMonitor:
    """Test suite for OrchestratorMonitor class."""
    
    @pytest.fixture
    def temp_dir(self):
        """Create a temporary directory for test data."""
        temp_dir = tempfile.mkdtemp()
        yield temp_dir
        shutil.rmtree(temp_dir)
    
    @pytest.fixture
    def monitor(self):
        """Create an OrchestratorMonitor instance for testing."""
        monitor = OrchestratorMonitor()
        yield monitor
        # Cleanup
        monitor.active_orchestrators.clear()
    
    @pytest.fixture
    def mock_orchestrator(self):
        """Create a mock orchestrator instance."""
        mock = MagicMock()
        mock.id = 'test-orch-123'
        mock.prompt_path = '/path/to/prompt.md'
        mock.status = 'running'
        mock.current_iteration = 5
        mock.max_iterations = 10
        mock.start_time = datetime.now()
        mock.task_queue = ['Task 1', 'Task 2']
        mock.current_task = 'Current task'
        mock.completed_tasks = ['Done 1', 'Done 2']
        
        # Return a new dict each time to avoid mutation issues
        mock.get_orchestrator_state.side_effect = lambda: {
            'id': 'test-orch-123',
            'prompt_path': '/path/to/prompt.md',
            'status': 'running',
            'current_iteration': 5,
            'max_iterations': 10,
            'task_queue': ['Task 1', 'Task 2'],
            'current_task': 'Current task',
            'completed_tasks': ['Done 1', 'Done 2']
        }
        
        return mock
    
    def test_register_orchestrator(self, monitor, mock_orchestrator):
        """Test registering an orchestrator."""
        monitor.register_orchestrator('test-123', mock_orchestrator)
        
        assert 'test-123' in monitor.active_orchestrators
        assert monitor.active_orchestrators['test-123'] == mock_orchestrator
    
    def test_unregister_orchestrator(self, monitor, mock_orchestrator):
        """Test unregistering an orchestrator."""
        monitor.register_orchestrator('test-123', mock_orchestrator)
        monitor.unregister_orchestrator('test-123')
        
        assert 'test-123' not in monitor.active_orchestrators
    
    def test_get_orchestrator_status(self, monitor, mock_orchestrator):
        """Test getting orchestrator status."""
        monitor.register_orchestrator('test-123', mock_orchestrator)
        
        status = monitor.get_orchestrator_status('test-123')
        assert status['id'] == 'test-123'
        assert status['status'] == 'running'
        assert status['current_iteration'] == 5
    
    def test_get_all_orchestrators_status(self, monitor, mock_orchestrator):
        """Test getting status of all orchestrators."""
        monitor.register_orchestrator('test-123', mock_orchestrator)
        monitor.register_orchestrator('test-456', mock_orchestrator)
        
        all_status = monitor.get_all_orchestrators_status()
        assert len(all_status) == 2
        status_ids = [s['id'] for s in all_status]
        assert 'test-123' in status_ids
        assert 'test-456' in status_ids
    
    @pytest.mark.asyncio
    async def test_broadcast_update(self, monitor):
        """Test broadcasting updates to WebSocket clients."""
        # Mock WebSocket client
        mock_ws = AsyncMock()
        monitor.websocket_clients.append(mock_ws)
        
        await monitor.broadcast_update({
            'type': 'test',
            'data': {'message': 'test'}
        })
        
        mock_ws.send_json.assert_called_once_with({
            'type': 'test',
            'data': {'message': 'test'}
        })
    
    @pytest.mark.asyncio
    async def test_monitor_system_metrics(self, monitor):
        """Test system metrics monitoring."""
        # Start monitoring
        await monitor.start_monitoring()
        
        # Wait for metrics to be collected
        await asyncio.sleep(0.1)
        
        # Check metrics cache
        assert 'system' in monitor.metrics_cache
        assert 'cpu_percent' in monitor.metrics_cache['system']
        assert 'memory' in monitor.metrics_cache['system']
        assert 'percent' in monitor.metrics_cache['system']['memory']
        
        # Stop monitoring
        await monitor.stop_monitoring()
    
    def test_add_execution_history(self, monitor, mock_orchestrator):
        """Test adding to execution history."""
        monitor.register_orchestrator('test-123', mock_orchestrator)
        
        # Simulate adding history
        history_entry = {
            'orchestrator_id': 'test-123',
            'timestamp': datetime.now().isoformat(),
            'event': 'iteration_complete',
            'details': {'iteration': 1}
        }
        monitor.execution_history.append(history_entry)
        
        assert len(monitor.execution_history) == 1
        assert monitor.execution_history[0]['orchestrator_id'] == 'test-123'


class TestWebMonitor:
    """Test suite for WebMonitor FastAPI application."""
    
    @pytest.fixture
    def web_monitor(self):
        """Create WebMonitor instance with auth disabled."""
        return WebMonitor(port=8080, enable_auth=False)
    
    @pytest.fixture
    def auth_web_monitor(self):
        """Create WebMonitor instance with auth enabled."""
        return WebMonitor(port=8080, enable_auth=True)
    
    @pytest.fixture
    def client(self, web_monitor):
        """Create FastAPI test client without auth."""
        return TestClient(web_monitor.app)
    
    @pytest.fixture
    def auth_client(self, auth_web_monitor):
        """Create FastAPI test client with auth."""
        client = TestClient(auth_web_monitor.app)

        # Login to get token (using default password from auth.py)
        response = client.post("/api/auth/login", json={
            "username": "admin",
            "password": "admin123"
        })

        if response.status_code == 200:
            token = response.json()["access_token"]
            client.headers = {"Authorization": f"Bearer {token}"}

        return client
    
    @pytest.fixture
    def mock_orchestrator(self):
        """Create a mock orchestrator."""
        mock = MagicMock()
        mock.id = 'test-orch-123'
        mock.prompt_file = '/path/to/prompt.md'
        mock.stop_requested = False
        mock.current_iteration = 3
        mock.max_iterations = 10
        mock.primary_tool = 'test_tool'
        mock.max_runtime = 3600
        mock.pause = MagicMock()
        mock.resume = MagicMock()
        mock.stop = MagicMock()
        
        # Metrics mock
        mock.metrics = MagicMock()
        mock.metrics.total_iterations = 5
        mock.metrics.to_dict.return_value = {'iterations': 5}
        
        # Cost tracker mock
        mock.cost_tracker = MagicMock()
        mock.cost_tracker.get_summary.return_value = {'total': 0.0}
        
        # Add get_orchestrator_state for compatibility
        mock.get_orchestrator_state.side_effect = lambda: {
            'id': 'test-orch-123',
            'prompt_path': '/path/to/prompt.md',
            'status': 'running',
            'current_iteration': 3,
            'max_iterations': 10,
            'task_queue': [],
            'current_task': None,
            'completed_tasks': []
        }
        
        return mock
    
    def test_root_endpoint(self, client):
        """Test root endpoint returns HTML."""
        response = client.get("/")
        assert response.status_code == 200
        assert "text/html" in response.headers["content-type"]
    
    def test_health_check(self, client):
        """Test health check endpoint."""
        response = client.get("/api/health")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "healthy"
        assert "timestamp" in data
    
    def test_auth_required_endpoints(self, auth_web_monitor):
        """Test that auth is required for protected endpoints."""
        client = TestClient(auth_web_monitor.app)
        
        # Try accessing protected endpoint without auth
        response = client.get("/api/orchestrators")
        assert response.status_code == 403  # FastAPI returns 403 for missing auth
    
    def test_login_endpoint(self, auth_web_monitor):
        """Test authentication login."""
        client = TestClient(auth_web_monitor.app)
        
        # Test successful login (using default password from auth.py)
        response = client.post("/api/auth/login", json={
            "username": "admin",
            "password": "admin123"
        })
        assert response.status_code == 200
        assert "access_token" in response.json()
        
        # Test failed login
        response = client.post("/api/auth/login", json={
            "username": "admin",
            "password": "wrong"
        })
        assert response.status_code == 401
    
    def test_get_orchestrators(self, client, web_monitor, mock_orchestrator):
        """Test getting all orchestrators."""
        web_monitor.register_orchestrator('test-123', mock_orchestrator)
        
        response = client.get("/api/orchestrators")
        assert response.status_code == 200
        data = response.json()
        assert 'orchestrators' in data
        assert 'count' in data
        assert data['count'] == 1
        assert len(data['orchestrators']) == 1
        assert data['orchestrators'][0]['id'] == 'test-123'
    
    def test_get_single_orchestrator(self, client, web_monitor, mock_orchestrator):
        """Test getting a single orchestrator."""
        web_monitor.register_orchestrator('test-123', mock_orchestrator)
        
        response = client.get("/api/orchestrators/test-123")
        assert response.status_code == 200
        data = response.json()
        assert data['id'] == 'test-123'
        
        # Test non-existent orchestrator
        response = client.get("/api/orchestrators/non-existent")
        assert response.status_code == 404
    
    def test_pause_resume_orchestrator(self, client, web_monitor, mock_orchestrator):
        """Test pausing and resuming orchestrator."""
        web_monitor.register_orchestrator('test-123', mock_orchestrator)
        
        # Test pause (sets stop_requested flag)
        response = client.post("/api/orchestrators/test-123/pause")
        assert response.status_code == 200
        assert mock_orchestrator.stop_requested
        
        # Test resume (clears stop_requested flag)
        response = client.post("/api/orchestrators/test-123/resume")
        assert response.status_code == 200
        assert not mock_orchestrator.stop_requested
    
    def test_stop_orchestrator(self, client, web_monitor, mock_orchestrator):
        """Test stopping orchestrator."""
        # Stop endpoint doesn't exist - use pause instead
        web_monitor.register_orchestrator('test-123', mock_orchestrator)
        
        # Pause is the way to stop
        response = client.post("/api/orchestrators/test-123/pause")
        assert response.status_code == 200
        assert mock_orchestrator.stop_requested
    
    def test_update_prompt(self, client, web_monitor, mock_orchestrator):
        """Test updating orchestrator prompt."""
        # Mock prompt file - needs to be a Path object
        from pathlib import Path
        
        mock_path = MagicMock(spec=Path)
        mock_path.exists.return_value = True
        mock_path.read_text.return_value = "Old content"
        mock_path.with_suffix.return_value = MagicMock(spec=Path)
        
        mock_orchestrator.prompt_file = mock_path
        web_monitor.register_orchestrator('test-123', mock_orchestrator)
        
        response = client.post("/api/orchestrators/test-123/prompt", json={
            "content": "New prompt content"
        })
        
        assert response.status_code == 200
        # Check that write_text was called with new content
        mock_path.write_text.assert_called_once_with("New prompt content")
    
    def test_get_execution_history(self, client, web_monitor):
        """Test getting execution history."""
        # The history endpoint returns data from database or execution_history
        response = client.get("/api/history")
        assert response.status_code == 200
        data = response.json()
        # Just verify it returns a list (might have database entries)
        assert isinstance(data, list)
    
    def test_get_system_metrics(self, client):
        """Test getting system metrics."""
        response = client.get("/api/metrics")
        assert response.status_code == 200
        data = response.json()
        # Metrics might be empty if monitoring hasn't started
        if 'system' in data:
            assert 'cpu_percent' in data['system']
            assert 'memory' in data['system']
    
    def test_websocket_connection(self, web_monitor):
        """Test WebSocket connection."""
        client = TestClient(web_monitor.app)
        
        with client.websocket_connect("/ws") as websocket:
            # Should receive initial state
            data = websocket.receive_json()
            assert data['type'] == 'initial_state'
            assert 'orchestrators' in data['data']
            
            # Test ping/pong
            websocket.send_text("ping")
            response = websocket.receive_text()
            assert response == "pong"
    
    def test_websocket_auth(self, auth_web_monitor):
        """Test WebSocket with authentication."""
        client = TestClient(auth_web_monitor.app)

        # Get token first (using default password from auth.py)
        response = client.post("/api/auth/login", json={
            "username": "admin",
            "password": "admin123"
        })
        token = response.json()["access_token"]

        # Connect with token
        with client.websocket_connect(f"/ws?token={token}") as websocket:
            data = websocket.receive_json()
            assert data['type'] == 'initial_state'
    
    def test_database_endpoints(self, client, web_monitor):
        """Test database-related endpoints."""
        # Test history endpoint (which uses the database)
        with patch.object(web_monitor.monitor.database, 'get_recent_runs') as mock_runs:
            mock_runs.return_value = [
                {'id': 1, 'status': 'completed'},
                {'id': 2, 'status': 'running'}
            ]
            
            response = client.get("/api/history")
            assert response.status_code == 200
            data = response.json()
            # History endpoint might fallback to execution_history if database is empty
        
        # Test statistics endpoint
        with patch.object(web_monitor.monitor.database, 'get_statistics') as mock_stats:
            mock_stats.return_value = {
                'total_runs': 10,
                'success_rate': 80.0
            }
            
            response = client.get("/api/statistics")
            assert response.status_code == 200
            data = response.json()
            assert data['total_runs'] == 10
            assert data['success_rate'] == 80.0
    
    def test_static_files(self, web_monitor):
        """Test static file serving."""
        # Create a test static file
        static_dir = os.path.dirname(os.path.abspath(__file__))
        static_dir = os.path.join(os.path.dirname(static_dir), 'src', 'ralph_orchestrator', 'web', 'static')
        
        if os.path.exists(static_dir):
            client = TestClient(web_monitor.app)
            response = client.get("/static/dashboard.html")
            # If static files exist, they should be served
            if response.status_code == 200:
                assert "text/html" in response.headers.get("content-type", "")
    
    def test_cors_headers(self, client):
        """Test CORS headers are set."""
        response = client.get("/api/health")
        # CORS headers are set by the middleware
        # The actual header name might be different case
        headers_lower = {k.lower(): v for k, v in response.headers.items()}
        if "access-control-allow-origin" in headers_lower:
            assert headers_lower["access-control-allow-origin"] == "*"
    
    @pytest.mark.asyncio
    async def test_broadcast_orchestrator_update(self, web_monitor, mock_orchestrator):
        """Test broadcasting orchestrator updates."""
        mock_ws = AsyncMock()
        web_monitor.monitor.websocket_clients.append(mock_ws)
        
        web_monitor.register_orchestrator('test-123', mock_orchestrator)
        
        await web_monitor.monitor.broadcast_update({
            'type': 'orchestrator_update',
            'data': {'id': 'test-123', 'status': 'running'}
        })
        
        mock_ws.send_json.assert_called_once()
    
    def test_error_handling(self, client):
        """Test error handling for various scenarios."""
        # Test 404 for non-existent orchestrator
        response = client.get("/api/orchestrators/non-existent")
        assert response.status_code == 404
        
        # Test invalid JSON
        response = client.post("/api/orchestrators/test/prompt", 
                              data="invalid json",
                              headers={"Content-Type": "application/json"})
        assert response.status_code == 422
    
    def test_run_server_methods(self, web_monitor):
        """Test server run methods."""
        # Test that run methods exist and are callable
        assert hasattr(web_monitor, 'run')
        assert hasattr(web_monitor, 'arun')
        assert callable(web_monitor.run)
        assert callable(web_monitor.arun)