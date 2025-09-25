//! Production Monitoring and Metrics
//! 
//! This module provides comprehensive monitoring, metrics collection,
//! and alerting for production deployment.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};
use serde::{Serialize, Deserialize};

use crate::{
    types::{AnalysisEvent, AnalysisAction},
    error::DeFiResult,
    jit_strategy_discovery::StrategyCandidate,
};

/// Production metrics collector
#[derive(Debug)]
pub struct ProductionMetrics {
    /// Strategy performance metrics
    strategy_metrics: Arc<RwLock<StrategyMetrics>>,
    /// System performance metrics
    system_metrics: Arc<RwLock<SystemMetrics>>,
    /// Alert manager
    alert_manager: AlertManager,
    /// Configuration
    config: MetricsConfig,
}

/// Strategy performance metrics
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct StrategyMetrics {
    /// Total events processed
    pub events_processed: u64,
    /// Total strategies discovered
    pub strategies_discovered: u64,
    /// Total strategies executed
    pub strategies_executed: u64,
    /// Total profit earned (ETH)
    pub total_profit_eth: f64,
    /// Total gas spent (ETH)
    pub total_gas_spent_eth: f64,
    /// Average strategy discovery time (ms)
    pub avg_discovery_time_ms: f64,
    /// Success rate percentage
    pub success_rate: f64,
    /// Arbitrage vs SMT strategy ratio
    pub arb_vs_smt_ratio: f64,
    /// Last update timestamp
    pub last_updated: u64,
}

/// System performance metrics
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct SystemMetrics {
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in MB
    pub memory_usage_mb: u64,
    /// Active connections
    pub active_connections: u32,
    /// Request queue length
    pub request_queue_length: u32,
    /// Average request processing time (ms)
    pub avg_request_time_ms: f64,
    /// Error rate percentage
    pub error_rate: f64,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Last update timestamp
    pub last_updated: u64,
}

/// Alert manager for critical events
#[derive(Debug)]
pub struct AlertManager {
    /// Alert rules
    rules: Vec<AlertRule>,
    /// Active alerts
    active_alerts: Arc<RwLock<Vec<Alert>>>,
    /// Alert history
    alert_history: Arc<RwLock<Vec<Alert>>>,
    /// Configuration
    config: AlertConfig,
}

/// Alert rule definition
#[derive(Debug, Clone)]
pub struct AlertRule {
    /// Rule name
    pub name: String,
    /// Condition to trigger alert
    pub condition: AlertCondition,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Enabled flag
    pub enabled: bool,
    /// Cooldown period to prevent spam
    pub cooldown: Duration,
    /// Last triggered time
    pub last_triggered: Option<Instant>,
}

/// Alert conditions
#[derive(Debug, Clone)]
pub enum AlertCondition {
    /// Error rate exceeds threshold
    ErrorRateExceeds(f64),
    /// Success rate below threshold
    SuccessRateBelow(f64),
    /// Memory usage exceeds threshold (MB)
    MemoryUsageExceeds(u64),
    /// CPU usage exceeds threshold (%)
    CpuUsageExceeds(f64),
    /// Request queue length exceeds threshold
    QueueLengthExceeds(u32),
    /// Strategy discovery time exceeds threshold (ms)
    DiscoveryTimeExceeds(f64),
    /// Profit rate below threshold (ETH/hour)
    ProfitRateBelow(f64),
}

/// Alert severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Alert instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Alert ID
    pub id: String,
    /// Alert message
    pub message: String,
    /// Severity level
    pub severity: AlertSeverity,
    /// Timestamp
    pub timestamp: u64,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Metrics configuration
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enable_collection: bool,
    /// Collection interval
    pub collection_interval: Duration,
    /// Metrics retention period
    pub retention_period: Duration,
    /// Enable alerting
    pub enable_alerting: bool,
    /// Alert configuration
    pub alert_config: AlertConfig,
}

/// Alert configuration
#[derive(Debug, Clone)]
pub struct AlertConfig {
    /// Enable alerting
    pub enable: bool,
    /// Webhook URL for alerts
    pub webhook_url: Option<String>,
    /// Email configuration
    pub email_config: Option<EmailConfig>,
    /// Slack configuration
    pub slack_config: Option<SlackConfig>,
}

/// Email alert configuration
#[derive(Debug, Clone)]
pub struct EmailConfig {
    /// SMTP server
    pub smtp_server: String,
    /// SMTP port
    pub smtp_port: u16,
    /// Username
    pub username: String,
    /// Password
    pub password: String,
    /// Recipients
    pub recipients: Vec<String>,
}

/// Slack alert configuration
#[derive(Debug, Clone)]
pub struct SlackConfig {
    /// Webhook URL
    pub webhook_url: String,
    /// Channel
    pub channel: String,
    /// Username
    pub username: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enable_collection: true,
            collection_interval: Duration::from_secs(30),
            retention_period: Duration::from_secs(86400), // 24 hours
            enable_alerting: true,
            alert_config: AlertConfig::default(),
        }
    }
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enable: true,
            webhook_url: None,
            email_config: None,
            slack_config: None,
        }
    }
}

