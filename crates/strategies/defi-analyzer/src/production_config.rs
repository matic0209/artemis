//! Production Configuration Management
//! 
//! This module provides production-ready configuration management with
//! environment variables, config files, and runtime validation.

use std::env;
use std::fs;
use std::path::Path;
use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use anyhow::{Result, Context};

use crate::{
    config::AnalyzerConfig,
    jit_strategy_discovery::JITConfig,
    negative_cycle_arbitrage::NegativeCycleConfig,
    error::DeFiResult,
};

/// Production configuration with environment variable support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionConfig {
    /// Analyzer configuration
    pub analyzer: AnalyzerConfig,
    /// JIT strategy configuration
    pub jit_strategy: JITConfig,
    /// Negative cycle arbitrage configuration
    pub negative_cycle: NegativeCycleConfig,
    /// Network configuration
    pub network: NetworkConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
    /// Security configuration
    pub security: SecurityConfig,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// RPC URL
    pub rpc_url: String,
    /// WebSocket URL
    pub ws_url: String,
    /// Chain ID
    pub chain_id: u64,
    /// Block confirmation requirement
    pub block_confirmations: u64,
    /// Request timeout in seconds
    pub request_timeout: u64,
    /// Max retries for failed requests
    pub max_retries: u32,
    /// Rate limiting (requests per second)
    pub rate_limit: u32,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    /// Connection pool size
    pub pool_size: u32,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Query timeout in seconds
    pub query_timeout: u64,
    /// Enable query logging
    pub enable_query_logging: bool,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Metrics port
    pub metrics_port: u16,
    /// Health check port
    pub health_check_port: u16,
    /// Log level
    pub log_level: String,
    /// Enable structured logging
    pub structured_logging: bool,
    /// Alerting webhook URL
    pub alerting_webhook: Option<String>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable API authentication
    pub enable_auth: bool,
    /// API key
    pub api_key: Option<String>,
    /// Enable rate limiting
    pub enable_rate_limiting: bool,
    /// Max requests per minute
    pub max_requests_per_minute: u32,
    /// Enable IP whitelisting
    pub enable_ip_whitelist: bool,
    /// Whitelisted IPs
    pub whitelisted_ips: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY".to_string(),
            ws_url: "wss://eth-mainnet.g.alchemy.com/v2/YOUR_KEY".to_string(),
            chain_id: 1,
            block_confirmations: 1,
            request_timeout: 30,
            max_retries: 3,
            rate_limit: 100,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost:5432/defi_analyzer".to_string(),
            pool_size: 10,
            connection_timeout: 30,
            query_timeout: 60,
            enable_query_logging: false,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            metrics_port: 9090,
            health_check_port: 8080,
            log_level: "info".to_string(),
            structured_logging: true,
            alerting_webhook: None,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_auth: true,
            api_key: None,
            enable_rate_limiting: true,
            max_requests_per_minute: 1000,
            enable_ip_whitelist: false,
            whitelisted_ips: vec![],
        }
    }
}

