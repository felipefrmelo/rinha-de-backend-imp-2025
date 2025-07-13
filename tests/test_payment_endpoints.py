def test_post_payments_returns_200_on_success(client, valid_payment_data):
    """Test that POST /payments returns 200 on successful payment processing"""
    # Act
    response = client.post("/payments", json=valid_payment_data)
    
    # Assert
    assert response.status_code == 200
    assert "message" in response.json()


def test_post_payments_returns_422_on_invalid_input(client):
    """Test that POST /payments returns 422 on invalid input"""
    # Arrange
    invalid_payment_data = {
        "correlationId": "not-a-uuid",
        "amount": "-10.00"
    }
    
    # Act
    response = client.post("/payments", json=invalid_payment_data)
    
    # Assert
    assert response.status_code == 422
    assert "detail" in response.json()
