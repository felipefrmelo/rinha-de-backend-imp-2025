use chrono::{DateTime, Utc};
use pgmq::PGMQueue;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::time::sleep;
use std::{error::Error, time::Duration};
use tracing::{info, error, debug, instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Serialize, Deserialize)]
struct PaymentMessage {
    #[serde(rename = "correlationId")]
    correlation_id: String,
    amount: f64,
    #[serde(rename = "requestedAt")]
    requested_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct PaymentRequest {
    #[serde(rename = "correlationId")]
    correlation_id: String,
    amount: f64,
    #[serde(rename = "requestedAt")]
    requested_at: String,
}

#[derive(Debug, Deserialize)]
struct PaymentResponse {
    message: String,
}

struct PaymentProcessor {
    client: Client,
    default_url: String,
}

impl PaymentProcessor {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            default_url: "http://payment-processor-default:8080".to_string(),
        }
    }

    #[instrument(skip(self), fields(correlation_id = %message.correlation_id, amount = message.amount))]
    pub async fn process_payment(
        &self,
        message: &PaymentMessage,
    ) -> Result<PaymentResponse, Box<dyn Error>> {
        let payment_request = PaymentRequest {
            correlation_id: message.correlation_id.clone(),
            amount: message.amount,
            requested_at: message.requested_at.to_rfc3339(),
        };

        let response = self
            .client
            .post(format!("{}/payments", self.default_url))
            .json(&payment_request)
            .send()
            .await?;

        if response.status().is_success() {
            let payment_response: PaymentResponse = response.json().await?;
            Ok(payment_response)
        } else {
            Err(format!("Payment failed with status: {}", response.status()).into())
        }
    }
}

struct PaymentWorker {
    queue: PGMQueue,
    queue_name: String,
    processor: PaymentProcessor,
    db_pool: Pool<Postgres>,
}

impl PaymentWorker {
    pub fn new(queue: PGMQueue, queue_name: String, db_pool: Pool<Postgres>) -> Self {
        Self {
            queue,
            queue_name,
            processor: PaymentProcessor::new(),
            db_pool,
        }
    }

    #[instrument(skip(self), fields(correlation_id = %message.correlation_id, processor = processor))]
    async fn save_processed_payment(
        &self,
        message: &PaymentMessage,
        processor: &str,
    ) -> Result<(), Box<dyn Error>> {
        sqlx::query(
            r#"
            INSERT INTO processed_payments (correlation_id, amount, requested_at, processor)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (correlation_id) DO NOTHING
            "#,
        )
        .bind(&message.correlation_id)
        .bind(message.amount)
        .bind(message.requested_at)
        .bind(processor)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn process_payments(&self) -> Result<(), Box<dyn Error>> {
        info!("Starting payment processing...");

        // Message polling loop
        //
        let visibility_timeout_seconds: i32 = 30;
        loop {
            match self
                .queue
                .read::<PaymentMessage>(&self.queue_name, Some(visibility_timeout_seconds))
                .await?
            {
                Some(message) => {
                    let span = tracing::info_span!("process_message", 
                        correlation_id = %message.message.correlation_id,
                        msg_id = message.msg_id
                    );
                    let _enter = span.enter();
                    
                    debug!("Received payment message: {:?}", message.message);
                    info!("Processing payment for correlation ID: {}", message.message.correlation_id);

                    match self.processor.process_payment(&message.message).await {
                        Ok(response) => {
                            info!("Payment processed successfully: {response:?}");
                            
                            // Save processed payment to database
                            if let Err(e) = self.save_processed_payment(&message.message, "default").await {
                                error!("Failed to save processed payment: {e}");
                            }
                            
                            // Archive message after successful processing
                            if let Err(e) = self.queue.archive(&self.queue_name, message.msg_id).await {
                                error!("Failed to archive message: {e}");
                            }
                        }
                        Err(e) => {
                            error!("Failed to process payment: {e}");
                            // Archive message even on failure to avoid infinite retry
                            if let Err(e) = self.queue.archive(&self.queue_name, message.msg_id).await {
                                error!("Failed to archive failed message: {e}");
                            }
                        }
                    }
                }

                None => {
                    // No messages available, wait a bit
                    debug!("No messages available, waiting...");
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
}

// Message polling loop

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "payment_worker=debug,reqwest=info,sqlx=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting payment worker...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@postgres:5432/payments".to_string());

    // Create database connection pool
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;


    let queue = PGMQueue::new_with_pool(db_pool.clone()).await;
    let queue_name = "payment_queue";

    queue.create(queue_name).await?;

    info!("Connected to database and PGMQ, listening for messages...");

    let worker = PaymentWorker::new(queue, queue_name.to_string(), db_pool);
    worker.process_payments().await
}
