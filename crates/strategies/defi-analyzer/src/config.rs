//! Configuration for DeFi Analyzer Strategy

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use alloy_primitives::U256;

/// Configuration for the DeFi Analyzer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerConfig {
    /// Enable symbolic execution analysis
    pub enable_symbolic_execution: bool,
    /// Enable feature extraction
    pub enable_feature_extraction: bool,
    /// Enable documentation comparison
    pub enable_documentation_comparison: bool,
    /// Analysis timeout in seconds
    pub analysis_timeout_seconds: u64,
    /// Maximum analysis depth
    pub max_analysis_depth: u32,
    /// Minimum profit threshold for arbitrage opportunities (wei)
    pub min_profit_threshold: U256,
    /// Risk tolerance (0-100)
    pub risk_tolerance: u8,
    /// Enable real-time monitoring
    pub enable_realtime_monitoring: bool,
    /// Monitoring interval in seconds
    pub monitoring_interval_seconds: u64,
    /// Contract addresses to monitor
    pub monitored_contracts: Vec<String>,
    /// ABI cache settings
    pub abi_cache_settings: AbiCacheSettings,
    /// Performance settings
    pub performance_settings: PerformanceSettings,
}

/// ABI cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbiCacheSettings {
    /// Enable ABI caching
    pub enable_caching: bool,
    /// Cache size limit
    pub cache_size_limit: usize,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
}

/// Performance settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    /// Maximum concurrent analyses
    pub max_concurrent_analyses: usize,
    /// Enable parallel processing
    pub enable_parallel_processing: bool,
    /// Memory usage limit (MB)
    pub memory_limit_mb: usize,
    /// CPU usage limit (percentage)
    pub cpu_limit_percentage: u8,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            enable_symbolic_execution: true,
            enable_feature_extraction: true,
            enable_documentation_comparison: true,
            analysis_timeout_seconds: 30,
            max_analysis_depth: 10,
            min_profit_threshold: U256::from(100_000_000_000_000_000u64), // 0.1 ETH
            risk_tolerance: 50,
            enable_realtime_monitoring: true,
            monitoring_interval_seconds: 60,
            monitored_contracts: Vec::new(),
            abi_cache_settings: AbiCacheSettings::default(),
            performance_settings: PerformanceSettings::default(),
        }
    }
}

impl Default for AbiCacheSettings {
    fn default() -> Self {
        Self {
            enable_caching: true,
            cache_size_limit: 1000,
            cache_ttl_seconds: 3600, // 1 hour
        }
    }
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_concurrent_analyses: 10,
            enable_parallel_processing: true,
            memory_limit_mb: 1024, // 1 GB
            cpu_limit_percentage: 80,
        }
    }
}

/// Configuration loader for the analyzer
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<AnalyzerConfig, Box<dyn std::error::Error>> {
        let mut config = AnalyzerConfig::default();
        
        // Load from environment variables
        if let Ok(value) = std::env::var("DEFI_ANALYZER_ENABLE_SYMBOLIC_EXECUTION") {
            config.enable_symbolic_execution = value.parse()?;
        }
        
        if let Ok(value) = std::env::var("DEFI_ANALYZER_ANALYSIS_TIMEOUT") {
            config.analysis_timeout_seconds = value.parse()?;
        }
        
        if let Ok(value) = std::env::var("DEFI_ANALYZER_MAX_DEPTH") {
            config.max_analysis_depth = value.parse()?;
        }
        
        if let Ok(value) = std::env::var("DEFI_ANALYZER_MIN_PROFIT_THRESHOLD") {
            config.min_profit_threshold = U256::from_dec_str(&value)?;
        }
        
        if let Ok(value) = std::env::var("DEFI_ANALYZER_RISK_TOLERANCE") {
            config.risk_tolerance = value.parse()?;
        }
        
        if let Ok(value) = std::env::var("DEFI_ANALYZER_MONITORED_CONTRACTS") {
            config.monitored_contracts = value.split(',').map(|s| s.trim().to_string()).collect();
        }
        
        Ok(config)
    }
    
    /// Load configuration from JSON file
    pub fn from_file(path: &str) -> Result<AnalyzerConfig, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: AnalyzerConfig = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    /// Save configuration to JSON file
    pub fn save_to_file(config: &AnalyzerConfig, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
