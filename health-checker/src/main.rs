use health_checker::{HealthCheckerConfig, HealthMonitor, RedisHealthStorage, ReqwestHttpClient};
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Payment Processor Health Checker...");

    // Load configuration
    let config = HealthCheckerConfig::from_env()?;
    config.log_configuration();

    // Create Redis storage
    let storage = Box::new(RedisHealthStorage::new(
        &config.redis_url,
        config.health_status_ttl,
        config.rate_limit_ttl,
    )?);

    // Create HTTP client
    let http_client = Box::new(ReqwestHttpClient::new(config.http_timeout)?);

    let health_monitor = HealthMonitor::build(storage, http_client)?;

    println!("Health checker initialized. Starting monitoring loop...");

    loop {
        match health_monitor.monitor_all_processors().await {
            Ok(()) => {
                println!("Health check cycle completed successfully");
            }
            Err(e) => {
                eprintln!("Error during health check cycle: {e}");
            }
        }

        time::sleep(health_monitor.get_cycle_interval()).await;
    }
}
