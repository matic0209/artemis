//! 监控仪表板和指标收集系统

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use metrics::{counter, gauge, histogram};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// 监控管理器
pub struct MonitoringManager {
    /// 策略指标
    strategy_metrics: Arc<DashMap<String, StrategyMetrics>>,
    /// 系统指标
    system_metrics: Arc<RwLock<SystemMetrics>>,
    /// 警报管理器
    alert_manager: AlertManager,
    /// 指标历史
    metrics_history: Arc<RwLock<MetricsHistory>>,
    /// 事件发布器
    event_publisher: broadcast::Sender<MonitoringEvent>,
}

/// 策略指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMetrics {
    /// 策略名称
    pub name: String,
    /// 处理的事件数
    pub events_processed: u64,
    /// 生成的动作数
    pub actions_generated: u64,
    /// 成功执行的动作数
    pub actions_executed: u64,
    /// 失败次数
    pub failures: u64,
    /// 平均处理时间（毫秒）
    pub avg_processing_time_ms: f64,
    /// 总利润（ETH）
    pub total_profit_eth: f64,
    /// 总 Gas 消耗
    pub total_gas_used: u64,
    /// 最后更新时间
    pub last_updated: u64,
    /// 成功率
    pub success_rate: f64,
    /// 每小时事件处理数
    pub events_per_hour: f64,
}

/// 系统指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// 系统启动时间
    pub startup_time: u64,
    /// CPU 使用率
    pub cpu_usage_percent: f64,
    /// 内存使用量（MB）
    pub memory_usage_mb: f64,
    /// 网络连接数
    pub active_connections: u32,
    /// 区块链连接状态
    pub blockchain_connected: bool,
    /// 最新区块号
    pub latest_block: u64,
    /// 区块同步延迟（秒）
    pub block_sync_delay_seconds: u64,
    /// 待处理事件数
    pub pending_events: u32,
    /// 待执行动作数
    pub pending_actions: u32,
}

/// 指标历史记录
#[derive(Debug, Clone)]
pub struct MetricsHistory {
    /// 策略指标历史
    strategy_history: HashMap<String, Vec<StrategyMetrics>>,
    /// 系统指标历史
    system_history: Vec<SystemMetrics>,
    /// 最大历史记录数
    max_history_size: usize,
}

/// 监控事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitoringEvent {
    /// 策略指标更新
    StrategyMetricsUpdated {
        strategy_name: String,
        metrics: StrategyMetrics,
    },
    /// 系统指标更新
    SystemMetricsUpdated {
        metrics: SystemMetrics,
    },
    /// 警报触发
    AlertTriggered {
        alert: Alert,
    },
    /// 性能异常
    PerformanceAnomaly {
        strategy_name: String,
        anomaly_type: String,
        description: String,
    },
}

/// 警报
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// 警报 ID
    pub id: String,
    /// 警报级别
    pub level: AlertLevel,
    /// 警报类型
    pub alert_type: String,
    /// 描述
    pub description: String,
    /// 触发时间
    pub triggered_at: u64,
    /// 相关策略
    pub strategy_name: Option<String>,
    /// 指标值
    pub metric_value: f64,
    /// 阈值
    pub threshold: f64,
}

/// 警报级别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

/// 警报管理器
#[derive(Debug)]
pub struct AlertManager {
    /// 警报规则
    rules: Vec<AlertRule>,
    /// 活跃警报
    active_alerts: Arc<DashMap<String, Alert>>,
    /// 警报历史
    alert_history: Arc<RwLock<Vec<Alert>>>,
}

/// 警报规则
#[derive(Debug, Clone)]
pub struct AlertRule {
    /// 规则 ID
    pub id: String,
    /// 指标名称
    pub metric_name: String,
    /// 条件
    pub condition: AlertCondition,
    /// 阈值
    pub threshold: f64,
    /// 警报级别
    pub level: AlertLevel,
    /// 描述模板
    pub description_template: String,
}

/// 警报条件
#[derive(Debug, Clone)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
}

