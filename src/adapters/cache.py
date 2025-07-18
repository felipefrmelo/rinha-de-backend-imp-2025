import json
import time
from typing import Optional

import redis.asyncio as redis

from src.domain.models import HealthStatus


class RedisHealthStatusCache:
    """Redis cache for health status."""
    
    def __init__(self, redis_url: str = "redis://localhost:6379"):
        self.redis = redis.from_url(redis_url, decode_responses=True)
    
    async def get(self, key: str) -> Optional[HealthStatus]:
        """Get cached health status for a service."""
        try:
            data = await self.redis.get(key)
            if data is None:
                return None
            return HealthStatus.model_validate_json(data)
        except Exception:
            return None
    
    async def set(self, key: str, health_status: HealthStatus, ttl_seconds: int) -> None:
        """Set health status in cache with TTL."""
        try:
            data = health_status.model_dump_json()
            await self.redis.setex(key, ttl_seconds, data)
        except Exception:
            pass


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


class CacheProxy:
    """Proxy cache that tries Redis first, falls back to in-memory cache."""
    
    def __init__(self, redis_url: str = "redis://localhost:6379"):
        self.redis_cache = RedisHealthStatusCache(redis_url)
        self.memory_cache = InMemoryHealthStatusCache()
    
    async def get(self, key: str) -> Optional[HealthStatus]:
        """Get cached health status, try Redis first, then memory."""
        result = await self.redis_cache.get(key)
        if result is not None:
            return result
        return await self.memory_cache.get(key)
    
    async def set(self, key: str, health_status: HealthStatus, ttl_seconds: int) -> None:
        """Set health status in both caches."""
        await self.redis_cache.set(key, health_status, ttl_seconds)
        await self.memory_cache.set(key, health_status, ttl_seconds)
