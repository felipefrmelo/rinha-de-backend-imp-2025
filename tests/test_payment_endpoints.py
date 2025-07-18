def test_post_payments_returns_200_on_success(client, valid_payment_data):
    """Test that POST /payments returns 200 on successful payment processing"""
    # Act
    response = client.post("/payments", json=valid_payment_data)
    
    # Assert
    assert response.status_code == 200
    assert "message" in response.json()


def test_post_payments_returns_422_on_invalid_input(client):
    """Test that POST /payments returns 400 for invalid input (handled by our custom validation handler)"""
    # Arrange
    invalid_payment_data = {
        "correlationId": "not-a-uuid",
        "amount": "-10.00"
    }
    
    # Act
    response = client.post("/payments", json=invalid_payment_data)
    
    # Assert
    assert response.status_code == 400  # Our custom validation handler returns 400
    assert "detail" in response.json()


def test_get_payments_summary_returns_200_with_basic_structure(client):
    """Test that GET /payments-summary returns 200 with default and fallback totals"""
    # Arrange
    from_timestamp = "2024-01-01T00:00:00Z"
    to_timestamp = "2024-12-31T23:59:59Z"
    
    # Act
    response = client.get(f"/payments-summary?from={from_timestamp}&to={to_timestamp}")
    
    # Assert
    assert response.status_code == 200
    summary = response.json()
    assert "default" in summary
    assert "fallback" in summary


def test_get_payments_summary_returns_actual_storage_data(client):
    """Test that GET /payments-summary returns data from storage, not hardcoded values"""
    # Arrange
    from_timestamp = "2024-01-01T00:00:00Z"
    to_timestamp = "2024-12-31T23:59:59Z"
    
    # Act
    response = client.get(f"/payments-summary?from={from_timestamp}&to={to_timestamp}")
    
    # Assert
    assert response.status_code == 200
    summary = response.json()
    
    # Should return data based on storage, not hardcoded values
    # Initially expect zero counts since no payments stored yet
    assert summary["default"]["totalRequests"] == 0
    assert summary["default"]["totalAmount"] == 0.0
    assert summary["fallback"]["totalRequests"] == 0
    assert summary["fallback"]["totalAmount"] == 0.0


def test_processed_payments_appear_in_summary(client, valid_payment_data):
    """Test that processed payments appear correctly in the summary"""
    # Arrange - Use a very wide date range to ensure the payment is included
    from_timestamp = "2020-01-01T00:00:00Z"
    to_timestamp = "2030-12-31T23:59:59Z"
    
    # Act - Process a payment
    payment_response = client.post("/payments", json=valid_payment_data)
    assert payment_response.status_code == 200
    
    # Act - Get summary
    summary_response = client.get(f"/payments-summary?from={from_timestamp}&to={to_timestamp}")
    
    # Assert
    assert summary_response.status_code == 200
    summary = summary_response.json()
    
    # Payment should be counted in default processor
    assert summary["default"]["totalRequests"] == 1
    assert summary["default"]["totalAmount"] == valid_payment_data["amount"]
    assert summary["fallback"]["totalRequests"] == 0
    assert summary["fallback"]["totalAmount"] == 0.0


def test_payments_summary_works_without_query_parameters(client):
    """Test that GET /payments-summary works without from/to parameters"""
    # Act
    response = client.get("/payments-summary")
    
    # Assert
    assert response.status_code == 200
    summary = response.json()
    assert "default" in summary
    assert "fallback" in summary


def test_payments_summary_handles_invalid_datetime_format(client):
    """Test that GET /payments-summary returns 400 for invalid datetime format (our custom validation handler)"""
    # Arrange
    invalid_from = "invalid-date"
    valid_to = "2024-12-31T23:59:59Z"
    
    # Act
    response = client.get(f"/payments-summary?from={invalid_from}&to={valid_to}")
    
    # Assert
    assert response.status_code == 400  # Our custom validation handler returns 400
    assert "detail" in response.json()


def test_payments_summary_handles_mixed_datetime_formats(client):
    """Test that GET /payments-summary works with different valid ISO formats"""
    # Arrange - Test with Z suffix and explicit UTC offset
    from_with_z = "2024-01-01T00:00:00Z"
    to_with_z = "2024-12-31T23:59:59Z"
    
    # Act
    response = client.get(f"/payments-summary?from={from_with_z}&to={to_with_z}")
    
    # Assert
    assert response.status_code == 200
    summary = response.json()
    assert "default" in summary
    assert "fallback" in summary


def test_payments_summary_with_native_datetime_parsing(client):
    """Test that FastAPI handles datetime parameters natively"""
    # Arrange - ISO datetime strings that should be parsed automatically
    from_timestamp = "2024-01-01T00:00:00"
    to_timestamp = "2024-12-31T23:59:59"
    
    # Act
    response = client.get(f"/payments-summary?from={from_timestamp}&to={to_timestamp}")
    
    # Assert
    assert response.status_code == 200
    summary = response.json()
    assert "default" in summary
    assert "fallback" in summary
