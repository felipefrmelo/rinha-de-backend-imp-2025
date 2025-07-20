#!/bin/bash

# Performance profiling script for Rinha de Backend 2025
set -e

echo "🔍 Starting performance profiling analysis..."

# Install profiling tools if not present
echo "📦 Installing profiling tools..."
cargo install --quiet flamegraph 2>/dev/null || echo "flamegraph already installed"
cargo install --quiet tokio-console 2>/dev/null || echo "tokio-console already installed"

# Clean and rebuild with profiling enabled
echo "🧹 Cleaning and rebuilding with profiling enabled..."
docker compose down --volumes --remove-orphans 2>/dev/null || true

echo "📦 Building containers with tracing enabled..."
docker compose build --quiet

# Start payment processors first
echo "🚀 Starting payment processors..."
cd payment-processor
docker compose up -d --quiet-pull
cd ..

# Wait for processors to be ready
echo "⏳ Waiting for payment processors..."
sleep 10

# Check processor health
for i in {1..10}; do
    if curl -f http://localhost:8001/payments/service-health >/dev/null 2>&1 && \
       curl -f http://localhost:8002/payments/service-health >/dev/null 2>&1; then
        echo "✅ Payment processors are healthy!"
        break
    fi
    if [ $i -eq 10 ]; then
        echo "❌ Payment processors failed to start"
        exit 1
    fi
    echo "⏳ Attempt $i/10 - waiting for processors..."
    sleep 2
done

# Start backend with tracing
echo "🚀 Starting backend with tracing enabled..."
RUST_LOG=debug docker compose up -d

# Wait for backend to be ready
echo "⏳ Waiting for backend..."
sleep 10

for i in {1..10}; do
    if curl -f http://localhost:9999/health >/dev/null 2>&1; then
        echo "✅ Backend is healthy!"
        break
    fi
    if [ $i -eq 10 ]; then
        echo "❌ Backend failed to start"
        exit 1
    fi
    echo "⏳ Attempt $i/10 - waiting for backend..."
    sleep 2
done

# Create profiling results directory
mkdir -p profiling-results
cd profiling-results

echo "📊 Starting performance baseline measurement..."

# Test 1: Single request latency baseline
echo "🧪 Test 1: Single request latency baseline"
echo "Testing single payment request..."
START_TIME=$(date +%s%N)
curl -s -X POST http://localhost:9999/payments \
  -H "Content-Type: application/json" \
  -d '{
    "correlationId": "123e4567-e89b-12d3-a456-426614174000",
    "amount": 100.50
  }' > single_request_response.json
END_TIME=$(date +%s%N)
SINGLE_LATENCY=$(( (END_TIME - START_TIME) / 1000000 ))
echo "Single request latency: ${SINGLE_LATENCY}ms"

# Test 2: Database query performance
echo "🧪 Test 2: Database query performance"
echo "Testing payments summary query..."
START_TIME=$(date +%s%N)
curl -s "http://localhost:9999/payments-summary" > summary_response.json
END_TIME=$(date +%s%N)
SUMMARY_LATENCY=$(( (END_TIME - START_TIME) / 1000000 ))
echo "Summary query latency: ${SUMMARY_LATENCY}ms"

# Test 3: Concurrent load test
echo "🧪 Test 3: Load test with profiling"
cd ../rinha-test

# Set lower load for profiling run
export MAX_REQUESTS=100
export RUST_LOG=info

echo "Running k6 load test with 100 concurrent requests..."
k6 run --quiet rinha.js > ../profiling-results/load_test_results.txt 2>&1

cd ../profiling-results

# Extract p99 from k6 results
P99_LATENCY=$(grep -o 'p(99).*[0-9]*\.[0-9]*ms' load_test_results.txt | head -1 | grep -o '[0-9]*\.[0-9]*ms' || echo "N/A")
echo "Load test P99 latency: $P99_LATENCY"

# Test 4: Resource utilization
echo "🧪 Test 4: Resource utilization analysis"
echo "Collecting Docker stats..."

# Get container stats
docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}" > container_stats.txt

# Test 5: Database performance
echo "🧪 Test 5: Database performance analysis"
echo "Analyzing database connections and queries..."

# Connect to postgres and run some analysis queries
docker exec rinha-de-backend-imp-2025-postgres-1 psql -U postgres -d payments -c "
SELECT 
    schemaname,
    tablename,
    attname,
    n_distinct,
    correlation
FROM pg_stats 
WHERE tablename = 'processed_payments';
" > database_stats.txt 2>/dev/null || echo "Could not analyze database stats"

# Check active connections
docker exec rinha-de-backend-imp-2025-postgres-1 psql -U postgres -d payments -c "
SELECT state, count(*) 
FROM pg_stat_activity 
WHERE datname = 'payments' 
GROUP BY state;
" > connection_stats.txt 2>/dev/null || echo "Could not analyze connection stats"

# Generate performance report
echo "📋 Generating performance baseline report..."

cat > performance_baseline_report.md << EOF
# Performance Baseline Report

Generated on: $(date)

## Test Results Summary

### Single Request Performance
- Single payment request latency: ${SINGLE_LATENCY}ms
- Payments summary query latency: ${SUMMARY_LATENCY}ms

### Load Test Performance (100 concurrent requests)
- P99 latency: $P99_LATENCY

### Resource Utilization
See container_stats.txt for detailed container resource usage.

### Database Performance
See database_stats.txt and connection_stats.txt for database analysis.

## Files Generated
- single_request_response.json: Sample payment response
- summary_response.json: Sample summary response  
- load_test_results.txt: Full k6 load test output
- container_stats.txt: Docker container resource stats
- database_stats.txt: Database table statistics
- connection_stats.txt: Database connection analysis

## Next Steps
Based on this baseline:
1. If P99 > 50ms: Focus on request processing optimization
2. If single request > 10ms: Investigate database/queue performance
3. If high CPU usage: Profile CPU-intensive operations
4. If high memory usage: Investigate memory leaks/allocations

## Performance Targets
- Current P99: $P99_LATENCY
- Target P99: <10ms (for bonus points)
- Resource limits: 1.5 CPU, 350MB memory total
EOF

echo "✅ Performance profiling complete!"
echo "📋 Results saved in profiling-results/ directory"
echo "📄 See performance_baseline_report.md for summary"

# Show quick summary
echo ""
echo "🎯 PERFORMANCE SUMMARY:"
echo "  Single request: ${SINGLE_LATENCY}ms"
echo "  Summary query: ${SUMMARY_LATENCY}ms"
echo "  Load test P99: $P99_LATENCY"
echo ""
echo "💡 Next: Analyze bottlenecks in profiling-results/ directory"