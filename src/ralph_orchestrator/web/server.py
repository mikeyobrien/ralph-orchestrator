# ABOUTME: FastAPI web server for Ralph Orchestrator monitoring dashboard
# ABOUTME: Provides REST API endpoints and WebSocket connections for real-time updates

"""FastAPI web server for Ralph Orchestrator monitoring."""

import json
import time
import asyncio
import logging
from pathlib import Path
from datetime import datetime
from typing import Optional, Dict, Any, List
from contextlib import asynccontextmanager

from fastapi import FastAPI, WebSocket, WebSocketDisconnect, HTTPException, Depends, status
from fastapi.responses import HTMLResponse, FileResponse
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import uvicorn
import psutil

from ..orchestrator import RalphOrchestrator
from .auth import (
    auth_manager, LoginRequest, TokenResponse,
    get_current_user, require_admin
)
from .database import DatabaseManager
from .rate_limit import rate_limit_middleware, setup_rate_limit_cleanup

logger = logging.getLogger(__name__)


class PromptUpdateRequest(BaseModel):
    """Request model for updating orchestrator prompt."""
    content: str


class OrchestratorMonitor:
    """Monitors and manages orchestrator instances."""
    
    def __init__(self):
        self.active_orchestrators: Dict[str, RalphOrchestrator] = {}
        self.execution_history: List[Dict[str, Any]] = []
        self.websocket_clients: List[WebSocket] = []
        self.metrics_cache: Dict[str, Any] = {}
        self.system_metrics_task: Optional[asyncio.Task] = None
        self.database = DatabaseManager()
        self.active_runs: Dict[str, int] = {}  # Maps orchestrator_id to run_id
        self.active_iterations: Dict[str, int] = {}  # Maps orchestrator_id to iteration_id
        
    async def start_monitoring(self):
        """Start background monitoring tasks."""
        if not self.system_metrics_task:
            self.system_metrics_task = asyncio.create_task(self._monitor_system_metrics())
    
    async def stop_monitoring(self):
        """Stop background monitoring tasks."""
        if self.system_metrics_task:
            self.system_metrics_task.cancel()
            try:
                await self.system_metrics_task
            except asyncio.CancelledError:
                pass
    
    async def _monitor_system_metrics(self):
        """Monitor system metrics continuously."""
        while True:
            try:
                # Collect system metrics
                metrics = {
                    "timestamp": datetime.now().isoformat(),
                    "cpu_percent": psutil.cpu_percent(interval=1),
                    "memory": {
                        "total": psutil.virtual_memory().total,
                        "available": psutil.virtual_memory().available,
                        "percent": psutil.virtual_memory().percent
                    },
                    "active_processes": len(psutil.pids()),
                    "orchestrators": len(self.active_orchestrators)
                }
                
                self.metrics_cache["system"] = metrics
                
                # Broadcast to WebSocket clients
                await self._broadcast_to_clients({
                    "type": "system_metrics",
                    "data": metrics
                })
                
                await asyncio.sleep(5)  # Update every 5 seconds
                
            except Exception as e:
                logger.error(f"Error monitoring system metrics: {e}")
                await asyncio.sleep(5)
    
    async def _broadcast_to_clients(self, message: Dict[str, Any]):
        """Broadcast message to all connected WebSocket clients."""
        disconnected_clients = []
        for client in self.websocket_clients:
            try:
                await client.send_json(message)
            except Exception:
                disconnected_clients.append(client)
        
        # Remove disconnected clients
        for client in disconnected_clients:
            if client in self.websocket_clients:
                self.websocket_clients.remove(client)
    
    def _schedule_broadcast(self, message: Dict[str, Any]):
        """Schedule a broadcast to clients, handling both sync and async contexts."""
        try:
            # Check if there's a running event loop (raises RuntimeError if not)
            asyncio.get_running_loop()
            # If we're in an async context, schedule the broadcast
            asyncio.create_task(self._broadcast_to_clients(message))
        except RuntimeError:
            # No event loop running - we're in a sync context (e.g., during testing)
            # The broadcast will be skipped in this case
            pass
    
    async def broadcast_update(self, message: Dict[str, Any]):
        """Public method to broadcast updates to WebSocket clients."""
        await self._broadcast_to_clients(message)
    
    def register_orchestrator(self, orchestrator_id: str, orchestrator: RalphOrchestrator):
        """Register an orchestrator instance."""
        self.active_orchestrators[orchestrator_id] = orchestrator
        
        # Create a new run in the database
        try:
            run_id = self.database.create_run(
                orchestrator_id=orchestrator_id,
                prompt_path=str(orchestrator.prompt_file),
                max_iterations=orchestrator.max_iterations,
                metadata={
                    "primary_tool": orchestrator.primary_tool,
                    "max_runtime": orchestrator.max_runtime
                }
            )
            self.active_runs[orchestrator_id] = run_id
            
            # Extract and store tasks if available
            if hasattr(orchestrator, 'task_queue'):
                for task in orchestrator.task_queue:
                    self.database.add_task(run_id, task['description'])
        except Exception as e:
            logger.error(f"Error creating database run for orchestrator {orchestrator_id}: {e}")
        
        self._schedule_broadcast({
            "type": "orchestrator_registered",
            "data": {"id": orchestrator_id, "timestamp": datetime.now().isoformat()}
        })
    
    def unregister_orchestrator(self, orchestrator_id: str):
        """Unregister an orchestrator instance."""
        if orchestrator_id in self.active_orchestrators:
            # Update database run status
            if orchestrator_id in self.active_runs:
                try:
                    orchestrator = self.active_orchestrators[orchestrator_id]
                    status = "completed" if not orchestrator.stop_requested else "stopped"
                    total_iterations = orchestrator.metrics.total_iterations if hasattr(orchestrator, 'metrics') else 0
                    self.database.update_run_status(
                        self.active_runs[orchestrator_id],
                        status=status,
                        total_iterations=total_iterations
                    )
                    del self.active_runs[orchestrator_id]
                except Exception as e:
                    logger.error(f"Error updating database run for orchestrator {orchestrator_id}: {e}")
            
            # Remove from active orchestrators
            del self.active_orchestrators[orchestrator_id]
            
            # Remove active iteration tracking if exists
            if orchestrator_id in self.active_iterations:
                del self.active_iterations[orchestrator_id]
            
            self._schedule_broadcast({
                "type": "orchestrator_unregistered",
                "data": {"id": orchestrator_id, "timestamp": datetime.now().isoformat()}
            })
    
    def get_orchestrator_status(self, orchestrator_id: str) -> Dict[str, Any]:
        """Get status of a specific orchestrator."""
        if orchestrator_id not in self.active_orchestrators:
            return None
        
        orchestrator = self.active_orchestrators[orchestrator_id]
        
        # Try to use the new get_orchestrator_state method if it exists
        if hasattr(orchestrator, 'get_orchestrator_state'):
            state = orchestrator.get_orchestrator_state()
            state['id'] = orchestrator_id  # Override with our ID
            return state
        else:
            # Fallback to old method for compatibility
            return {
                "id": orchestrator_id,
                "status": "running" if not orchestrator.stop_requested else "stopping",
                "metrics": orchestrator.metrics.to_dict(),
                "cost": orchestrator.cost_tracker.get_summary() if orchestrator.cost_tracker else None,
                "config": {
                    "primary_tool": orchestrator.primary_tool,
                    "max_iterations": orchestrator.max_iterations,
                    "max_runtime": orchestrator.max_runtime,
                    "prompt_file": str(orchestrator.prompt_file)
                }
            }
    
    def get_all_orchestrators_status(self) -> List[Dict[str, Any]]:
        """Get status of all orchestrators."""
        return [
            self.get_orchestrator_status(orch_id)
            for orch_id in self.active_orchestrators
        ]
    
    def start_iteration(self, orchestrator_id: str, iteration_number: int, 
                       current_task: Optional[str] = None) -> Optional[int]:
        """Start tracking a new iteration.
        
        Args:
            orchestrator_id: ID of the orchestrator
            iteration_number: Iteration number
            current_task: Current task being executed
            
        Returns:
            Iteration ID if successful, None otherwise
        """
        if orchestrator_id not in self.active_runs:
            return None
        
        try:
            iteration_id = self.database.add_iteration(
                run_id=self.active_runs[orchestrator_id],
                iteration_number=iteration_number,
                current_task=current_task,
                metrics=None  # Can be enhanced to include metrics
            )
            self.active_iterations[orchestrator_id] = iteration_id
            return iteration_id
        except Exception as e:
            logger.error(f"Error starting iteration for orchestrator {orchestrator_id}: {e}")
            return None
    
    def end_iteration(self, orchestrator_id: str, status: str = "completed",
                     agent_output: Optional[str] = None, error_message: Optional[str] = None):
        """End tracking for the current iteration.
        
        Args:
            orchestrator_id: ID of the orchestrator
            status: Status of the iteration (completed, failed)
            agent_output: Output from the agent
            error_message: Error message if failed
        """
        if orchestrator_id not in self.active_iterations:
            return
        
        try:
            self.database.update_iteration(
                iteration_id=self.active_iterations[orchestrator_id],
                status=status,
                agent_output=agent_output,
                error_message=error_message
            )
            del self.active_iterations[orchestrator_id]
        except Exception as e:
            logger.error(f"Error ending iteration for orchestrator {orchestrator_id}: {e}")


