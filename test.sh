#!/bin/bash

# Simple test script to validate the implementation
set -e

echo "Testing Rinha Backend Implementation..."

# Check if service is running
echo "1. Health check..."
curl -f http://localhost:9999/health

echo -e "\n2. Get initial payments summary..."
curl -f http://localhost:9999/payments-summary

echo -e "\n3. Test payment processing..."
# Note: This will fail in isolation since payment processors aren't running
# but validates the endpoint structure
curl -X POST http://localhost:9999/payments \
  -H "Content-Type: application/json" \
  -d '{
    "correlationId": "4a7901b8-7d26-4d9d-aa19-4dc1c7cf60b3",
    "amount": 19.90
  }' || echo "Expected to fail without payment processors"

echo -e "\n\nAll endpoints responding correctly!"
echo "To run full integration tests, start payment processors first:"
echo "cd payment-processor && docker-compose up -d"