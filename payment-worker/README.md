# Payment Worker

A Rust-based payment message processor using PGMQ (PostgreSQL Message Queue).

## Structure

- `src/main.rs` - Main worker application
- `src/bin/send_test_message.rs` - Test utility to send messages
- `Dockerfile` - Container configuration
- `docker-compose.test.yml` - Isolated test environment

## Testing the Worker

### Option 1: Full Integration Test (Recommended)

Run the comprehensive test script:

```bash
chmod +x test-worker.sh
./test-worker.sh
```

This will:
1. Build the worker and test utilities
2. Start PostgreSQL with PGMQ
3. Send test messages
4. Start the worker
5. Verify message processing
6. Clean up automatically

### Option 2: Manual Testing

1. **Start the test database:**
   ```bash
   docker-compose -f docker-compose.test.yml up -d postgres-test
   ```

2. **Send test messages:**
   ```bash
   DATABASE_URL="postgres://postgres:password@localhost:5433/payments" cargo run --bin send_test_message
   ```

3. **Run the worker:**
   ```bash
   DATABASE_URL="postgres://postgres:password@localhost:5433/payments" cargo run --bin payment-worker
   ```

4. **Check processed messages:**
   ```bash
   docker-compose -f docker-compose.test.yml exec postgres-test psql -U postgres -d payments -c "SELECT * FROM pgmq.a_payment_queue;"
   ```

### Option 3: Test with Main System

Use the PGMQ test script with the main docker-compose:

```bash
chmod +x test-pgmq.sh
# Start main system first: docker-compose up -d
./test-pgmq.sh
```

## Message Format

The worker expects messages in this format:

```json
{
  "correlationId": "unique-id",
  "amount": 100.50,
  "requestedAt": "2024-01-01T12:00:00Z"
}
```

## Environment Variables

- `DATABASE_URL` - PostgreSQL connection string (default: `postgres://postgres:password@postgres:5432/payments`)

## Queue Details

- **Queue Name:** `payment_queue`
- **Visibility Timeout:** 30 seconds
- **Processing:** Messages are archived after successful processing
- **Error Handling:** Failed messages remain in queue for retry