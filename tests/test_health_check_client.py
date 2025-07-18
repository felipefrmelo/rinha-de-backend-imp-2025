from typing import Optional

import pytest

from src.domain.health_check import HealthStatus


class MockHealthStatusCache:
    def __init__(self):
        self._cache = {}

    async def get(self, key: str) -> Optional[HealthStatus]:
        return self._cache.get(key)

    async def set(
        self, key: str, health_status: HealthStatus, ttl_seconds: int
    ) -> None:
        self._cache[key] = health_status


class MockHttpClient:
    def __init__(self):
        self.call_count = 0

    async def get(self, url: str):
        self.call_count += 1
        return MockResponse(
            status_code=200, json_data={"failing": False, "minResponseTime": 100}
        )


class MockResponse:
    def __init__(self, status_code: int, json_data: dict):
        self.status_code = status_code
        self._json_data = json_data

    def json(self):
        return self._json_data


def test_health_check_client_can_be_instantiated():
    """Test that HealthCheckClient can be instantiated"""
    from src.domain.health_check import HealthCheckClient

    http_client = MockHttpClient()
    cache = MockHealthStatusCache()
    client = HealthCheckClient(
        processor_urls={"default": "http://example.com:8080"}, 
        http_client=http_client, 
        cache=cache
    )

    assert client is not None
    assert isinstance(client, HealthCheckClient)


@pytest.mark.asyncio
async def test_health_check_client_can_check_service_health():
    """Test that HealthCheckClient can check service health"""
    from src.domain.health_check import HealthCheckClient

    # Arrange
    http_client = MockHttpClient()
    cache = MockHealthStatusCache()
    client = HealthCheckClient(
        processor_urls={"default": "http://example.com:8080"}, 
        http_client=http_client, 
        cache=cache
    )

    # Pre-populate cache with health status
    await cache.set("default", HealthStatus(failing=False, min_response_time=100), 300)

    health_status = await client.check_health()

    assert health_status is not None
    assert health_status.failing is False
    assert health_status.min_response_time == 100


@pytest.mark.asyncio
async def test_health_check_client_gets_cached_status():
    """Test that HealthCheckClient can get cached health status"""
    from src.domain.health_check import HealthCheckClient

    # Arrange
    http_client = MockHttpClient()
    cache = MockHealthStatusCache()
    client = HealthCheckClient(
        processor_urls={"default": "http://example.com:8080"}, 
        http_client=http_client, 
        cache=cache
    )

    # Pre-populate cache with health status
    health_status = HealthStatus(failing=False, min_response_time=100)
    await cache.set("default", health_status, 300)

    # Act - Get health status from cache
    cached_status = await client.get_health_status("default")
    
    # Assert
    assert cached_status is not None
    assert cached_status.failing is False
    assert cached_status.min_response_time == 100
    assert http_client.call_count == 0  # No HTTP call should be made
