use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use rsmq_async::{Rsmq, RsmqConnection, RsmqOptions};
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
    queue: Rsmq,
}

async fn create_payment(
    State(app_state): State<AppState>,
    axum::Json(payload): axum::Json<PaymentRequest>,
) -> StatusCode {
    let mut queue = app_state.queue.clone();

    tokio::spawn(async move {
        let requested_at = Utc::now();
        let message = PaymentMessage {
            correlation_id: payload.correlation_id.to_string(),
            amount: payload.amount,
            requested_at,
        };
        let message_json = match serde_json::to_string(&message) {
            Ok(json) => json,
            Err(_) => return,
        };

        if let Err(e) = queue
            .send_message("payment_queue", message_json.as_str(), None)
            .await
        {
            eprintln!("Erro ao enviar mensagem para a fila: {}", e);
        }
    });

    // Retorna imediatamente
    StatusCode::ACCEPTED
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn get_payments_summary(
    State(app_state): State<AppState>,
    Query(params): Query<PaymentsSummaryQuery>,
) -> Result<Json<PaymentsSummaryResponse>, StatusCode> {
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

    Ok(Json(response))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting payment API server...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@postgres:5432/payments".to_string());

    let db_pool = PgPool::connect(&database_url).await?;

    let mut queue = Rsmq::new(RsmqOptions {
        host: "redis".to_string(),
        port: 6379,
        ..Default::default()
    })
    .await?;

    // Ensure queue exists - create if doesn't exist
    match queue.create_queue("payment_queue", None, None, None).await {
        Ok(_) => println!("Payment queue created successfully"),
        Err(e) => {
            if e.to_string().contains("already exists") {
                println!("Payment queue already exists");
            } else {
                println!("Failed to create payment queue: {}", e);
                return Err(e.into());
            }
        }
    }

    let app_state = AppState { db_pool, queue };

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());

    let host = format!("0.0.0.0:{port}");
    println!("Server running on http://{host}");

    let app = Router::new()
        .route("/payments", post(create_payment))
        .route("/payments-summary", get(get_payments_summary))
        .route("/health", get(health))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&host).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
