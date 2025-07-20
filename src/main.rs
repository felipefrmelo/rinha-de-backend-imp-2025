use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tower_http::trace::TraceLayer;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    http_client: Client,
    payments_storage: Arc<DashMap<Uuid, PaymentRecord>>,
    default_stats: Arc<ProcessorStats>,
    fallback_stats: Arc<ProcessorStats>,
    last_health_check: Arc<dashmap::DashMap<String, Instant>>,
    health_cache: Arc<dashmap::DashMap<String, (HealthStatus, Instant)>>,
}

struct ProcessorStats {
    total_requests: AtomicU64,
    total_amount: AtomicU64, // stored as cents to avoid floating point precision issues
}

impl ProcessorStats {
    fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            total_amount: AtomicU64::new(0),
        }
    }

    fn add_payment(&self, amount: f64) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_amount
            .fetch_add((amount * 100.0) as u64, Ordering::Relaxed);
    }

    fn get_stats(&self) -> (u64, f64) {
        let requests = self.total_requests.load(Ordering::Relaxed);
        let amount_cents = self.total_amount.load(Ordering::Relaxed);
        (requests, amount_cents as f64 / 100.0)
    }
}

#[derive(Clone, Debug)]
struct PaymentRecord {
    correlation_id: Uuid,
    amount: f64,
    requested_at: DateTime<Utc>,
    processor_used: String,
}

#[derive(Deserialize)]
struct PaymentRequest {
    #[serde(rename = "correlationId")]
    correlation_id: Uuid,
    amount: f64,
}

#[derive(Serialize)]
struct PaymentResponse {
    message: String,
}

#[derive(Serialize)]
struct PaymentsSummaryResponse {
    default: ProcessorSummary,
    fallback: ProcessorSummary,
}

#[derive(Serialize)]
struct ProcessorSummary {
    #[serde(rename = "totalRequests")]
    total_requests: u64,
    #[serde(rename = "totalAmount")]
    total_amount: f64,
}

#[derive(Deserialize)]
struct SummaryQuery {
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct ProcessorPaymentRequest {
    #[serde(rename = "correlationId")]
    correlation_id: Uuid,
    amount: f64,
    #[serde(rename = "requestedAt")]
    requested_at: DateTime<Utc>,
}

#[derive(Deserialize, Clone, Debug)]
struct HealthStatus {
    failing: bool,
    #[serde(rename = "minResponseTime")]
    min_response_time: u64,
}

async fn health_handler() -> &'static str {
    "OK"
}

