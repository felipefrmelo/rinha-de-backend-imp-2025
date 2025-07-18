import asyncio
from datetime import datetime
from decimal import Decimal
import os
from uuid import uuid4

import pytest
import pytest_asyncio
import redis.asyncio as redis

from src.adapters.storage import RedisPaymentStorage
from src.domain.models import PaymentRequest


@pytest_asyncio.fixture
async def redis_client():
    redis_host = os.environ.get('REDIS_HOST', 'localhost')  # Default to 'redis' for production
    client = redis.Redis(host=redis_host, port=6379, db=1, decode_responses=True)
    await client.flushdb()
    yield client
    await client.flushdb()
    await client.aclose()


@pytest.mark.integration
class TestRedisPaymentStorage:
    """Test suite for Redis-based payment storage adapter."""

    @pytest.mark.asyncio
    async def test_redis_storage_initialization(self, redis_client):
        """Test that RedisPaymentStorage can be initialized with Redis connection."""
        storage = RedisPaymentStorage(redis_client=redis_client)

        assert storage.redis_client == redis_client

    @pytest.mark.asyncio
    async def test_store_payment_stores_in_redis(self, redis_client):
        """Test that store_payment stores payment data in Redis."""
        # This test should fail since store_payment method doesn't exist yet
        storage = RedisPaymentStorage(redis_client=redis_client)

        uuid = uuid4()
        payment_request = PaymentRequest(
            correlationId=uuid, amount=Decimal("100.5")
        )
        processed_at = datetime(2024, 1, 1, 12, 0, 0)

        await storage.store_payment(payment_request, "default", processed_at)

        # Give background task time to complete
        await asyncio.sleep(0.1)

        # Verify data was stored in Redis
        payment_data = await redis_client.hgetall(
            f"payment:{uuid}"
        )
        assert payment_data["amount"] == "100.5"
        assert payment_data["processor_used"] == "default"

    @pytest.mark.asyncio
    async def test_get_payments_summary_aggregates_by_processor(self, redis_client):
        """Test that get_payments_summary aggregates payments by processor type."""
        # This test should fail since get_payments_summary method doesn't exist yet
        storage = RedisPaymentStorage(redis_client=redis_client)

        # Store some test payments
        payment1 = PaymentRequest(correlationId=uuid4(), amount=Decimal("100.00"))
        payment2 = PaymentRequest(correlationId=uuid4(), amount=Decimal("200.00"))
        payment3 = PaymentRequest(correlationId=uuid4(), amount=Decimal("50.00"))

        processed_at = datetime(2024, 1, 1, 12, 0, 0)

        await storage.store_payment(payment1, "default", processed_at)
        await storage.store_payment(payment2, "default", processed_at)
        await storage.store_payment(payment3, "fallback", processed_at)

        # Give background tasks time to complete
        await asyncio.sleep(0.1)

        # Get summary
        from_time = datetime(2024, 1, 1, 11, 0, 0)
        to_time = datetime(2024, 1, 1, 13, 0, 0)

        summary = await storage.get_payments_summary(from_time, to_time)

        # Should aggregate by processor type
        assert summary["default"]["totalRequests"] == 2
        assert summary["default"]["totalAmount"] == 300.00
        assert summary["fallback"]["totalRequests"] == 1
        assert summary["fallback"]["totalAmount"] == 50.00

    @pytest.mark.asyncio
    async def test_store_payment_fire_and_forget_performance(self, redis_client):
        """Test that store_payment returns immediately for fire-and-forget performance."""
        storage = RedisPaymentStorage(redis_client=redis_client)
        
        payment_request = PaymentRequest(
            correlationId=uuid4(),
            amount=Decimal("100.50")
        )
        processed_at = datetime(2024, 1, 1, 12, 0, 0)
        
        # Measure time for store_payment call
        import time
        start_time = time.time()
        await storage.store_payment(payment_request, "default", processed_at)
        elapsed = time.time() - start_time
        
        # Should return very quickly (< 10ms for fire-and-forget)
        assert elapsed < 0.01  # 10ms
        
        # Give background task time to complete
        await asyncio.sleep(0.1)
        
        # Data should eventually be stored
        payment_data = await redis_client.hgetall(f"payment:{payment_request.correlationId}")
        assert payment_data["amount"] == "100.50"
        assert payment_data["processor_used"] == "default"

    @pytest.mark.asyncio
    async def test_decimal_precision_consistency(self, redis_client):
        """Test that decimal precision is maintained to avoid inconsistency fines."""
        storage = RedisPaymentStorage(redis_client=redis_client)
        
        # Test with amounts that could cause precision issues
        amounts = [Decimal("19.90"), Decimal("100.33"), Decimal("0.01"), Decimal("999.99")]
        
        processed_at = datetime(2024, 1, 1, 12, 0, 0)
        
        for i, amount in enumerate(amounts):
            payment_request = PaymentRequest(
                correlationId=uuid4(),
                amount=amount
            )
            await storage.store_payment(payment_request, "default", processed_at)
        
        # Give background tasks time to complete
        await asyncio.sleep(0.1)
        
        # Get summary
        from_time = datetime(2024, 1, 1, 11, 0, 0)
        to_time = datetime(2024, 1, 1, 13, 0, 0)
        
        summary = await storage.get_payments_summary(from_time, to_time)
        
        # Verify the sum maintains precision
        expected_total = sum(amounts)
        actual_total = summary["default"]["totalAmount"]
        
        # Should be exactly equal (no floating point precision errors)
        assert actual_total == float(expected_total)
        assert summary["default"]["totalRequests"] == len(amounts)
