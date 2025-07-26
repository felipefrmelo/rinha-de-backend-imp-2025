// Suggested improvements for api/src/main.rs

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use pgmq::{Message, PGMQueue};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{collections::HashMap, error::Error};
use uuid::Uuid;
use tower_http::trace::TraceLayer;
use tracing::{info, error, warn, instrument, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ... existing structs ...

#[instrument(skip(app_state), fields(
    correlation_id = %payload.correlation_id, 
    amount = payload.amount,
    request_id = tracing::field::Empty
))]
async fn create_payment(
    State(app_state): State<AppState>,
    Json(payload): Json<PaymentRequest>,
) -> Result<ResponseJson<PaymentResponse>, StatusCode> {
    // Add request ID for better tracing
    let request_id = uuid::Uuid::new_v4();
    tracing::Span::current().record("request_id", request_id.to_string());

    let message = PaymentMessage {
        correlation_id: payload.correlation_id,
        amount: payload.amount,
        timestamp: Utc::now(),
    };

    match app_state.queue.send("payment_queue", &message).await {
        Ok(_) => {
            info!(
                correlation_id = %payload.correlation_id,
                amount = payload.amount,
                "Payment queued successfully"
            );
            let response = PaymentResponse {
                status: "accepted".to_string(),
            };
            Ok(ResponseJson(response))
        }
        Err(e) => {
            error!(
                correlation_id = %payload.correlation_id,
                amount = payload.amount,
                error = %e,
                "Failed to queue payment"
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[instrument]
async fn health() -> StatusCode {
    tracing::debug!("Health check requested");
    StatusCode::OK
}

#[instrument(skip(app_state), fields(
    processor = tracing::field::Empty,
    date_from = tracing::field::Empty,
    date_to = tracing::field::Empty
))]
async fn get_payments_summary(
    State(app_state): State<AppState>,
    Query(params): Query<PaymentsSummaryQuery>,
) -> Result<ResponseJson<PaymentsSummaryResponse>, StatusCode> {
    // Record query parameters in span
    if let Some(processor) = &params.processor {
        tracing::Span::current().record("processor", processor);
    }
    if let Some(date_from) = &params.date_from {
        tracing::Span::current().record("date_from", date_from.to_string());
    }
    if let Some(date_to) = &params.date_to {
        tracing::Span::current().record("date_to", date_to.to_string());
    }

    // ... existing query logic ...

    let rows = match rows {
        Ok(rows) => {
            info!(
                row_count = rows.len(),
                "Successfully queried payments summary"
            );
            rows
        },
        Err(e) => {
            error!(
                error = %e,
                "Failed to query payments summary"
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // ... rest of existing logic ...
}

// Enhanced tracing initialization
fn init_tracing() -> Result<(), Box<dyn Error>> {
    let env_filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| {
            match std::env::var("RUST_ENV").as_deref() {
                Ok("production") => "api=info,tower_http=warn,axum=warn,sqlx=warn",
                Ok("development") => "api=debug,tower_http=debug,axum=trace,sqlx=info",
                Ok("test") => "api=warn,tower_http=warn",
                _ => "api=info,tower_http=warn,axum=warn,sqlx=warn",
            }
        });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_file(cfg!(debug_assertions)) // Only include file info in debug builds
        .compact();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(env_filter))
        .with(fmt_layer)
        .init();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing first
    init_tracing()?;

    info!("Starting payment API server...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@postgres:5432/payments".to_string());

    let pool = PgPool::connect(&database_url).await?;
    let queue = PGMQueue::new(pool.clone()).await?;

    let app_state = AppState { pool, queue };

    let app = Router::new()
        .route("/payments", post(create_payment))
        .route("/payments-summary", get(get_payments_summary))
        .route("/health", get(health))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::extract::Request| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        version = ?request.version(),
                    )
                })
                .on_response(|response: &axum::response::Response, latency: std::time::Duration, _span: &tracing::Span| {
                    tracing::info!(
                        status = response.status().as_u16(),
                        latency_ms = latency.as_millis(),
                        "HTTP response"
                    );
                })
        )
        .with_state(app_state);

    // Enhanced binding configuration
    let bind_addr = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| {
            match std::env::var("RUST_ENV").as_deref() {
                Ok("production") => {
                    warn!("Production environment detected, consider setting BIND_ADDRESS explicitly");
                    "0.0.0.0:3000"
                },
                _ => "0.0.0.0:3000"
            }
        });

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!(bind_address = %bind_addr, "Server running");

    axum::serve(listener, app).await?;

    Ok(())
}