impl ProductionMetrics {
    /// Create new production metrics collector
    pub fn new(config: MetricsConfig) -> Self {
        let alert_manager = AlertManager::new(config.alert_config.clone());
        
        Self {
            strategy_metrics: Arc::new(RwLock::new(StrategyMetrics::default())),
            system_metrics: Arc::new(RwLock::new(SystemMetrics::default())),
            alert_manager,
            config,
        }
    }
    
    /// Start metrics collection
    pub async fn start_collection(&self) -> DeFiResult<()> {
        info!("Starting production metrics collection");
        
        let strategy_metrics = Arc::clone(&self.strategy_metrics);
        let system_metrics = Arc::clone(&self.system_metrics);
        let interval = self.config.collection_interval;
        
        // Start metrics collection task
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                // Update system metrics
                {
                    let mut metrics = system_metrics.write().await;
                    metrics.cpu_usage = Self::get_cpu_usage();
                    metrics.memory_usage_mb = Self::get_memory_usage();
                    metrics.uptime_seconds = Self::get_uptime();
                    metrics.last_updated = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                }
                
                debug!("Updated system metrics");
            }
        });
        
        Ok(())
    }
    
    /// Record strategy discovery event
    pub async fn record_strategy_discovery(&self, event: &AnalysisEvent, candidate: Option<&StrategyCandidate>, discovery_time: Duration) {
        let mut metrics = self.strategy_metrics.write().await;
        
        metrics.events_processed += 1;
        
        if let Some(strategy) = candidate {
            metrics.strategies_discovered += 1;
            
            // Update profit tracking
            let profit_eth = strategy.net_profit.as_limbs()[0] as f64 / 1e18;
            metrics.total_profit_eth += profit_eth;
            
            // Update ARB vs SMT ratio
            let total_strategies = metrics.strategies_discovered;
            if strategy.strategy_type == crate::jit_strategy_discovery::StrategyType::ARB {
                metrics.arb_vs_smt_ratio = (metrics.arb_vs_smt_ratio * (total_strategies - 1) as f64 + 1.0) / total_strategies as f64;
            } else {
                metrics.arb_vs_smt_ratio = (metrics.arb_vs_smt_ratio * (total_strategies - 1) as f64) / total_strategies as f64;
            }
        }
        
        // Update discovery time
        let discovery_ms = discovery_time.as_millis() as f64;
        metrics.avg_discovery_time_ms = (metrics.avg_discovery_time_ms * (metrics.events_processed - 1) as f64 + discovery_ms) / metrics.events_processed as f64;
        
        // Update success rate
        metrics.success_rate = metrics.strategies_discovered as f64 / metrics.events_processed as f64 * 100.0;
        
        metrics.last_updated = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    }
    
    /// Record strategy execution
    pub async fn record_strategy_execution(&self, success: bool, gas_used: U256) {
        let mut metrics = self.strategy_metrics.write().await;
        
        if success {
            metrics.strategies_executed += 1;
        }
        
        // Update gas tracking
        let gas_eth = gas_used.as_limbs()[0] as f64 / 1e18;
        metrics.total_gas_spent_eth += gas_eth;
    }
    
    /// Get strategy metrics
    pub async fn get_strategy_metrics(&self) -> StrategyMetrics {
        self.strategy_metrics.read().await.clone()
    }
    
    /// Get system metrics
    pub async fn get_system_metrics(&self) -> SystemMetrics {
        self.system_metrics.read().await.clone()
    }
    
    /// Helper functions for system metrics
    fn get_cpu_usage() -> f64 {
        // This would integrate with system monitoring
        // For now, return mock value
        20.5
    }
    
    fn get_memory_usage() -> u64 {
        // This would integrate with system monitoring
        // For now, return mock value
        512
    }
    
    fn get_uptime() -> u64 {
        // This would track actual uptime
        // For now, return mock value
        3600
    }
}

impl AlertManager {
    fn new(config: AlertConfig) -> Self {
        let mut rules = Vec::new();
        
        // Add default alert rules
        rules.push(AlertRule {
            name: "High Error Rate".to_string(),
            condition: AlertCondition::ErrorRateExceeds(5.0),
            severity: AlertSeverity::Warning,
            enabled: true,
            cooldown: Duration::from_mins(5),
            last_triggered: None,
        });
        
        rules.push(AlertRule {
            name: "Low Success Rate".to_string(),
            condition: AlertCondition::SuccessRateBelow(50.0),
            severity: AlertSeverity::Critical,
            enabled: true,
            cooldown: Duration::from_mins(10),
            last_triggered: None,
        });
        
        rules.push(AlertRule {
            name: "High Memory Usage".to_string(),
            condition: AlertCondition::MemoryUsageExceeds(1024),
            severity: AlertSeverity::Warning,
            enabled: true,
            cooldown: Duration::from_mins(5),
            last_triggered: None,
        });
        
        Self {
            rules,
            active_alerts: Arc::new(RwLock::new(Vec::new())),
            alert_history: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }
}
