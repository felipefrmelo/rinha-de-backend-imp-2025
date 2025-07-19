from src.domain.models import PaymentRequest
from src.domain.queue_manager import QueueManager
from src.domain.services import PaymentService


class PaymentWorker:
    def __init__(
        self, 
        queue_manager: QueueManager, 
        payment_service: PaymentService
    ) -> None:
        self.queue_manager = queue_manager
        self.payment_service = payment_service

    async def process_next_payment(self) -> bool:
        """Process the next payment from the queue. Returns True if processed, False if no payment or error."""
        payment_data = await self.queue_manager.get_next_payment()
        
        if payment_data is None:
            return False
            
        try:
            # Convert dict back to PaymentRequest (handle string serialization)
            from uuid import UUID
            from decimal import Decimal
            
            payment_request = PaymentRequest(
                correlationId=UUID(payment_data["correlationId"]),  # Convert string back to UUID
                amount=Decimal(payment_data["amount"])  # Convert string back to Decimal
            )
            
            await self.payment_service.process_payment(payment_request)
            return True
        except Exception:
            # Log error and return False to indicate failure
            return False