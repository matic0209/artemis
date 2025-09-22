//! 策略配置管理模块

use std::path::Path;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tracing::{info, warn};

use crate::strategy_impl::ArbConfig;

/// 策略配置文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// 套利配置
    pub arbitrage: ArbConfig,
    /// 网络配置
    pub network: NetworkConfig,
    /// 性能配置
    pub performance: PerformanceConfig,
    /// 监控配置
    pub monitoring: MonitoringConfig,
}

/// 网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// RPC 端点
    pub rpc_url: String,
    /// WebSocket 端点
    pub ws_url: Option<String>,
    /// 链 ID
    pub chain_id: u64,
    /// 超时设置 (秒)
    pub timeout_seconds: u64,
    /// 重试次数
    pub max_retries: u32,
}

/// 性能配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// 最大并发连接数
    pub max_concurrent_connections: usize,
    /// 缓存大小
    pub cache_size: usize,
    /// 批处理大小
    pub batch_size: usize,
    /// 处理超时 (毫秒)
    pub processing_timeout_ms: u64,
}

/// 监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// 是否启用指标收集
    pub enable_metrics: bool,
    /// 指标端口
    pub metrics_port: u16,
    /// 是否启用详细日志
    pub verbose_logging: bool,
    /// 日志级别
    pub log_level: String,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            arbitrage: ArbConfig::default(),
            network: NetworkConfig::default(),
            performance: PerformanceConfig::default(),
            monitoring: MonitoringConfig::default(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            ws_url: Some("ws://localhost:8546".to_string()),
            chain_id: 1, // Ethereum mainnet
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_connections: 10,
            cache_size: 1000,
            batch_size: 10,
            processing_timeout_ms: 5000,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            metrics_port: 9090,
            verbose_logging: false,
            log_level: "info".to_string(),
        }
    }
}

impl StrategyConfig {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file: {}", e))?;
        
        let config: StrategyConfig = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file: {}", e))?;
        
        info!("Loaded strategy configuration from file");
        Ok(config)
    }
    
    /// 保存配置到文件
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
        
        std::fs::write(path, content)
            .map_err(|e| anyhow::anyhow!("Failed to write config file: {}", e))?;
        
        info!("Saved strategy configuration to file");
        Ok(())
    }
    
    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 验证套利配置
        if self.arbitrage.min_profit_threshold.is_zero() {
            return Err(anyhow::anyhow!("Min profit threshold cannot be zero"));
        }
        
        if self.arbitrage.max_slippage_bps > 10000 {
            return Err(anyhow::anyhow!("Max slippage cannot exceed 100%"));
        }
        
        // 验证网络配置
        if self.network.rpc_url.is_empty() {
            return Err(anyhow::anyhow!("RPC URL cannot be empty"));
        }
        
        if self.network.chain_id == 0 {
            return Err(anyhow::anyhow!("Chain ID cannot be zero"));
        }
        
        // 验证性能配置
        if self.performance.max_concurrent_connections == 0 {
            return Err(anyhow::anyhow!("Max concurrent connections must be > 0"));
        }
        
        if self.performance.cache_size == 0 {
            return Err(anyhow::anyhow!("Cache size must be > 0"));
        }
        
        info!("Configuration validation passed");
        Ok(())
    }
    
    /// 创建示例配置文件
    pub fn create_example_config<P: AsRef<Path>>(path: P) -> Result<()> {
        let config = StrategyConfig {
            arbitrage: ArbConfig {
                min_profit_threshold: artemis_core::eth::U256::from(100_000_000_000_000_000u64), // 0.1 ETH
                max_slippage_bps: 300, // 3%
                max_gas_price: artemis_core::eth::U256::from(50_000_000_000u64), // 50 gwei
                enable_dynamic_sizing: true,
                price_prediction_window: 5,
                risk_threshold: 0.7,
            },
            network: NetworkConfig {
                rpc_url: "https://eth-mainnet.alchemyapi.io/v2/YOUR_API_KEY".to_string(),
                ws_url: Some("wss://eth-mainnet.alchemyapi.io/v2/YOUR_API_KEY".to_string()),
                chain_id: 1,
                timeout_seconds: 30,
                max_retries: 3,
            },
            performance: PerformanceConfig {
                max_concurrent_connections: 10,
                cache_size: 1000,
                batch_size: 10,
                processing_timeout_ms: 5000,
            },
            monitoring: MonitoringConfig {
                enable_metrics: true,
                metrics_port: 9090,
                verbose_logging: false,
                log_level: "info".to_string(),
            },
        };
        
        config.to_file(path)?;
        info!("Created example configuration file");
        Ok(())
    }
    
    /// 合并另一个配置（用于覆盖默认值）
    pub fn merge(&mut self, other: StrategyConfig) {
        // 合并套利配置
        self.arbitrage.min_profit_threshold = other.arbitrage.min_profit_threshold;
        self.arbitrage.max_slippage_bps = other.arbitrage.max_slippage_bps;
        self.arbitrage.max_gas_price = other.arbitrage.max_gas_price;
        self.arbitrage.enable_dynamic_sizing = other.arbitrage.enable_dynamic_sizing;
        self.arbitrage.price_prediction_window = other.arbitrage.price_prediction_window;
        self.arbitrage.risk_threshold = other.arbitrage.risk_threshold;
        
        // 合并网络配置
        if !other.network.rpc_url.is_empty() {
            self.network.rpc_url = other.network.rpc_url;
        }
        if other.network.ws_url.is_some() {
            self.network.ws_url = other.network.ws_url;
        }
        if other.network.chain_id != 0 {
            self.network.chain_id = other.network.chain_id;
        }
        if other.network.timeout_seconds > 0 {
            self.network.timeout_seconds = other.network.timeout_seconds;
        }
        if other.network.max_retries > 0 {
            self.network.max_retries = other.network.max_retries;
        }
        
        // 合并性能配置
        if other.performance.max_concurrent_connections > 0 {
            self.performance.max_concurrent_connections = other.performance.max_concurrent_connections;
        }
        if other.performance.cache_size > 0 {
            self.performance.cache_size = other.performance.cache_size;
        }
        if other.performance.batch_size > 0 {
            self.performance.batch_size = other.performance.batch_size;
        }
        if other.performance.processing_timeout_ms > 0 {
            self.performance.processing_timeout_ms = other.performance.processing_timeout_ms;
        }
        
        // 合并监控配置
        self.monitoring.enable_metrics = other.monitoring.enable_metrics;
        if other.monitoring.metrics_port > 0 {
            self.monitoring.metrics_port = other.monitoring.metrics_port;
        }
        self.monitoring.verbose_logging = other.monitoring.verbose_logging;
        if !other.monitoring.log_level.is_empty() {
            self.monitoring.log_level = other.monitoring.log_level;
        }
    }
}

