from datetime import datetime, timezone

import httpx

from src.domain.protocols import PaymentProcessor, PaymentResponse, HttpClient
from src.domain.models import PaymentRequest


class HttpPaymentProcessor:
    """HTTP-based payment processor implementation."""
    
    def __init__(self, base_url: str):
        self.base_url = base_url
    
    async def process_payment(self, payment_request: PaymentRequest) -> PaymentResponse:
        """Process a payment request via HTTP."""
        async with httpx.AsyncClient() as client:
            # Add requestedAt timestamp as required by the payment processors
            request_data = {
                "correlationId": str(payment_request.correlationId),
                "amount": float(payment_request.amount),
                "requestedAt": datetime.now(timezone.utc).isoformat(),
            }
            
            response = await client.post(
                f"{self.base_url}/payments",
                json=request_data,
                timeout=10.0,
            )
            
            if response.status_code >= 500:
                raise Exception(f"HTTP {response.status_code} - {response.text}")
            
            response.raise_for_status()
            return PaymentResponse(message="Payment processed successfully")


class HttpxHttpClient:
    """HTTP client adapter for httpx."""
    
    def __init__(self):
        self.client = httpx.AsyncClient()
    
    async def get(self, url: str):
        """Make an HTTP GET request."""
        response = await self.client.get(url)
        return response
    
    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()
