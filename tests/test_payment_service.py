from decimal import Decimal
from uuid import uuid4

import pytest

from src.models import PaymentRequest
from src.services import PaymentService


def test_payment_service_can_be_instantiated(
    mock_processor, mock_fallback_processor, mock_storage
):
    """Test that PaymentService can be instantiated"""
    service = PaymentService(
        default_processor=mock_processor,
        fallback_processor=mock_fallback_processor,
        storage=mock_storage,
    )

    assert service is not None
    assert isinstance(service, PaymentService)


@pytest.mark.asyncio
async def test_payment_service_routes_payment_to_default_processor(
    mock_processor, mock_fallback_processor, mock_storage
):
    """Test that PaymentService routes payment to default processor successfully"""
    # Arrange
    service = PaymentService(
        default_processor=mock_processor,
        fallback_processor=mock_fallback_processor,
        storage=mock_storage,
    )
    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)

    # Assert
    assert result is not None
    assert result.message == "payment processed successfully by default-processor"


@pytest.mark.asyncio
async def test_payment_service_routes_to_fallback_when_default_fails(
    mock_failing_processor, mock_fallback_processor, mock_storage
):
    """Test that PaymentService routes to fallback processor when default fails"""
    # Arrange
    service = PaymentService(
        default_processor=mock_failing_processor,
        fallback_processor=mock_fallback_processor,
        storage=mock_storage,
    )

    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)

    # Assert
    assert result is not None
    assert result.message == "payment processed successfully by fallback-processor"


@pytest.mark.asyncio
async def test_payment_service_stores_successful_payment(
    mock_processor, mock_fallback_processor, mock_storage
):
    """Test that PaymentService stores payment records when processing succeeds"""
    # Arrange
    service = PaymentService(
        default_processor=mock_processor,
        fallback_processor=mock_fallback_processor,
        storage=mock_storage,
    )

    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)

    # Assert
    assert result is not None
    assert len(mock_storage.stored_payments) == 1

    stored_payment = mock_storage.stored_payments[0]
    assert stored_payment["correlation_id"] == payment_request.correlationId
    assert stored_payment["amount"] == payment_request.amount
    assert stored_payment["processor_used"] == "default"
    assert stored_payment["processed_at"] is not None


@pytest.mark.asyncio
async def test_payment_service_stores_fallback_payment(
    mock_failing_processor, mock_fallback_processor, mock_storage
):
    """Test that PaymentService stores payment records when using fallback processor"""
    # Arrange
    service = PaymentService(
        default_processor=mock_failing_processor,
        fallback_processor=mock_fallback_processor,
        storage=mock_storage,
    )
    
    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))
    
    # Act
    result = await service.process_payment(payment_request)
    
    # Assert
    assert result is not None
    assert len(mock_storage.stored_payments) == 1
    
    stored_payment = mock_storage.stored_payments[0]
    assert stored_payment["correlation_id"] == payment_request.correlationId
    assert stored_payment["amount"] == payment_request.amount
    assert stored_payment["processor_used"] == "fallback"
    assert stored_payment["processed_at"] is not None
