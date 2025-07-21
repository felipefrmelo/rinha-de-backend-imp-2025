use chrono::{DateTime, Utc};
use rsmq_async::{Rsmq, RsmqConnection, RsmqOptions};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::time::sleep;
use std::{error::Error, time::Duration};
use health_checker::RedisHealthClient;

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
    fallback_url: String,
    health_client: RedisHealthClient,
}

impl PaymentProcessor {
    pub fn new(health_client: RedisHealthClient) -> Self {
        Self {
            client: Client::new(),
            default_url: "http://payment-processor-default:8080".to_string(),
            fallback_url: "http://payment-processor-fallback:8080".to_string(),
            health_client,
        }
    }

    async fn get_best_processor(&self) -> Result<(&str, &str), Box<dyn Error>> {
        let default_health = self.health_client.get_processor_health("default").await?;
        let fallback_health = self.health_client.get_processor_health("fallback").await?;

        match (default_health, fallback_health) {
            (Some(default), Some(fallback)) => {
                if !default.failing {
                    Ok(("default", &self.default_url))
                } else if !fallback.failing {
                    Ok(("fallback", &self.fallback_url))
                } else {
                    Ok(("default", &self.default_url))
                }
            }
            (Some(default), None) => {
                if !default.failing {
                    Ok(("default", &self.default_url))
                } else {
                    Ok(("fallback", &self.fallback_url))
                }
            }
            (None, Some(fallback)) => {
                if !fallback.failing {
                    Ok(("fallback", &self.fallback_url))
                } else {
                    Ok(("default", &self.default_url))
                }
            }
            (None, None) => {
                Ok(("default", &self.default_url))
            }
        }
    }

    pub async fn process_payment(
        &self,
        message: &PaymentMessage,
    ) -> Result<(PaymentResponse, String), Box<dyn Error>> {
        let payment_request = PaymentRequest {
            correlation_id: message.correlation_id.clone(),
            amount: message.amount,
            requested_at: message.requested_at.to_rfc3339(),
        };

        let (processor_name, processor_url) = self.get_best_processor().await?;

        let response = self
            .client
            .post(format!("{}/payments", processor_url))
            .json(&payment_request)
            .send()
            .await?;

        if response.status().is_success() {
            let payment_response: PaymentResponse = response.json().await?;
            Ok((payment_response, processor_name.to_string()))
        } else {
            Err(format!("Payment failed with status: {}", response.status()).into())
        }
    }
}

struct PaymentWorker {
    queue: Rsmq,
    queue_name: String,
    processor: PaymentProcessor,
    db_pool: Pool<Postgres>,
}

impl PaymentWorker {
    pub fn new(queue: Rsmq, queue_name: String, db_pool: Pool<Postgres>, health_client: RedisHealthClient) -> Self {
        Self {
            queue,
            queue_name,
            processor: PaymentProcessor::new(health_client),
            db_pool,
        }
    }

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

    pub async fn process_payments(&mut self) -> Result<(), Box<dyn Error>> {
        println!("Starting payment processing...");

        loop {
            match self.queue.receive_message::<String>(&self.queue_name, Some(Duration::from_secs(30))).await? {
                Some(message) => {
                    let payment_message: PaymentMessage = match serde_json::from_str(&message.message) {
                        Ok(msg) => msg,
                        Err(e) => {
                            println!("Failed to deserialize message: {e}");
                            self.queue.delete_message(&self.queue_name, &message.id).await?;
                            continue;
                        }
                    };
                    
                    println!("Received payment message: {:?}", payment_message);
                    println!(
                        "Processing payment for correlation ID: {}",
                        payment_message.correlation_id
                    );

                    match self.processor.process_payment(&payment_message).await {
                        Ok((response, processor_used)) => {
                            println!("Payment processed successfully: {response:?}");
                            
                            // Save processed payment to database
                            if let Err(e) = self.save_processed_payment(&payment_message, &processor_used).await {
                                println!("Failed to save processed payment: {e}");
                            }
                            
                            // Delete message after successful processing
                            self.queue.delete_message(&self.queue_name, &message.id).await?;
                        }
                        Err(e) => {
                            println!("Failed to process payment: {e}");
                            // Delete message even on failure to avoid infinite retry
                            self.queue.delete_message(&self.queue_name, &message.id).await?;
                        }
                    }
                }

                None => {
                    // No messages available, wait a bit
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
}

// Message polling loop

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting payment worker...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@postgres:5432/payments".to_string());
    
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://redis:6379".to_string());

    // Create database connection pool
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let mut queue = Rsmq::new(RsmqOptions {
        host: "redis".to_string(),
        port: 6379,
        ..Default::default()
    }).await?;
    let queue_name = "payment_queue";

    // Ensure queue exists - create if doesn't exist
    match queue.create_queue(queue_name, None, None, None).await {
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

    // Create health client for querying processor health
    let health_client = RedisHealthClient::new(&redis_url, 30, 5)?;

    println!("Connected to database and Redis queue, listening for messages...");

    let mut worker = PaymentWorker::new(queue, queue_name.to_string(), db_pool, health_client);
    worker.process_payments().await
}
