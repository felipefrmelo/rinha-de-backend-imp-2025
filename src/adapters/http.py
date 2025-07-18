from datetime import datetime, timezone
import logging

import httpx

from src.domain.protocols import PaymentProcessor, PaymentResponse, HttpClient
from src.domain.models import PaymentRequest

logger = logging.getLogger(__name__)

# Custom exceptions
class PaymentProcessorError(Exception):
    """Raised when payment processor communication fails."""
    pass

class PaymentProcessorTimeoutError(PaymentProcessorError):
    """Raised when payment processor times out."""
    pass


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
        
        try:
            logger.debug(f"Sending payment {payment_request.correlationId} to {self.base_url}")
            response = await self.client.post(
                f"{self.base_url}/payments",
                json=request_data,
            )
            
            if response.status_code >= 500:
                logger.error(f"Payment processor server error for {payment_request.correlationId}: {response.status_code} - {response.text}")
                raise PaymentProcessorError(f"Payment processor server error: {response.status_code}")
            
            response.raise_for_status()
            logger.debug(f"Payment {payment_request.correlationId} processed successfully by {self.base_url}")
            return PaymentResponse(message="Payment processed successfully")
            
        except httpx.TimeoutException as e:
            logger.error(f"Payment processor timeout for {payment_request.correlationId}: {e}")
            raise PaymentProcessorTimeoutError(f"Payment processor timeout: {e}") from e
        except httpx.HTTPStatusError as e:
            logger.error(f"Payment processor HTTP error for {payment_request.correlationId}: {e}")
            raise PaymentProcessorError(f"Payment processor HTTP error: {e}") from e
        except httpx.RequestError as e:
            logger.error(f"Payment processor request error for {payment_request.correlationId}: {e}")
            raise PaymentProcessorError(f"Payment processor request error: {e}") from e
    
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
        try:
            logger.debug(f"Making GET request to {url}")
            response = await self.client.get(url)
            logger.debug(f"GET request to {url} completed with status {response.status_code}")
            return response
        except httpx.TimeoutException as e:
            logger.error(f"Timeout for GET request to {url}: {e}")
            raise
        except httpx.RequestError as e:
            logger.error(f"Request error for GET request to {url}: {e}")
            raise
    
    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()
