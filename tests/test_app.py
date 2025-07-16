import pytest
from fastapi.testclient import TestClient


def test_fastapi_app_can_be_created():
    """Test that the main FastAPI application can be created and started."""
    from main import app
    
    client = TestClient(app)
    # Basic health check - the app should be able to serve requests
    response = client.get("/health")
    assert response.status_code == 200


def test_fastapi_app_includes_payment_routes():
    """Test that the main app includes the payment processing routes."""
    from main import app
    
    client = TestClient(app)
    # Should have our payment endpoints
    response = client.get("/payments-summary")
    assert response.status_code == 200
