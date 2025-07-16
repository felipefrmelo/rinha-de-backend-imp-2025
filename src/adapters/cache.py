import time
from typing import Optional

from src.domain.models import  HealthStatus


class InMemoryHealthStatusCache:
    """In-memory cache for health status."""
    
    def __init__(self):
        self.cache = {}
        self.expires = {}
    
    async def get(self, key: str) -> Optional[HealthStatus]:
        """Get cached health status for a service."""
        if key not in self.cache:
            return None
        
        # Check if expired
        if key in self.expires and time.time() > self.expires[key]:
            del self.cache[key]
            del self.expires[key]
            return None
        
        return self.cache[key]
    
    async def set(self, key: str, health_status: HealthStatus, ttl_seconds: int) -> None:
        """Set health status in cache with TTL."""
        self.cache[key] = health_status
        self.expires[key] = time.time() + ttl_seconds