impl Default for ProductionConfig {
    fn default() -> Self {
        Self {
            analyzer: AnalyzerConfig::default(),
            jit_strategy: JITConfig::default(),
            negative_cycle: NegativeCycleConfig::default(),
            network: NetworkConfig::default(),
            database: DatabaseConfig::default(),
            monitoring: MonitoringConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl ProductionConfig {
    /// Load configuration from environment variables and config file
    pub fn load() -> Result<Self> {
        info!("Loading production configuration...");
        
        // Start with defaults
        let mut config = Self::default();
        
        // Load from config file if exists
        if let Ok(file_config) = Self::load_from_file("config.toml") {
            config = file_config;
        } else if let Ok(file_config) = Self::load_from_file("/etc/defi-analyzer/config.toml") {
            config = file_config;
        }
        
        // Override with environment variables
        config.apply_env_overrides()?;
        
        // Validate configuration
        config.validate()?;
        
        info!("Production configuration loaded successfully");
        Ok(config)
    }
    
    /// Load configuration from file
    fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .context("Failed to read config file")?;
        
        let config: Self = toml::from_str(&content)
            .context("Failed to parse config file")?;
        
        Ok(config)
    }
    
    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) -> Result<()> {
        // Network configuration
        if let Ok(rpc_url) = env::var("DEFI_ANALYZER_RPC_URL") {
            self.network.rpc_url = rpc_url;
        }
        
        if let Ok(ws_url) = env::var("DEFI_ANALYZER_WS_URL") {
            self.network.ws_url = ws_url;
        }
        
        if let Ok(chain_id) = env::var("DEFI_ANALYZER_CHAIN_ID") {
            self.network.chain_id = chain_id.parse().context("Invalid CHAIN_ID")?;
        }
        
        // Database configuration
        if let Ok(db_url) = env::var("DEFI_ANALYZER_DATABASE_URL") {
            self.database.url = db_url;
        }
        
        // Security configuration
        if let Ok(api_key) = env::var("DEFI_ANALYZER_API_KEY") {
            self.security.api_key = Some(api_key);
        }
        
        // Monitoring configuration
        if let Ok(log_level) = env::var("DEFI_ANALYZER_LOG_LEVEL") {
            self.monitoring.log_level = log_level;
        }
        
        if let Ok(webhook) = env::var("DEFI_ANALYZER_ALERTING_WEBHOOK") {
            self.monitoring.alerting_webhook = Some(webhook);
        }
        
        // JIT Strategy configuration
        if let Ok(target_min) = env::var("DEFI_ANALYZER_TARGET_MIN") {
            let target_wei: u64 = target_min.parse().context("Invalid TARGET_MIN")?;
            self.jit_strategy.target_min = U256::from(target_wei);
        }
        
        if let Ok(time_budget) = env::var("DEFI_ANALYZER_TIME_BUDGET_MS") {
            let budget_ms: u64 = time_budget.parse().context("Invalid TIME_BUDGET_MS")?;
            self.jit_strategy.time_budget = std::time::Duration::from_millis(budget_ms);
        }
        
        Ok(())
    }
    
    /// Validate configuration
    fn validate(&self) -> Result<()> {
        // Validate network configuration
        if self.network.rpc_url.is_empty() {
            return Err(anyhow::anyhow!("RPC URL cannot be empty"));
        }
        
        if self.network.chain_id == 0 {
            return Err(anyhow::anyhow!("Chain ID must be greater than 0"));
        }
        
        // Validate profit thresholds
        if self.jit_strategy.target_min == U256::ZERO {
            warn!("Target minimum profit is zero - this may generate too many low-profit strategies");
        }
        
        // Validate time budget
        if self.jit_strategy.time_budget.as_millis() > 10_000 {
            warn!("Time budget is very high (>10s) - this may cause block processing delays");
        }
        
        // Validate security settings
        if self.security.enable_auth && self.security.api_key.is_none() {
            return Err(anyhow::anyhow!("Authentication enabled but no API key provided"));
        }
        
        info!("Configuration validation passed");
        Ok(())
    }
    
    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        fs::write(path.as_ref(), content)
            .context("Failed to write config file")?;
        
        Ok(())
    }
    
    /// Get configuration summary for logging
    pub fn summary(&self) -> String {
        format!(
            "DeFi Analyzer Production Config:\n\
            ├─ Network: {} (Chain ID: {})\n\
            ├─ JIT Target: {} ETH\n\
            ├─ Time Budget: {}ms\n\
            ├─ Max Path Length: {}\n\
            ├─ Security: {} (Auth: {})\n\
            └─ Monitoring: {} (Level: {})",
            self.network.rpc_url.split('/').last().unwrap_or("unknown"),
            self.network.chain_id,
            self.jit_strategy.target_min,
            self.jit_strategy.time_budget.as_millis(),
            self.jit_strategy.max_path_length,
            if self.security.enable_auth { "Enabled" } else { "Disabled" },
            self.security.enable_auth,
            if self.monitoring.enable_metrics { "Enabled" } else { "Disabled" },
            self.monitoring.log_level
        )
    }
}
