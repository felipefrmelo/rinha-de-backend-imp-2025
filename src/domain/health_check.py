import asyncio
from typing import Dict, Optional
from src.domain.models import HealthStatus
from src.domain.protocols import HttpClient, HealthStatusCache


class HealthCheckClient:
    """Background service that periodically checks health of payment processors."""
    
    def __init__(
        self, 
        processor_urls: Dict[str, str], 
        http_client: HttpClient,
        cache: HealthStatusCache,
        check_interval_seconds: float = 5.2  # Check every 5.2 seconds to respect 5s rate limit per service
    ):
        self.processor_urls = processor_urls
        self.http_client = http_client
        self.cache = cache
        self.check_interval_seconds = check_interval_seconds
        self._running = False
        self._task: Optional[asyncio.Task] = None
    
    async def start(self):
        """Start the background health monitoring."""
        if self._running:
            return
        
        self._running = True
        self._task = asyncio.create_task(self._monitor_health())
    
    async def stop(self):
        """Stop the background health monitoring."""
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
    
    async def get_health_status(self, processor_name: str) -> Optional[HealthStatus]:
        """Get cached health status for a processor without making HTTP calls."""
        return await self.cache.get(processor_name)
    
    async def check_health(self) -> Optional[HealthStatus]:
        """Compatibility method for existing code that expects to check health of a single processor."""
        # For backward compatibility, check the first processor if any exist
        if self.processor_urls:
            first_processor = next(iter(self.processor_urls.keys()))
            return await self.get_health_status(first_processor)
        return None
    
    async def _monitor_health(self):
        """Background task that periodically checks health of all processors.
        
        Each processor has independent rate limits, so we can check them concurrently.
        """
        while self._running:
            try:
                # Check health of all processors concurrently (independent rate limits)
                tasks = []
                for name, url in self.processor_urls.items():
                    task = asyncio.create_task(self._update_health_status(name, url))
                    tasks.append(task)
                
                if tasks:
                    await asyncio.gather(*tasks, return_exceptions=True)
                
                await asyncio.sleep(self.check_interval_seconds)
            except asyncio.CancelledError:
                break
            except Exception:
                # Continue monitoring even if some checks fail
                await asyncio.sleep(self.check_interval_seconds)
    
    async def _update_health_status(self, processor_name: str, base_url: str):
        """Update health status for a single processor."""
        try:
            # Force a fresh health check
            url = f"{base_url}/payments/service-health"
            response = await self.http_client.get(url)
            data = response.json()
            print(f"health_status {data}")
            health_status = HealthStatus(
                failing=data["failing"], 
                min_response_time=data["minResponseTime"]
            )
        except Exception:
            # If health check fails, mark as failing
            health_status = HealthStatus(failing=True, min_response_time=0)
        
        # Store in cache with TTL slightly longer than check interval
        await self.cache.set(processor_name, health_status, ttl_seconds=8)
