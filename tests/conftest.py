from uuid import uuid4

from fastapi.params import Query
import pytest
from fastapi.testclient import TestClient

from src.api import create_app
from src.domain.models import PaymentRequest, PaymentResponse, HealthStatus
from src.domain.health_check import HealthCheckClient
from src.domain.queue_manager import QueueManager


class MockPaymentProcessor:
    def __init__(self, falling: bool = False, id: str = "default-processor"):
        self.falling = falling
        self.id = id

    async def process_payment(self, payment_request: PaymentRequest, processed_at) -> PaymentResponse:
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



class MockHealthCheckClient(HealthCheckClient):
    def __init__(self, health_status: HealthStatus | None = None):
        self.health_status = health_status or HealthStatus(failing=False, min_response_time=100)

    async def get_health_status(self, processor_name: str) -> HealthStatus:
        return self.health_status
    
    async def check_health(self) -> HealthStatus:
        return self.health_status
    
    async def start(self):
        pass
    
    async def stop(self):
        pass


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


class MockQueueClient:
    def __init__(self):
        self.queue_data = []

    async def enqueue(self, queue_name: str, message: dict) -> None:
        self.queue_data.append(message)

    async def dequeue(self, queue_name: str, timeout_ms: int = 1000) -> dict | None:
        if self.queue_data:
            return self.queue_data.pop(0)
        return None



@pytest.fixture
def client(payment_service_factory):
    """Create a test client with mock processor and queue manager"""
    payment_service = payment_service_factory()
    queue_manager = QueueManager(MockQueueClient())
    # Don't start background worker in tests to avoid interference
    app = create_app(
        payment_service=payment_service,
        queue_manager=queue_manager,
        background_worker=None,
    )
    return TestClient(app)


@pytest.fixture
def valid_payment_data():
    """Create valid payment data for testing"""
    return {"correlationId": str(uuid4()), "amount": 19.90}


@pytest.fixture
def mock_health_check_client():
    """Create a mock health check client for testing"""
    return MockHealthCheckClient()


@pytest.fixture
def mock_failing_health_check_client():
    """Create a mock health check client that reports failure"""
    return MockHealthCheckClient(HealthStatus(failing=True, min_response_time=5000))


def create_payment_service(
    mock_processor=None,
    mock_fallback_processor=None, 
    mock_storage=None,
    mock_health_check_client=None,
    mock_fallback_health_check_client=None
):
    """Factory function to create PaymentService with sensible defaults"""
    from src.domain.services import PaymentService, PaymentProvider
    
    # Use defaults if not provided
    processor = mock_processor or MockPaymentProcessor()
    fallback_processor = mock_fallback_processor or MockPaymentProcessor(id="fallback-processor")
    storage = mock_storage or MockPaymentStorage()
    health_check = mock_health_check_client or MockHealthCheckClient()
    fallback_health_check = mock_fallback_health_check_client or MockHealthCheckClient()
    
    # Create PaymentProvider objects
    default_provider = PaymentProvider(
        processor=processor,
        health_check=health_check,
        name="default"
    )
    
    fallback_provider = PaymentProvider(
        processor=fallback_processor,
        health_check=fallback_health_check,
        name="fallback"
    )
    
    return PaymentService(
        default=default_provider,
        fallback=fallback_provider,
        storage=storage,
    )


@pytest.fixture
def payment_service_factory():
    """Fixture that returns the payment service factory function"""
    return create_payment_service