impl MonitoringManager {
    /// 创建新的监控管理器
    pub fn new() -> Self {
        let (event_publisher, _) = broadcast::channel(1000);
        
        Self {
            strategy_metrics: Arc::new(DashMap::new()),
            system_metrics: Arc::new(RwLock::new(SystemMetrics::default())),
            alert_manager: AlertManager::new(),
            metrics_history: Arc::new(RwLock::new(MetricsHistory::new(1000))),
            event_publisher,
        }
    }

    /// 注册策略
    pub fn register_strategy(&self, name: String) {
        let metrics = StrategyMetrics::new(name.clone());
        self.strategy_metrics.insert(name.clone(), metrics.clone());
        
        // 注册 Prometheus 指标
        // 注册指标（使用 metrics 宏）
        counter!("strategy_events_processed", 0, "strategy" => name.clone());
        counter!("strategy_actions_generated", 0, "strategy" => name.clone());
        counter!("strategy_actions_executed", 0, "strategy" => name.clone());
        counter!("strategy_failures", 0, "strategy" => name.clone());
        histogram!("strategy_processing_time_ms", 0.0, "strategy" => name.clone());
        gauge!("strategy_success_rate", 0.0, "strategy" => name.clone());
        gauge!("strategy_total_profit_eth", 0.0, "strategy" => name.clone());
    }

    /// 更新策略指标
    pub fn update_strategy_metrics<F>(&self, strategy_name: &str, updater: F)
    where
        F: FnOnce(&mut StrategyMetrics),
    {
        if let Some(mut metrics) = self.strategy_metrics.get_mut(strategy_name) {
            updater(&mut metrics);
            metrics.last_updated = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // 更新 Prometheus 指标
            counter!("strategy_events_processed", "strategy" => strategy_name.to_string())
                .absolute(metrics.events_processed);
            counter!("strategy_actions_generated", "strategy" => strategy_name.to_string())
                .absolute(metrics.actions_generated);
            gauge!("strategy_success_rate", "strategy" => strategy_name.to_string())
                .set(metrics.success_rate);
            gauge!("strategy_total_profit_eth", "strategy" => strategy_name.to_string())
                .set(metrics.total_profit_eth);

            // 发布事件
            let event = MonitoringEvent::StrategyMetricsUpdated {
                strategy_name: strategy_name.to_string(),
                metrics: metrics.clone(),
            };
            let _ = self.event_publisher.send(event);

            // 添加到历史记录
            let mut history = self.metrics_history.write();
            history.add_strategy_metrics(strategy_name.to_string(), metrics.clone());

            // 检查警报
            self.alert_manager.check_strategy_alerts(strategy_name, &metrics);
        }
    }

    /// 更新系统指标
    pub fn update_system_metrics<F>(&self, updater: F)
    where
        F: FnOnce(&mut SystemMetrics),
    {
        let mut metrics = self.system_metrics.write();
        updater(&mut metrics);

        // 更新 Prometheus 指标
        gauge!("system_cpu_usage_percent").set(metrics.cpu_usage_percent);
        gauge!("system_memory_usage_mb").set(metrics.memory_usage_mb);
        gauge!("system_active_connections").set(metrics.active_connections as f64);
        gauge!("system_latest_block").set(metrics.latest_block as f64);
        gauge!("system_pending_events").set(metrics.pending_events as f64);
        gauge!("system_pending_actions").set(metrics.pending_actions as f64);

        // 发布事件
        let event = MonitoringEvent::SystemMetricsUpdated {
            metrics: metrics.clone(),
        };
        let _ = self.event_publisher.send(event);

        // 添加到历史记录
        let mut history = self.metrics_history.write();
        history.add_system_metrics(metrics.clone());

        // 检查系统警报
        self.alert_manager.check_system_alerts(&metrics);
    }

    /// 记录策略事件处理
    pub fn record_event_processed(&self, strategy_name: &str, processing_time: Duration) {
        self.update_strategy_metrics(strategy_name, |metrics| {
            metrics.events_processed += 1;
            
            // 更新平均处理时间
            let new_time = processing_time.as_millis() as f64;
            let total_events = metrics.events_processed as f64;
            metrics.avg_processing_time_ms = 
                (metrics.avg_processing_time_ms * (total_events - 1.0) + new_time) / total_events;
            
            // 更新每小时事件处理数
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let time_diff_hours = (current_time - metrics.last_updated) as f64 / 3600.0;
            if time_diff_hours > 0.0 {
                metrics.events_per_hour = 1.0 / time_diff_hours;
            }
        });

        histogram!("strategy_processing_time_ms", "strategy" => strategy_name.to_string())
            .record(processing_time.as_millis() as f64);
    }

