import pytest
from fastapi.testclient import TestClient
from uuid import uuid4

from src.models import PaymentRequest
from src.services import PaymentResponse
from src.api import create_app


class MockPaymentProcessor:
    def process_payment(self, payment_request: PaymentRequest) -> PaymentResponse:
        return PaymentResponse(message="payment processed successfully")


@pytest.fixture
def mock_processor():
    """Create a mock payment processor for testing"""
    return MockPaymentProcessor()


@pytest.fixture
def client(mock_processor):
    """Create a test client with mock processor"""
    app = create_app(default_processor=mock_processor)
    return TestClient(app)


@pytest.fixture
def valid_payment_data():
    """Create valid payment data for testing"""
    return {
        "correlationId": str(uuid4()),
        "amount": "19.90"
    }