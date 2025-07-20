use axum::{
    extract::Json,
    http::StatusCode,
    response::Json as ResponseJson,
    routing::post,
    Router,
};
use chrono::{DateTime, Utc};
use pgmq::PGMQueue;
use serde::{Deserialize, Serialize};
use std::error::Error;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting payment API server...");

    let app = Router::new()
        .route("/payments", post(create_payment))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server running on http://0.0.0.0:3000");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}
