from src.domain.models import HealthStatus
from src.domain.protocols import HttpClient, HealthStatusCache


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
        key = self.base_url

        # Try to get from cache first
        cached_status = await self.cache.get(key)
        if cached_status is not None:
            return cached_status

        # Make HTTP request with error handling
        try:
            url = f"{self.base_url}/payments/service-health"
            response = await self.http_client.get(url)
            
            data = response.json()
            health_status = HealthStatus(
                failing=data["failing"], min_response_time=data["minResponseTime"]
            )
        except Exception:
            # If health check fails, assume the service is failing
            health_status = HealthStatus(failing=True, min_response_time=0)

        await self.cache.set(key, health_status, ttl_seconds=5)

        return health_status
