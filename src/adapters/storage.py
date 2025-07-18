import asyncio
from datetime import datetime, timezone
from decimal import Decimal

from src.domain.models import PaymentRequest


class InMemoryPaymentStorage:
    """In-memory implementation of PaymentStorage for development/testing."""
    
    def __init__(self):
        self.stored_payments = []
    
    async def store_payment(
        self,
        payment_request: PaymentRequest,
        processor_used: str,
        processed_at: datetime,
    ) -> None:
        """Store a payment request."""
        self.stored_payments.append({
            "correlation_id": payment_request.correlationId,
            "amount": payment_request.amount,
            "processor_used": processor_used,
            "processed_at": processed_at,
        })
    
    async def get_payments_summary(
        self, from_timestamp: datetime, to_timestamp: datetime
    ) -> dict:
        """Get payment summary grouped by processor type."""
        # Filter payments by timestamp range
        filtered_payments = [
            p for p in self.stored_payments
            if from_timestamp <= p["processed_at"] <= to_timestamp
        ]
        
        # Group by processor type
        default_payments = [p for p in filtered_payments if p["processor_used"] == "default"]
        fallback_payments = [p for p in filtered_payments if p["processor_used"] == "fallback"]
        
        return {
            "default": {
                "totalRequests": len(default_payments),
                "totalAmount": sum(p["amount"] for p in default_payments)
            },
            "fallback": {
                "totalRequests": len(fallback_payments),
                "totalAmount": sum(p["amount"] for p in fallback_payments)
            }
        }


class RedisPaymentStorage:
    """Redis-based implementation of PaymentStorage for production."""
    
    def __init__(self, redis_client):
        self.redis_client = redis_client
    
    async def store_payment(
        self,
        payment_request: PaymentRequest,
        processor_used: str,
        processed_at: datetime,
    ) -> None:
        """Store a payment request in Redis using fire-and-forget pattern."""
        # Create background task for storage - don't await it
        asyncio.create_task(
            self._store_payment_background(payment_request, processor_used, processed_at)
        )
    
    async def _store_payment_background(
        self,
        payment_request: PaymentRequest,
        processor_used: str,
        processed_at: datetime,
    ) -> None:
        """Background task to store payment in Redis."""
        try:
            await self.redis_client.hset(
                f"payment:{payment_request.correlationId}",
                mapping={
                    "amount": str(payment_request.amount),
                    "processor_used": processor_used,
                    "processed_at": processed_at.isoformat(),
                }
            )
        except Exception as e:
            # Log error but don't propagate to main request flow
            # In production, you'd want proper logging here
            print(f"Background storage error: {e}")
    
    async def get_payments_summary(
        self, from_timestamp: datetime, to_timestamp: datetime
    ) -> dict:
        """Get payment summary grouped by processor type."""
        # Ensure timestamps are timezone-aware (assume UTC if naive)
        if from_timestamp.tzinfo is None:
            from_timestamp = from_timestamp.replace(tzinfo=timezone.utc)
        if to_timestamp.tzinfo is None:
            to_timestamp = to_timestamp.replace(tzinfo=timezone.utc)
            
        # Use SCAN instead of KEYS for better performance in production
        # KEYS blocks Redis but SCAN doesn't
        payment_keys = []
        cursor = 0
        while True:
            cursor, keys = await self.redis_client.scan(cursor, match="payment:*", count=100)
            payment_keys.extend(keys)
            if cursor == 0:
                break
        
        default_count = 0
        default_amount = Decimal('0')
        fallback_count = 0
        fallback_amount = Decimal('0')
        
        # Process each payment
        for key in payment_keys:
            payment_data = await self.redis_client.hgetall(key)
            if not payment_data:
                continue
                
            processed_at = datetime.fromisoformat(payment_data["processed_at"])
            
            # Check if payment is in time range
            if from_timestamp <= processed_at <= to_timestamp:
                amount = Decimal(payment_data["amount"])
                processor = payment_data["processor_used"]
                
                if processor == "default":
                    default_count += 1
                    default_amount += amount
                elif processor == "fallback":
                    fallback_count += 1
                    fallback_amount += amount
        
        return {
            "default": {
                "totalRequests": default_count,
                "totalAmount": float(default_amount)
            },
            "fallback": {
                "totalRequests": fallback_count,
                "totalAmount": float(fallback_amount)
            }
        }
