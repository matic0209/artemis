//! DeFi Analyzer Configuration
//!
//! This module defines all configuration options for the DeFi analyzer,
//! including analysis parameters, performance settings, security thresholds,
//! and feature toggles. Configuration is designed to be flexible and
//! production-ready.

use alloy_primitives::U256;
use anyhow::Result;
use std::collections::HashMap;

/// Analyzer configuration
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Enable symbolic execution
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
    /// Maximum number of execution paths to explore per analysis
    pub max_execution_paths: u32,
    /// Maximum call depth during execution analysis
    pub max_depth: u32,
    /// Enable arbitrage detection heuristics
    pub enable_arbitrage_detection: bool,
    /// Enable consistency checking between protocol states
    pub enable_consistency_checking: bool,
    /// Contract addresses to monitor
    pub monitored_contracts: Vec<String>,
    /// ABI cache settings
    pub abi_cache_settings: AbiCacheSettings,
    /// Performance settings
    pub performance_settings: PerformanceSettings,
}

/// ABI cache settings
#[derive(Debug, Clone)]
pub struct AbiCacheSettings {
    /// Enable caching
    pub enable_caching: bool,
    /// Cache size limit
    pub cache_size_limit: usize,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
}

/// Performance settings
#[derive(Debug, Clone)]
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
            max_execution_paths: 500,
            max_depth: 16,
            enable_arbitrage_detection: true,
            enable_consistency_checking: true,
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

/// Configuration loader
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration from file
    pub fn from_file(_path: &str) -> Result<AnalyzerConfig> {
        // For now, just return default config
        Ok(AnalyzerConfig::default())
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<AnalyzerConfig> {
        // For now, just return default config
        Ok(AnalyzerConfig::default())
    }
}
