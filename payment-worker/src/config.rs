use std::error::Error;

#[derive(Debug, Clone)]
pub struct PaymentWorkerConfig {
    pub database_url: String,
    pub database_max_connections: u32,
    pub redis_host: String,
    pub redis_port: u16,
    pub queue_name: String,
    pub worker_concurrency: usize,
    pub http_client_timeout_secs: u64,
    pub queue_receive_timeout_secs: u64,
    pub poll_sleep_millis: u64,
    pub error_sleep_millis: u64,
    pub process_sleep_millis: u64,
}

impl PaymentWorkerConfig {
    pub fn from_env() -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/payments".to_string()),
            database_max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            redis_host: std::env::var("REDIS_HOST")
                .unwrap_or_else(|_| "redis".to_string()),
            redis_port: std::env::var("REDIS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6379),
            queue_name: std::env::var("QUEUE_NAME")
                .unwrap_or_else(|_| "payment_queue".to_string()),
            worker_concurrency: std::env::var("WORKER_CONCURRENCY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4),
            http_client_timeout_secs: std::env::var("HTTP_CLIENT_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            queue_receive_timeout_secs: std::env::var("QUEUE_RECEIVE_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            poll_sleep_millis: std::env::var("POLL_SLEEP_MILLIS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
            error_sleep_millis: std::env::var("ERROR_SLEEP_MILLIS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(200),
            process_sleep_millis: std::env::var("PROCESS_SLEEP_MILLIS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
        })
    }

    pub fn log_configuration(&self) {
        println!("Payment Worker Configuration:");
        println!("  Database URL: {}", self.database_url);
        println!("  Database Max Connections: {}", self.database_max_connections);
        println!("  Redis Host: {}", self.redis_host);
        println!("  Redis Port: {}", self.redis_port);
        println!("  Queue Name: {}", self.queue_name);
        println!("  Worker Concurrency: {}", self.worker_concurrency);
        println!("  HTTP Client Timeout: {}s", self.http_client_timeout_secs);
        println!("  Queue Receive Timeout: {}s", self.queue_receive_timeout_secs);
        println!("  Poll Sleep: {}ms", self.poll_sleep_millis);
        println!("  Error Sleep: {}ms", self.error_sleep_millis);
        println!("  Process Sleep: {}ms", self.process_sleep_millis);
    }
}
