import pytest
from uuid import uuid4
from decimal import Decimal
from pydantic import ValidationError
from src.domain.models import PaymentRequest

def test_payment_request_accepts_valid_correlation_id_and_amount():
    """Test that PaymentRequest accepts valid correlationId and amount"""
    
    correlation_id = uuid4()
    amount = Decimal("19.90")
    
    payment_request = PaymentRequest(
        correlationId=correlation_id,
        amount=amount
    )
    
    assert payment_request.correlationId == correlation_id
    assert payment_request.amount == amount


def test_payment_request_rejects_missing_correlation_id():
    """Test that PaymentRequest rejects missing correlationId"""
    
    amount = Decimal("19.90")
    
    with pytest.raises(ValidationError) as exc_info:
        PaymentRequest(amount=amount)
    
    error = exc_info.value
    assert "correlationId" in str(error)


def test_payment_request_rejects_missing_amount():
    """Test that PaymentRequest rejects missing amount"""
    
    correlation_id = str(uuid4())
    
    with pytest.raises(ValidationError) as exc_info:
        PaymentRequest(correlationId=correlation_id)
    
    error = exc_info.value
    assert "amount" in str(error)


def test_payment_request_rejects_invalid_uuid_format():
    """Test that PaymentRequest rejects invalid UUID format for correlationId"""
    
    invalid_correlation_id = "not-a-uuid"
    amount = Decimal("19.90")
    
    with pytest.raises(ValidationError) as exc_info:
        PaymentRequest(
            correlationId=invalid_correlation_id,
            amount=amount
        )
    
    error = exc_info.value
    assert "correlationId" in str(error) or "uuid" in str(error).lower()


def test_payment_request_rejects_negative_amount():
    """Test that PaymentRequest rejects negative amount"""
    
    correlation_id = uuid4()
    negative_amount = Decimal("-10.00")
    
    with pytest.raises(ValidationError) as exc_info:
        PaymentRequest(
            correlationId=correlation_id,
            amount=negative_amount
        )
    
    error = exc_info.value
    assert "amount" in str(error)


def test_payment_request_rejects_zero_amount():
    """Test that PaymentRequest rejects zero amount"""
    
    correlation_id = uuid4()
    zero_amount = Decimal("0.00")
    
    with pytest.raises(ValidationError) as exc_info:
        PaymentRequest(
            correlationId=correlation_id,
            amount=zero_amount
        )
    
    error = exc_info.value
    assert "amount" in str(error)
