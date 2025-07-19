import asyncio
from datetime import datetime, timezone
from decimal import Decimal
import os
from uuid import uuid4

import pytest
import pytest_asyncio
import redis.asyncio as redis

from src.adapters.redis_client import RedisQueueClient


@pytest_asyncio.fixture
async def redis_client():
    redis_host = os.environ.get('REDIS_HOST', 'localhost')  # Default to 'redis' for production
    client = redis.Redis(host=redis_host, port=6379, db=1, decode_responses=True)
    await client.flushdb()
    yield client
    await client.flushdb()
    await client.aclose()


@pytest.mark.integration
class TestRedisQueueClient:
    """Test suite for Redis-based queue client adapter."""

    @pytest.mark.asyncio
    async def test_redis_queue_initialization(self, redis_client):
        """Test that RedisQueueClient can be initialized with Redis connection."""
        queue_client = RedisQueueClient(redis_connection=redis_client)

        assert queue_client.redis_connection == redis_client

    @pytest.mark.asyncio
    async def test_enqueue_stores_in_redis_stream(self, redis_client):
        """Test that enqueue stores message in Redis stream."""
        queue_client = RedisQueueClient(redis_connection=redis_client)

        correlation_id = uuid4()
        queue_name = "payments:queue"
        message = {
            "correlationId": str(correlation_id),
            "amount": "100.50",
            "requestedAt": datetime.now(timezone.utc).isoformat()
        }

        await queue_client.enqueue(queue_name, message)

        # Verify data was stored in Redis stream
        stream_data = await redis_client.xread({queue_name: "0"}, count=1)
        assert len(stream_data) == 1
        assert len(stream_data[0][1]) == 1  # One message
        
        stored_fields = stream_data[0][1][0][1]  # [stream_name, [[msg_id, fields]]]
        assert stored_fields["correlationId"] == str(correlation_id)
        assert stored_fields["amount"] == "100.50"

    @pytest.mark.asyncio
    async def test_dequeue_gets_from_redis_stream(self, redis_client):
        """Test that dequeue gets message from Redis stream."""
        queue_client = RedisQueueClient(redis_connection=redis_client)

        correlation_id = uuid4()
        queue_name = "payments:queue"
        
        # First add a message directly to Redis
        await redis_client.xadd(queue_name, {
            "correlationId": str(correlation_id),
            "amount": "75.25",
            "requestedAt": datetime.now(timezone.utc).isoformat()
        })

        # Now dequeue it
        message = await queue_client.dequeue(queue_name)

        assert message is not None
        assert message["correlationId"] == str(correlation_id)
        assert message["amount"] == "75.25"

    @pytest.mark.asyncio
    async def test_dequeue_returns_none_when_stream_empty(self, redis_client):
        """Test that dequeue returns None when Redis stream is empty."""
        queue_client = RedisQueueClient(redis_connection=redis_client)

        message = await queue_client.dequeue("empty_queue")

        assert message is None

    @pytest.mark.asyncio
    async def test_fifo_message_processing(self, redis_client):
        """Test that messages are processed in FIFO order."""
        queue_client = RedisQueueClient(redis_connection=redis_client)
        queue_name = "test_queue"

        # Enqueue 3 messages
        messages = []
        for i in range(3):
            correlation_id = uuid4()
            message = {
                "correlationId": str(correlation_id),
                "amount": f"{100 + i}.00",
                "requestedAt": datetime.now(timezone.utc).isoformat()
            }
            messages.append(message)
            await queue_client.enqueue(queue_name, message)

        # Dequeue them and verify FIFO order
        for expected_message in messages:
            dequeued = await queue_client.dequeue(queue_name)
            assert dequeued["correlationId"] == expected_message["correlationId"]
            assert dequeued["amount"] == expected_message["amount"]

        # Queue should be empty now
        assert await queue_client.dequeue(queue_name) is None