    /// 记录动作生成
    pub fn record_action_generated(&self, strategy_name: &str) {
        self.update_strategy_metrics(strategy_name, |metrics| {
            metrics.actions_generated += 1;
        });
    }

    /// 记录动作执行
    pub fn record_action_executed(&self, strategy_name: &str, success: bool, profit_eth: f64, gas_used: u64) {
        self.update_strategy_metrics(strategy_name, |metrics| {
            if success {
                metrics.actions_executed += 1;
                metrics.total_profit_eth += profit_eth;
                metrics.total_gas_used += gas_used;
            } else {
                metrics.failures += 1;
            }
            
            // 更新成功率
            let total_attempts = metrics.actions_executed + metrics.failures;
            if total_attempts > 0 {
                metrics.success_rate = metrics.actions_executed as f64 / total_attempts as f64;
            }
        });
    }

    /// 获取策略指标
    pub fn get_strategy_metrics(&self, strategy_name: &str) -> Option<StrategyMetrics> {
        self.strategy_metrics.get(strategy_name).map(|m| m.clone())
    }

    /// 获取所有策略指标
    pub fn get_all_strategy_metrics(&self) -> HashMap<String, StrategyMetrics> {
        self.strategy_metrics
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// 获取系统指标
    pub fn get_system_metrics(&self) -> SystemMetrics {
        self.system_metrics.read().clone()
    }

    /// 订阅监控事件
    pub fn subscribe_events(&self) -> broadcast::Receiver<MonitoringEvent> {
        self.event_publisher.subscribe()
    }

    /// 生成监控报告
    pub fn generate_report(&self) -> MonitoringReport {
        let strategy_metrics = self.get_all_strategy_metrics();
        let system_metrics = self.get_system_metrics();
        let active_alerts = self.alert_manager.get_active_alerts();
        
        MonitoringReport {
            generated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            strategy_metrics,
            system_metrics,
            active_alerts,
            summary: self.generate_summary(),
        }
    }

    /// 生成摘要
    fn generate_summary(&self) -> MonitoringSummary {
        let strategy_metrics = self.get_all_strategy_metrics();
        let system_metrics = self.get_system_metrics();
        
        let total_events_processed = strategy_metrics.values()
            .map(|m| m.events_processed)
            .sum();
        
        let total_actions_executed = strategy_metrics.values()
            .map(|m| m.actions_executed)
            .sum();
        
        let total_profit_eth = strategy_metrics.values()
            .map(|m| m.total_profit_eth)
            .sum();
        
        let avg_success_rate = if !strategy_metrics.is_empty() {
            strategy_metrics.values()
                .map(|m| m.success_rate)
                .sum::<f64>() / strategy_metrics.len() as f64
        } else {
            0.0
        };

        MonitoringSummary {
            total_strategies: strategy_metrics.len() as u32,
            total_events_processed,
            total_actions_executed,
            total_profit_eth,
            avg_success_rate,
            system_health_score: self.calculate_health_score(&system_metrics),
        }
    }

    /// 计算系统健康评分
    fn calculate_health_score(&self, metrics: &SystemMetrics) -> f64 {
        let mut score = 100.0;
        
        // CPU 使用率影响
        if metrics.cpu_usage_percent > 80.0 {
            score -= 20.0;
        } else if metrics.cpu_usage_percent > 60.0 {
            score -= 10.0;
        }
        
        // 内存使用率影响
        if metrics.memory_usage_mb > 8000.0 {
            score -= 15.0;
        } else if metrics.memory_usage_mb > 4000.0 {
            score -= 5.0;
        }
        
        // 区块同步延迟影响
        if metrics.block_sync_delay_seconds > 30 {
            score -= 25.0;
        } else if metrics.block_sync_delay_seconds > 10 {
            score -= 10.0;
        }
        
        // 连接状态影响
        if !metrics.blockchain_connected {
            score -= 50.0;
        }
        
        score.max(0.0)
    }
}

impl StrategyMetrics {
    fn new(name: String) -> Self {
        Self {
            name,
            events_processed: 0,
            actions_generated: 0,
            actions_executed: 0,
            failures: 0,
            avg_processing_time_ms: 0.0,
            total_profit_eth: 0.0,
            total_gas_used: 0,
            last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            success_rate: 0.0,
            events_per_hour: 0.0,
        }
    }
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            startup_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            cpu_usage_percent: 0.0,
            memory_usage_mb: 0.0,
            active_connections: 0,
            blockchain_connected: false,
            latest_block: 0,
            block_sync_delay_seconds: 0,
            pending_events: 0,
            pending_actions: 0,
        }
    }
}

