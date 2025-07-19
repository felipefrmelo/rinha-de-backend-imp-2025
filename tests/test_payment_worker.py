from typing import Any
from uuid import uuid4
import pytest
from datetime import datetime, timezone
from decimal import Decimal

from src.domain.models import PaymentRequest, PaymentProcessRequest, PaymentResponse
from src.domain.payment_worker import PaymentWorker


class TestPaymentWorker:
    """Test suite for PaymentWorker async processing."""

    @pytest.fixture
    def payment_worker(self):
        """Create PaymentWorker instance with mock dependencies."""
        
        class MockQueueManager:
            def __init__(self):
                self.queue_data = []
                
            async def get_next_payment(self):
                if self.queue_data:
                    return self.queue_data.pop(0)
                return None
        
        class MockPaymentService:
            def __init__(self):
                self.processed_payments = []
                
            async def process_payment(self, payment_request: PaymentRequest):
                self.processed_payments.append(payment_request)
                return PaymentResponse(message="Payment processed successfully")
        
        mock_queue_manager = MockQueueManager()
        mock_payment_service = MockPaymentService()
        
        return PaymentWorker(
            queue_manager=mock_queue_manager,
            payment_service=mock_payment_service
        ), mock_queue_manager, mock_payment_service

    @pytest.mark.asyncio
    async def test_process_single_payment_from_queue(
        self, payment_worker: tuple[PaymentWorker, Any, Any]
    ):
        """Test that worker can process a single payment from queue."""
        # Arrange
        correlation_id = uuid4()
        worker, mock_queue, mock_service = payment_worker
        
        # Queue has one payment waiting (serialized format)
        mock_queue.queue_data = [{
            "correlationId": str(correlation_id),
            "amount": "100.50",
            "requestedAt": datetime.now(timezone.utc).isoformat()
        }]
        
        # Act
        result = await worker.process_next_payment()
        
        # Assert
        assert result is True  # Successfully processed
        assert len(mock_service.processed_payments) == 1
        processed = mock_service.processed_payments[0]
        assert processed.correlationId == correlation_id
        assert processed.amount == Decimal("100.50")

    @pytest.mark.asyncio
    async def test_process_next_payment_returns_false_when_queue_empty(
        self, payment_worker: tuple[PaymentWorker, Any, Any]
    ):
        """Test that worker returns False when no payments in queue."""
        # Arrange
        worker, mock_queue, mock_service = payment_worker
        # Queue is empty (no data added)
        
        # Act
        result = await worker.process_next_payment()
        
        # Assert
        assert result is False  # No payment to process
        assert len(mock_service.processed_payments) == 0  # Nothing processed

    @pytest.mark.asyncio
    async def test_process_next_payment_handles_processing_error(
        self, payment_worker: tuple[PaymentWorker, Any, Any]
    ):
        """Test that worker handles processing errors and returns False."""
        # Arrange
        correlation_id = uuid4()
        worker, mock_queue, mock_service = payment_worker
        
        # Queue has one payment waiting (serialized format)
        mock_queue.queue_data = [{
            "correlationId": str(correlation_id),
            "amount": "100.50",
            "requestedAt": datetime.now(timezone.utc).isoformat()
        }]
        
        # Make service throw an error
        async def failing_process_payment(payment_request):
            raise Exception("Payment processor unavailable")
        
        mock_service.process_payment = failing_process_payment
        
        # Act
        result = await worker.process_next_payment()
        
        # Assert
        assert result is False  # Failed to process due to error

