use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::task::spawn;
use tower_http::trace::TraceLayer;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    http_client: Client,
    default_stats: Arc<ProcessorStats>,
    fallback_stats: Arc<ProcessorStats>,
    health_manager: Arc<HealthManager>,
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

struct HealthManager {
    default_healthy: AtomicBool,
    fallback_healthy: AtomicBool,
    last_health_check: AtomicU64, // timestamp in millis
}

impl HealthManager {
    fn new() -> Self {
        Self {
            default_healthy: AtomicBool::new(true),
            fallback_healthy: AtomicBool::new(true),
            last_health_check: AtomicU64::new(0),
        }
    }

    fn get_best_processor(&self) -> &'static str {
        if self.default_healthy.load(Ordering::Relaxed) {
            "http://payment-processor-default:8080/payments"
        } else if self.fallback_healthy.load(Ordering::Relaxed) {
            "http://payment-processor-fallback:8080/payments"
        } else {
            // If both seem unhealthy, still try default first (lower fees)
            "http://payment-processor-default:8080/payments"
        }
    }

    fn can_check_health(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let last_check = self.last_health_check.load(Ordering::Relaxed);
        now.saturating_sub(last_check) >= 5000 // 5 seconds in millis
    }

    fn update_health_status(&self, default_healthy: bool, fallback_healthy: bool) {
        self.default_healthy.store(default_healthy, Ordering::Relaxed);
        self.fallback_healthy.store(fallback_healthy, Ordering::Relaxed);
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.last_health_check.store(now, Ordering::Relaxed);
    }
}

#[derive(Deserialize)]
struct PaymentRequest {
    #[serde(rename = "correlationId")]
    correlation_id: Uuid,
    amount: f64,
}

#[derive(Serialize)]
struct PaymentResponse {
    message: &'static str,
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

const SUCCESS_RESPONSE: PaymentResponse = PaymentResponse {
    message: "payment processed successfully",
};

async fn health_handler() -> &'static str {
    "OK"
}

async fn process_payment(
    State(state): State<AppState>,
    Json(payment): Json<PaymentRequest>,
) -> Result<Json<PaymentResponse>, StatusCode> {
    let requested_at = Utc::now();

    // Get the best processor without blocking on health checks
    let processor_url = state.health_manager.get_best_processor();

    let processor_request = ProcessorPaymentRequest {
        correlation_id: payment.correlation_id,
        amount: payment.amount,
        requested_at,
    };

    // Attempt to process payment with chosen processor
    match send_payment_request(&state.http_client, processor_url, &processor_request).await {
        Ok(_) => {
            // Update stats based on processor used
            if processor_url.contains("default") {
                state.default_stats.add_payment(payment.amount);
            } else {
                state.fallback_stats.add_payment(payment.amount);
            }
            Ok(Json(SUCCESS_RESPONSE))
        }
        Err(_) => {
            // Try fallback if default failed
            if processor_url.contains("default") {
                let fallback_url = "http://payment-processor-fallback:8080/payments";
                match send_payment_request(&state.http_client, fallback_url, &processor_request)
                    .await
                {
                    Ok(_) => {
                        state.fallback_stats.add_payment(payment.amount);
                        Ok(Json(SUCCESS_RESPONSE))
                    }
                    Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
                }
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
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
        .timeout(Duration::from_millis(5000)) // Reduced timeout for faster failover
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
    let (default_requests, default_amount) = state.default_stats.get_stats();
    let (fallback_requests, fallback_amount) = state.fallback_stats.get_stats();

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

// Background task to periodically check health status
async fn health_check_task(client: Client, health_manager: Arc<HealthManager>) {
    let mut interval = tokio::time::interval(Duration::from_secs(6));
    
    loop {
        interval.tick().await;
        
        if !health_manager.can_check_health() {
            continue;
        }

        // Check both processors concurrently
        let default_health_future = check_processor_health(&client, "http://payment-processor-default:8080/payments/service-health");
        let fallback_health_future = check_processor_health(&client, "http://payment-processor-fallback:8080/payments/service-health");
        
        let (default_healthy, fallback_healthy) = tokio::join!(default_health_future, fallback_health_future);
        
        health_manager.update_health_status(default_healthy, fallback_healthy);
    }
}

async fn check_processor_health(client: &Client, health_url: &str) -> bool {
    match client
        .get(health_url)
        .timeout(Duration::from_millis(2000))
        .send()
        .await
    {
        Ok(response) => {
            if let Ok(health) = response.json::<HealthStatus>().await {
                !health.failing
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create optimized HTTP client
    let http_client = Client::builder()
        .timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(20) // Increased pool size
        .pool_idle_timeout(Duration::from_secs(60))
        .tcp_keepalive(Duration::from_secs(30))
        .tcp_nodelay(true) // Disable Nagle's algorithm for lower latency
        .build()?;

    let health_manager = Arc::new(HealthManager::new());

    let state = AppState {
        http_client: http_client.clone(),
        default_stats: Arc::new(ProcessorStats::new()),
        fallback_stats: Arc::new(ProcessorStats::new()),
        health_manager: health_manager.clone(),
    };

    // Start background health checking task
    spawn(health_check_task(http_client, health_manager));

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