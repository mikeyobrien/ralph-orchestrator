# ABOUTME: Logging configuration module for Ralph Orchestrator
# ABOUTME: Provides centralized logging setup with proper formatters and handlers

"""Logging configuration for Ralph Orchestrator."""

import logging
import logging.handlers
import os
import sys
from pathlib import Path
from typing import Optional, Dict, Any


class RalphLogger:
    """Centralized logging configuration for Ralph Orchestrator."""
    
    # Logger names for different components
    ORCHESTRATOR = "ralph.orchestrator"
    ADAPTER_BASE = "ralph.adapter"
    ADAPTER_QCHAT = "ralph.adapter.qchat"
    ADAPTER_KIRO = "ralph.adapter.kiro"
    ADAPTER_CLAUDE = "ralph.adapter.claude"
    ADAPTER_GEMINI = "ralph.adapter.gemini"
    SAFETY = "ralph.safety"
    METRICS = "ralph.metrics"
    
    # Default log format
    DEFAULT_FORMAT = "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    DETAILED_FORMAT = "%(asctime)s - %(name)s - %(levelname)s - [%(filename)s:%(lineno)d] - %(funcName)s() - %(message)s"
    
    _initialized = False
    _log_dir: Optional[Path] = None
    
    @classmethod
    def initialize(cls, 
                   log_level: Optional[str] = None,
                   log_file: Optional[str] = None,
                   log_dir: Optional[str] = None,
                   console_output: Optional[bool] = None,
                   detailed_format: bool = False) -> None:
        """Initialize logging configuration.
        
        Args:
            log_level: Logging level (DEBUG, INFO, WARNING, ERROR, CRITICAL)
            log_file: Path to log file (optional)
            log_dir: Directory for log files (optional)
            console_output: Whether to output to console
            detailed_format: Use detailed format with file/line info
        """
        if cls._initialized:
            return
        
        # Get configuration from environment variables
        log_level = log_level or os.getenv("RALPH_LOG_LEVEL", "INFO")
        log_file = log_file or os.getenv("RALPH_LOG_FILE")
        log_dir = log_dir or os.getenv("RALPH_LOG_DIR", ".logs")
        
        # Handle console_output properly - only use env var if not explicitly set
        if console_output is None:
            console_output = os.getenv("RALPH_LOG_CONSOLE", "true").lower() == "true"
        
        detailed_format = detailed_format or \
                         os.getenv("RALPH_LOG_DETAILED", "false").lower() == "true"
        
        # Convert log level string to logging constant
        numeric_level = getattr(logging, log_level.upper(), logging.INFO)
        
        # Choose format
        log_format = cls.DETAILED_FORMAT if detailed_format else cls.DEFAULT_FORMAT
        
        # Create formatter
        formatter = logging.Formatter(log_format)
        
        # Configure root logger
        root_logger = logging.getLogger("ralph")
        root_logger.setLevel(numeric_level)
        root_logger.handlers = []  # Clear existing handlers
        
        # Add console handler
        if console_output:
            console_handler = logging.StreamHandler(sys.stderr)
            console_handler.setFormatter(formatter)
            console_handler.setLevel(numeric_level)
            root_logger.addHandler(console_handler)
        
        # Add file handler if specified
        if log_file or log_dir:
            cls._setup_file_handler(root_logger, formatter, log_file, log_dir, numeric_level)
        
        cls._initialized = True

        # Suppress verbose INFO logs from claude-agent-sdk internals
        # The SDK logs operational details at INFO level (e.g., "Using bundled Claude Code CLI")
        logging.getLogger('claude_agent_sdk').setLevel(logging.WARNING)

        # Log initialization
        logger = logging.getLogger(cls.ORCHESTRATOR)
        logger.debug(f"Logging initialized - Level: {log_level}, Console: {console_output}, "
                    f"File: {log_file or 'None'}, Dir: {log_dir or 'None'}")
    
    @classmethod
    def _setup_file_handler(cls, 
                           logger: logging.Logger,
                           formatter: logging.Formatter,
                           log_file: Optional[str],
                           log_dir: Optional[str],
                           level: int) -> None:
        """Setup file handler for logging.
        
        Args:
            logger: Logger to add handler to
            formatter: Log formatter
            log_file: Specific log file path
            log_dir: Directory for log files
            level: Logging level
        """
        # Determine log file path
        if log_file:
            log_path = Path(log_file)
        else:
            # Use log directory with default filename
            cls._log_dir = Path(log_dir)
            cls._log_dir.mkdir(parents=True, exist_ok=True)
            log_path = cls._log_dir / "ralph_orchestrator.log"
        
        # Create parent directories if needed
        log_path.parent.mkdir(parents=True, exist_ok=True)
        
        # Use rotating file handler
        max_bytes = int(os.getenv("RALPH_LOG_MAX_BYTES", "10485760"))  # 10MB default
        backup_count = int(os.getenv("RALPH_LOG_BACKUP_COUNT", "5"))
        
        file_handler = logging.handlers.RotatingFileHandler(
            log_path,
            maxBytes=max_bytes,
            backupCount=backup_count
        )
        file_handler.setFormatter(formatter)
        file_handler.setLevel(level)
        logger.addHandler(file_handler)
    
    @classmethod
    def get_logger(cls, name: str) -> logging.Logger:
        """Get a logger instance.
        
        Args:
            name: Logger name (use class constants for consistency)
            
        Returns:
            Configured logger instance
        """
        if not cls._initialized:
            cls.initialize()
        
        return logging.getLogger(name)
    
    @classmethod
    def log_config(cls) -> Dict[str, Any]:
        """Get current logging configuration.
        
        Returns:
            Dictionary with current logging settings
        """
        root_logger = logging.getLogger("ralph")
        
        config = {
            "level": logging.getLevelName(root_logger.level),
            "handlers": [],
            "log_dir": str(cls._log_dir) if cls._log_dir else None,
            "initialized": cls._initialized
        }
        
        for handler in root_logger.handlers:
            handler_info = {
                "type": handler.__class__.__name__,
                "level": logging.getLevelName(handler.level)
            }
            
            if hasattr(handler, 'baseFilename'):
                handler_info["file"] = handler.baseFilename
            
            config["handlers"].append(handler_info)
        
        return config
    
    @classmethod
    def set_level(cls, level: str, logger_name: Optional[str] = None) -> None:
        """Dynamically set logging level.
        
        Args:
            level: New logging level
            logger_name: Specific logger to update (None for root)
        """
        numeric_level = getattr(logging, level.upper(), logging.INFO)
        
        if logger_name:
            logger = logging.getLogger(logger_name)
        else:
            logger = logging.getLogger("ralph")
        
        logger.setLevel(numeric_level)
        
        # Update handlers
        for handler in logger.handlers:
            handler.setLevel(numeric_level)


# Convenience function for getting loggers
def get_logger(name: str) -> logging.Logger:
    """Get a configured logger instance.
    
    Args:
        name: Logger name
        
    Returns:
        Logger instance
    """
    return RalphLogger.get_logger(name)