import os
import logging

def get_log_level() -> str:
    """Get log level from environment variable."""
    return os.getenv("LOG_LEVEL", "INFO").upper()

def get_uvicorn_log_level() -> str:
    """Get log level for Uvicorn (lowercase)."""
    return get_log_level().lower()

def setup_logging() -> None:
    """Setup simple logging configuration."""
    log_level = get_log_level()
    
    # Validate log level
    valid_levels = ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]
    if log_level not in valid_levels:
        log_level = "INFO"
    
    # Configure basic logging
    logging.basicConfig(
        level=getattr(logging, log_level),
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S"
    )
    
    # Set specific logger levels
    logging.getLogger("uvicorn").setLevel(getattr(logging, log_level))
    logging.getLogger("uvicorn.access").setLevel(getattr(logging, log_level))
    logging.getLogger("uvicorn.error").setLevel(getattr(logging, log_level))
    
    # Keep some loggers quiet unless debugging
    if log_level != "DEBUG":
        logging.getLogger("httpx").setLevel(logging.WARNING)
        logging.getLogger("redis").setLevel(logging.WARNING)
    
    logger = logging.getLogger(__name__)
    logger.info(f"Logging configured with level: {log_level}")