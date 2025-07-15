from uuid import uuid4

import pytest
from fastapi.testclient import TestClient

from src.api import create_app
from src.models import PaymentRequest
from src.services import PaymentResponse


class MockPaymentProcessor:
    def __init__(self, falling: bool = False, id: str = "default-processor"):
        self.falling = falling
        self.id = id

    async def process_payment(self, payment_request: PaymentRequest) -> PaymentResponse:
        if self.falling:
            raise Exception("HTTP 500 - Internal Server Error")
        return PaymentResponse(message="payment processed successfully by " + self.id)


class MockPaymentStorage:
    def __init__(self):
        self.stored_payments = []

    async def store_payment(
        self, payment_request: PaymentRequest, processor_used: str, processed_at
    ) -> None:
        self.stored_payments.append(
            {
                "correlation_id": payment_request.correlationId,
                "amount": payment_request.amount,
                "processor_used": processor_used,
                "processed_at": processed_at,
            }
        )

    async def get_payments_summary(self, from_timestamp, to_timestamp) -> dict:
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


@pytest.fixture
def mock_processor():
    """Create a mock payment processor for testing"""
    return MockPaymentProcessor()


@pytest.fixture
def mock_storage():
    """Create a mock payment storage for testing"""
    return MockPaymentStorage()


@pytest.fixture
def mock_failing_processor():
    """Create a mock payment processor that always fails"""
    return MockPaymentProcessor(falling=True)


@pytest.fixture
def mock_fallback_processor():
    """Create a mock fallback payment processor that works"""
    return MockPaymentProcessor(id="fallback-processor")


@pytest.fixture
def client(mock_processor, mock_fallback_processor, mock_storage):
    """Create a test client with mock processor"""
    app = create_app(
        default_processor=mock_processor,
        fallback_processor=mock_fallback_processor,
        storage=mock_storage,
    )
    return TestClient(app)


@pytest.fixture
def valid_payment_data():
    """Create valid payment data for testing"""
    return {"correlationId": str(uuid4()), "amount": "19.90"}
