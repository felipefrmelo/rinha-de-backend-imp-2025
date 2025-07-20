use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HealthCheckerConfig {
    pub redis_url: String,
    pub health_check_cycle_interval: Duration,
    pub http_timeout: Duration,
    pub inter_check_delay: Duration,
    pub health_status_ttl: u64,
    pub rate_limit_ttl: u64,
    pub default_processor_url: String,
    pub fallback_processor_url: String,
    pub failed_response_time_value: u64,
}

impl HealthCheckerConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let config = Self {
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://redis:6379".to_string()),
            
            health_check_cycle_interval: Duration::from_secs(
                std::env::var("HEALTH_CHECK_CYCLE_INTERVAL_SECS")
                    .unwrap_or_else(|_| "4".to_string())
                    .parse::<u64>()?
            ),
            
            http_timeout: Duration::from_secs(
                std::env::var("HTTP_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse::<u64>()?
            ),
            
            inter_check_delay: Duration::from_millis(
                std::env::var("INTER_CHECK_DELAY_MILLIS")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse::<u64>()?
            ),
            
            health_status_ttl: std::env::var("HEALTH_STATUS_TTL_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse::<u64>()?,
            
            rate_limit_ttl: std::env::var("RATE_LIMIT_TTL_SECS")
                .unwrap_or_else(|_| "5".to_string())
                .parse::<u64>()?,
            
            default_processor_url: std::env::var("DEFAULT_PROCESSOR_URL")
                .unwrap_or_else(|_| "http://payment-processor-default:8080".to_string()),
            
            fallback_processor_url: std::env::var("FALLBACK_PROCESSOR_URL")
                .unwrap_or_else(|_| "http://payment-processor-fallback:8080".to_string()),
            
            failed_response_time_value: std::env::var("FAILED_RESPONSE_TIME_VALUE")
                .unwrap_or_else(|_| u64::MAX.to_string())
                .parse::<u64>()?,
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.health_check_cycle_interval.as_secs() == 0 {
            return Err("Health check cycle interval must be greater than 0".into());
        }
        
        if self.http_timeout.as_secs() == 0 {
            return Err("HTTP timeout must be greater than 0".into());
        }
        
        if self.health_status_ttl == 0 {
            return Err("Health status TTL must be greater than 0".into());
        }
        
        if self.rate_limit_ttl == 0 {
            return Err("Rate limit TTL must be greater than 0".into());
        }
        
        if self.default_processor_url.is_empty() {
            return Err("Default processor URL cannot be empty".into());
        }
        
        if self.fallback_processor_url.is_empty() {
            return Err("Fallback processor URL cannot be empty".into());
        }

        Ok(())
    }

    pub fn log_configuration(&self) {
        println!("Health Checker Configuration:");
        println!("  Redis URL: {}", self.redis_url);
        println!("  Health check cycle interval: {:?}", self.health_check_cycle_interval);
        println!("  HTTP timeout: {:?}", self.http_timeout);
        println!("  Inter-check delay: {:?}", self.inter_check_delay);
        println!("  Health status TTL: {}s", self.health_status_ttl);
        println!("  Rate limit TTL: {}s", self.rate_limit_ttl);
        println!("  Default processor URL: {}", self.default_processor_url);
        println!("  Fallback processor URL: {}", self.fallback_processor_url);
        println!("  Failed response time value: {}", self.failed_response_time_value);
    }
}
