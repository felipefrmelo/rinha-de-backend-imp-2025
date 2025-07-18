import asyncio
import pytest
from datetime import datetime, timedelta
from src.domain.models import HealthStatus
from src.domain.health_check import HealthCheckClient


@pytest.fixture
def mock_http_client():
    class MockHttpClient:
        def __init__(self):
            self.get_calls = 0
            
        async def get(self, url):
            self.get_calls += 1
            class MockResponse:
                def json(self):
                    return {"failing": False, "minResponseTime": 10}
            return MockResponse()
    
    return MockHttpClient()


@pytest.fixture
def mock_cache():
    class MockCache:
        def __init__(self):
            self.storage = {}
            self.get_calls = 0
            self.set_calls = 0
        
        async def get(self, key):
            self.get_calls += 1
            return self.storage.get(key)
        
        async def set(self, key, value, ttl_seconds):
            self.set_calls += 1
            self.storage[key] = value
    
    return MockCache()


@pytest.mark.asyncio
async def test_background_health_service_starts_monitoring(mock_http_client, mock_cache):
    """Test that HealthCheckClient starts periodic health monitoring."""
    # Arrange
    service = HealthCheckClient(
        processor_urls={"default": "http://test-processor"},
        http_client=mock_http_client,
        cache=mock_cache,
        check_interval_seconds=0.1  # Fast for testing
    )
    
    # Act
    await service.start()
    await service.stop()
    
    # Assert - should be able to start and stop without errors
    assert service._running == False


@pytest.mark.asyncio
async def test_cached_health_status_is_available_immediately(mock_http_client, mock_cache):
    """Test that health status is immediately available from cache without HTTP calls."""
    # Arrange
    cached_status = HealthStatus(failing=False, min_response_time=5)
    await mock_cache.set("default", cached_status, 300)
    
    service = HealthCheckClient(
        processor_urls={"default": "http://test-processor"},
        http_client=mock_http_client,
        cache=mock_cache
    )
    
    # Act
    status = await service.get_health_status("default")
    
    # Assert
    assert status == cached_status
    assert mock_http_client.get_calls == 0  # No HTTP call should be made


@pytest.mark.asyncio
async def test_background_service_updates_cache_periodically(mock_http_client, mock_cache):
    """Test that background service updates health status in cache periodically."""
    # Arrange
    service = HealthCheckClient(
        processor_urls={"default": "http://test-processor"},
        http_client=mock_http_client,
        cache=mock_cache,
        check_interval_seconds=0.1
    )
    
    # Act
    await service.start()
    await asyncio.sleep(0.25)  # Wait for at least two checks
    await service.stop()
    
    # Assert - should have made health checks and updated cache
    assert mock_cache.set_calls >= 2  # Should have made at least 2 cache updates
