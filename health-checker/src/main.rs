use health_checker::HealthMonitor;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Payment Processor Health Checker...");
    
    // Redis connection
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379".to_string());
    
    let health_monitor = HealthMonitor::new(&redis_url)?;
    
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
        
        // Wait 4 seconds between cycles to respect the 5-second rate limit
        // This ensures we don't hit the rate limit while being responsive
        time::sleep(Duration::from_secs(4)).await;
    }
}
