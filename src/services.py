from typing import Protocol

from src.models import PaymentRequest


from pydantic import BaseModel
class PaymentResponse(BaseModel):
    message: str


class PaymentProcessor(Protocol):
    def process_payment(self, payment_request: PaymentRequest) -> PaymentResponse:
        """Process a payment request."""
        ...


class PaymentService:
    def __init__(self, default_processor: PaymentProcessor):
        """Initialize the PaymentService with a default processor."""
        self.default_processor = default_processor

    def process_payment(self, payment_request: PaymentRequest):
        """Process a payment request using the default processor."""

        return self.default_processor.process_payment(payment_request)

