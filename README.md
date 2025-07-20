# Rinha de Backend 2025 - High-Performance Rust Implementation

This is a high-performance payment intermediation service implemented in Rust for the Rinha de Backend 2025 challenge.

## Technologies Used

- **Language**: Rust (2021 edition)
- **Web Framework**: Axum (lightweight, fast async web framework)
- **HTTP Client**: Reqwest with connection pooling
- **Load Balancer**: Nginx with optimized configuration
- **Concurrency**: Tokio async runtime
- **Data Storage**: In-memory with atomic operations
- **Containerization**: Docker with multi-stage builds

## Performance Optimizations

### Application Level
- **Async Processing**: Full async/await pipeline for non-blocking I/O
- **Connection Pooling**: HTTP client with persistent connections and TCP keepalive
- **Smart Health Checking**: Background health monitoring with rate limiting respect
- **Lock-Free Data Structures**: Atomic operations for counters, avoiding mutex overhead
- **Memory Efficiency**: Minimal allocations, compile-time optimizations
- **Circuit Breaker Pattern**: Fast failover between payment processors

### Configuration Optimizations
- **Rust Compiler**: LTO, single codegen unit, panic=abort for optimal binary size
- **Network Stack**: TCP_NODELAY enabled, optimized timeouts
- **Nginx**: Optimized worker connections, keepalive, compression settings
- **Docker**: Multi-stage builds for minimal image size

### Resource Allocation
- **nginx**: 0.2 CPU, 50MB RAM - Load balancing and request routing
- **api1**: 0.6 CPU, 150MB RAM - Primary application instance  
- **api2**: 0.6 CPU, 150MB RAM - Secondary application instance
- **Total**: 1.4 CPU, 350MB RAM (within 1.5 CPU, 350MB limits)

## Architecture

```
Internet → nginx:9999 → {api1:3000, api2:3000} → Payment Processors
```

The service intelligently routes payments to the default processor (lower fees) when healthy, falling back to the fallback processor when needed. Health checks run in background tasks to avoid blocking payment processing.

## Performance Targets

- **Target p99 Latency**: < 10ms for performance bonus
- **Payment Routing**: Preference for default processor (lower fees)
- **Failover**: Fast automatic failover with minimal impact
- **Consistency**: Accurate payment tracking for audit compliance

## Source Code

Repository: https://github.com/felipefrmelo/rinha-de-backend-imp-2025

## Running

```bash
# Start payment processors first
cd payment-processor
docker-compose up -d

# Start the backend
docker-compose up -d
```

The service will be available at http://localhost:9999