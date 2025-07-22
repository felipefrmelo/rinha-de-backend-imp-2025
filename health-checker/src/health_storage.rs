use async_trait::async_trait;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::health_monitor::ProcessorHealthStatus;

#[async_trait]
pub trait HealthStorage: Send + Sync {
    async fn set_processor_health(
        &self,
        processor_name: &str,
        health_status: &ProcessorHealthStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    async fn get_processor_health(
        &self,
        processor_name: &str,
    ) -> Result<Option<ProcessorHealthStatus>, Box<dyn std::error::Error + Send + Sync>>;

    async fn check_rate_limit(
        &self,
        processor_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    async fn set_rate_limit(
        &self,
        processor_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

pub struct RedisHealthStorage {
    client: redis::Client,
    health_status_ttl: u64,
    rate_limit_ttl: u64,
}

impl RedisHealthStorage {
    pub fn new(
        redis_url: &str,
        health_status_ttl: u64,
        rate_limit_ttl: u64,
    ) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self {
            client,
            health_status_ttl,
            rate_limit_ttl,
        })
    }
}

#[async_trait]
impl HealthStorage for RedisHealthStorage {
    async fn set_processor_health(
        &self,
        processor_name: &str,
        health_status: &ProcessorHealthStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let key = format!("health:{processor_name}");
        let json_data = serde_json::to_string(health_status)?;

        let _: () = conn.set_ex(&key, json_data, self.health_status_ttl).await?;
        Ok(())
    }

    async fn get_processor_health(
        &self,
        processor_name: &str,
    ) -> Result<Option<ProcessorHealthStatus>, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let key = format!("health:{processor_name}");

        let json_data: Option<String> = conn.get::<_, Option<String>>(&key).await?;
        match json_data {
            Some(data) => {
                let health_status: ProcessorHealthStatus = serde_json::from_str(&data)?;
                Ok(Some(health_status))
            }
            None => Ok(None),
        }
    }

    async fn check_rate_limit(
        &self,
        processor_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let rate_limit_key = format!("rate_limit:{processor_name}");

        let exists: bool = conn.exists(&rate_limit_key).await?;
        Ok(!exists)
    }

    async fn set_rate_limit(
        &self,
        processor_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let rate_limit_key = format!("rate_limit:{processor_name}");

        let _: () = conn
            .set_ex(&rate_limit_key, "1", self.rate_limit_ttl)
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct RateLimitEntry {
    timestamp: std::time::Instant,
    ttl_seconds: u64,
}

impl RateLimitEntry {
    fn is_expired(&self) -> bool {
        self.timestamp.elapsed().as_secs() > self.ttl_seconds
    }
}

#[allow(dead_code)]
pub struct MockHealthStorage {
    health_data: Arc<Mutex<HashMap<String, ProcessorHealthStatus>>>,
    rate_limits: Arc<Mutex<HashMap<String, RateLimitEntry>>>,
    health_status_ttl: u64,
    rate_limit_ttl: u64,
}

impl MockHealthStorage {
    pub fn new(health_status_ttl: u64, rate_limit_ttl: u64) -> Self {
        Self {
            health_data: Arc::new(Mutex::new(HashMap::new())),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
            health_status_ttl,
            rate_limit_ttl,
        }
    }
}

#[async_trait]
impl HealthStorage for MockHealthStorage {
    async fn set_processor_health(
        &self,
        processor_name: &str,
        health_status: &ProcessorHealthStatus,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut health_data = self.health_data.lock().unwrap();
        health_data.insert(processor_name.to_string(), health_status.clone());
        Ok(())
    }

    async fn get_processor_health(
        &self,
        processor_name: &str,
    ) -> Result<Option<ProcessorHealthStatus>, Box<dyn std::error::Error + Send + Sync>> {
        let health_data = self.health_data.lock().unwrap();
        Ok(health_data.get(processor_name).cloned())
    }

    async fn check_rate_limit(
        &self,
        processor_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut rate_limits = self.rate_limits.lock().unwrap();

        if let Some(entry) = rate_limits.get(processor_name) {
            if entry.is_expired() {
                rate_limits.remove(processor_name);
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(true)
        }
    }

    async fn set_rate_limit(
        &self,
        processor_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut rate_limits = self.rate_limits.lock().unwrap();
        rate_limits.insert(
            processor_name.to_string(),
            RateLimitEntry {
                timestamp: std::time::Instant::now(),
                ttl_seconds: self.rate_limit_ttl,
            },
        );
        Ok(())
    }
}

