import datetime
from datetime import datetime, timezone
from typing import Protocol

from pydantic import BaseModel

from src.models import PaymentRequest


class PaymentResponse(BaseModel):
    message: str


class PaymentProcessor(Protocol):
    async def process_payment(self, payment_request: PaymentRequest) -> PaymentResponse:
        """Process a payment request."""
        ...


class PaymentStorage(Protocol):
    async def store_payment(
        self,
        payment_request: PaymentRequest,
        processor_used: str,
        processed_at: datetime,
    ) -> None:
        """Store a payment request."""
        ...

    async def get_payments_summary(
        self, from_timestamp: datetime, to_timestamp: datetime
    ) -> dict:
        """Get payment summary grouped by processor type."""
        ...


class PaymentService:
    def __init__(
        self,
        default_processor: PaymentProcessor,
        fallback_processor: PaymentProcessor,
        storage: PaymentStorage,
    ):
        """Initialize the PaymentService with a default processor."""
        self.default_processor = default_processor
        self.fallback_processor = fallback_processor
        self.storage = storage

    async def process_payment(self, payment_request: PaymentRequest):
        """Process a payment request using the default processor."""

        processed_at = datetime.now(timezone.utc)
        try:
            response = await self.default_processor.process_payment(payment_request)
            await self.storage.store_payment(payment_request, "default", processed_at)
            return response
        except Exception:
            response = await self.fallback_processor.process_payment(payment_request)
            await self.storage.store_payment(payment_request, "fallback", processed_at)
            return response
