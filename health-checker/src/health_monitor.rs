use crate::health_storage::HealthStorage;
use crate::http_client::HttpClient;
use crate::config::HealthCheckerConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time;


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


#[derive(Debug, Serialize, Deserialize)]
struct ServiceHealthResponse {
    failing: bool,
    #[serde(rename = "minResponseTime")]
    min_response_time: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Processor {
    pub name: String,
    pub url: String,
}

pub struct HealthMonitor {
    storage: Box<dyn HealthStorage>,
    http_client: Box<dyn HttpClient>,
    config: HealthCheckerConfig,
}

impl HealthMonitor {
    pub fn new(
        storage: Box<dyn HealthStorage>,
        http_client: Box<dyn HttpClient>,
        config: HealthCheckerConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            storage,
            http_client,
            config,
        })
    }

    pub fn get_cycle_interval(&self) -> Duration {
        self.config.health_check_cycle_interval
    }

    pub async fn check_processor_health(
        &self,
        processor: &Processor,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check rate limit before making the call
        if !self.storage.check_rate_limit(&processor.name).await? {
            println!("Rate limit: Skipping health check for {} (within 5-second window)", processor.name);
            return Ok(());
        }

        let url = format!("{}/payments/service-health", processor.url);
        println!("Checking health for {} at {}", processor.name, url);

        // Set rate limit immediately before making the call
        self.storage.set_rate_limit(&processor.name).await?;

        let response = match self.http_client.get(&url).await {
            Ok(resp) => resp,
            Err(_) => {
                let health_status = ProcessorHealthStatus::new(true, self.config.failed_response_time_value);
                if let Err(storage_err) = self.storage.set_processor_health(&processor.name, &health_status).await {
                    eprintln!("Failed to update storage with unhealthy status for {}: {}", processor.name, storage_err);
                }
                return Ok(());
            }
        };

        if response.is_success {
            match response.json::<ServiceHealthResponse>() {
                Ok(health_data) => {
                    let health_status = ProcessorHealthStatus::new(
                        health_data.failing,
                        health_data.min_response_time,
                    );
                    self.storage.set_processor_health(&processor.name, &health_status).await?;
                    println!(
                        "Health check for {}: failing={}, min_response_time={}ms",
                        processor.name, health_data.failing, health_data.min_response_time
                    );
                }
                Err(e) => {
                    eprintln!("Failed to parse health response for {}: {}", processor.name, e);
                }
            }
        } else if response.status_code() == 429 {
            eprintln!("Rate limited by {} (HTTP 429)", processor.name);
        } else {
            eprintln!("Health check failed for {} with status: {}", processor.name, response.status_code());
        }

        Ok(())
    }

    pub async fn monitor_all_processors(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check default processor
        let default_processor = Processor {
            name: "default".to_string(),
            url: self.config.default_processor_url.clone(),
        };
        self.check_processor_health(&default_processor).await?;
        
        // Small delay between checks to avoid overwhelming
        time::sleep(self.config.inter_check_delay).await;
        
        // Check fallback processor
        let fallback_processor = Processor {
            name: "fallback".to_string(),
            url: self.config.fallback_processor_url.clone(),
        };
        self.check_processor_health(&fallback_processor).await?;
        
        Ok(())
    }

    pub async fn get_best_processor(&self) -> Result<Processor, Box<dyn std::error::Error + Send + Sync>> {
        let default_health = self.storage.get_processor_health("default").await?;
        let fallback_health = self.storage.get_processor_health("fallback").await?;

        let processor_name = match (default_health, fallback_health) {
            (Some(default), Some(fallback)) => {
                // Both processors available - compare performance
                if !default.failing && !fallback.failing {
                    // Both healthy - prefer fallback if it's significantly faster
                    if fallback.min_response_time * 2 < default.min_response_time {
                        "fallback"
                    } else {
                        "default" // Default for lower fees
                    }
                } else if !default.failing {
                    "default"
                } else if !fallback.failing {
                    "fallback"
                } else {
                    // Both failing, choose the one with better response time
                    if fallback.min_response_time < default.min_response_time {
                        "fallback"
                    } else {
                        "default"
                    }
                }
            }
            (Some(default), None) => {
                if !default.failing {
                    "default"
                } else {
                    "fallback" // Try fallback as last resort
                }
            }
            (None, Some(fallback)) => {
                if !fallback.failing {
                    "fallback"
                } else {
                    "default" // Try default as last resort
                }
            }
            (None, None) => {
                // No health data available, default to default processor
                "default"
            }
        };

        let url = match processor_name {
            "default" => &self.config.default_processor_url,
            "fallback" => &self.config.fallback_processor_url,
            _ => &self.config.default_processor_url,
        };

        Ok(Processor {
            name: processor_name.to_string(),
            url: url.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::MockHttpClient;
    use crate::health_storage::MockHealthStorage;
    use std::time::Duration;

    fn create_test_config() -> HealthCheckerConfig {
        HealthCheckerConfig {
            redis_url: "redis://localhost".to_string(),
            health_status_ttl: 60,
            rate_limit_ttl: 5,
            http_timeout: Duration::from_secs(10),
            health_check_cycle_interval: Duration::from_secs(30),
            inter_check_delay: Duration::from_millis(100),
            default_processor_url: "http://default.example.com".to_string(),
            fallback_processor_url: "http://fallback.example.com".to_string(),
            failed_response_time_value: 9999,
        }
    }

    fn create_test_monitor(
        storage: MockHealthStorage,
        http_client: MockHttpClient,
    ) -> HealthMonitor {
        let config = create_test_config();
        HealthMonitor::new(
            Box::new(storage),
            Box::new(http_client),
            config,
        ).unwrap()
    }

    #[tokio::test]
    async fn test_check_processor_health_success() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new()
            .with_response(
                "http://example.com/payments/service-health",
                200,
                r#"{"failing": false, "minResponseTime": 150}"#,
            );

        let monitor = create_test_monitor(storage, http_client);

        let processor = Processor {
            name: "test".to_string(),
            url: "http://example.com".to_string(),
        };
        let result = monitor.check_processor_health(&processor).await;
        assert!(result.is_ok());

        let health_status = monitor.get_best_processor().await;
        assert!(health_status.is_ok());
        assert_eq!(health_status.unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_check_processor_health_failing_service() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new()
            .with_response(
                "http://example.com/payments/service-health",
                200,
                r#"{"failing": true, "minResponseTime": 5000}"#,
            );

        let monitor = create_test_monitor(storage, http_client);

        let processor = Processor {
            name: "test".to_string(),
            url: "http://example.com".to_string(),
        };
        let result = monitor.check_processor_health(&processor).await;
        assert!(result.is_ok());
        assert!(monitor.get_best_processor().await.is_ok());
    }

    #[tokio::test]
    async fn test_check_processor_health_http_error() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new()
            .with_response("http://example.com/payments/service-health", 500, "Internal Server Error");

        let monitor = create_test_monitor(storage, http_client);

        let processor = Processor {
            name: "test".to_string(),
            url: "http://example.com".to_string(),
        };
        let result = monitor.check_processor_health(&processor).await;
        assert!(result.is_ok()); // Should not fail, just log the error
    }

    #[tokio::test]
    async fn test_check_processor_health_rate_limited() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new()
            .with_response("http://example.com/payments/service-health", 429, "Too Many Requests");

        let monitor = create_test_monitor(storage, http_client);

        let processor = Processor {
            name: "test".to_string(),
            url: "http://example.com".to_string(),
        };
        let result = monitor.check_processor_health(&processor).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_processor_health_invalid_json() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new()
            .with_response("http://example.com/payments/service-health", 200, "invalid json");

        let monitor = create_test_monitor(storage, http_client);

        let processor = Processor {
            name: "test".to_string(),
            url: "http://example.com".to_string(),
        };
        let result = monitor.check_processor_health(&processor).await;
        assert!(result.is_ok()); // Should not fail, just log the error
    }

    #[tokio::test]
    async fn test_get_best_processor_both_healthy_default_faster() {
        let storage = MockHealthStorage::new(60, 5);
        
        // Set up health data - default faster than fallback
        let default_health = ProcessorHealthStatus::new(false, 100);
        let fallback_health = ProcessorHealthStatus::new(false, 300);
        
        storage.set_processor_health("default", &default_health).await.unwrap();
        storage.set_processor_health("fallback", &fallback_health).await.unwrap();

        let http_client = MockHttpClient::new();
        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.get_best_processor().await.unwrap();
        assert_eq!(result.name, "default");
        assert_eq!(result.url, "http://default.example.com");
    }

    #[tokio::test]
    async fn test_get_best_processor_both_healthy_fallback_significantly_faster() {
        let storage = MockHealthStorage::new(60, 5);
        
        // Set up health data - fallback significantly faster (more than 2x)
        let default_health = ProcessorHealthStatus::new(false, 1000);
        let fallback_health = ProcessorHealthStatus::new(false, 400); // Less than half
        
        storage.set_processor_health("default", &default_health).await.unwrap();
        storage.set_processor_health("fallback", &fallback_health).await.unwrap();

        let http_client = MockHttpClient::new();
        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.get_best_processor().await.unwrap();
        assert_eq!(result.name, "fallback");
        assert_eq!(result.url, "http://fallback.example.com");
        assert_eq!(result.url, "http://fallback.example.com");
    }

    #[tokio::test]
    async fn test_get_best_processor_only_default_healthy() {
        let storage = MockHealthStorage::new(60, 5);
        
        let default_health = ProcessorHealthStatus::new(false, 200);
        let fallback_health = ProcessorHealthStatus::new(true, 5000);
        
        storage.set_processor_health("default", &default_health).await.unwrap();
        storage.set_processor_health("fallback", &fallback_health).await.unwrap();

        let http_client = MockHttpClient::new();
        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.get_best_processor().await.unwrap();
        assert_eq!(result.name, "default");
        assert_eq!(result.url, "http://default.example.com");
    }

    #[tokio::test]
    async fn test_get_best_processor_only_fallback_healthy() {
        let storage = MockHealthStorage::new(60, 5);
        
        let default_health = ProcessorHealthStatus::new(true, 5000);
        let fallback_health = ProcessorHealthStatus::new(false, 200);
        
        storage.set_processor_health("default", &default_health).await.unwrap();
        storage.set_processor_health("fallback", &fallback_health).await.unwrap();

        let http_client = MockHttpClient::new();
        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.get_best_processor().await.unwrap();
        assert_eq!(result.name, "fallback");
        assert_eq!(result.url, "http://fallback.example.com");
    }

    #[tokio::test]
    async fn test_get_best_processor_both_failing_choose_faster() {
        let storage = MockHealthStorage::new(60, 5);
        
        let default_health = ProcessorHealthStatus::new(true, 3000);
        let fallback_health = ProcessorHealthStatus::new(true, 2000); // Faster even though failing
        
        storage.set_processor_health("default", &default_health).await.unwrap();
        storage.set_processor_health("fallback", &fallback_health).await.unwrap();

        let http_client = MockHttpClient::new();
        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.get_best_processor().await.unwrap();
        assert_eq!(result.name, "fallback");
        assert_eq!(result.url, "http://fallback.example.com");
    }

    #[tokio::test]
    async fn test_get_best_processor_no_health_data() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new();
        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.get_best_processor().await.unwrap();
        assert_eq!(result.name, "default");
        assert_eq!(result.url, "http://default.example.com"); // Should default to default processor
    }

    #[tokio::test]
    async fn test_get_best_processor_only_default_data() {
        let storage = MockHealthStorage::new(60, 5);
        
        let default_health = ProcessorHealthStatus::new(false, 200);
        storage.set_processor_health("default", &default_health).await.unwrap();

        let http_client = MockHttpClient::new();
        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.get_best_processor().await.unwrap();
        assert_eq!(result.name, "default");
        assert_eq!(result.url, "http://default.example.com");
    }

    #[tokio::test]
    async fn test_get_best_processor_only_fallback_data() {
        let storage = MockHealthStorage::new(60, 5);
        
        let fallback_health = ProcessorHealthStatus::new(false, 200);
        storage.set_processor_health("fallback", &fallback_health).await.unwrap();

        let http_client = MockHttpClient::new();
        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.get_best_processor().await.unwrap();
        assert_eq!(result.name, "fallback");
        assert_eq!(result.url, "http://fallback.example.com");
    }

    #[tokio::test]
    async fn test_monitor_all_processors() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new()
            .with_response(
                "http://default.example.com/payments/service-health",
                200,
                r#"{"failing": false, "minResponseTime": 150}"#,
            )
            .with_response(
                "http://fallback.example.com/payments/service-health",
                200,
                r#"{"failing": false, "minResponseTime": 200}"#,
            );

        let monitor = create_test_monitor(storage, http_client);

        let result = monitor.monitor_all_processors().await;
        assert!(result.is_ok());
    }

    #[tokio::test] 
    async fn test_rate_limiting_blocks_subsequent_calls() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new()
            .with_response(
                "http://example.com/payments/service-health",
                200,
                r#"{"failing": false, "minResponseTime": 150}"#,
            );

        let monitor = create_test_monitor(storage, http_client);

        // First call should succeed and set rate limit
        let processor = Processor {
            name: "test".to_string(),
            url: "http://example.com".to_string(),
        };
        // First call should succeed and set rate limit
        let result1 = monitor.check_processor_health(&processor).await;
        assert!(result1.is_ok());

        // Second call should be blocked by rate limit
        // Second call should be blocked by rate limit
        let result2 = monitor.check_processor_health(&processor).await;
        assert!(result2.is_ok()); // Still returns OK but should be skipped
    }
}
