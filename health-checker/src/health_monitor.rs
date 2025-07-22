use crate::config::HealthCheckerConfig;
use crate::health_storage::HealthStorage;
use crate::http_client::HttpClient;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tokio::time;

#[derive(Error, Debug)]
pub enum HealthMonitorError {
    #[error("Storage error: {0}")]
    Storage(#[from] crate::health_storage::HealthStorageError),
    #[error("HTTP client error: {0}")]
    Http(#[from] crate::http_client::HttpClientError),
    #[error("Failed to parse health response: {0}")]
    ParseError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Processor not found: {0}")]
    ProcessorNotFound(String),
}

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
pub struct ProcessorDefault {
    url: String,
}

impl ProcessorDefault {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub const fn name(&self) -> &'static str {
        "default"
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessorFallback {
    url: String,
}

impl ProcessorFallback {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub const fn name(&self) -> &'static str {
        "fallback"
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Processor {
    Default(ProcessorDefault),
    Fallback(ProcessorFallback),
}

impl Processor {
    pub const fn name(&self) -> &'static str {
        match self {
            Processor::Default(p) => p.name(),
            Processor::Fallback(p) => p.name(),
        }
    }

    pub fn url(&self) -> &str {
        match self {
            Processor::Default(p) => p.url(),
            Processor::Fallback(p) => p.url(),
        }
    }
}

pub struct HealthMonitor {
    storage: Box<dyn HealthStorage>,
    http_client: Box<dyn HttpClient>,
    config: HealthCheckerConfig,
    processors: Vec<Processor>,
}

impl HealthMonitor {
    pub fn new(
        storage: Box<dyn HealthStorage>,
        http_client: Box<dyn HttpClient>,
        config: HealthCheckerConfig,
        processors: Vec<Processor>,
    ) -> Result<Self, HealthMonitorError> {
        Ok(Self {
            storage,
            http_client,
            config,
            processors,
        })
    }

    pub fn build(
        storage: Box<dyn HealthStorage>,
        http_client: Box<dyn HttpClient>,
    ) -> Result<Self, HealthMonitorError> {
        let config = HealthCheckerConfig::from_env()
            .map_err(|e| HealthMonitorError::ConfigError(e.to_string()))?;
        let processor_default = ProcessorDefault::new(config.default_processor_url.clone());
        let processor_fallback = ProcessorFallback::new(config.fallback_processor_url.clone());
        let processors = vec![
            Processor::Default(processor_default),
            Processor::Fallback(processor_fallback),
        ];
        Self::new(storage, http_client, config, processors)
    }

    pub fn get_cycle_interval(&self) -> Duration {
        self.config.health_check_cycle_interval
    }

    pub async fn check_processor_health(
        &self,
        processor: &Processor,
    ) -> Result<(), HealthMonitorError> {
        // Check rate limit before making the call
        if !self.storage.check_rate_limit(processor.name()).await? {
//            println!(
//                "Rate limit: Skipping health check for {} (within 5-second window)",
//                processor.name()
//            );
            return Ok(());
        }

        let url = format!("{}/payments/service-health", processor.url());
        println!("Checking health for {} at {}", processor.name(), url);

        self.storage.set_rate_limit(processor.name()).await?;

        let response = match self.http_client.get(&url).await {
            Ok(resp) => resp,
            Err(e) => {
                let health_status =
                    ProcessorHealthStatus::new(true, self.config.failed_response_time_value);
                self.storage
                    .set_processor_health(processor.name(), &health_status)
                    .await?;
                return Err(HealthMonitorError::Http(e));
            }
        };

        if !response.is_success {
            return Err(HealthMonitorError::ParseError(format!(
                "HTTP request failed with status: {}",
                response.status_code
            )));
        }

        let health_data = response
            .json::<ServiceHealthResponse>()
            .map_err(|e| HealthMonitorError::ParseError(e.to_string()))?;

        let health_status =
            ProcessorHealthStatus::new(health_data.failing, health_data.min_response_time);
        self.storage
            .set_processor_health(processor.name(), &health_status)
            .await?;

        Ok(())
    }

    pub async fn monitor_all_processors(&self) -> Result<(), HealthMonitorError> {
        // Check default processor
        let default_processor = Processor::Default(ProcessorDefault::new(
            self.config.default_processor_url.clone(),
        ));
        self.check_processor_health(&default_processor).await?;

        // Small delay between checks to avoid overwhelming
        time::sleep(self.config.inter_check_delay).await;

        // Check fallback processor
        let fallback_processor = Processor::Fallback(ProcessorFallback::new(
            self.config.fallback_processor_url.clone(),
        ));
        self.check_processor_health(&fallback_processor).await?;

        Ok(())
    }

    pub async fn get_best_processor(&self) -> Result<Processor, HealthMonitorError> {
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

        let processor = match processor_name {
            "default" => Processor::Default(ProcessorDefault::new(
                self.config.default_processor_url.clone(),
            )),
            "fallback" => Processor::Fallback(ProcessorFallback::new(
                self.config.fallback_processor_url.clone(),
            )),
            _ => Processor::Default(ProcessorDefault::new(
                self.config.default_processor_url.clone(),
            )),
        };

        Ok(processor)
    }

    pub async fn run(&self) -> ! {
        println!("Health checker initialized. Starting monitoring loop...");

        loop {
            match self.monitor_all_processors().await {
                Ok(()) => {
                    println!("Health check cycle completed successfully");
                }
                Err(e) => {
                    eprintln!("Error during health check cycle: {e}");
                }
            }

            time::sleep(self.get_cycle_interval()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health_storage::MockHealthStorage;
    use crate::http_client::MockHttpClient;
    use std::time::Duration;

    // Test constants
    const DEFAULT_URL: &str = "http://default-processor:8080";
    const FALLBACK_URL: &str = "http://fallback-processor:8080";
    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    // Test data helpers
    struct TestHealthData {
        failing: bool,
        response_time: u64,
    }

    impl TestHealthData {
        const fn healthy(response_time: u64) -> Self {
            Self {
                failing: false,
                response_time,
            }
        }

        const fn unhealthy(response_time: u64) -> Self {
            Self {
                failing: true,
                response_time,
            }
        }
    }

    // Test setup helpers
    fn create_test_config(default_url: &str, fallback_url: &str) -> HealthCheckerConfig {
        HealthCheckerConfig {
            redis_url: "redis://localhost:6379".to_string(),
            health_status_ttl: 60,
            rate_limit_ttl: 5,
            http_timeout: TEST_TIMEOUT,
            health_check_cycle_interval: Duration::from_secs(30),
            inter_check_delay: Duration::from_millis(100),
            default_processor_url: default_url.to_string(),
            fallback_processor_url: fallback_url.to_string(),
            failed_response_time_value: 9999,
        }
    }

    fn create_monitor_with_urls(
        storage: MockHealthStorage,
        http_client: MockHttpClient,
        default_url: &str,
        fallback_url: &str,
    ) -> HealthMonitor {
        let config = create_test_config(default_url, fallback_url);
        let processors = vec![
            Processor::Default(ProcessorDefault::new(default_url.to_string())),
            Processor::Fallback(ProcessorFallback::new(fallback_url.to_string())),
        ];
        HealthMonitor::new(Box::new(storage), Box::new(http_client), config, processors).unwrap()
    }

    async fn setup_health_data(
        storage: &dyn HealthStorage,
        default: TestHealthData,
        fallback: TestHealthData,
    ) {
        let default_health = ProcessorHealthStatus::new(default.failing, default.response_time);
        let fallback_health = ProcessorHealthStatus::new(fallback.failing, fallback.response_time);

        storage
            .set_processor_health("default", &default_health)
            .await
            .unwrap();
        storage
            .set_processor_health("fallback", &fallback_health)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_check_processor_health() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new().with_response(
            &format!("{DEFAULT_URL}/payments/service-health"),
            200,
            r#"{"failing": false, "minResponseTime": 150}"#,
        );
        let monitor = create_monitor_with_urls(storage, http_client, DEFAULT_URL, FALLBACK_URL);
        let processor = Processor::Default(ProcessorDefault::new(DEFAULT_URL.to_string()));

        let result = monitor.check_processor_health(&processor).await;
        assert!(result.is_ok());

        let processor = monitor.get_best_processor().await.unwrap();

        assert_eq!(processor.name(), "default");
    }

    #[tokio::test]
    async fn test_get_best_processor() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new();
        let monitor = create_monitor_with_urls(storage, http_client, DEFAULT_URL, FALLBACK_URL);

        // Test 1: No health data - should default to default
        let best = monitor.get_best_processor().await.unwrap();
        assert_eq!(best.name(), "default");

        // Test 2: Fallback significantly faster - should choose fallback
        setup_health_data(
            monitor.storage.as_ref(),
            TestHealthData::healthy(300),
            TestHealthData::healthy(100),
        )
        .await;
        let best = monitor.get_best_processor().await.unwrap();
        assert_eq!(best.name(), "fallback");

        // Test 3: Default healthy, fallback failing - should choose default
        setup_health_data(
            monitor.storage.as_ref(),
            TestHealthData::healthy(200),
            TestHealthData::unhealthy(500),
        )
        .await;
        let best = monitor.get_best_processor().await.unwrap();
        assert_eq!(best.name(), "default");
    }

    #[tokio::test]
    async fn test_monitor_all_processors() {
        let storage = MockHealthStorage::new(60, 5);
        let http_client = MockHttpClient::new()
            .with_response(
                &format!("{DEFAULT_URL}/payments/service-health"),
                200,
                r#"{"failing": false, "minResponseTime": 180}"#,
            )
            .with_response(
                &format!("{FALLBACK_URL}/payments/service-health"),
                200,
                r#"{"failing": false, "minResponseTime": 220}"#,
            );
        let monitor = create_monitor_with_urls(storage, http_client, DEFAULT_URL, FALLBACK_URL);

        let result = monitor.monitor_all_processors().await;
        assert!(result.is_ok());

        let default_health = monitor
            .storage
            .get_processor_health("default")
            .await
            .unwrap()
            .unwrap();
        let fallback_health = monitor
            .storage
            .get_processor_health("fallback")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(default_health.min_response_time, 180);
        assert_eq!(fallback_health.min_response_time, 220);
    }
}
