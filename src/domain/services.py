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
        health_status = await self.health_check.get_health_status(self.name)
        if health_status is None:
            return HealthStatus(failing=True, min_response_time=0)
        return health_status

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
        Process a payment request using optimized routing.
        Uses cached health status and async fire-and-forget storage.
        """
        processed_at = datetime.now(timezone.utc)

        # Get cached health status (non-blocking)
        default_health = await self.default.check_health()
        fallback_health = await self.fallback.check_health()
        
        # Determine processing order based on health and response time
        providers = []
        if not default_health.failing:
            providers.append((self.default, self.default.name, default_health.min_response_time))
        if not fallback_health.failing:
            providers.append((self.fallback, self.fallback.name, fallback_health.min_response_time))
        
        # Sort by response time for optimal performance
        providers.sort(key=lambda x: x[2])
        
        if not providers:
            raise Exception("Both payment processors are currently unavailable")

        last_exception = None
        for provider, provider_name, _ in providers:
            try:
                response = await provider.process_payment(payment_request, processed_at)
                # Store payment synchronously to ensure consistency
                await self.storage.store_payment(payment_request, provider_name, processed_at)
                return response
            except Exception as exc:
                last_exception = exc
                continue

        raise Exception("All available payment processors failed") from last_exception

    async def get_payments_summary(self, from_: datetime, to_: datetime) -> dict:
        """Get payment summary grouped by processor type."""
        return await self.storage.get_payments_summary(from_, to_)
