use axum::{
    extract::{Json, Query},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use pgmq::PGMQueue;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{collections::HashMap, error::Error};
use tower_http::cors::CorsLayer;
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

async fn create_payment(
    Json(payload): Json<PaymentRequest>,
) -> Result<ResponseJson<PaymentResponse>, StatusCode> {
    let queue_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/payments".to_string());

    let queue = match PGMQueue::new(queue_url).await {
        Ok(q) => q,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let requested_at = Utc::now();
    
    let message = PaymentMessage {
        correlation_id: payload.correlation_id.to_string(),
        amount: payload.amount,
        requested_at,
    };

    match queue.send("payment_queue", &message).await {
        Ok(_) => {
            let response = PaymentResponse {
                status: "accepted".to_string(),
            };
            Ok(ResponseJson(response))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_payments_summary(
    Query(params): Query<PaymentsSummaryQuery>,
) -> Result<ResponseJson<PaymentsSummaryResponse>, StatusCode> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/payments".to_string());

    let pool = match PgPool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let mut query = "
        SELECT 
            processor, 
            COUNT(*) as total_requests, 
            COALESCE(SUM(amount), 0) as total_amount
        FROM processed_payments 
        WHERE 1=1
    ".to_string();

    let mut bind_values = Vec::new();
    let mut param_count = 1;

    if let Some(from) = params.from {
        query.push_str(&format!(" AND processed_at >= ${param_count}"));
        bind_values.push(from);
        param_count += 1;
    }

    if let Some(to) = params.to {
        query.push_str(&format!(" AND processed_at <= ${param_count}"));
        bind_values.push(to);
    }

    query.push_str(" GROUP BY processor");

    let mut sqlx_query = sqlx::query(&query);
    for value in bind_values {
        sqlx_query = sqlx_query.bind(value);
    }

    let rows = match sqlx_query.fetch_all(&pool).await {
        Ok(rows) => rows,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let mut stats: HashMap<String, PaymentTypeStats> = HashMap::new();

    for row in rows {
        let processor: String = row.get("processor");
        let total_requests: i64 = row.get("total_requests");
        let total_amount: f64 = row.get::<sqlx::types::BigDecimal, _>("total_amount").to_string().parse().unwrap_or(0.0);
        
        stats.insert(processor, PaymentTypeStats {
            total_requests,
            total_amount,
        });
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

    let app = Router::new()
        .route("/payments", post(create_payment))
        .route("/payments-summary", get(get_payments_summary))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server running on http://0.0.0.0:3000");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}
