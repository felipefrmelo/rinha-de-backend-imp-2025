from datetime import datetime

from src.domain.protocols import PaymentStorage
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
