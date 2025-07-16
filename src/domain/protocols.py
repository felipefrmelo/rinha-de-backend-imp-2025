from datetime import datetime
from typing import Optional, Protocol

from src.domain.models import PaymentRequest, PaymentResponse, HealthStatus


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


class HttpResponse(Protocol):
    status_code: int

    def json(self) -> dict:
        """Parse the response as JSON."""
        ...


class HttpClient(Protocol):
    async def get(self, url: str) -> HttpResponse:
        """Make an HTTP GET request."""
        ...


class HealthStatusCache(Protocol):
    async def get(self, key: str) -> Optional[HealthStatus]:
        """Get cached health status for a service."""
        ...

    async def set(
        self, key: str, health_status: HealthStatus, ttl_seconds: int
    ) -> None:
        """Set health status in cache with TTL."""
        ...