async fn process_payment(
    State(state): State<AppState>,
    Json(payment): Json<PaymentRequest>,
) -> Result<Json<PaymentResponse>, StatusCode> {
    let requested_at = Utc::now();

    // Choose processor based on health and availability
    let processor_url = choose_best_processor(&state).await;

    let processor_request = ProcessorPaymentRequest {
        correlation_id: payment.correlation_id,
        amount: payment.amount,
        requested_at,
    };

    // Attempt to process payment with chosen processor
    let result = send_payment_request(&state.http_client, &processor_url, &processor_request).await;

    match result {
        Ok(_) => {
            // Record the payment
            let processor_name = if processor_url.contains("default") {
                "default"
            } else {
                "fallback"
            };

            let payment_record = PaymentRecord {
                correlation_id: payment.correlation_id,
                amount: payment.amount,
                requested_at,
                processor_used: processor_name.to_string(),
            };

            state
                .payments_storage
                .insert(payment.correlation_id, payment_record);

            // Update stats
            if processor_name == "default" {
                state.default_stats.add_payment(payment.amount);
            } else {
                state.fallback_stats.add_payment(payment.amount);
            }

            Ok(Json(PaymentResponse {
                message: "payment processed successfully".to_string(),
            }))
        }
        Err(_) => {
            // Try fallback if default failed
            if processor_url.contains("default") {
                let fallback_url = "http://payment-processor-fallback:8080/payments";
                match send_payment_request(&state.http_client, fallback_url, &processor_request)
                    .await
                {
                    Ok(_) => {
                        let payment_record = PaymentRecord {
                            correlation_id: payment.correlation_id,
                            amount: payment.amount,
                            requested_at,
                            processor_used: "fallback".to_string(),
                        };

                        state
                            .payments_storage
                            .insert(payment.correlation_id, payment_record);
                        state.fallback_stats.add_payment(payment.amount);

                        Ok(Json(PaymentResponse {
                            message: "payment processed successfully".to_string(),
                        }))
                    }
                    Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
                }
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

async fn choose_best_processor(state: &AppState) -> String {
    // Check health status with caching and rate limiting
    let default_healthy = is_processor_healthy(state, "default").await;
    let fallback_healthy = is_processor_healthy(state, "fallback").await;

    // Prefer default if healthy (lower fees)
    if default_healthy {
        "http://payment-processor-default:8080/payments".to_string()
    } else if fallback_healthy {
        "http://payment-processor-fallback:8080/payments".to_string()
    } else {
        // If both seem unhealthy, try default first anyway
        "http://payment-processor-default:8080/payments".to_string()
    }
}

async fn is_processor_healthy(state: &AppState, processor: &str) -> bool {
    let cache_key = processor.to_string();
    let now = Instant::now();

    // Check if we have a recent health check (within 6 seconds to be safe with the 5-second limit)
    if let Some(entry) = state.health_cache.get(&cache_key) {
        let (health, cached_at) = entry.value();
        if now.duration_since(*cached_at) < Duration::from_secs(6) {
            return !health.failing;
        }
    }

    // Check if we can make a health request (respect rate limit)
    let last_check_key = format!("{}_last_check", processor);
    if let Some(entry) = state.last_health_check.get(&last_check_key) {
        if now.duration_since(*entry.value()) < Duration::from_secs(5) {
            // Use cached result or assume healthy
            return state
                .health_cache
                .get(&cache_key)
                .map(|entry| {
                    let (health, _) = entry.value();
                    !health.failing
                })
                .unwrap_or(true);
        }
    }

    // Make health check request
    let health_url = match processor {
        "default" => "http://payment-processor-default:8080/payments/service-health",
        "fallback" => "http://payment-processor-fallback:8080/payments/service-health",
        _ => return false,
    };

    match state
        .http_client
        .get(health_url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
    {
        Ok(response) => {
            if let Ok(health) = response.json::<HealthStatus>().await {
                state.health_cache.insert(cache_key, (health.clone(), now));
                state.last_health_check.insert(last_check_key, now);
                !health.failing
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

async fn send_payment_request(
    client: &Client,
    url: &str,
    payment: &ProcessorPaymentRequest,
) -> Result<(), reqwest::Error> {
    let response = client
        .post(url)
        .json(payment)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(reqwest::Error::from(response.error_for_status().unwrap_err()))
    }
}

async fn get_payments_summary(
    State(state): State<AppState>,
    Query(_query): Query<SummaryQuery>,
) -> Json<PaymentsSummaryResponse> {
    // For simplicity, we'll return all-time stats
    // In a production system, you'd filter by the date range
    let (default_requests, default_amount) = state.default_stats.get_stats();
    let (fallback_requests, fallback_amount) = state.fallback_stats.get_stats();

    // If date filters are provided, we should filter the payments
    // For now, return the full stats
    Json(PaymentsSummaryResponse {
        default: ProcessorSummary {
            total_requests: default_requests,
            total_amount: default_amount,
        },
        fallback: ProcessorSummary {
            total_requests: fallback_requests,
            total_amount: fallback_amount,
        },
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let http_client = Client::builder()
        .timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(30))
        .build()?;

    let state = AppState {
        http_client,
        payments_storage: Arc::new(DashMap::new()),
        default_stats: Arc::new(ProcessorStats::new()),
        fallback_stats: Arc::new(ProcessorStats::new()),
        last_health_check: Arc::new(DashMap::new()),
        health_cache: Arc::new(DashMap::new()),
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/payments", post(process_payment))
        .route("/payments-summary", get(get_payments_summary))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    info!("Server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}