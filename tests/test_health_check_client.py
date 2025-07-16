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
        base_url="http://example.com:8080", http_client=http_client, cache=cache
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
        base_url="http://example.com:8080", http_client=http_client, cache=cache
    )

    health_status = await client.check_health()

    assert health_status is not None
    assert health_status.failing is False
    assert health_status.min_response_time == 100


@pytest.mark.asyncio
async def test_health_check_client_respects_rate_limiting():
    """Test that HealthCheckClient respects rate limiting (1 call per 5 seconds)"""
    from src.domain.health_check import HealthCheckClient

    # Arrange
    http_client = MockHttpClient()
    cache = MockHealthStatusCache()
    client = HealthCheckClient(
        base_url="http://example.com:8080", http_client=http_client, cache=cache
    )

    # Act - First call should work and make HTTP request
    health_status1 = await client.check_health()
    assert health_status1 is not None

    # Act - Second call immediately should return cached result (no HTTP request)
    health_status2 = await client.check_health()
    assert health_status2 is not None
    assert health_status1.failing == health_status2.failing
    assert health_status1.min_response_time == health_status2.min_response_time

    # Assert - Only one HTTP call should have been made due to caching
    assert http_client.call_count == 1
