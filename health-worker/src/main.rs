use reqwest::Client;
use serde::Deserialize;
use std::{collections::HashMap, error::Error, time::Duration};
use tokio::time::Instant;

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub failing: bool,
    #[serde(rename = "minResponseTime")]
    pub min_response_time: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessorInfo {
    pub name: String,
    pub url: String,
    pub is_healthy: bool,
    pub min_response_time: u64,
    pub last_checked: Instant,
    pub failure_count: u32,
}

impl ProcessorInfo {
    pub fn new(name: String, url: String) -> Self {
        Self {
            name,
            url,
            is_healthy: true,
            min_response_time: 0,
            last_checked: Instant::now() - Duration::from_secs(10), // Allow immediate check
            failure_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthWorker {
    client: Client,
    processors: HashMap<String, ProcessorInfo>,
}

impl HealthWorker {
    pub fn new() -> Self {
        let mut processors = HashMap::new();
        
        processors.insert(
            "default".to_string(),
            ProcessorInfo::new(
                "default".to_string(),
                "http://payment-processor-default:8080".to_string(),
            ),
        );
        
        processors.insert(
            "fallback".to_string(),
            ProcessorInfo::new(
                "fallback".to_string(),
                "http://payment-processor-fallback:8080".to_string(),
            ),
        );

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| Client::new()),
            processors,
        }
    }

    /// Get all processors information
    pub fn get_processors(&self) -> &HashMap<String, ProcessorInfo> {
        &self.processors
    }

    /// Get a specific processor by name
    pub fn get_processor(&self, name: &str) -> Option<&ProcessorInfo> {
        self.processors.get(name)
    }

    /// Check health of a specific processor
    pub async fn check_processor_health(&self, processor_name: &str) -> Result<HealthResponse, Box<dyn Error>> {
        let processor = self.processors.get(processor_name)
            .ok_or_else(|| format!("Processor '{}' not found", processor_name))?;

        let response = self
            .client
            .get(format!("{}/payments/service-health", processor.url))
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if response.status().is_success() {
            let health: HealthResponse = response.json().await?;
            Ok(health)
        } else {
            Err(format!("Health check failed with status: {} for processor '{}'", 
                       response.status(), processor_name).into())
        }
    }

    /// Update health status for a specific processor
    pub async fn update_processor_health(&mut self, processor_name: &str) -> Result<(), Box<dyn Error>> {
        let should_check = {
            let processor = self.processors.get(processor_name)
                .ok_or_else(|| format!("Processor '{}' not found", processor_name))?;
            
            // Respect 5-second rate limit
            processor.last_checked.elapsed() >= Duration::from_secs(5)
        };

        if should_check {
            match self.check_processor_health(processor_name).await {
                Ok(health_response) => {
                    if let Some(processor) = self.processors.get_mut(processor_name) {
                        processor.is_healthy = !health_response.failing;
                        processor.min_response_time = health_response.min_response_time;
                        processor.last_checked = Instant::now();
                        if processor.is_healthy {
                            processor.failure_count = 0;
                        }
                        println!("Updated {} processor health: healthy={}, min_response_time={}ms", 
                                processor_name, processor.is_healthy, processor.min_response_time);
                    }
                }
                Err(e) => {
                    if let Some(processor) = self.processors.get_mut(processor_name) {
                        processor.is_healthy = false;
                        processor.failure_count += 1;
                        processor.last_checked = Instant::now();
                        println!("Failed to check {} processor health: {}, marking as unhealthy", 
                                processor_name, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Update health for all processors
    pub async fn update_all_processors_health(&mut self) -> Result<(), Box<dyn Error>> {
        let processor_names: Vec<String> = self.processors.keys().cloned().collect();
        
        for processor_name in processor_names {
            self.update_processor_health(&processor_name).await?;
        }

        Ok(())
    }

    /// Get healthy processors
    pub fn get_healthy_processors(&self) -> Vec<&ProcessorInfo> {
        self.processors
            .values()
            .filter(|processor| processor.is_healthy)
            .collect()
    }

    /// Get unhealthy processors
    pub fn get_unhealthy_processors(&self) -> Vec<&ProcessorInfo> {
        self.processors
            .values()
            .filter(|processor| !processor.is_healthy)
            .collect()
    }

    /// Run health monitoring loop
    pub async fn run_health_monitoring(&mut self, interval_seconds: u64) -> Result<(), Box<dyn Error>> {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.update_all_processors_health().await {
                eprintln!("Error updating processor health: {}", e);
            }

            // Log current status
            println!("Health Check Summary:");
            for (name, processor) in &self.processors {
                println!("  {} -> healthy: {}, response_time: {}ms, failures: {}", 
                        name, processor.is_healthy, processor.min_response_time, processor.failure_count);
            }
        }
    }
}

impl Default for HealthWorker {
    fn default() -> Self {
        Self::new()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting Health Worker...");

    let mut health_worker = HealthWorker::new();

    // Print initial processors
    println!("Initialized processors:");
    for (name, processor) in health_worker.get_processors() {
        println!("  {} -> {}", name, processor.url);
    }

    // Run health monitoring every 10 seconds
    health_worker.run_health_monitoring(10).await?;

    Ok(())
}