impl MetricsHistory {
    fn new(max_size: usize) -> Self {
        Self {
            strategy_history: HashMap::new(),
            system_history: Vec::new(),
            max_history_size: max_size,
        }
    }

    fn add_strategy_metrics(&mut self, strategy_name: String, metrics: StrategyMetrics) {
        let history = self.strategy_history.entry(strategy_name).or_insert_with(Vec::new);
        history.push(metrics);
        
        if history.len() > self.max_history_size {
            history.remove(0);
        }
    }

    fn add_system_metrics(&mut self, metrics: SystemMetrics) {
        self.system_history.push(metrics);
        
        if self.system_history.len() > self.max_history_size {
            self.system_history.remove(0);
        }
    }
}

impl AlertManager {
    fn new() -> Self {
        Self {
            rules: Self::default_rules(),
            active_alerts: Arc::new(DashMap::new()),
            alert_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn default_rules() -> Vec<AlertRule> {
        vec![
            AlertRule {
                id: "high_failure_rate".to_string(),
                metric_name: "failure_rate".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 0.1, // 10%
                level: AlertLevel::Warning,
                description_template: "Strategy {} has high failure rate: {:.2}%".to_string(),
            },
            AlertRule {
                id: "low_success_rate".to_string(),
                metric_name: "success_rate".to_string(),
                condition: AlertCondition::LessThan,
                threshold: 0.8, // 80%
                level: AlertLevel::Warning,
                description_template: "Strategy {} has low success rate: {:.2}%".to_string(),
            },
            AlertRule {
                id: "high_cpu_usage".to_string(),
                metric_name: "cpu_usage_percent".to_string(),
                condition: AlertCondition::GreaterThan,
                threshold: 80.0,
                level: AlertLevel::Error,
                description_template: "High CPU usage: {:.1}%".to_string(),
            },
        ]
    }

    fn check_strategy_alerts(&self, strategy_name: &str, metrics: &StrategyMetrics) {
        for rule in &self.rules {
            let metric_value = match rule.metric_name.as_str() {
                "success_rate" => metrics.success_rate,
                "failure_rate" => {
                    let total = metrics.actions_executed + metrics.failures;
                    if total > 0 { metrics.failures as f64 / total as f64 } else { 0.0 }
                },
                _ => continue,
            };

            if self.should_trigger_alert(&rule.condition, metric_value, rule.threshold) {
                let alert = Alert {
                    id: format!("{}_{}", rule.id, strategy_name),
                    level: rule.level.clone(),
                    alert_type: rule.id.clone(),
                    description: rule.description_template
                        .replace("{}", strategy_name)
                        .replace("{:.2}%", &format!("{:.2}%", metric_value * 100.0)),
                    triggered_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    strategy_name: Some(strategy_name.to_string()),
                    metric_value,
                    threshold: rule.threshold,
                };

                self.trigger_alert(alert);
            }
        }
    }

    fn check_system_alerts(&self, metrics: &SystemMetrics) {
        for rule in &self.rules {
            let metric_value = match rule.metric_name.as_str() {
                "cpu_usage_percent" => metrics.cpu_usage_percent,
                _ => continue,
            };

            if self.should_trigger_alert(&rule.condition, metric_value, rule.threshold) {
                let alert = Alert {
                    id: rule.id.clone(),
                    level: rule.level.clone(),
                    alert_type: rule.id.clone(),
                    description: rule.description_template
                        .replace("{:.1}%", &format!("{:.1}%", metric_value)),
                    triggered_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    strategy_name: None,
                    metric_value,
                    threshold: rule.threshold,
                };

                self.trigger_alert(alert);
            }
        }
    }

    fn should_trigger_alert(&self, condition: &AlertCondition, value: f64, threshold: f64) -> bool {
        match condition {
            AlertCondition::GreaterThan => value > threshold,
            AlertCondition::LessThan => value < threshold,
            AlertCondition::Equal => (value - threshold).abs() < 0.001,
            AlertCondition::NotEqual => (value - threshold).abs() >= 0.001,
        }
    }

    fn trigger_alert(&self, alert: Alert) {
        let alert_id = alert.id.clone();
        
        // 避免重复警报
        if self.active_alerts.contains_key(&alert_id) {
            return;
        }

        self.active_alerts.insert(alert_id, alert.clone());
        
        // 添加到历史记录
        let mut history = self.alert_history.write();
        history.push(alert.clone());
        
        tracing::warn!("Alert triggered: {}", alert.description);
    }

    fn get_active_alerts(&self) -> Vec<Alert> {
        self.active_alerts.iter().map(|entry| entry.value().clone()).collect()
    }
}

/// 监控报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringReport {
    /// 生成时间
    pub generated_at: u64,
    /// 策略指标
    pub strategy_metrics: HashMap<String, StrategyMetrics>,
    /// 系统指标
    pub system_metrics: SystemMetrics,
    /// 活跃警报
    pub active_alerts: Vec<Alert>,
    /// 摘要
    pub summary: MonitoringSummary,
}

/// 监控摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSummary {
    /// 策略总数
    pub total_strategies: u32,
    /// 总处理事件数
    pub total_events_processed: u64,
    /// 总执行动作数
    pub total_actions_executed: u64,
    /// 总利润
    pub total_profit_eth: f64,
    /// 平均成功率
    pub avg_success_rate: f64,
    /// 系统健康评分
    pub system_health_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitoring_manager_creation() {
        let manager = MonitoringManager::new();
        assert_eq!(manager.strategy_metrics.len(), 0);
    }

