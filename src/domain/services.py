from dataclasses import dataclass
from datetime import datetime, timezone

from src.domain.models import PaymentRequest, PaymentResponse, HealthStatus
from src.domain.protocols import PaymentProcessor, PaymentStorage
from src.domain.health_check import HealthCheckClient


@dataclass
class PaymentProvider:
    """Represents a payment provider with processing capability and health monitoring."""

    processor: PaymentProcessor
    health_check: HealthCheckClient
    name: str

    async def check_health(self) -> HealthStatus:
        return await self.health_check.check_health()

    async def process_payment(self, payment_request: PaymentRequest) -> PaymentResponse:
        return await self.processor.process_payment(payment_request)


class PaymentService:
    def __init__(
        self,
        default: PaymentProvider,
        fallback: PaymentProvider,
        storage: PaymentStorage,
    ):
        """Initialize the PaymentService with payment providers and storage."""
        self.default = default
        self.fallback = fallback
        self.storage = storage

    async def process_payment(self, payment_request: PaymentRequest):
        """Process a payment request using smart routing based on health checks."""

        processed_at = datetime.now(timezone.utc)

        # Check health of both providers
        default_health = await self.default.check_health()
        fallback_health = await self.fallback.check_health()

        # If both providers are failing, raise an exception
        if default_health.failing and fallback_health.failing:
            raise Exception("Both payment processors are currently unavailable")

        # If default provider is failing but fallback is healthy, route to fallback
        if default_health.failing:
            response = await self.fallback.process_payment(payment_request)
            await self.storage.store_payment(
                payment_request, self.fallback.name, processed_at
            )
            return response

        # Otherwise try default provider with fallback on exception
        try:
            response = await self.default.process_payment(payment_request)
            await self.storage.store_payment(
                payment_request, self.default.name, processed_at
            )
            return response
        except Exception:
            # Only try fallback if it's healthy
            if not fallback_health.failing:
                response = await self.fallback.process_payment(
                    payment_request
                )
                await self.storage.store_payment(
                    payment_request, self.fallback.name, processed_at
                )
                return response
            else:
                # Re-raise the original exception if fallback is also unhealthy
                raise

    async def get_payments_summary(self, from_: datetime, to_: datetime) -> dict:
        """Get payment summary grouped by processor type."""
        return await self.storage.get_payments_summary(from_, to_)
