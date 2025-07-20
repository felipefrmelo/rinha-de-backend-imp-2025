use serde::{Deserialize, Serialize};
use redis::AsyncCommands;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorHealthStatus {
    pub failing: bool,
    pub min_response_time: u64,
}

impl ProcessorHealthStatus {
    pub fn new(failing: bool, min_response_time: u64) -> Self {
        Self {
            failing,
            min_response_time,
        }
    }
}

pub struct RedisHealthClient {
    client: redis::Client,
    health_status_ttl: u64,
    rate_limit_ttl: u64,
}

impl RedisHealthClient {
    pub fn new(redis_url: &str, health_status_ttl: u64, rate_limit_ttl: u64) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self { 
            client,
            health_status_ttl,
            rate_limit_ttl,
        })
    }

    pub async fn set_processor_health(
        &self,
        processor_name: &str,
        health_status: &ProcessorHealthStatus,
    ) -> Result<(), redis::RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let key = format!("health:{processor_name}");
        let json_data = serde_json::to_string(health_status).map_err(|e| {
            redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "JSON serialization error",
                e.to_string(),
            ))
        })?;

        let _: () = conn.set_ex(&key, json_data, self.health_status_ttl).await?;
        Ok(())
    }

    pub async fn get_processor_health(
        &self,
        processor_name: &str,
    ) -> Result<Option<ProcessorHealthStatus>, redis::RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let key = format!("health:{processor_name}");

        let json_data: Option<String> = conn.get::<_, Option<String>>(&key).await?;
        match json_data {
            Some(data) => {
                let health_status: ProcessorHealthStatus =
                    serde_json::from_str(&data).map_err(|e| {
                        redis::RedisError::from((
                            redis::ErrorKind::TypeError,
                            "JSON deserialization error",
                            e.to_string(),
                        ))
                    })?;
                Ok(Some(health_status))
            }
            None => Ok(None),
        }
    }

    pub async fn check_rate_limit(&self, processor_name: &str) -> Result<bool, redis::RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let rate_limit_key = format!("rate_limit:{processor_name}");
        
        // Check if rate limit key exists
        let exists: bool = conn.exists(&rate_limit_key).await?;
        Ok(!exists) // Can make call if key doesn't exist
    }

    pub async fn set_rate_limit(&self, processor_name: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.client.get_multiplexed_tokio_connection().await?;
        let rate_limit_key = format!("rate_limit:{processor_name}");
        
        // Set rate limit key with configured TTL
        let _: () = conn.set_ex(&rate_limit_key, "1", self.rate_limit_ttl).await?;
        Ok(())
    }
}
