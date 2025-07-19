import asyncio
from uuid import uuid4
import pytest
from datetime import datetime, timezone
from decimal import Decimal

from src.domain.models import PaymentRequest, PaymentResponse
from src.domain.background_worker import BackgroundWorker


class TestBackgroundWorker:
    """Test suite for BackgroundWorker continuous processing."""

    @pytest.fixture
    def background_worker_setup(self):
        """Set up background worker with mock dependencies."""
        
        class MockPaymentWorker:
            def __init__(self):
                self.process_calls = 0
                self.should_return = True  # True = payment processed, False = no payment
                
            async def process_next_payment(self):
                self.process_calls += 1
                return self.should_return
        
        mock_payment_worker = MockPaymentWorker()
        background_worker = BackgroundWorker(
            payment_worker=mock_payment_worker,
            poll_interval=0.001  # 1ms for fast testing
        )
        
        return background_worker, mock_payment_worker

    @pytest.mark.asyncio
    async def test_background_worker_can_start_and_stop(self, background_worker_setup):
        """Test that background worker can start and stop cleanly."""
        # Arrange
        background_worker, mock_worker = background_worker_setup
        mock_worker.should_return = False  # No payments to process
        
        # Act
        background_worker.start()
        
        # Let it run briefly
        await asyncio.sleep(0.01)
        
        # Stop the worker
        await background_worker.stop()
        
        # Assert
        assert mock_worker.process_calls > 0  # Should have tried to process
        assert not background_worker.is_running()

    @pytest.mark.asyncio
    async def test_background_worker_processes_payments_continuously(self, background_worker_setup):
        """Test that background worker processes payments in a loop."""
        # Arrange
        background_worker, mock_worker = background_worker_setup
        mock_worker.should_return = True  # Always has payments to process
        
        # Act
        background_worker.start()
        
        # Let it run and process multiple payments
        await asyncio.sleep(0.01)
        
        await background_worker.stop()
        
        # Assert
        assert mock_worker.process_calls >= 3  # Should have processed multiple times

    @pytest.mark.asyncio
    async def test_background_worker_handles_no_payments_gracefully(self, background_worker_setup):
        """Test that background worker handles empty queue without busy waiting."""
        # Arrange
        background_worker, mock_worker = background_worker_setup
        mock_worker.should_return = False  # No payments available
        
        # Act
        background_worker.start()
        await asyncio.sleep(0.01)  # Let it run briefly
        await background_worker.stop()
        
        # Assert
        assert mock_worker.process_calls > 0  # Should have tried to process
        # Worker should continue polling even when no payments

    @pytest.mark.asyncio
    async def test_background_worker_status_tracking(self, background_worker_setup):
        """Test that background worker correctly tracks running status."""
        # Arrange
        background_worker, _ = background_worker_setup
        
        # Assert initial state
        assert not background_worker.is_running()
        
        # Act - Start worker
        background_worker.start()
        assert background_worker.is_running()
        
        # Act - Stop worker
        await background_worker.stop()
        assert not background_worker.is_running()
