import asyncio
from decimal import Decimal
from uuid import uuid4

import pytest

from src.domain.models import PaymentRequest
from src.domain.services import PaymentService
from src.domain.health_check import HealthStatus


def test_payment_service_can_be_instantiated(payment_service_factory):
    """Test that PaymentService can be instantiated"""
    service = payment_service_factory()

    assert service is not None
    assert isinstance(service, PaymentService)


@pytest.mark.asyncio
async def test_payment_service_routes_payment_to_default_processor(
    mock_processor, payment_service_factory
):
    """Test that PaymentService routes payment to default processor successfully"""
    # Arrange
    service = payment_service_factory(mock_processor=mock_processor)
    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)

    # Assert
    assert result is not None
    assert result.message == "payment processed successfully by default-processor"


@pytest.mark.asyncio
async def test_payment_service_routes_to_fallback_when_default_fails(
    mock_failing_processor, mock_fallback_processor, payment_service_factory
):
    """Test that PaymentService routes to fallback processor when default fails"""
    # Arrange
    service = payment_service_factory(
        mock_processor=mock_failing_processor,
        mock_fallback_processor=mock_fallback_processor
    )

    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)

    # Assert
    assert result is not None
    assert result.message == "payment processed successfully by fallback-processor"


@pytest.mark.asyncio
async def test_payment_service_stores_successful_payment(
    mock_processor, mock_storage, payment_service_factory
):
    """Test that PaymentService stores payment records when processing succeeds"""
    # Arrange
    service = payment_service_factory(
        mock_processor=mock_processor,
        mock_storage=mock_storage
    )

    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)
    
    # Allow fire-and-forget storage task to complete
    await asyncio.sleep(0.01)

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
    mock_failing_processor, mock_fallback_processor, mock_storage, payment_service_factory
):
    """Test that PaymentService stores payment records when using fallback processor"""
    # Arrange
    service = payment_service_factory(
        mock_processor=mock_failing_processor,
        mock_fallback_processor=mock_fallback_processor,
        mock_storage=mock_storage
    )

    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)
    
    # Allow fire-and-forget storage task to complete
    await asyncio.sleep(0.01)

    # Assert
    assert result is not None
    assert len(mock_storage.stored_payments) == 1

    stored_payment = mock_storage.stored_payments[0]
    assert stored_payment["correlation_id"] == payment_request.correlationId
    assert stored_payment["amount"] == payment_request.amount
    assert stored_payment["processor_used"] == "fallback"
    assert stored_payment["processed_at"] is not None


@pytest.mark.asyncio
async def test_payment_service_routes_to_fallback_when_health_check_shows_default_failing(
    mock_processor, mock_fallback_processor, mock_storage, mock_failing_health_check_client, payment_service_factory
):
    """Test that PaymentService routes to fallback when health check shows default is failing"""
    # Arrange
    service = payment_service_factory(
        mock_processor=mock_processor,
        mock_fallback_processor=mock_fallback_processor,
        mock_storage=mock_storage,
        mock_health_check_client=mock_failing_health_check_client
    )

    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)
    
    # Allow fire-and-forget storage task to complete
    await asyncio.sleep(0.01)

    # Assert - should route to fallback due to health check
    assert result.message == "payment processed successfully by fallback-processor"

    # Verify storage shows fallback was used
    stored_payment = mock_storage.stored_payments[0]
    assert stored_payment["processor_used"] == "fallback"


@pytest.mark.asyncio
async def test_payment_service_fails_when_both_processors_unhealthy(
    mock_processor, mock_fallback_processor, mock_storage, payment_service_factory
):
    """Test that PaymentService fails gracefully when both processors are unhealthy"""
    from tests.conftest import MockHealthCheckClient
    
    # Arrange - both health checks report failing
    failing_default_health = MockHealthCheckClient(HealthStatus(failing=True, min_response_time=5000))
    failing_fallback_health = MockHealthCheckClient(HealthStatus(failing=True, min_response_time=5000))
    
    service = payment_service_factory(
        mock_processor=mock_processor,
        mock_fallback_processor=mock_fallback_processor,
        mock_storage=mock_storage,
        mock_health_check_client=failing_default_health,
        mock_fallback_health_check_client=failing_fallback_health
    )

    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act & Assert - should raise our custom exception when both processors are unhealthy
    with pytest.raises(Exception) as exc_info:
        await service.process_payment(payment_request)
    
    assert "All payment processors are currently unavailable or too slow" in str(exc_info.value)


@pytest.mark.asyncio
async def test_payment_service_optimizes_for_lowest_fees_by_preferring_default(
    mock_processor, mock_fallback_processor, mock_storage, payment_service_factory
):
    """Test that PaymentService optimizes for lowest fees by preferring default processor"""
    from tests.conftest import MockHealthCheckClient
    
    # Arrange - both processors are healthy (default has lower fees per requirements)
    default_health = MockHealthCheckClient(HealthStatus(failing=False, min_response_time=100))
    fallback_health = MockHealthCheckClient(HealthStatus(failing=False, min_response_time=50))  # Even with better performance
    
    service = payment_service_factory(
        mock_processor=mock_processor,
        mock_fallback_processor=mock_fallback_processor,
        mock_storage=mock_storage,
        mock_health_check_client=default_health,
        mock_fallback_health_check_client=fallback_health,
    )

    payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("19.90"))

    # Act
    result = await service.process_payment(payment_request)
    
    # Allow fire-and-forget storage task to complete
    await asyncio.sleep(0.01)

    # Assert - should prefer default processor (lower fees) even if fallback has better performance
    assert result.message == "payment processed successfully by default-processor"
    
    # Verify storage shows default was used (fee optimization over performance optimization)
    stored_payment = mock_storage.stored_payments[0]
    assert stored_payment["processor_used"] == "default"


@pytest.mark.asyncio  
async def test_payment_service_maintains_fee_optimization_under_load(
    mock_processor, mock_fallback_processor, mock_storage, payment_service_factory
):
    """Test that PaymentService maintains fee optimization across multiple requests"""
    from tests.conftest import MockHealthCheckClient
    
    # Arrange - both processors healthy, default has lower fees
    default_health = MockHealthCheckClient(HealthStatus(failing=False, min_response_time=100))
    fallback_health = MockHealthCheckClient(HealthStatus(failing=False, min_response_time=80))
    
    service = payment_service_factory(
        mock_processor=mock_processor,
        mock_fallback_processor=mock_fallback_processor,
        mock_storage=mock_storage,
        mock_health_check_client=default_health,
        mock_fallback_health_check_client=fallback_health,
    )

    # Act - process multiple payments
    for i in range(5):
        payment_request = PaymentRequest(correlationId=uuid4(), amount=Decimal("10.00"))
        result = await service.process_payment(payment_request)
        assert result.message == "payment processed successfully by default-processor"

    # Allow fire-and-forget storage tasks to complete
    await asyncio.sleep(0.05)

    # Assert - all payments should use default processor for fee optimization
    assert len(mock_storage.stored_payments) == 5
    for payment in mock_storage.stored_payments:
        assert payment["processor_used"] == "default"
