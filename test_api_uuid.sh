#!/bin/bash
# Start the API (assumes you run this in another terminal):
# cd api && cargo run
# Ensure postgres is running: docker-compose up -d postgres

# Test the /payments endpoint with a valid UUID
curl -X POST http://localhost:3000/payments \
  -H 'Content-Type: application/json' \
  -d '{"amount": 100.0, "correlationId": "124e4567-e89b-12d3-a456-426614174000"}'

