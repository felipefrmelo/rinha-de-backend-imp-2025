import asyncio
from datetime import datetime, timezone
from decimal import Decimal
import logging

from src.domain.models import PaymentRequest

logger = logging.getLogger(__name__)

# Custom exceptions
class StorageError(Exception):
    """Raised when storage operations fail."""
    pass

class StorageConnectionError(StorageError):
    """Raised when storage connection fails."""
    pass


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
        """Store a payment request in Redis synchronously for consistency."""
        try:
            logger.debug(f"Storing payment {payment_request.correlationId} processed by {processor_used}")
            await self.redis_client.hset(
                f"payment:{payment_request.correlationId}",
                mapping={
                    "amount": str(payment_request.amount),
                    "processor_used": processor_used,
                    "processed_at": processed_at.isoformat(),
                }
            )
            logger.debug(f"Payment {payment_request.correlationId} stored successfully")
        except Exception as e:
            logger.error(f"Storage error for payment {payment_request.correlationId}: {e}")
            raise StorageError(f"Failed to store payment {payment_request.correlationId}: {e}") from e
    
    
    async def get_payments_summary(
        self, from_timestamp: datetime, to_timestamp: datetime
    ) -> dict:
        """Get payment summary grouped by processor type."""
        # Ensure timestamps are timezone-aware (assume UTC if naive)
        if from_timestamp.tzinfo is None:
            from_timestamp = from_timestamp.replace(tzinfo=timezone.utc)
        if to_timestamp.tzinfo is None:
            to_timestamp = to_timestamp.replace(tzinfo=timezone.utc)
            
        try:
            logger.debug(f"Retrieving payments summary from {from_timestamp} to {to_timestamp}")
            
            # Use optimized Redis pipeline for batch operations
            pipeline = self.redis_client.pipeline()
            
            # Scan for payment keys
            payment_keys = []
            cursor = 0
            while True:
                cursor, keys = await self.redis_client.scan(cursor, match="payment:*", count=1000)
                payment_keys.extend(keys)
                if cursor == 0:
                    break
            
            logger.debug(f"Found {len(payment_keys)} payment keys")
            
            # Batch fetch all payment data using pipeline
            for key in payment_keys:
                pipeline.hgetall(key)
            
            payment_data_list = await pipeline.execute()
            
        except Exception as e:
            logger.error(f"Failed to retrieve payment data from Redis: {e}")
            raise StorageConnectionError(f"Redis connection error: {e}") from e
        
        default_count = 0
        default_amount = Decimal('0')
        fallback_count = 0
        fallback_amount = Decimal('0')
        
        # Process each payment
        for payment_data in payment_data_list:
            if not payment_data:
                continue
                
            try:
                processed_at = datetime.fromisoformat(payment_data["processed_at"])
                
                # Ensure processed_at is timezone-aware (assume UTC if naive)
                if processed_at.tzinfo is None:
                    processed_at = processed_at.replace(tzinfo=timezone.utc)
                
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
            except (KeyError, ValueError):
                # Skip malformed data
                continue
        
        result = {
            "default": {
                "totalRequests": default_count,
                "totalAmount": float(default_amount)
            },
            "fallback": {
                "totalRequests": fallback_count,
                "totalAmount": float(fallback_amount)
            }
        }
        
        logger.debug(f"Payments summary result: {result}")
        return result
