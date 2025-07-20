use chrono::{DateTime, Utc};
use pgmq::PGMQueue;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::time::{sleep, Duration, Instant};
use std::{error::Error, sync::Arc};
use tokio::sync::RwLock;

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

#[derive(Debug, Deserialize)]
struct HealthResponse {
    failing: bool,
    #[serde(rename = "minResponseTime")]
    min_response_time: u64,
}

#[derive(Debug, Clone)]
struct ProcessorHealth {
    is_healthy: bool,
    min_response_time: u64,
    last_checked: Instant,
    failure_count: u32,
}

impl Default for ProcessorHealth {
    fn default() -> Self {
        Self {
            is_healthy: true,
            min_response_time: 0,
            last_checked: Instant::now() - Duration::from_secs(10), // Allow immediate check
            failure_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ProcessorType {
    Default,
    Fallback,
}

impl ProcessorType {
    fn as_str(&self) -> &'static str {
        match self {
            ProcessorType::Default => "default",
            ProcessorType::Fallback => "fallback",
        }
    }
}

struct PaymentProcessor {
    client: Client,
    default_url: String,
    fallback_url: String,
    health_cache: Arc<RwLock<(ProcessorHealth, ProcessorHealth)>>, // (default, fallback)
}

impl PaymentProcessor {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
            default_url: "http://payment-processor-default:8080".to_string(),
            fallback_url: "http://payment-processor-fallback:8080".to_string(),
            health_cache: Arc::new(RwLock::new((
                ProcessorHealth::default(),
                ProcessorHealth::default(),
            ))),
        }
    }

    async fn check_health(&self, processor_type: ProcessorType) -> Result<HealthResponse, Box<dyn Error>> {
        let url = match processor_type {
            ProcessorType::Default => &self.default_url,
            ProcessorType::Fallback => &self.fallback_url,
        };

        let response = self
            .client
            .get(format!("{}/payments/service-health", url))
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if response.status().is_success() {
            let health: HealthResponse = response.json().await?;
            Ok(health)
        } else {
            Err(format!("Health check failed with status: {}", response.status()).into())
        }
    }

    async fn update_health_if_needed(&self, processor_type: ProcessorType) {
        let health_cache = self.health_cache.clone();
        let mut cache = health_cache.write().await;
        
        let health = match processor_type {
            ProcessorType::Default => &mut cache.0,
            ProcessorType::Fallback => &mut cache.1,
        };

        // Check if we can update health (respecting 5-second limit)
        if health.last_checked.elapsed() >= Duration::from_secs(5) {
            if let Ok(health_response) = self.check_health(processor_type).await {
                health.is_healthy = !health_response.failing;
                health.min_response_time = health_response.min_response_time;
                health.last_checked = Instant::now();
                if health.is_healthy {
                    health.failure_count = 0;
                }
                println!("Updated {} processor health: healthy={}, min_response_time={}ms", 
                         processor_type.as_str(), health.is_healthy, health.min_response_time);
            } else {
                health.is_healthy = false;
                health.failure_count += 1;
                health.last_checked = Instant::now();
                println!("Failed to check {} processor health, marking as unhealthy", processor_type.as_str());
            }
        }
    }

    async fn choose_processor(&self) -> ProcessorType {
        // Update health for both processors if needed
        self.update_health_if_needed(ProcessorType::Default).await;
        self.update_health_if_needed(ProcessorType::Fallback).await;

        let cache = self.health_cache.read().await;
        let (default_health, fallback_health) = &*cache;

        // Strategy: Prefer default processor if healthy, otherwise use fallback
        // If both are unhealthy, try default first (as it has lower fees)
        if default_health.is_healthy {
            ProcessorType::Default
        } else if fallback_health.is_healthy {
            ProcessorType::Fallback
        } else {
            // Both unhealthy, try default first (it has lower fees)
            ProcessorType::Default
        }
    }

