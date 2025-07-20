use chrono::{DateTime, Utc};
use pgmq::PGMQueue;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use std::{error::Error, time::Duration};

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
}

impl PaymentWorker {
    pub fn new(queue: PGMQueue, queue_name: String) -> Self {
        Self {
            queue,
            queue_name,
            processor: PaymentProcessor::new(),
        }
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
                        Ok(response) => {
                            println!("Payment processed successfully: {response:?}");
                            // Archive message after successful processing
                            self.queue.archive(&self.queue_name, message.msg_id).await?;
                        }
                        Err(e) => {
                            println!("Failed to process payment: {e}");
                            // Archive message even on failure to avoid infinite retry
                            self.queue.archive(&self.queue_name, message.msg_id).await?;
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

    let queue_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@postgres:5432/payments".to_string());

    let queue = PGMQueue::new(queue_url).await?;
    let queue_name = "payment_queue";

    queue.create(queue_name).await?;

    println!("Connected to PGMQ, listening for messages...");

    let worker = PaymentWorker::new(queue, queue_name.to_string());
    worker.process_payments().await
}
