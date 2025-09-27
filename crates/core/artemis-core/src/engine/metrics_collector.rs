//! Advanced Metrics Collection System
//!
//! Comprehensive performance monitoring, alerting, and analytics
//! for the Artemis high-performance engine.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, warn, error};
use serde::{Serialize, Deserialize};

/// Performance metrics for strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Strategy identifier
    pub strategy_id: String,
    /// Total executions
    pub executions: u64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average latency
    pub avg_latency: Duration,
    /// Profit generated
    pub profit: f64,
    /// Timestamp when metrics were recorded
    pub timestamp: SystemTime,
}

/// System resource metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetrics {
    /// CPU utilization percentage
    pub cpu_usage: f64,
    /// Memory usage in MB
    pub memory_usage_mb: u64,
    /// Network I/O rates
    pub network_io: NetworkIO,
    /// Disk I/O rates
    pub disk_io: DiskIO,
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Network I/O metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIO {
    /// Bytes received per second
    pub bytes_in_per_sec: u64,
    /// Bytes sent per second
    pub bytes_out_per_sec: u64,
    /// Packets received per second
    pub packets_in_per_sec: u64,
    /// Packets sent per second
    pub packets_out_per_sec: u64,
}

/// Disk I/O metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIO {
    /// Read operations per second
    pub reads_per_sec: u64,
    /// Write operations per second
    pub writes_per_sec: u64,
    /// Bytes read per second
    pub bytes_read_per_sec: u64,
    /// Bytes written per second
    pub bytes_written_per_sec: u64,
}

/// Alert configuration
#[derive(Debug, Clone)]
pub struct AlertConfig {
    /// CPU usage threshold (percentage)
    pub cpu_threshold: f64,
    /// Memory usage threshold (MB)
    pub memory_threshold_mb: u64,
    /// Latency threshold
    pub latency_threshold: Duration,
    /// Success rate threshold
    pub success_rate_threshold: f64,
    /// Alert cooldown period
    pub alert_cooldown: Duration,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            cpu_threshold: 80.0,
            memory_threshold_mb: 1024,
            latency_threshold: Duration::from_millis(1000),
            success_rate_threshold: 0.95,
            alert_cooldown: Duration::from_secs(5 * 60),
        }
    }
}

/// Alert severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Alert message
#[derive(Debug, Clone)]
pub struct Alert {
    /// Alert ID
    pub id: String,
    /// Severity level
    pub severity: AlertSeverity,
    /// Alert message
    pub message: String,
    /// Component that triggered the alert
    pub component: String,
    /// Timestamp when alert was generated
    pub timestamp: Instant,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Time series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint<T> {
    /// Timestamp
    pub timestamp: u64, // Unix timestamp in milliseconds
    /// Value
    pub value: T,
    /// Optional tags for grouping
    pub tags: HashMap<String, String>,
}

/// Metrics aggregation window
#[derive(Debug, Clone, Copy)]
pub enum AggregationWindow {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    OneDay,
}

impl AggregationWindow {
    fn duration(&self) -> Duration {
        match self {
            AggregationWindow::OneMinute => Duration::from_secs(60),
            AggregationWindow::FiveMinutes => Duration::from_secs(300),
            AggregationWindow::FifteenMinutes => Duration::from_secs(900),
            AggregationWindow::OneHour => Duration::from_secs(3600),
            AggregationWindow::OneDay => Duration::from_secs(86400),
        }
    }
}

/// Aggregated metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    /// Minimum value in window
    pub min: f64,
    /// Maximum value in window
    pub max: f64,
    /// Average value in window
    pub avg: f64,
    /// Sum of values in window
    pub sum: f64,
    /// Number of data points
    pub count: u64,
    /// Standard deviation
    pub std_dev: f64,
    /// Percentiles (50th, 95th, 99th)
    pub percentiles: HashMap<u8, f64>,
}

