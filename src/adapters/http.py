from datetime import datetime, timezone

import httpx

from src.domain.protocols import PaymentProcessor, PaymentResponse, HttpClient
from src.domain.models import PaymentRequest


class HttpPaymentProcessor:
    """HTTP-based payment processor implementation with connection pooling."""
    
    def __init__(self, base_url: str):
        self.base_url = base_url
        # Create persistent client with connection pooling
        self.client = httpx.AsyncClient(
            timeout=1.0,
            limits=httpx.Limits(max_connections=50, max_keepalive_connections=20)
        )
    
    async def process_payment(self, payment_request: PaymentRequest, processed_at: datetime) -> PaymentResponse:
        """Process a payment request via HTTP."""
        # Add requestedAt timestamp as required by the payment processors
        request_data = {
            "correlationId": str(payment_request.correlationId),
            "amount": float(payment_request.amount),
            "requestedAt": processed_at.isoformat(),
        }
        
        response = await self.client.post(
            f"{self.base_url}/payments",
            json=request_data,
        )
        
        if response.status_code >= 500:
            raise Exception(f"HTTP {response.status_code} - {response.text}")
        
        response.raise_for_status()
        return PaymentResponse(message="Payment processed successfully")
    
    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()


class HttpxHttpClient:
    """HTTP client adapter for httpx with connection pooling."""
    
    def __init__(self):
        self.client = httpx.AsyncClient(
            timeout=0.5,
            limits=httpx.Limits(max_connections=20, max_keepalive_connections=10)
        )
    
    async def get(self, url: str):
        """Make an HTTP GET request."""
        response = await self.client.get(url)
        return response
    
    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()