    async fn process_payment_with_processor(
        &self,
        message: &PaymentMessage,
        processor_type: ProcessorType,
    ) -> Result<PaymentResponse, Box<dyn Error>> {
        let url = match processor_type {
            ProcessorType::Default => &self.default_url,
            ProcessorType::Fallback => &self.fallback_url,
        };

        let payment_request = PaymentRequest {
            correlation_id: message.correlation_id.clone(),
            amount: message.amount,
            requested_at: message.requested_at.to_rfc3339(),
        };

        let response = self
            .client
            .post(format!("{}/payments", url))
            .json(&payment_request)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        if response.status().is_success() {
            let payment_response: PaymentResponse = response.json().await?;
            Ok(payment_response)
        } else {
            Err(format!("Payment failed with status: {} for {} processor", 
                       response.status(), processor_type.as_str()).into())
        }
    }

    pub async fn process_payment(
        &self,
        message: &PaymentMessage,
    ) -> Result<(PaymentResponse, ProcessorType), Box<dyn Error>> {
        let chosen_processor = self.choose_processor().await;
        
        println!("Attempting payment with {} processor for correlation ID: {}", 
                 chosen_processor.as_str(), message.correlation_id);

        // Try the chosen processor first
        match self.process_payment_with_processor(message, chosen_processor).await {
            Ok(response) => {
                println!("Payment successful with {} processor", chosen_processor.as_str());
                return Ok((response, chosen_processor));
            }
            Err(e) => {
                println!("Payment failed with {} processor: {}", chosen_processor.as_str(), e);
                
                // Mark processor as having failure
                let mut cache = self.health_cache.write().await;
                match chosen_processor {
                    ProcessorType::Default => cache.0.failure_count += 1,
                    ProcessorType::Fallback => cache.1.failure_count += 1,
                }
                drop(cache);

                // Try the other processor as fallback
                let fallback_processor = match chosen_processor {
                    ProcessorType::Default => ProcessorType::Fallback,
                    ProcessorType::Fallback => ProcessorType::Default,
                };

                println!("Attempting fallback with {} processor", fallback_processor.as_str());
                
                match self.process_payment_with_processor(message, fallback_processor).await {
                    Ok(response) => {
                        println!("Payment successful with fallback {} processor", fallback_processor.as_str());
                        Ok((response, fallback_processor))
                    }
                    Err(fallback_error) => {
                        println!("Both processors failed. Default: {}, Fallback: {}", e, fallback_error);
                        Err(format!("Both processors failed - {}: {}, {}: {}", 
                                   chosen_processor.as_str(), e,
                                   fallback_processor.as_str(), fallback_error).into())
                    }
                }
            }
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

    pub async fn process_payments(&self) -> Result<(), Box<dyn Error>> {
        println!("Starting payment processing...");

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
                    println!("Received payment message: {:?}", message.message);
                    println!(
                        "Processing payment for correlation ID: {}",
                        message.message.correlation_id
                    );

                    match self.processor.process_payment(&message.message).await {
                        Ok((response, processor_type)) => {
                            println!("Payment processed successfully with {} processor: {response:?}", 
                                     processor_type.as_str());
                            
                            // Save processed payment to database
                            if let Err(e) = self.save_processed_payment(&message.message, processor_type.as_str()).await {
                                println!("Failed to save processed payment: {e}");
                            }
                            
                            // Archive message after successful processing
                            self.queue.archive(&self.queue_name, message.msg_id).await?;
                        }
                        Err(e) => {
                            println!("Failed to process payment with both processors: {e}");
                            // Archive message to avoid infinite retry
                            self.queue.archive(&self.queue_name, message.msg_id).await?;
                        }
                    }
                }

                None => {
                    // No messages available, wait a bit
                    // Use exponential backoff to reduce CPU usage when idle
                    sleep(Duration::from_millis(250)).await;
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

    // Create database connection pool with optimized settings
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .connect(&database_url)
        .await?;


    let queue = PGMQueue::new_with_pool(db_pool.clone()).await;
    let queue_name = "payment_queue";

    queue.create(queue_name).await?;

    println!("Connected to database and PGMQ, listening for messages...");

    let worker = PaymentWorker::new(queue, queue_name.to_string(), db_pool);
    worker.process_payments().await
}
