# TDD Implementation Plan - Rinha de Backend 2025

## Project Overview
Building a payment processing intermediary service that routes payments between two payment processors (default and fallback) while optimizing for lowest fees.

## Core Features to Implement (in TDD order)

### Phase 1: Basic Payment Processing
1. **Payment Request Model**
   - Accept payment requests with correlationId and amount
   - Validate input format and required fields
   - Handle UUID validation for correlationId

2. **Payment Service Core**
   - Create PaymentService class
   - Route payments to default processor
   - Handle successful payment responses

3. **Payment Endpoints**
   - POST /payments endpoint
   - Return appropriate HTTP responses
   - Handle request/response serialization

### Phase 2: Fallback Logic
4. **Health Check Integration**
   - Implement health check client for payment processors
   - Respect rate limiting (1 call per 5 seconds)
   - Parse health check responses

5. **Fallback Strategy**
   - Route to fallback when default processor fails
   - Handle HTTP 5xx errors from default processor
   - Implement retry logic

### Phase 3: Tracking and Reporting
6. **Payment Storage**
   - Store payment requests and processor responses
   - Track which processor was used
   - Store timestamps and amounts

7. **Summary Endpoint**
   - GET /payments-summary endpoint
   - Filter by date range
   - Aggregate totals by processor type

### Phase 4: Optimization and Resilience
8. **Smart Routing**
   - Use health check data for routing decisions
   - Implement circuit breaker pattern
   - Optimize for lowest fees

9. **Error Handling**
   - Handle network timeouts
   - Implement proper error responses
   - Log errors appropriately

### Phase 5: Performance and Infrastructure
10. **Load Balancing Setup**
    - Configure multiple web server instances
    - Docker compose configuration
    - Resource constraints

## Test Categories
- **Unit Tests**: Individual component behavior
- **Integration Tests**: Component interactions
- **API Tests**: Endpoint functionality
- **Contract Tests**: Payment processor integration

## Success Criteria
- All endpoints working according to specification
- Proper fallback behavior when processors fail
- Accurate payment tracking and reporting
- Performance targets met (p99 < 11ms goal)
- All tests passing with good coverage