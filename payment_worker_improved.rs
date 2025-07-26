// Suggested improvements for payment-worker/src/main.rs

use chrono::{DateTime, Utc};
use pgmq::{Message, PGMQueue};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::time::sleep;
use std::{error::Error, time::Duration};
use tracing::{info, error, warn, debug, instrument, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ... existing structs ...

impl PaymentProcessor {
    pub fn new(endpoint: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint,
        }
    }

    #[instrument(
        skip(self), 
        fields(
            correlation_id = %message.correlation_id, 
            amount = message.amount,
            endpoint = %self.endpoint,
            response_time_ms = tracing::field::Empty,
            processor_response_status = tracing::field::Empty
        )
    )]
    pub async fn process_payment(
        &self,
        message: &PaymentMessage,
    ) -> Result<PaymentResponse, Box<dyn Error>> {
        let start = std::time::Instant::now();

        let response = self.client
            .post(&self.endpoint)
            .json(&PaymentRequest {
                correlation_id: message.correlation_id,
                amount: message.amount,
            })
            .send()
            .await?;

        let response_time = start.elapsed();
        tracing::Span::current().record("response_time_ms", response_time.as_millis());
        tracing::Span::current().record("processor_response_status", response.status().as_u16());

        if !response.status().is_success() {
            error!(
                status = response.status().as_u16(),
                response_time_ms = response_time.as_millis(),
                "Payment processor returned error status"
            );
            return Err(format!("Payment processor error: {}", response.status()).into());
        }

        let payment_response: PaymentResponse = response.json().await?;

        info!(
            response_time_ms = response_time.as_millis(),
            "Payment processed successfully"
        );

        Ok(payment_response)
    }
}

impl PaymentWorker {
    pub fn new(queue: PGMQueue, queue_name: String, db_pool: Pool<Postgres>) -> Self {
        Self {
            queue,
            queue_name,
            processor: PaymentProcessor::new(
                "http://payment-processor-default:8080/payments".to_string(),
            ),
            db_pool,
        }
    }