    #[test]
    fn test_strategy_registration() {
        let manager = MonitoringManager::new();
        manager.register_strategy("test_strategy".to_string());
        
        assert!(manager.strategy_metrics.contains_key("test_strategy"));
    }

    #[test]
    fn test_metrics_update() {
        let manager = MonitoringManager::new();
        manager.register_strategy("test_strategy".to_string());
        
        manager.record_event_processed("test_strategy", Duration::from_millis(100));
        
        let metrics = manager.get_strategy_metrics("test_strategy").unwrap();
        assert_eq!(metrics.events_processed, 1);
        assert_eq!(metrics.avg_processing_time_ms, 100.0);
    }

    #[test]
    fn test_success_rate_calculation() {
        let manager = MonitoringManager::new();
        manager.register_strategy("test_strategy".to_string());
        
        manager.record_action_executed("test_strategy", true, 0.1, 21000);
        manager.record_action_executed("test_strategy", true, 0.2, 21000);
        manager.record_action_executed("test_strategy", false, 0.0, 21000);
        
        let metrics = manager.get_strategy_metrics("test_strategy").unwrap();
        assert_eq!(metrics.actions_executed, 2);
        assert_eq!(metrics.failures, 1);
        assert!((metrics.success_rate - 0.666667).abs() < 0.001);
    }

    #[test]
    fn test_health_score_calculation() {
        let manager = MonitoringManager::new();
        
        let mut metrics = SystemMetrics::default();
        metrics.cpu_usage_percent = 50.0;
        metrics.memory_usage_mb = 2000.0;
        metrics.blockchain_connected = true;
        metrics.block_sync_delay_seconds = 5;
        
        let score = manager.calculate_health_score(&metrics);
        assert_eq!(score, 100.0); // 应该是满分
        
        metrics.cpu_usage_percent = 90.0;
        let score = manager.calculate_health_score(&metrics);
        assert_eq!(score, 80.0); // 应该扣除 20 分
    }
}
