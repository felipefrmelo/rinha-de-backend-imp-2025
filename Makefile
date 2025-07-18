.PHONY: test-unit test-integration test-all setup-test-env cleanup-test-env

# Run unit tests (no external dependencies)
test-unit:
	uv run pytest -m "not integration" -v

# Run integration tests with Docker services
test-integration:
	docker compose -f docker-compose.test.yml up -d
	docker compose -f docker-compose.test.yml exec -T redis-test redis-cli ping || sleep 5
	REDIS_HOST=localhost uv run pytest -m integration -v
	docker compose -f docker-compose.test.yml down

# Run all tests
test-all: test-unit test-integration

# Setup test environment
setup-test-env:
	docker compose -f docker-compose.test.yml up -d
	@echo "Waiting for services to be ready..."
	sleep 5

# Cleanup test environment
cleanup-test-env:
	docker compose -f docker-compose.test.yml down -v
