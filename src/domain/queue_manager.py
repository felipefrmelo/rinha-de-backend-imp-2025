from datetime import datetime, timezone
from typing import Optional
from src.domain.models import PaymentRequest, PaymentProcessRequest
from src.domain.protocols import QueueClient


class QueueManager:
    def __init__(self, queue_client: QueueClient) -> None:
        self.queue_client = queue_client

    async def add_payment_to_queue(self, payment_request: PaymentRequest):
        # Convert to PaymentProcessRequest with timestamp
        process_request = PaymentProcessRequest(
            correlationId=payment_request.correlationId,
            amount=payment_request.amount,
            requestedAt=datetime.now(timezone.utc)
        )
        # Use mode='json' to ensure proper serialization for Redis
        serialized_data = process_request.model_dump(mode='json')
        await self.queue_client.enqueue("payments:queue", serialized_data)

    async def get_next_payment(self) -> Optional[dict]:
        """Get the next payment from the queue."""
        return await self.queue_client.dequeue("payments:queue")
