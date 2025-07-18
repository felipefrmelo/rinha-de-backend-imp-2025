import asyncio
from dataclasses import dataclass
from datetime import datetime, timezone
import logging

from src.domain.models import PaymentRequest, PaymentResponse, HealthStatus
from src.domain.protocols import PaymentProcessor, PaymentStorage
from src.domain.health_check import HealthCheckClient

logger = logging.getLogger(__name__)

# Custom exceptions
class PaymentProcessingError(Exception):
    """Raised when payment processing fails."""
    pass

class PaymentProvidersUnavailableError(PaymentProcessingError):
    """Raised when all payment providers are unavailable."""
    pass

class StorageError(Exception):
    """Raised when storage operations fail."""
    pass


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
        
        providers = [(self.default, default_health), (self.fallback, fallback_health)]

        providers = [p  for p in providers if not p[1].failing and p[1].min_response_time < 1000]
        
        
        if not providers:
            logger.error(f"No available payment providers for payment {payment_request.correlationId}")
            raise PaymentProvidersUnavailableError(
                "All payment processors are currently unavailable or too slow"
            )

        last_exception = None
        for provider, _ in providers:
            try:
                logger.info(f"Processing payment {payment_request.correlationId} with {provider.name}")
                response = await provider.process_payment(payment_request, processed_at)
                
                # Store payment with error handling
                try:
                    await self.storage.store_payment(payment_request, provider.name, processed_at)
                except Exception as storage_exc:
                    logger.error(f"Storage failed for payment {payment_request.correlationId}: {storage_exc}")
                    # Don't fail the payment for storage errors, just log them
                    
                logger.info(f"Payment {payment_request.correlationId} processed successfully with {provider.name}")
                return response
            except Exception as exc:
                logger.warning(f"Payment {payment_request.correlationId} failed with {provider.name}: {exc}")
                last_exception = exc
                continue

        logger.error(f"All providers failed for payment {payment_request.correlationId}")
        raise PaymentProcessingError(
            f"All available payment processors failed to process payment {payment_request.correlationId}"
        ) from last_exception

    async def get_payments_summary(self, from_: datetime, to_: datetime) -> dict:
        """Get payment summary grouped by processor type."""
        try:
            logger.info(f"Retrieving payments summary from {from_} to {to_}")
            summary = await self.storage.get_payments_summary(from_, to_)
            logger.info(f"Successfully retrieved payments summary: {summary}")
            return summary
        except Exception as exc:
            logger.error(f"Failed to retrieve payments summary: {exc}")
            raise StorageError(f"Unable to retrieve payments summary: {exc}") from exc