class WebMonitor:
    """Web monitoring server for Ralph Orchestrator."""
    
    def __init__(self, host: str = "0.0.0.0", port: int = 8080, enable_auth: bool = True):
        self.host = host
        self.port = port
        self.enable_auth = enable_auth
        self.monitor = OrchestratorMonitor()
        self.app = None
        self._setup_app()
    
    def _setup_app(self):
        """Setup FastAPI application."""
        
        @asynccontextmanager
        async def lifespan(app: FastAPI):
            # Startup
            await self.monitor.start_monitoring()
            # Start rate limit cleanup task
            cleanup_task = await setup_rate_limit_cleanup()
            yield
            # Shutdown
            cleanup_task.cancel()
            try:
                await cleanup_task
            except asyncio.CancelledError:
                pass
            await self.monitor.stop_monitoring()
        
        self.app = FastAPI(
            title="Ralph Orchestrator Monitor",
            description="Real-time monitoring for Ralph AI Orchestrator",
            version="1.0.0",
            lifespan=lifespan
        )
        
        # Add CORS middleware
        self.app.add_middleware(
            CORSMiddleware,
            allow_origins=["*"],
            allow_credentials=True,
            allow_methods=["*"],
            allow_headers=["*"],
        )
        
        # Add rate limiting middleware
        self.app.middleware("http")(rate_limit_middleware)
        
        # Mount static files directory if it exists
        static_dir = Path(__file__).parent / "static"
        if static_dir.exists():
            self.app.mount("/static", StaticFiles(directory=str(static_dir)), name="static")
        
        # Setup routes
        self._setup_routes()
    
    def _setup_routes(self):
        """Setup API routes."""
        
        # Authentication endpoints (public)
        @self.app.post("/api/auth/login", response_model=TokenResponse)
        async def login(request: LoginRequest):
            """Login and receive an access token."""
            user = auth_manager.authenticate_user(request.username, request.password)
            if not user:
                raise HTTPException(
                    status_code=status.HTTP_401_UNAUTHORIZED,
                    detail="Incorrect username or password",
                    headers={"WWW-Authenticate": "Bearer"},
                )
            
            access_token = auth_manager.create_access_token(
                data={"sub": user["username"]}
            )
            
            return TokenResponse(
                access_token=access_token,
                expires_in=auth_manager.access_token_expire_minutes * 60
            )
        
        @self.app.get("/api/auth/verify")
        async def verify_token(current_user: Dict[str, Any] = Depends(get_current_user)):
            """Verify the current token is valid."""
            return {
                "valid": True,
                "username": current_user["username"],
                "is_admin": current_user["user"].get("is_admin", False)
            }
        
        # Public endpoints - HTML pages
        @self.app.get("/login.html")
        async def login_page():
            """Serve the login page."""
            html_file = Path(__file__).parent / "static" / "login.html"
            if html_file.exists():
                return FileResponse(html_file, media_type="text/html")
            else:
                return HTMLResponse(content="<h1>Login page not found</h1>", status_code=404)
        
        @self.app.get("/")
        async def index():
            """Serve the main dashboard."""
            html_file = Path(__file__).parent / "static" / "index.html"
            if html_file.exists():
                return FileResponse(html_file, media_type="text/html")
            else:
                # Return a basic HTML page if static file doesn't exist yet
                return HTMLResponse(content="""
                <!DOCTYPE html>
                <html>
                <head>
                    <title>Ralph Orchestrator Monitor</title>
                    <style>
                        body { font-family: Arial, sans-serif; margin: 20px; }
                        h1 { color: #333; }
                        .status { padding: 10px; margin: 10px 0; background: #f0f0f0; border-radius: 5px; }
                    </style>
                </head>
                <body>
                    <h1>Ralph Orchestrator Monitor</h1>
                    <div id="status" class="status">
                        <p>Web monitor is running. Dashboard file not found.</p>
                        <p>API Endpoints:</p>
                        <ul>
                            <li><a href="/api/status">/api/status</a> - System status</li>
                            <li><a href="/api/orchestrators">/api/orchestrators</a> - Active orchestrators</li>
                            <li><a href="/api/metrics">/api/metrics</a> - System metrics</li>
                            <li><a href="/docs">/docs</a> - API documentation</li>
                        </ul>
                    </div>
                </body>
                </html>
                """)
        
        # Create dependency for auth if enabled
        auth_dependency = Depends(get_current_user) if self.enable_auth else None
        
        @self.app.get("/api/health")
        async def health_check():
            """Health check endpoint."""
            return {
                "status": "healthy",
                "timestamp": datetime.now().isoformat()
            }
        
        @self.app.get("/api/status", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_status():
            """Get overall system status."""
            return {
                "status": "online",
                "timestamp": datetime.now().isoformat(),
                "active_orchestrators": len(self.monitor.active_orchestrators),
                "connected_clients": len(self.monitor.websocket_clients),
                "system_metrics": self.monitor.metrics_cache.get("system", {})
            }
        
        @self.app.get("/api/orchestrators", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_orchestrators():
            """Get all active orchestrators."""
            return {
                "orchestrators": self.monitor.get_all_orchestrators_status(),
                "count": len(self.monitor.active_orchestrators)
            }
        
        @self.app.get("/api/orchestrators/{orchestrator_id}", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_orchestrator(orchestrator_id: str):
            """Get specific orchestrator status."""
            status = self.monitor.get_orchestrator_status(orchestrator_id)
            if not status:
                raise HTTPException(status_code=404, detail="Orchestrator not found")
            return status
        
        @self.app.get("/api/orchestrators/{orchestrator_id}/tasks", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_orchestrator_tasks(orchestrator_id: str):
            """Get task queue status for an orchestrator."""
            if orchestrator_id not in self.monitor.active_orchestrators:
                raise HTTPException(status_code=404, detail="Orchestrator not found")
            
            orchestrator = self.monitor.active_orchestrators[orchestrator_id]
            task_status = orchestrator.get_task_status()
            
            return {
                "orchestrator_id": orchestrator_id,
                "tasks": task_status
            }
        
        @self.app.post("/api/orchestrators/{orchestrator_id}/pause", dependencies=[auth_dependency] if self.enable_auth else [])
        async def pause_orchestrator(orchestrator_id: str):
            """Pause an orchestrator."""
            if orchestrator_id not in self.monitor.active_orchestrators:
                raise HTTPException(status_code=404, detail="Orchestrator not found")
            
            orchestrator = self.monitor.active_orchestrators[orchestrator_id]
            orchestrator.stop_requested = True
            
            return {"status": "paused", "orchestrator_id": orchestrator_id}
        
        @self.app.post("/api/orchestrators/{orchestrator_id}/resume", dependencies=[auth_dependency] if self.enable_auth else [])
        async def resume_orchestrator(orchestrator_id: str):
            """Resume an orchestrator."""
            if orchestrator_id not in self.monitor.active_orchestrators:
                raise HTTPException(status_code=404, detail="Orchestrator not found")
            
            orchestrator = self.monitor.active_orchestrators[orchestrator_id]
            orchestrator.stop_requested = False
            
            return {"status": "resumed", "orchestrator_id": orchestrator_id}
        
        @self.app.get("/api/orchestrators/{orchestrator_id}/prompt", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_orchestrator_prompt(orchestrator_id: str):
            """Get the current prompt for an orchestrator."""
            if orchestrator_id not in self.monitor.active_orchestrators:
                raise HTTPException(status_code=404, detail="Orchestrator not found")
            
            orchestrator = self.monitor.active_orchestrators[orchestrator_id]
            prompt_file = orchestrator.prompt_file
            
            if not prompt_file.exists():
                raise HTTPException(status_code=404, detail="Prompt file not found")
            
            content = prompt_file.read_text()
            return {
                "orchestrator_id": orchestrator_id,
                "prompt_file": str(prompt_file),
                "content": content,
                "last_modified": prompt_file.stat().st_mtime
            }
        
        @self.app.post("/api/orchestrators/{orchestrator_id}/prompt", dependencies=[auth_dependency] if self.enable_auth else [])
        async def update_orchestrator_prompt(orchestrator_id: str, request: PromptUpdateRequest):
            """Update the prompt for an orchestrator."""
            if orchestrator_id not in self.monitor.active_orchestrators:
                raise HTTPException(status_code=404, detail="Orchestrator not found")
            
            orchestrator = self.monitor.active_orchestrators[orchestrator_id]
            prompt_file = orchestrator.prompt_file
            
            try:
                # Create backup before updating
                backup_file = prompt_file.with_suffix(f".{int(time.time())}.backup")
                if prompt_file.exists():
                    backup_file.write_text(prompt_file.read_text())
                
                # Write the new content
                prompt_file.write_text(request.content)
                
                # Notify the orchestrator of the update
                if hasattr(orchestrator, '_reload_prompt'):
                    orchestrator._reload_prompt()
                
                # Broadcast update to WebSocket clients
                await self.monitor._broadcast_to_clients({
                    "type": "prompt_updated",
                    "data": {
                        "orchestrator_id": orchestrator_id,
                        "timestamp": datetime.now().isoformat()
                    }
                })
                
                return {
                    "status": "success",
                    "orchestrator_id": orchestrator_id,
                    "backup_file": str(backup_file),
                    "timestamp": datetime.now().isoformat()
                }
            except Exception as e:
                logger.error(f"Error updating prompt: {e}")
                raise HTTPException(status_code=500, detail=f"Failed to update prompt: {str(e)}")
        
        @self.app.get("/api/metrics", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_metrics():
            """Get system metrics."""
            return self.monitor.metrics_cache
        
        @self.app.get("/api/history", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_history(limit: int = 50):
            """Get execution history from database.
            
            Args:
                limit: Maximum number of runs to return (default 50)
            """
            try:
                # Get recent runs from database
                history = self.monitor.database.get_recent_runs(limit=limit)
                return history
            except Exception as e:
                logger.error(f"Error fetching history from database: {e}")
                # Fallback to file-based history if database fails
                metrics_dir = Path(".agent") / "metrics"
                history = []
                
                if metrics_dir.exists():
                    for metrics_file in sorted(metrics_dir.glob("metrics_*.json")):
                        try:
                            data = json.loads(metrics_file.read_text())
                            data["filename"] = metrics_file.name
                            history.append(data)
                        except Exception as e:
                            logger.error(f"Error reading metrics file {metrics_file}: {e}")
            
            return {"history": history[-50:]}  # Return last 50 entries
        
        @self.app.get("/api/history/{run_id}", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_run_details(run_id: int):
            """Get detailed information about a specific run.
            
            Args:
                run_id: ID of the run to retrieve
            """
            run_details = self.monitor.database.get_run_details(run_id)
            if not run_details:
                raise HTTPException(status_code=404, detail="Run not found")
            return run_details
        
        @self.app.get("/api/statistics", dependencies=[auth_dependency] if self.enable_auth else [])
        async def get_statistics():
            """Get database statistics."""
            return self.monitor.database.get_statistics()
        
        @self.app.post("/api/database/cleanup", dependencies=[auth_dependency] if self.enable_auth else [])
        async def cleanup_database(days: int = 30):
            """Clean up old records from the database.
            
            Args:
                days: Number of days of history to keep (default 30)
            """
            try:
                self.monitor.database.cleanup_old_records(days=days)
                return {"status": "success", "message": f"Cleaned up records older than {days} days"}
            except Exception as e:
                logger.error(f"Error cleaning up database: {e}")
                raise HTTPException(status_code=500, detail=str(e))
        
        # Admin endpoints for user management
        @self.app.post("/api/admin/users", dependencies=[Depends(require_admin)] if self.enable_auth else [])
        async def add_user(username: str, password: str, is_admin: bool = False):
            """Add a new user (admin only)."""
            if auth_manager.add_user(username, password, is_admin):
                return {"status": "success", "message": f"User {username} created"}
            else:
                raise HTTPException(
                    status_code=status.HTTP_400_BAD_REQUEST,
                    detail="User already exists"
                )
        
        @self.app.delete("/api/admin/users/{username}", dependencies=[Depends(require_admin)] if self.enable_auth else [])
        async def remove_user(username: str):
            """Remove a user (admin only)."""
            if auth_manager.remove_user(username):
                return {"status": "success", "message": f"User {username} removed"}
            else:
                raise HTTPException(
                    status_code=status.HTTP_400_BAD_REQUEST,
                    detail="Cannot remove user"
                )
        
        @self.app.post("/api/auth/change-password", dependencies=[auth_dependency] if self.enable_auth else [])
        async def change_password(
            old_password: str, 
            new_password: str, 
            current_user: Dict[str, Any] = Depends(get_current_user) if self.enable_auth else None
        ):
            """Change the current user's password."""
            if not self.enable_auth:
                raise HTTPException(status_code=404, detail="Authentication not enabled")
            
            # Verify old password
            user = auth_manager.authenticate_user(current_user["username"], old_password)
            if not user:
                raise HTTPException(
                    status_code=status.HTTP_401_UNAUTHORIZED,
                    detail="Incorrect old password"
                )
            
            # Update password
            if auth_manager.update_password(current_user["username"], new_password):
                return {"status": "success", "message": "Password updated"}
            else:
                raise HTTPException(
                    status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                    detail="Failed to update password"
                )
        
        @self.app.websocket("/ws")
        async def websocket_endpoint(websocket: WebSocket, token: Optional[str] = None):
            """WebSocket endpoint for real-time updates."""
            # Verify token if auth is enabled
            if self.enable_auth and token:
                try:
                    auth_manager.verify_token(token)
                except HTTPException:
                    await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
                    return
            elif self.enable_auth:
                # Auth is enabled but no token provided
                await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
                return
            
            await websocket.accept()
            self.monitor.websocket_clients.append(websocket)
            
            # Send initial state
            await websocket.send_json({
                "type": "initial_state",
                "data": {
                    "orchestrators": self.monitor.get_all_orchestrators_status(),
                    "system_metrics": self.monitor.metrics_cache.get("system", {})
                }
            })
            
            try:
                while True:
                    # Keep connection alive and handle incoming messages
                    data = await websocket.receive_text()
                    # Handle ping/pong or other commands if needed
                    if data == "ping":
                        await websocket.send_text("pong")
            except WebSocketDisconnect:
                self.monitor.websocket_clients.remove(websocket)
                logger.info("WebSocket client disconnected")
    
    def run(self):
        """Run the web server."""
        logger.info(f"Starting web monitor on {self.host}:{self.port}")
        uvicorn.run(self.app, host=self.host, port=self.port)
    
    async def arun(self):
        """Run the web server asynchronously."""
        logger.info(f"Starting web monitor on {self.host}:{self.port}")
        config = uvicorn.Config(app=self.app, host=self.host, port=self.port)
        server = uvicorn.Server(config)
        await server.serve()
    
    def register_orchestrator(self, orchestrator_id: str, orchestrator: RalphOrchestrator):
        """Register an orchestrator with the monitor."""
        self.monitor.register_orchestrator(orchestrator_id, orchestrator)
    
    def unregister_orchestrator(self, orchestrator_id: str):
        """Unregister an orchestrator from the monitor."""
        self.monitor.unregister_orchestrator(orchestrator_id)