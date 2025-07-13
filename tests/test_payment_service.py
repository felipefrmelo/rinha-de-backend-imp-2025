from decimal import Decimal
from uuid import uuid4

from src.models import PaymentRequest
from src.services import PaymentService


def test_payment_service_can_be_instantiated(mock_processor):
    """Test that PaymentService can be instantiated"""
    service = PaymentService(default_processor=mock_processor)

    assert service is not None
    assert isinstance(service, PaymentService)


def test_payment_service_routes_payment_to_default_processor(mock_processor):
    """Test that PaymentService routes payment to default processor successfully"""
    # Arrange
    service = PaymentService(default_processor=mock_processor)
    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = service.process_payment(payment_request)

    # Assert
    assert result is not None
    assert result.message == "payment processed successfully"
