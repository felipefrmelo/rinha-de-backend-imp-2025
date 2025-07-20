# Performance Baseline Report

Generated on: Sun Jul 20 09:07:46 AM -03 2025

## Test Results Summary

### Single Request Performance
- Single payment request latency: 16ms
- Payments summary query latency: 14ms

### Load Test Performance (100 concurrent requests)
- P99 latency: 11.49ms

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
- Current P99: 11.49ms
- Target P99: <10ms (for bonus points)
- Resource limits: 1.5 CPU, 350MB memory total