/// 配置加载器
pub struct ConfigLoader;

impl ConfigLoader {
    /// 从多个源加载配置
    pub fn load() -> Result<StrategyConfig> {
        let mut config = StrategyConfig::default();
        
        // 尝试从文件加载
        if let Ok(file_config) = StrategyConfig::from_file("config.toml") {
            config.merge(file_config);
            info!("Loaded configuration from config.toml");
        } else {
            warn!("No config.toml found, using default configuration");
        }
        
        // 从环境变量覆盖
        Self::load_from_env(&mut config)?;
        
        // 验证配置
        config.validate()?;
        
        Ok(config)
    }
    
    /// 从环境变量加载配置
    fn load_from_env(config: &mut StrategyConfig) -> Result<()> {
        // 网络配置
        if let Ok(rpc_url) = std::env::var("ARTEMIS_RPC_URL") {
            config.network.rpc_url = rpc_url;
        }
        
        if let Ok(ws_url) = std::env::var("ARTEMIS_WS_URL") {
            config.network.ws_url = Some(ws_url);
        }
        
        if let Ok(chain_id) = std::env::var("ARTEMIS_CHAIN_ID") {
            config.network.chain_id = chain_id.parse()
                .map_err(|e| anyhow::anyhow!("Invalid chain ID: {}", e))?;
        }
        
        // 套利配置
        if let Ok(min_profit) = std::env::var("ARTEMIS_MIN_PROFIT_ETH") {
            let profit_eth: f64 = min_profit.parse()
                .map_err(|e| anyhow::anyhow!("Invalid min profit: {}", e))?;
            config.arbitrage.min_profit_threshold = artemis_core::eth::U256::from(
                (profit_eth * 1e18) as u128
            );
        }
        
        if let Ok(max_gas) = std::env::var("ARTEMIS_MAX_GAS_GWEI") {
            let gas_gwei: f64 = max_gas.parse()
                .map_err(|e| anyhow::anyhow!("Invalid max gas: {}", e))?;
            config.arbitrage.max_gas_price = artemis_core::eth::U256::from(
                (gas_gwei * 1e9) as u64
            );
        }
        
        // 监控配置
        if let Ok(log_level) = std::env::var("ARTEMIS_LOG_LEVEL") {
            config.monitoring.log_level = log_level;
        }
        
        if let Ok(metrics_port) = std::env::var("ARTEMIS_METRICS_PORT") {
            config.monitoring.metrics_port = metrics_port.parse()
                .map_err(|e| anyhow::anyhow!("Invalid metrics port: {}", e))?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_config_serialization() {
        let config = StrategyConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: StrategyConfig = toml::from_str(&serialized).unwrap();
        
        assert_eq!(config.arbitrage.min_profit_threshold, deserialized.arbitrage.min_profit_threshold);
        assert_eq!(config.network.rpc_url, deserialized.network.rpc_url);
    }
    
    #[test]
    fn test_config_file_operations() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        
        let original_config = StrategyConfig::default();
        original_config.to_file(&config_path).unwrap();
        
        let loaded_config = StrategyConfig::from_file(&config_path).unwrap();
        
        assert_eq!(original_config.arbitrage.min_profit_threshold, loaded_config.arbitrage.min_profit_threshold);
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = StrategyConfig::default();
        assert!(config.validate().is_ok());
        
        // 测试无效配置
        config.arbitrage.min_profit_threshold = artemis_core::eth::U256::zero();
        assert!(config.validate().is_err());
        
        config.arbitrage.min_profit_threshold = artemis_core::eth::U256::from(100_000_000_000_000_000u64);
        config.network.rpc_url = String::new();
        assert!(config.validate().is_err());
    }
}