    #[instrument(
        skip(self), 
        fields(
            correlation_id = %message.correlation_id, 
            processor = processor,
            amount = message.amount,
            db_operation_time_ms = tracing::field::Empty
        )
    )]
    async fn save_processed_payment(
        &self,
        message: &PaymentMessage,
        processor: &str,
    ) -> Result<(), Box<dyn Error>> {
        let start = std::time::Instant::now();

        sqlx::query!(
            "INSERT INTO processed_payments (correlation_id, amount, requested_at, processor) VALUES ($1, $2, $3, $4)",
            message.correlation_id,
            message.amount,
            message.timestamp,
            processor
        )
        .execute(&self.db_pool)
        .await?;

        let db_time = start.elapsed();
        tracing::Span::current().record("db_operation_time_ms", db_time.as_millis());

        debug!(
            db_operation_time_ms = db_time.as_millis(),
            "Payment saved to database"
        );

        Ok(())
    }

    #[instrument(skip(self), fields(processed_count = tracing::field::Empty))]
    pub async fn process_payments(&self) -> Result<(), Box<dyn Error>> {
        info!("Starting payment processing...");
        let mut processed_count = 0u64;

        loop {
            // Wrap processing in panic recovery
            let processing_result = std::panic::AssertUnwindSafe(async {
                self.process_single_batch().await
            });

            match tokio::task::spawn(processing_result).await {
                Ok(Ok(batch_count)) => {
                    processed_count += batch_count;
                    if batch_count > 0 {
                        tracing::Span::current().record("processed_count", processed_count);
                        debug!(batch_count = batch_count, total_processed = processed_count);
                    }
                }
                Ok(Err(e)) => {
                    error!(error = %e, "Error processing payment batch");
                    // Brief pause before continuing
                    sleep(Duration::from_millis(1000)).await;
                }
                Err(panic_info) => {
                    error!("Payment worker panicked: {:?}", panic_info);
                    // Longer pause after panic
                    sleep(Duration::from_millis(5000)).await;
                }
            }
        }
    }

    // Extract single batch processing for better error handling
    async fn process_single_batch(&self) -> Result<u64, Box<dyn Error>> {
        match self
            .queue
            .read::<PaymentMessage>(&self.queue_name, Some(1))
            .await?
        {
            Some(message) => {
                let span = tracing::info_span!(
                    "process_message", 
                    correlation_id = %message.message.correlation_id,
                    msg_id = message.msg_id,
                    amount = message.message.amount,
                    processing_time_ms = tracing::field::Empty
                );
                let _enter = span.enter();
                
                let start = std::time::Instant::now();
                
                if tracing::enabled!(Level::DEBUG) {
                    debug!("Received payment message: {:?}", message.message);
                }
                
                info!("Processing payment");

                match self.processor.process_payment(&message.message).await {
                    Ok(response) => {
                        info!(
                            response = ?response,
                            "Payment processed successfully"
                        );
                        
                        // Save processed payment to database
                        if let Err(e) = self.save_processed_payment(&message.message, "default").await {
                            error!(
                                correlation_id = %message.message.correlation_id,
                                processor = "default",
                                amount = message.message.amount,
                                error = %e,
                                "Failed to save processed payment"
                            );
                        }
                        
                        // Archive message after successful processing
                        if let Err(e) = self.queue.archive(&self.queue_name, message.msg_id).await {
                            error!(
                                correlation_id = %message.message.correlation_id,
                                msg_id = message.msg_id,
                                error = %e,
                                "Failed to archive message"
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            correlation_id = %message.message.correlation_id,
                            error = %e,
                            "Failed to process payment"
                        );
                        
                        // Archive message even on failure to avoid infinite retry
                        if let Err(archive_err) = self.queue.archive(&self.queue_name, message.msg_id).await {
                            error!(
                                correlation_id = %message.message.correlation_id,
                                msg_id = message.msg_id,
                                archive_error = %archive_err,
                                original_error = %e,
                                "Failed to archive failed message"
                            );
                        }
                    }
                }

                let processing_time = start.elapsed();
                tracing::Span::current().record("processing_time_ms", processing_time.as_millis());

                Ok(1) // Processed 1 message
            }
            None => {
                // No messages available, wait a bit
                if tracing::enabled!(Level::DEBUG) {
                    debug!("No messages available, waiting...");
                }
                sleep(Duration::from_millis(100)).await;
                Ok(0) // Processed 0 messages
            }
        }
    }
}

// Enhanced tracing initialization
fn init_tracing() -> Result<(), Box<dyn Error>> {
    let env_filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| {
            match std::env::var("RUST_ENV").as_deref() {
                Ok("production") => "payment_worker=info,reqwest=warn,sqlx=warn",
                Ok("development") => "payment_worker=debug,reqwest=info,sqlx=debug",
                Ok("test") => "payment_worker=warn,reqwest=warn,sqlx=warn",
                _ => "payment_worker=info,reqwest=warn,sqlx=warn",
            }
        });

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_file(cfg!(debug_assertions))
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

    info!("Starting payment worker...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@postgres:5432/payments".to_string());

    let db_pool = Pool::<Postgres>::connect(&database_url).await?;

    let mut queue = PGMQueue::new(db_pool.clone()).await?;

    let queue_name = "payment_queue";

    queue.create(queue_name).await?;

    info!(
        database_url = %database_url.replace(
            std::env::var("DATABASE_PASSWORD").unwrap_or_default().as_str(), 
            "***"
        ),
        queue_name = queue_name,
        "Connected to database and PGMQ, listening for messages"
    );

    let worker = PaymentWorker::new(queue, queue_name.to_string(), db_pool);
    
    // Log startup completion
    info!("Payment worker initialized successfully, starting message processing");
    
    worker.process_payments().await
}