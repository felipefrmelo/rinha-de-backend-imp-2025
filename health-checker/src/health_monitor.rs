use crate::redis_client::{RedisHealthClient, ProcessorHealthStatus};
use crate::config::HealthCheckerConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time;

#[derive(Debug, Serialize, Deserialize)]
struct ServiceHealthResponse {
    failing: bool,
    #[serde(rename = "minResponseTime")]
    min_response_time: u64,
}

pub struct HealthMonitor {
    redis_client: RedisHealthClient,
    http_client: Client,
    config: HealthCheckerConfig,
}

impl HealthMonitor {
    pub fn new(config: HealthCheckerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let redis_client = RedisHealthClient::new(&config.redis_url, config.health_status_ttl, config.rate_limit_ttl)?;
        let http_client = Client::builder()
            .timeout(config.http_timeout)
            .build()?;

        Ok(Self {
            redis_client,
            http_client,
            config,
        })
    }

    pub fn get_cycle_interval(&self) -> Duration {
        self.config.health_check_cycle_interval
    }

    pub async fn check_processor_health(
        &self,
        processor_name: &str,
        processor_url: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check rate limit before making the call
        if !self.redis_client.check_rate_limit(processor_name).await? {
            println!("Rate limit: Skipping health check for {processor_name} (within 5-second window)");
            return Ok(());
        }

        let url = format!("{processor_url}/payments/service-health");
        println!("Checking health for {processor_name} at {url}");

        // Set rate limit immediately before making the call
        self.redis_client.set_rate_limit(processor_name).await?;

        match self.http_client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<ServiceHealthResponse>().await {
                        Ok(health_data) => {
                            let health_status = ProcessorHealthStatus::new(
                                health_data.failing,
                                health_data.min_response_time,
                            );
                            
                            // Update Redis with health status
                            self.redis_client.set_processor_health(processor_name, &health_status).await?;
                            
                            println!(
                                "Health check for {processor_name}: failing={}, min_response_time={}ms",
                                health_data.failing, health_data.min_response_time
                            );
                        }
                        Err(e) => {
                            eprintln!("Failed to parse health response for {processor_name}: {e}");
                        }
                    }
                } else if response.status().as_u16() == 429 {
                    eprintln!("Rate limited by {processor_name} (HTTP 429)");
                } else {
                    eprintln!("Health check failed for {processor_name} with status: {}", response.status());
                }
            }
            Err(e) => {
                eprintln!("Failed to connect to {processor_name} for health check: {e}");
                
                // Store unhealthy status on connection failure
                let health_status = ProcessorHealthStatus::new(true, self.config.failed_response_time_value);
                if let Err(redis_err) = self.redis_client.set_processor_health(processor_name, &health_status).await {
                    eprintln!("Failed to update Redis with unhealthy status for {processor_name}: {redis_err}");
                }
            }
        }

        Ok(())
    }

    pub async fn monitor_all_processors(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Check default processor
        self.check_processor_health("default", &self.config.default_processor_url).await?;
        
        // Small delay between checks to avoid overwhelming
        time::sleep(self.config.inter_check_delay).await;
        
        // Check fallback processor
        self.check_processor_health("fallback", &self.config.fallback_processor_url).await?;
        
        Ok(())
    }

    pub async fn get_best_processor(&self) -> Result<String, Box<dyn std::error::Error>> {
        let default_health = self.redis_client.get_processor_health("default").await?;
        let fallback_health = self.redis_client.get_processor_health("fallback").await?;

        match (default_health, fallback_health) {
            (Some(default), Some(fallback)) => {
                // Prefer default if it's not failing (lower fees)
                if !default.failing {
                    Ok("default".to_string())
                } else if !fallback.failing {
                    Ok("fallback".to_string())
                } else {
                    // Both failing, choose default anyway (original behavior)
                    Ok("default".to_string())
                }
            }
            (Some(default), None) => {
                if !default.failing {
                    Ok("default".to_string())
                } else {
                    Ok("fallback".to_string()) // Try fallback as last resort
                }
            }
            (None, Some(fallback)) => {
                if !fallback.failing {
                    Ok("fallback".to_string())
                } else {
                    Ok("default".to_string()) // Try default as last resort
                }
            }
            (None, None) => {
                // No health data available, default to default processor
                Ok("default".to_string())
            }
        }
    }
}
