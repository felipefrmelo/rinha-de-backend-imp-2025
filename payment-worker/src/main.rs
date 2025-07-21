use chrono::{DateTime, Utc};
use rsmq_async::{Rsmq, RsmqConnection, RsmqOptions};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::time::sleep;
use std::{error::Error, time::Duration};
use health_checker::{HealthMonitor,HealthCheckerConfig};

mod config;
use config::PaymentWorkerConfig;


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
    health_monitor: HealthMonitor,
}

impl PaymentProcessor {
    pub fn new(health_monitor: HealthMonitor, config: &PaymentWorkerConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(config.http_client_timeout_secs))
                .build()
                .expect("Failed to create HTTP client"),
            default_url: config.payment_processor_default_url.clone(),
            fallback_url: config.payment_processor_fallback_url.clone(),
            health_monitor,
        }
    }

    async fn get_best_processor(&self) -> Result<(&str, &str), Box<dyn Error + Send + Sync>> {
        let processor = self.health_monitor
            .get_best_processor()
            .await?;

        if processor == "default" {
            Ok(("default", &self.default_url))
        } else if processor == "fallback" {
            Ok(("fallback", &self.fallback_url))
        } else {
            Err(format!("Unknown processor: {}", processor).into())
        }


    }

    pub async fn process_payment(
        &self,
        message: &PaymentMessage,
    ) -> Result<(PaymentResponse, String), Box<dyn Error + Send + Sync>> {
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

use std::sync::Arc;
use tokio::sync::Mutex;

struct PaymentWorker {
    queue: Arc<Mutex<Rsmq>>,
    queue_name: String,
    processor: Arc<PaymentProcessor>,
    db_pool: Pool<Postgres>,
    config: PaymentWorkerConfig,
}

impl PaymentWorker {
    pub fn new(queue: Rsmq, queue_name: String, db_pool: Pool<Postgres>, processor: Arc<PaymentProcessor>, config: PaymentWorkerConfig) -> Self {
        Self {
            queue: Arc::new(Mutex::new(queue)),
            queue_name,
            processor,
            db_pool,
            config,
        }
    }

    async fn save_processed_payment(
        &self,
        message: &PaymentMessage,
        processor: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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

    pub async fn worker_loop(self: Arc<Self>) {
        println!("Worker loop started");
        loop {
            println!("Polling for messages...");
            let mut queue = self.queue.lock().await;
            match queue.receive_message::<String>(&self.queue_name, Some(Duration::from_secs(self.config.queue_receive_timeout_secs))).await {
                Ok(Some(message)) => {
                    let payment_message: PaymentMessage = match serde_json::from_str(&message.message) {
                        Ok(msg) => msg,
                        Err(e) => {
                            println!("Failed to deserialize message: {e}");
                            let _ = queue.delete_message(&self.queue_name, &message.id).await;
                            continue;
                        }
                    };
                    println!("Received payment message: {payment_message:?}");
                    println!("Processing payment for correlation ID: {}", payment_message.correlation_id);

                    match self.processor.process_payment(&payment_message).await {
                        Ok((response, processor_used)) => {
                            println!("Payment processed successfully: {response:?}");
                            if let Err(e) = self.save_processed_payment(&payment_message, &processor_used).await {
                                println!("Failed to save processed payment: {e}");
                            } else {
                                println!("Payment saved to database successfully");
                            }
                            let _ = queue.delete_message(&self.queue_name, &message.id).await;
                            println!("Message deleted from queue, continuing to next message...");
                            sleep(Duration::from_millis(self.config.process_sleep_millis)).await;
                        }
                        Err(e) => {
                            println!("Failed to process payment: {e}");
                        }
                    }
                }
                Ok(None) => {
                    println!("No messages available, waiting...");
                    sleep(Duration::from_millis(self.config.poll_sleep_millis)).await;
                }
                Err(e) => {
                    println!("Error receiving message: {e}");
                    sleep(Duration::from_millis(self.config.error_sleep_millis)).await;
                }
            }
        }
    }
}

// Message polling loop

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    println!("Starting payment worker...");

    let config = PaymentWorkerConfig::from_env()?;
    config.log_configuration();

    // Create database connection pool
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .connect(&config.database_url)
        .await?;

    let mut queue = Rsmq::new(RsmqOptions {
        host: config.redis_host.clone(),
        port: config.redis_port,
        ..Default::default()
    }).await?;

    // Ensure queue exists - create if doesn't exist
    match queue.create_queue(&config.queue_name, None, None, None).await {
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


    let health_config = HealthCheckerConfig::from_env().unwrap();
    health_config.log_configuration();
    
    let health_monitor = HealthMonitor::new(health_config).unwrap();


    println!("Connected to database and Redis queue, listening for messages...");

    let processor = Arc::new(PaymentProcessor::new(health_monitor, &config));

    let concurrency = config.worker_concurrency;

    let mut handles = Vec::new();
    for _ in 0..concurrency {
        // Cada worker recebe sua própria instância de Rsmq
        let queue = Rsmq::new(RsmqOptions {
            host: config.redis_host.clone(),
            port: config.redis_port,
            ..Default::default()
        }).await.expect("Failed to create Rsmq instance");
        let worker = Arc::new(PaymentWorker::new(queue, config.queue_name.clone(), db_pool.clone(), processor.clone(), config.clone()));
        let worker_clone = worker.clone();
        handles.push(tokio::spawn(async move {
            worker_clone.worker_loop().await;
        }));
    }
    for handle in handles {
        let _ = handle.await;
    }
    println!("All workers have been started, waiting for messages...");
    Ok(())
}
