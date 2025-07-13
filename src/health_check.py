from typing import Optional, Protocol

from pydantic import BaseModel


class HttpResponse(Protocol):
    status_code: int

    def json(self) -> dict:
        """Parse the response as JSON."""
        ...


class HttpClient(Protocol):
    async def get(self, url: str) -> HttpResponse:
        """Make an HTTP GET request."""
        ...


class HealthStatus(BaseModel):
    failing: bool
    min_response_time: int


class HealthStatusCache(Protocol):
    async def get(self, key: str) -> Optional[HealthStatus]:
        """Get cached health status for a service."""
        ...

    async def set(
        self, key: str, health_status: HealthStatus, ttl_seconds: int
    ) -> None:
        """Set health status in cache with TTL."""
        ...


class HealthCheckClient:
    def __init__(
        self, base_url: str, http_client: HttpClient, cache: HealthStatusCache
    ):
        self.base_url = base_url
        self.http_client = http_client
        self.cache = cache

    async def check_health(self) -> HealthStatus:
        """Check the health of the payment processor service.

        Uses cache to respect rate limiting (1 call per 5 seconds).
        """
        key = "payments_service_health"

        # Try to get from cache first
        cached_status = await self.cache.get(key)
        if cached_status is not None:
            return cached_status

        # Make HTTP request
        url = f"{self.base_url}/payments/service-health"
        response = await self.http_client.get(url)

        data = response.json()
        health_status = HealthStatus(
            failing=data["failing"], min_response_time=data["minResponseTime"]
        )

        await self.cache.set(key, health_status, ttl_seconds=5)

        return health_status