/// Advanced metrics collector with time series support
pub struct MetricsCollector {
    /// Strategy performance metrics
    strategy_metrics: Arc<RwLock<HashMap<String, VecDeque<PerformanceMetrics>>>>,
    /// System resource metrics
    resource_metrics: Arc<RwLock<VecDeque<ResourceMetrics>>>,
    /// Active alerts
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    /// Alert history
    alert_history: Arc<RwLock<VecDeque<Alert>>>,
    /// Alert configuration
    alert_config: AlertConfig,
    /// Last alert timestamps for cooldown
    last_alert_times: Arc<RwLock<HashMap<String, Instant>>>,
    /// Custom metrics registry
    custom_metrics: Arc<RwLock<HashMap<String, VecDeque<TimeSeriesPoint<f64>>>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self::with_alert_config(AlertConfig::default())
    }

    /// Create metrics collector with custom alert configuration
    pub fn with_alert_config(alert_config: AlertConfig) -> Self {
        Self {
            strategy_metrics: Arc::new(RwLock::new(HashMap::new())),
            resource_metrics: Arc::new(RwLock::new(VecDeque::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(VecDeque::new())),
            alert_config,
            last_alert_times: Arc::new(RwLock::new(HashMap::new())),
            custom_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record strategy performance metrics
    pub async fn record_strategy_metrics(&self, metrics: PerformanceMetrics) {
        let strategy_id = metrics.strategy_id.clone();

        // Store metrics
        {
            let mut strategy_metrics = self.strategy_metrics.write().await;
            let strategy_history = strategy_metrics.entry(strategy_id.clone()).or_insert_with(VecDeque::new);

            strategy_history.push_back(metrics.clone());

            // Keep only recent data (last hour)
            let cutoff_time = SystemTime::now()
                .checked_sub(Duration::from_secs(3600))
                .unwrap_or(SystemTime::UNIX_EPOCH);
            while let Some(front) = strategy_history.front() {
                if front.timestamp < cutoff_time {
                    strategy_history.pop_front();
                } else {
                    break;
                }
            }
        }

        // Check for alerts
        self.check_strategy_alerts(&metrics).await;

        debug!("Recorded metrics for strategy: {}", strategy_id);
    }

    /// Record system resource metrics
    pub async fn record_resource_metrics(&self, metrics: ResourceMetrics) {
        // Store metrics
        {
            let mut resource_metrics = self.resource_metrics.write().await;
            resource_metrics.push_back(metrics.clone());

            // Keep only recent data (last hour)
            let cutoff_time = SystemTime::now()
                .checked_sub(Duration::from_secs(3600))
                .unwrap_or(SystemTime::UNIX_EPOCH);
            while let Some(front) = resource_metrics.front() {
                if front.timestamp < cutoff_time {
                    resource_metrics.pop_front();
                } else {
                    break;
                }
            }
        }

        // Check for alerts
        self.check_resource_alerts(&metrics).await;

        debug!("Recorded resource metrics");
    }

    /// Record custom metric
    pub async fn record_custom_metric(&self, metric_name: String, value: f64, tags: HashMap<String, String>) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let point = TimeSeriesPoint {
            timestamp,
            value,
            tags,
        };

        let mut custom_metrics = self.custom_metrics.write().await;
        let metric_history = custom_metrics.entry(metric_name).or_insert_with(VecDeque::new);

        metric_history.push_back(point);

        // Keep only recent data
        let cutoff_timestamp = timestamp - 3600_000; // Last hour
        while let Some(front) = metric_history.front() {
            if front.timestamp < cutoff_timestamp {
                metric_history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get strategy metrics for a specific strategy
    pub async fn get_strategy_metrics(&self, strategy_id: &str) -> Option<Vec<PerformanceMetrics>> {
        let strategy_metrics = self.strategy_metrics.read().await;
        strategy_metrics.get(strategy_id).map(|deque| deque.iter().cloned().collect())
    }

    /// Get aggregated metrics for a strategy over a time window
    pub async fn get_aggregated_strategy_metrics(
        &self,
        strategy_id: &str,
        window: AggregationWindow,
    ) -> Option<AggregatedMetrics> {
        let strategy_metrics = self.strategy_metrics.read().await;
        let metrics = strategy_metrics.get(strategy_id)?;

        let window_start = SystemTime::now()
            .checked_sub(window.duration())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let windowed_metrics: Vec<_> = metrics.iter()
            .filter(|m| m.timestamp >= window_start)
            .collect();

        if windowed_metrics.is_empty() {
            return None;
        }

        let latencies: Vec<f64> = windowed_metrics.iter()
            .map(|m| m.avg_latency.as_millis() as f64)
            .collect();

        Some(self.calculate_aggregated_metrics(&latencies))
    }

    /// Get current resource metrics
    pub async fn get_current_resource_metrics(&self) -> Option<ResourceMetrics> {
        let resource_metrics = self.resource_metrics.read().await;
        resource_metrics.back().cloned()
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let active_alerts = self.active_alerts.read().await;
        active_alerts.values().cloned().collect()
    }

    /// Get alert history
    pub async fn get_alert_history(&self, limit: Option<usize>) -> Vec<Alert> {
        let alert_history = self.alert_history.read().await;
        let history: Vec<_> = alert_history.iter().cloned().collect();

        if let Some(limit) = limit {
            history.into_iter().rev().take(limit).collect()
        } else {
            history
        }
    }

    /// Get comprehensive metrics summary
    pub async fn get_summary(&self) -> MetricsSummary {
        let strategy_metrics = self.strategy_metrics.read().await;
        let active_alerts = self.active_alerts.read().await;

        let total_strategies = strategy_metrics.len();
        let total_executions: u64 = strategy_metrics.values()
            .flat_map(|metrics| metrics.iter())
            .map(|m| m.executions)
            .sum();

        let avg_success_rate = if total_strategies > 0 {
            strategy_metrics.values()
                .filter_map(|metrics| metrics.back())
                .map(|m| m.success_rate)
                .sum::<f64>() / total_strategies as f64
        } else {
            0.0
        };

        let active_alert_count = active_alerts.len();

        MetricsSummary {
            total_strategies,
            total_executions,
            average_success_rate: avg_success_rate,
            active_alerts: active_alert_count,
            uptime: Instant::now(), // Would track actual uptime
        }
    }

    /// Check for strategy-related alerts
    async fn check_strategy_alerts(&self, metrics: &PerformanceMetrics) {
        // Check success rate
        if metrics.success_rate < self.alert_config.success_rate_threshold {
            self.trigger_alert(
                format!("strategy_success_rate_{}", metrics.strategy_id),
                AlertSeverity::Warning,
                format!(
                    "Strategy {} success rate ({:.2}%) below threshold ({:.2}%)",
                    metrics.strategy_id,
                    metrics.success_rate * 100.0,
                    self.alert_config.success_rate_threshold * 100.0
                ),
                "strategy".to_string(),
                HashMap::from([
                    ("strategy_id".to_string(), metrics.strategy_id.clone()),
                    ("success_rate".to_string(), metrics.success_rate.to_string()),
                ]),
            ).await;
        }

        // Check latency
        if metrics.avg_latency > self.alert_config.latency_threshold {
            self.trigger_alert(
                format!("strategy_latency_{}", metrics.strategy_id),
                AlertSeverity::Warning,
                format!(
                    "Strategy {} latency ({:?}) above threshold ({:?})",
                    metrics.strategy_id,
                    metrics.avg_latency,
                    self.alert_config.latency_threshold
                ),
                "strategy".to_string(),
                HashMap::from([
                    ("strategy_id".to_string(), metrics.strategy_id.clone()),
                    ("latency_ms".to_string(), metrics.avg_latency.as_millis().to_string()),
                ]),
            ).await;
        }
    }

    /// Check for resource-related alerts
    async fn check_resource_alerts(&self, metrics: &ResourceMetrics) {
        // Check CPU usage
        if metrics.cpu_usage > self.alert_config.cpu_threshold {
            self.trigger_alert(
                "high_cpu_usage".to_string(),
                AlertSeverity::Warning,
                format!(
                    "CPU usage ({:.1}%) above threshold ({:.1}%)",
                    metrics.cpu_usage,
                    self.alert_config.cpu_threshold
                ),
                "system".to_string(),
                HashMap::from([
                    ("cpu_usage".to_string(), metrics.cpu_usage.to_string()),
                ]),
            ).await;
        }

        // Check memory usage
        if metrics.memory_usage_mb > self.alert_config.memory_threshold_mb {
            self.trigger_alert(
                "high_memory_usage".to_string(),
                AlertSeverity::Warning,
                format!(
                    "Memory usage ({} MB) above threshold ({} MB)",
                    metrics.memory_usage_mb,
                    self.alert_config.memory_threshold_mb
                ),
                "system".to_string(),
                HashMap::from([
                    ("memory_usage_mb".to_string(), metrics.memory_usage_mb.to_string()),
                ]),
            ).await;
        }
    }

    /// Trigger an alert with cooldown logic
    async fn trigger_alert(
        &self,
        alert_id: String,
        severity: AlertSeverity,
        message: String,
        component: String,
        metadata: HashMap<String, String>,
    ) {
        // Check cooldown
        {
            let last_alert_times = self.last_alert_times.read().await;
            if let Some(last_time) = last_alert_times.get(&alert_id) {
                if last_time.elapsed() < self.alert_config.alert_cooldown {
                    return; // Still in cooldown period
                }
            }
        }

        let alert = Alert {
            id: alert_id.clone(),
            severity: severity.clone(),
            message: message.clone(),
            component,
            timestamp: Instant::now(),
            metadata,
        };

        // Add to active alerts
        {
            let mut active_alerts = self.active_alerts.write().await;
            active_alerts.insert(alert_id.clone(), alert.clone());
        }

        // Add to alert history
        {
            let mut alert_history = self.alert_history.write().await;
            alert_history.push_back(alert.clone());

            // Keep only recent alerts (last 1000)
            if alert_history.len() > 1000 {
                alert_history.pop_front();
            }
        }

        // Update last alert time
        {
            let mut last_alert_times = self.last_alert_times.write().await;
            last_alert_times.insert(alert_id, Instant::now());
        }

        match severity {
            AlertSeverity::Critical => error!("CRITICAL ALERT: {}", message),
            AlertSeverity::Warning => warn!("WARNING ALERT: {}", message),
            AlertSeverity::Info => debug!("INFO ALERT: {}", message),
        }
    }

    /// Resolve an active alert
    pub async fn resolve_alert(&self, alert_id: &str) {
        let mut active_alerts = self.active_alerts.write().await;
        if let Some(alert) = active_alerts.remove(alert_id) {
            debug!("Resolved alert: {} - {}", alert_id, alert.message);
        }
    }

    /// Calculate aggregated metrics from a series of values
    fn calculate_aggregated_metrics(&self, values: &[f64]) -> AggregatedMetrics {
        if values.is_empty() {
            return AggregatedMetrics {
                min: 0.0,
                max: 0.0,
                avg: 0.0,
                sum: 0.0,
                count: 0,
                std_dev: 0.0,
                percentiles: HashMap::new(),
            };
        }

        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let sum: f64 = values.iter().sum();
        let count = values.len() as u64;
        let avg = sum / count as f64;

        // Calculate standard deviation
        let variance: f64 = values.iter()
            .map(|x| (x - avg).powi(2))
            .sum::<f64>() / count as f64;
        let std_dev = variance.sqrt();

        // Calculate percentiles
        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut percentiles = HashMap::new();
        for &p in &[50, 95, 99] {
            let index = ((p as f64 / 100.0) * (count - 1) as f64) as usize;
            let percentile = sorted_values.get(index).copied().unwrap_or(0.0);
            percentiles.insert(p, percentile);
        }

        AggregatedMetrics {
            min,
            max,
            avg,
            sum,
            count,
            std_dev,
            percentiles,
        }
    }

    /// Export metrics in Prometheus format
    pub async fn export_prometheus_metrics(&self) -> String {
        let mut output = String::new();

        // Strategy metrics
        let strategy_metrics = self.strategy_metrics.read().await;
        for (strategy_id, metrics) in strategy_metrics.iter() {
            if let Some(latest) = metrics.back() {
                output.push_str(&format!(
                    "# HELP artemis_strategy_executions_total Total number of strategy executions\n"
                ));
                output.push_str(&format!(
                    "# TYPE artemis_strategy_executions_total counter\n"
                ));
                output.push_str(&format!(
                    "artemis_strategy_executions_total{{strategy=\"{}\"}} {}\n",
                    strategy_id, latest.executions
                ));

                output.push_str(&format!(
                    "# HELP artemis_strategy_success_rate Strategy success rate\n"
                ));
                output.push_str(&format!(
                    "# TYPE artemis_strategy_success_rate gauge\n"
                ));
                output.push_str(&format!(
                    "artemis_strategy_success_rate{{strategy=\"{}\"}} {}\n",
                    strategy_id, latest.success_rate
                ));
            }
        }

        // Resource metrics
        if let Some(latest_resources) = self.resource_metrics.read().await.back() {
            output.push_str(&format!(
                "# HELP artemis_cpu_usage_percent CPU usage percentage\n"
            ));
            output.push_str(&format!(
                "# TYPE artemis_cpu_usage_percent gauge\n"
            ));
            output.push_str(&format!(
                "artemis_cpu_usage_percent {}\n",
                latest_resources.cpu_usage
            ));

            output.push_str(&format!(
                "# HELP artemis_memory_usage_mb Memory usage in megabytes\n"
            ));
            output.push_str(&format!(
                "# TYPE artemis_memory_usage_mb gauge\n"
            ));
            output.push_str(&format!(
                "artemis_memory_usage_mb {}\n",
                latest_resources.memory_usage_mb
            ));
        }

        output
    }
}

/// Summary of all metrics
#[derive(Debug)]
pub struct MetricsSummary {
    pub total_strategies: usize,
    pub total_executions: u64,
    pub average_success_rate: f64,
    pub active_alerts: usize,
    pub uptime: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        let summary = collector.get_summary().await;
        assert_eq!(summary.total_strategies, 0);
    }

    #[tokio::test]
    async fn test_record_strategy_metrics() {
        let collector = MetricsCollector::new();

        let metrics = PerformanceMetrics {
            strategy_id: "test_strategy".to_string(),
            executions: 100,
            success_rate: 0.95,
            avg_latency: Duration::from_millis(150),
            profit: 1000.0,
            timestamp: SystemTime::now(),
        };

        collector.record_strategy_metrics(metrics).await;

        let strategy_metrics = collector.get_strategy_metrics("test_strategy").await;
        assert!(strategy_metrics.is_some());
        assert_eq!(strategy_metrics.unwrap().len(), 1);
    }

    #[test]
    fn test_aggregated_metrics_calculation() {
        let collector = MetricsCollector::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let aggregated = collector.calculate_aggregated_metrics(&values);

        assert_eq!(aggregated.min, 1.0);
        assert_eq!(aggregated.max, 5.0);
        assert_eq!(aggregated.avg, 3.0);
        assert_eq!(aggregated.sum, 15.0);
        assert_eq!(aggregated.count, 5);
    }
}
