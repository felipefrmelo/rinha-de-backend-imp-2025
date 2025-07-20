#!/bin/bash
# Start the API (assumes you run this in another terminal):
# cd api && cargo run
# Ensure postgres is running: docker-compose up -d postgres

# Test the /payments endpoint with a valid UUID
# Generate a random UUID (use uuidgen if available, else fallback to Python)
if command -v uuidgen > /dev/null; then
  UUID=$(uuidgen)
else
  UUID=$(python3 -c 'import uuid; print(uuid.uuid4())')
fi

curl -X POST http://localhost:9999/payments \
  -H 'Content-Type: application/json' \
  -d '{"amount": 100.0, "correlationId": "'$UUID'"}'

#docker stats  --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.MemPerc}}" $(docker ps --filter "name=rinha" -q)
