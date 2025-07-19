from typing import Any
from uuid import uuid4
import pytest
from datetime import datetime
from decimal import Decimal

from src.domain.models import PaymentRequest
from src.domain.queue_manager import QueueManager


class TestQueueManager:
    """Test suite for QueueManager Redis streams integration."""

    @pytest.fixture
    def queue_manager(self):
        """Create QueueManager instance with mock Redis."""

        # Use a simple mock that we can control
        class MockRedis:
            def __init__(self):
                self.xadd_calls = []
                self.xread_data = []

            async def enqueue(self, queue_name, message):
                self.xadd_calls.append((queue_name, message))

            async def dequeue(self, queue_name, timeout_ms=1000):
                if self.xread_data:
                    return self.xread_data.pop(0)
                return None

        mock_redis = MockRedis()
        return QueueManager(queue_client=mock_redis), mock_redis

    @pytest.mark.asyncio
    async def test_add_payment_to_queue_creates_stream_entry(
        self, queue_manager: tuple[QueueManager, Any]
    ):
        """Test that adding a payment creates a Redis stream entry with PaymentProcessRequest."""
        # Arrange
        correlation_id = uuid4()
        queue_mgr, mock_redis = queue_manager
        payment_request = PaymentRequest(
            correlationId=correlation_id, amount=Decimal("100.50")
        )

        # Act
        await queue_mgr.add_payment_to_queue(payment_request)

        # Assert
        assert len(mock_redis.xadd_calls) == 1
        stream_name, fields = mock_redis.xadd_calls[0]
        assert stream_name == "payments:queue"

        # Should be a PaymentProcessRequest with requestedAt added, properly serialized
        assert fields["correlationId"] == str(correlation_id)  # UUID serialized as string
        assert fields["amount"] == "100.50"  # Decimal serialized as string
        assert "requestedAt" in fields
        assert isinstance(fields["requestedAt"], str)  # Datetime serialized as string

    @pytest.mark.asyncio
    async def test_get_next_payment_returns_payment_from_queue(
        self, queue_manager: tuple[QueueManager, Any]
    ):
        """Test that getting next payment dequeues from Redis stream."""
        # Arrange
        correlation_id = uuid4()
        queue_mgr, mock_redis = queue_manager

        # Simulate Redis stream data
        mock_redis.xread_data = [
            {
                "correlationId": correlation_id,
                "amount": Decimal("75.25"),
                "requestedAt": datetime.now(),
            }
        ]

        # Act
        payment = await queue_mgr.get_next_payment()

        # Assert
        assert payment is not None
        assert payment["correlationId"] == correlation_id
        assert payment["amount"] == Decimal("75.25")
        assert "requestedAt" in payment

    @pytest.mark.asyncio
    async def test_get_next_payment_returns_none_when_queue_empty(
        self, queue_manager: tuple[QueueManager, Any]
    ):
        """Test that getting next payment returns None when queue is empty."""
        # Arrange
        queue_mgr, _ = queue_manager
        payment = await queue_mgr.get_next_payment()

        assert payment is None
