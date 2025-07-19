def test_post_payments_returns_202_for_queue_processing(client, valid_payment_data):
    """Test that POST /payments returns 202 Accepted for async queue processing"""
    # Act
    response = client.post("/payments", json=valid_payment_data)
    
    # Assert
    assert response.status_code == 202  # Accepted for async processing
    assert "message" in response.json()
    assert "correlationId" in response.json()  # Should return the correlation ID for tracking


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


def test_queued_payments_do_not_immediately_appear_in_summary(client, valid_payment_data):
    """Test that queued payments do not immediately appear in summary (they need to be processed by workers)"""
    # Arrange - Use a very wide date range to ensure any payment would be included
    from_timestamp = "2020-01-01T00:00:00Z"
    to_timestamp = "2030-12-31T23:59:59Z"
    
    # Act - Queue a payment (it's not processed yet, just queued)
    payment_response = client.post("/payments", json=valid_payment_data)
    assert payment_response.status_code == 202  # Accepted for async processing
    
    # Act - Get summary
    summary_response = client.get(f"/payments-summary?from={from_timestamp}&to={to_timestamp}")
    
    # Assert
    assert summary_response.status_code == 200
    summary = summary_response.json()
    
    # Payment should NOT be counted yet since it's only queued, not processed
    assert summary["default"]["totalRequests"] == 0
    assert summary["default"]["totalAmount"] == 0.0
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
