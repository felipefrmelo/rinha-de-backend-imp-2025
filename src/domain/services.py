import asyncio
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

    async def process_payment(self, payment_request: PaymentRequest, processed_at: datetime) -> PaymentResponse:
        return await self.processor.process_payment(payment_request, processed_at)


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
        """
        Process a payment request using smart routing based on health checks.
        Tries the default provider first if healthy, otherwise tries fallback.
        Raises an exception if both are unavailable or fail.
        """
        processed_at = datetime.now(timezone.utc)

        # Check health of both providers concurrently
        default_health_task = asyncio.create_task(self.default.check_health())
        fallback_health_task = asyncio.create_task(self.fallback.check_health())
        
        default_health, fallback_health = await asyncio.gather(
            default_health_task, fallback_health_task
        )

        if default_health.failing and fallback_health.failing:
            raise Exception("Both payment processors are currently unavailable")

        providers: list[tuple[PaymentProvider, str]] = []
        if not default_health.failing:
            providers.append((self.default, self.default.name))
        if not fallback_health.failing:
            providers.append((self.fallback, self.fallback.name))

        last_exception = None
        for provider, provider_name in providers:
            try:
                response = await provider.process_payment(payment_request, processed_at)
                await self.storage.store_payment(payment_request, provider_name, processed_at)
                return response
            except Exception as exc:
                last_exception = exc
                continue

        raise Exception("Both payment processors are currently unavailable") from last_exception

    async def get_payments_summary(self, from_: datetime, to_: datetime) -> dict:
        """Get payment summary grouped by processor type."""
        return await self.storage.get_payments_summary(from_, to_)
