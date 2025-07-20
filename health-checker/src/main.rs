use health_checker::{HealthMonitor, HealthCheckerConfig};
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Payment Processor Health Checker...");
    
    // Load configuration
    let config = HealthCheckerConfig::from_env()?;
    config.log_configuration();
    
    let health_monitor = HealthMonitor::new(config)?;
    
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
