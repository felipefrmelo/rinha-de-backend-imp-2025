use axum::{
    Router,
    extract::{Json, Query, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use pgmq::PGMQueue;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{collections::HashMap, error::Error};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaymentMessage {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    pub amount: f64,
    #[serde(rename = "requestedAt")]
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentRequest {
    pub amount: f64,
    #[serde(rename = "correlationId")]
    pub correlation_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct PaymentResponse {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct PaymentsSummaryQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PaymentTypeStats {
    #[serde(rename = "totalRequests")]
    pub total_requests: i64,
    #[serde(rename = "totalAmount")]
    pub total_amount: f64,
}

#[derive(Debug, Serialize)]
pub struct PaymentsSummaryResponse {
    pub default: PaymentTypeStats,
    pub fallback: PaymentTypeStats,
}

#[derive(Clone)]
struct AppState {
    db_pool: PgPool,
    queue: PGMQueue,
}

async fn create_payment(
    State(app_state): State<AppState>,
    Json(payload): Json<PaymentRequest>,
) -> Result<ResponseJson<PaymentResponse>, StatusCode> {
    let requested_at = Utc::now();

    let message = PaymentMessage {
        correlation_id: payload.correlation_id.to_string(),
        amount: payload.amount,
        requested_at,
    };

    match app_state.queue.send("payment_queue", &message).await {
        Ok(_) => {
            let response = PaymentResponse {
                status: "accepted".to_string(),
            };
            Ok(ResponseJson(response))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn get_payments_summary(
    State(app_state): State<AppState>,
    Query(params): Query<PaymentsSummaryQuery>,
) -> Result<ResponseJson<PaymentsSummaryResponse>, StatusCode> {
    let rows = match (params.from, params.to) {
        (Some(from), Some(to)) => {
            sqlx::query("
                SELECT processor, COUNT(*) as total_requests, COALESCE(SUM(amount), 0) as total_amount
                FROM processed_payments 
                WHERE requested_at >= $1 AND requested_at <= $2
                GROUP BY processor
            ")
            .bind(from)
            .bind(to)
            .fetch_all(&app_state.db_pool)
            .await
        },
        (Some(from), None) => {
            sqlx::query("
                SELECT processor, COUNT(*) as total_requests, COALESCE(SUM(amount), 0) as total_amount
                FROM processed_payments 
                WHERE requested_at >= $1
                GROUP BY processor
            ")
            .bind(from)
            .fetch_all(&app_state.db_pool)
            .await
        },
        (None, Some(to)) => {
            sqlx::query("
                SELECT processor, COUNT(*) as total_requests, COALESCE(SUM(amount), 0) as total_amount
                FROM processed_payments 
                WHERE requested_at <= $1
                GROUP BY processor
            ")
            .bind(to)
            .fetch_all(&app_state.db_pool)
            .await
        },
        (None, None) => {
            sqlx::query("
                SELECT processor, COUNT(*) as total_requests, COALESCE(SUM(amount), 0) as total_amount
                FROM processed_payments 
                GROUP BY processor
            ")
            .fetch_all(&app_state.db_pool)
            .await
        }
    };

    let rows = match rows {
        Ok(rows) => rows,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let mut stats: HashMap<String, PaymentTypeStats> = HashMap::new();

    for row in rows {
        let processor: String = row.get("processor");
        let total_requests: i64 = row.get("total_requests");
        let total_amount: f64 = row
            .try_get::<sqlx::types::BigDecimal, _>("total_amount")
            .map(|bd| bd.to_string().parse().unwrap_or(0.0))
            .unwrap_or(0.0);

        stats.insert(
            processor,
            PaymentTypeStats {
                total_requests,
                total_amount,
            },
        );
    }

    let default_stats = stats.get("default").cloned().unwrap_or(PaymentTypeStats {
        total_requests: 0,
        total_amount: 0.0,
    });

    let fallback_stats = stats.get("fallback").cloned().unwrap_or(PaymentTypeStats {
        total_requests: 0,
        total_amount: 0.0,
    });

    let response = PaymentsSummaryResponse {
        default: default_stats,
        fallback: fallback_stats,
    };

    Ok(ResponseJson(response))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting payment API server...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@postgres:5432/payments".to_string());

    let db_pool = PgPool::connect(&database_url).await?;
    let queue = PGMQueue::new_with_pool(db_pool.clone()).await;

    let app_state = AppState { db_pool, queue };

    let app = Router::new()
        .route("/payments", post(create_payment))
        .route("/payments-summary", get(get_payments_summary))
        .route("/health", get(health))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server running on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;

    Ok(())
}
