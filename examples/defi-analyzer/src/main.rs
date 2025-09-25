//! DeFi Analyzer Example

use anyhow::Result;
use alloy_primitives::{Address, U256, Bytes};
use std::collections::HashMap;
use tracing::{info, error};

use defi_analyzer::{
    DeFiAnalyzerStrategy,
    config::{AnalyzerConfig, ConfigLoader},
    types::{AnalysisEvent, EventType},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("🚀 Starting DeFi Analyzer Example");
    
    // Load configuration
    let config = load_config().await?;
    info!("Configuration loaded successfully");
    
    // Create strategy
    let mut strategy = DeFiAnalyzerStrategy::new(config);
    
    // Initialize strategy
    strategy.initialize().await?;
    info!("Strategy initialized successfully");
    
    // Create sample events
    let sample_events = create_sample_events();
    
    // Process events
    for (i, event) in sample_events.into_iter().enumerate() {
        info!("Processing event {}: {:?}", i + 1, event.event_type);
        
        // Process event
        let actions = strategy.process_event(event).await;
        
        info!("Generated {} actions", actions.len());
        
        // Print statistics
        let stats = strategy.get_stats();
        info!("Statistics: events={}, opportunities={}, actions={}", 
              stats.events_processed, stats.opportunities_found, stats.actions_generated);
    }
    
    // Log performance report
    info!("📊 Performance Report:");
    strategy.log_performance_report();
    
    info!("✅ DeFi Analyzer Example completed successfully");
    Ok(())
}

async fn load_config() -> Result<AnalyzerConfig> {
    // Try to load from file first
    if let Ok(config) = ConfigLoader::from_file("config/defi_analyzer.toml") {
        return Ok(config);
    }
    
    // Fall back to environment variables
    if let Ok(config) = ConfigLoader::from_env() {
        return Ok(config);
    }
    
    // Use default configuration
    info!("Using default configuration");
    Ok(AnalyzerConfig::default())
}

fn create_sample_events() -> Vec<AnalysisEvent> {
    vec![
        // Mempool transaction event
        AnalysisEvent {
            event_type: "mempool_analysis".to_string(),
            event_kind: Some(EventType::MempoolTransaction),
            contract_address: Address::from([0x01; 20]),
            transaction_hash: [0u8; 32],
            transaction_data: vec![0x12, 0x34, 0x56, 0x78],
            tx_data: Some(vec![0x12, 0x34, 0x56, 0x78]),
            event_data: vec![],
            block_number: 18500000,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        },
        
        // Block with DeFi activity
        AnalysisEvent {
            event_type: "block_analysis".to_string(),
            event_kind: Some(EventType::BlockWithDeFiActivity),
            contract_address: Address::from([0x02; 20]),
            transaction_hash: [0u8; 32],
            transaction_data: vec![],
            tx_data: None,
            event_data: vec![],
            block_number: 18500001,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        },
        
        // Contract deployment
        AnalysisEvent {
            event_type: "contract_deployment".to_string(),
            event_kind: Some(EventType::ContractDeployment),
            contract_address: Address::from([0x03; 20]),
            transaction_hash: [0u8; 32],
            transaction_data: vec![0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45],
            tx_data: Some(vec![0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45]),
            event_data: vec![],
            block_number: 18500002,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        },
        
        // MEV-Share event
        AnalysisEvent {
            event_type: "mevshare".to_string(),
            event_kind: Some(EventType::MevShareEvent),
            contract_address: Address::from([0x04; 20]),
            transaction_hash: [0u8; 32],
            transaction_data: vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
            tx_data: Some(vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]),
            event_data: vec![],
            block_number: 18500003,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: HashMap::new(),
        },
        
        // Custom analysis
        AnalysisEvent {
            event_type: "custom".to_string(),
            event_kind: Some(EventType::CustomAnalysis),
            contract_address: Address::from([0x05; 20]),
            transaction_hash: [0u8; 32],
            transaction_data: vec![],
            tx_data: None,
            event_data: vec![],
            block_number: 18500004,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("analysis_type".to_string(), "custom_arbitrage".to_string());
                metadata.insert("priority".to_string(), "high".to_string());
                metadata
            },
        },
    ]
}
