use criterion::{black_box, criterion_group, criterion_main, Criterion};
use health_checker::{
    health_monitor::{ProcessorDefault, ProcessorFallback, ProcessorHealthStatus},
    HealthCheckerConfig, HealthMonitor, HealthStorage, Processor, RedisHealthStorage,
    ReqwestHttpClient,
};
use std::time::Duration;

fn make_health_config() -> HealthCheckerConfig {
    HealthCheckerConfig {
        redis_url: "redis://localhost:6379".to_string(),
        health_status_ttl: 60,
        rate_limit_ttl: 5,
        http_timeout: Duration::from_secs(10),
        health_check_cycle_interval: Duration::from_secs(30),
        inter_check_delay: Duration::from_millis(0),
        default_processor_url: "http://localhost:8000".to_string(),
        fallback_processor_url: "http://localhost:8001".to_string(),
        failed_response_time_value: 9999,
    }
}

fn make_healt_monitor() -> HealthMonitor {
    let config = make_health_config();

    let storage = Box::new(
        RedisHealthStorage::new(
            &config.redis_url,
            config.health_status_ttl,
            config.rate_limit_ttl,
        )
        .unwrap(),
    );

    let http_client = Box::new(ReqwestHttpClient::new(config.http_timeout).unwrap());

    let processors = vec![
        Processor::Default(ProcessorDefault::new(config.default_processor_url.clone())),
        Processor::Fallback(ProcessorFallback::new(
            config.fallback_processor_url.clone(),
        )),
    ];

    HealthMonitor::new(storage, http_client, config, processors).unwrap()
}

fn bench_health_check_endpoint(c: &mut Criterion) {
    c.bench_function("health_check_endpoint", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let monitor = make_healt_monitor();

                // Benchmark the get_best_processor method (main health check logic)
                let result = monitor.get_best_processor().await;
                let _ = black_box(result);
            })
        })
    });
}

fn bench_storage_operations(c: &mut Criterion) {
    c.bench_function("storage_health_status", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let storage = RedisHealthStorage::new("redis://localhost:6379", 60, 5).unwrap();

                let health_status = storage.get_processor_health("default").await;
                let _ = black_box(health_status);
            })
        })
    });
}

fn bench_processor_selection_logic(c: &mut Criterion) {
    c.bench_function("processor_selection", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let config = make_health_config();
                let storage = RedisHealthStorage::new(
                    &config.redis_url,
                    config.health_status_ttl,
                    config.rate_limit_ttl,
                )
                .unwrap();

                // Pre-populate storage with health data for selectineveron logic testing
                let default_health = ProcessorHealthStatus::new(true, 200);
                let fallback_health = ProcessorHealthStatus::new(false, 100);

                let _ = storage
                    .set_processor_health("default", &default_health)
                    .await;
                let _ = storage
                    .set_processor_health("fallback", &fallback_health)
                    .await;

                let monitor = make_healt_monitor();

                let best_processor = monitor.get_best_processor().await;
                let _ = black_box(best_processor);
            })
        })
    });
}

fn bench_monitor_all_processors(c: &mut Criterion) {
    c.bench_function("monitor_all_processors", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let monitor = make_healt_monitor();

                // This benchmarks the full monitoring cycle for all processors
                let result = monitor.monitor_all_processors().await;
                let _ = black_box(result);
            })
        })
    });
}

criterion_group!(
    benches,
    bench_health_check_endpoint,
    bench_storage_operations,
    bench_processor_selection_logic,
    bench_monitor_all_processors
);
criterion_main!(benches);
