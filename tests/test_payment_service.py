import pytest


def test_payment_service_can_be_instantiated():
    """Test that PaymentService can be instantiated"""
    from src.services import PaymentService
    
    service = PaymentService()
    
    assert service is not None
    assert isinstance(service, PaymentService)