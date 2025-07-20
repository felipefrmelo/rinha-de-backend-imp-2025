# Suggested Cargo.toml improvements for enhanced observability

## api/Cargo.toml
```toml
[package]
name = "api"
version = "0.1.0"
edition = "2024"

[features]
default = []
console = ["tokio-console", "tokio/tracing"]
metrics = ["metrics", "metrics-exporter-prometheus"]

[dependencies]
tokio = { version = "1.45.1", features = ["full", "tracing"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
pgmq = "0.30.1"
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", features = ["json"] }
uuid = { version = "1", features = ["v4", "serde"] }
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid", "bigdecimal"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Optional dependencies
tokio-console = { version = "0.1", optional = true }
metrics = { version = "0.22", optional = true }
metrics-exporter-prometheus = { version = "0.13", optional = true }

[profile.dev]
debug = true

[profile.release]
debug = 1
lto = "thin"
panic = "abort"
```

## payment-worker/Cargo.toml
```toml
[package]
name = "payment-worker"
version = "0.1.0"
edition = "2021"

[features]
default = []
console = ["tokio-console", "tokio/tracing"]
metrics = ["metrics", "metrics-exporter-prometheus"]

[dependencies]
tokio = { version = "1.45.1", features = ["full", "tracing"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
pgmq = "0.30.1"
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", features = ["json"] }
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Optional dependencies
tokio-console = { version = "0.1", optional = true }
metrics = { version = "0.22", optional = true }
metrics-exporter-prometheus = { version = "0.13", optional = true }
```

## Environment Configuration Examples

### .env.development
```bash
RUST_LOG=api=debug,payment_worker=debug,tower_http=debug,axum=trace,sqlx=info
RUST_ENV=development
BIND_ADDRESS=127.0.0.1:3000
DATABASE_URL=postgres://postgres:password@localhost:5432/payments
```

### .env.production
```bash
RUST_LOG=api=info,payment_worker=info,tower_http=warn,axum=warn,sqlx=warn
RUST_ENV=production
BIND_ADDRESS=0.0.0.0:3000
DATABASE_URL=postgres://user:password@db:5432/payments
```

### .env.test
```bash
RUST_LOG=api=warn,payment_worker=warn,tower_http=warn,sqlx=warn
RUST_ENV=test
BIND_ADDRESS=127.0.0.1:3000
DATABASE_URL=postgres://postgres:password@localhost:5432/payments_test
```

## Docker Configuration Updates

### docker-compose.yml additions
```yaml
services:
  api:
    # ... existing config
    environment:
      - RUST_LOG=${RUST_LOG:-api=info,tower_http=warn}
      - RUST_ENV=${RUST_ENV:-production}
      - BIND_ADDRESS=${BIND_ADDRESS:-0.0.0.0:3000}
    # Add health check
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  payment-worker:
    # ... existing config
    environment:
      - RUST_LOG=${RUST_LOG:-payment_worker=info,reqwest=warn,sqlx=warn}
      - RUST_ENV=${RUST_ENV:-production}
    # Add health check (if you add a health endpoint)
    healthcheck:
      test: ["CMD", "ps", "aux", "|", "grep", "payment-worker"]
      interval: 30s
      timeout: 10s
      retries: 3
```

## Tracing Configuration Module

### Create src/tracing_config.rs for both services
```rust
use std::error::Error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_tracing() -> Result<(), Box<dyn Error>> {
    let service_name = env!("CARGO_PKG_NAME");
    
    let env_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        match std::env::var("RUST_ENV").as_deref() {
            Ok("production") => format!("{}=info,tower_http=warn,axum=warn,sqlx=warn", service_name),
            Ok("development") => format!("{}=debug,tower_http=debug,axum=trace,sqlx=info", service_name),
            Ok("test") => format!("{}=warn,tower_http=warn,sqlx=warn", service_name),
            _ => format!("{}=info,tower_http=warn,axum=warn,sqlx=warn", service_name),
        }
    });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_file(cfg!(debug_assertions))
        .compact()
        .with_ansi(std::env::var("NO_COLOR").is_err());

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::new(env_filter))
        .with(fmt_layer);

    // Add JSON formatting for production
    if std::env::var("RUST_ENV").as_deref() == Ok("production") {
        let json_layer = tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(true);
        
        subscriber.with(json_layer).init();
    } else {
        subscriber.init();
    }

    Ok(())
}

#[cfg(feature = "console")]
pub fn init_console() {
    console_subscriber::init();
}

#[cfg(feature = "metrics")]
pub fn init_metrics() -> Result<(), Box<dyn Error>> {
    use metrics_exporter_prometheus::PrometheusBuilder;
    
    let builder = PrometheusBuilder::new();
    builder.install()?;
    
    tracing::info!("Prometheus metrics exporter initialized");
    Ok(())
}
```

## Performance Optimizations

### Conditional Compilation for Debug Logs
```rust
// Instead of:
debug!("Expensive operation: {:?}", expensive_computation());

// Use:
#[cfg(debug_assertions)]
debug!("Expensive operation: {:?}", expensive_computation());

// Or:
if tracing::enabled!(tracing::Level::DEBUG) {
    debug!("Expensive operation: {:?}", expensive_computation());
}
```

### Structured Field Performance
```rust
// Efficient structured logging
info!(
    correlation_id = %payment.correlation_id,
    amount = payment.amount,
    processor = "default",
    "Payment processed"
);

// Instead of string formatting:
info!("Payment {} for amount {} processed by default", payment.correlation_id, payment.amount);
```

### Span Performance
```rust
// Use skip for large objects
#[instrument(skip(large_payload), fields(correlation_id = %large_payload.correlation_id))]
async fn process_large_payload(large_payload: &LargePayload) {
    // ...
}

// Use skip_all for very hot paths
#[instrument(skip_all, fields(correlation_id = %message.correlation_id))]
async fn hot_path_function(message: &Message, context: &Context) {
    // ...
}
```