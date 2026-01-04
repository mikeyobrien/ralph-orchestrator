# ABOUTME: Daemon module for background orchestration
# ABOUTME: Enables ralph run --daemon to return immediately

from .manager import DaemonManager

__all__ = ["DaemonManager"]
