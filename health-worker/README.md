# Health Worker

A dedicated worker service for monitoring payment processor health in the Rinha de Backend 2025 challenge.

## Features

- **Processor Discovery**: Tracks multiple payment processors with `get_processors()` method
- **Health Monitoring**: Periodically checks processor health status
- **Rate Limiting**: Respects 5-second minimum interval between health checks per processor
- **Failure Tracking**: Monitors and tracks failure counts for each processor
- **Status Reporting**: Provides detailed health status information

## Structure

The main `HealthWorker` struct provides:

- `get_processors()` - Returns all configured processors
- `get_processor(name)` - Gets a specific processor by name
- `check_processor_health(name)` - Checks health of a specific processor
- `update_processor_health(name)` - Updates health status for a processor
- `update_all_processors_health()` - Updates health for all processors
- `get_healthy_processors()` - Returns list of healthy processors
- `get_unhealthy_processors()` - Returns list of unhealthy processors
- `run_health_monitoring(interval)` - Runs continuous health monitoring

## Usage

```rust
let mut health_worker = HealthWorker::new();

// Get all processors
let processors = health_worker.get_processors();

// Update health for all processors
health_worker.update_all_processors_health().await?;

// Run continuous monitoring
health_worker.run_health_monitoring(10).await?; // Check every 10 seconds
```

## Configuration

The worker is preconfigured with:
- Default processor: `http://payment-processor-default:8080`
- Fallback processor: `http://payment-processor-fallback:8080`

## Health Check Endpoint

Monitors the `/payments/service-health` endpoint on each processor, expecting:
```json
{
  "failing": false,
  "minResponseTime": 150
}
```