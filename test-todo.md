# Queue-Based Payment Processing Plan

## Architecture Overview
Transform the current synchronous payment processing into a high-performance async queue system.

## Key Components

### 1. **Queue Infrastructure**
- [ ] **Redis Streams**: Primary queue for payment requests
- [ ] **Priority Queues**: Default processor queue (high priority) + Fallback queue (low priority)
- [ ] **Dead Letter Queue**: Failed payments for retry/investigation
- [ ] **Status Tracking**: Real-time payment status updates

### 2. **API Changes**
- [ ] **POST /payments**: Return 202 Accepted immediately (~1-2ms response)
- [ ] **GET /payments/{correlationId}/status**: Check payment processing status
- [ ] **Maintain compatibility**: Keep existing `/payments-summary` endpoint

### 3. **Worker Pool System**
- [ ] **4-6 async workers** per app instance (8-12 total across 2 instances)
- [ ] **Processor-specific workers**: Dedicated workers for default/fallback
- [ ] **Dynamic scaling**: Scale workers based on queue depth
- [ ] **Health-aware routing**: Route to healthy processors first

### 4. **Performance Optimizations**
- [ ] **Batch processing**: Process 10-20 payments concurrently per worker
- [ ] **Connection pooling**: Persistent HTTP connections (200 max, 100 keepalive)
- [ ] **Circuit breaker**: Fast-fail for unhealthy processors
- [ ] **Predictive routing**: Route based on processor response time trends

### 5. **Implementation Steps - Simple First, Improve Later**
- [x] **QueueManager**: Redis-based queue operations ✅
- [x] **PaymentWorker**: Basic async payment processing workers ✅  
- [x] **Redis Client Adapter**: Implement actual Redis connection for QueueClient ✅
- [x] **API Update**: Make POST /payments use queue instead of direct processing ✅
- [x] **Simple Queue Integration**: Connect API → Queue → Worker (basic flow) ✅
- [ ] **Background Worker Loop**: Worker that continuously processes queue
- [ ] **Basic Status Endpoint**: Simple GET /payments/{id}/status endpoint

### 6. **Later Improvements (After Basic System Works)**
- [ ] **PaymentStatusTracker**: Track payment states (queued→processing→completed/failed)
- [ ] **WorkerPool**: Manage worker lifecycle and scaling
- [ ] **Batch processing**: Process 10-20 payments concurrently per worker
- [ ] **Connection pooling**: Persistent HTTP connections
- [ ] **Circuit breaker**: Fast-fail for unhealthy processors

## Expected Performance Impact
- **Response time**: 277ms → <5ms (immediate queue acceptance)
- **Throughput**: ~430 RPS → >2000 RPS
- **Success rate**: 43.2% → >95% (better processor selection)
- **p99 latency**: Achieve <11ms target for bonus scoring

## Queue Benefits
- **Resilience**: Survive processor outages without request loss
- **Scalability**: Handle traffic spikes by queuing
- **Optimization**: Batch process and route intelligently
- **Monitoring**: Clear visibility into processing pipeline

## Previous Implementation Status (Completed)

### Phase 1: Basic Payment Processing ✅
- [x] Payment Request Model validation
- [x] Payment Service Core with dependency injection
- [x] Payment Endpoints (POST /payments)

### Phase 2: Fallback Logic ✅
- [x] Health check client implementation with caching protocol
- [x] Fallback routing strategy (automatic failover)
- [x] Error handling for failed processors (HTTP 5xx)

### Phase 3: Tracking and Reporting ✅
- [x] Payment storage interface (PaymentStorage protocol)
- [x] Payment tracking integration in PaymentService
- [x] GET /payments-summary endpoint implementation
- [x] Date range filtering for summary

### Phase 4: Optimization and Resilience ✅
- [x] Smart routing logic (using health check data for routing decisions)
- [x] PaymentProvider refactoring (improved maintainability and extensibility)
- [x] Fee optimization routing (always prefer default processor for lower fees)

## Current Status: Phase 4 Complete - Performance Crisis Identified

### Code Refactoring Complete ✅
- ✅ **Domain-Driven Structure**: Separated business logic from implementation details
- ✅ **Clean Architecture**: Created domain/ and adapters/ folders with clear separation
- ✅ **Protocols in Domain**: Moved all interface definitions to src/domain/protocols.py
- ✅ **Business Logic**: Moved PaymentService to src/domain/services.py
- ✅ **Adapters**: Created dedicated adapters for storage, HTTP, and cache implementations
- ✅ **Dependency Injection**: Created factories.py for service configuration
- ✅ **Backwards Compatibility**: Maintained existing imports with deprecated src/services.py
- ✅ **All Tests Passing**: 29/29 tests pass after refactoring

## Current Status: Queue Foundation Complete ✅

### Queue Infrastructure Completed Today:
- ✅ **QueueManager**: Redis-based queue operations with proper async interface
- ✅ **PaymentWorker**: Basic async payment processing workers with error handling  
- ✅ **RedisQueueClient**: Real Redis Streams implementation (XADD/XREAD/XDEL)
- ✅ **API Integration**: POST /payments returns 202 Accepted for queue processing
- ✅ **Simple Queue Flow**: Full API → Queue → Worker integration working
- ✅ **Integration Tests**: Redis integration tests with proper setup/teardown
- ✅ **Unit Test Coverage**: 36/36 unit tests passing
- ✅ **TDD Implementation**: Strict Red-Green-Refactor cycle throughout

### Previous Implementation Status (Completed):
- ✅ **Async Refactoring**: Made all payment processing async
- ✅ **PaymentStorage Protocol**: Dependency injection ready for database
- ✅ **Payment Tracking**: Service now stores all payments with processor info
- ✅ **Fallback Storage**: Correctly tracks which processor was used
- ✅ **Integration**: API, Service, and Storage all working together
- ✅ **Smart Routing**: Health check based routing with dual processor monitoring
- ✅ **PaymentProvider Architecture**: Clean abstraction for processor + health check
- ✅ **Test Factory Pattern**: Maintainable tests that resist constructor changes
- ✅ **Fee Optimization**: Always prefer default processor (lower fees) when healthy
- ✅ **Resilience Logic**: Graceful handling when both processors fail



## Notes
- Following strict Red-Green-Refactor cycle
- Each test must fail before writing production code
- Minimal code to make tests pass
- Refactor only when tests are green
