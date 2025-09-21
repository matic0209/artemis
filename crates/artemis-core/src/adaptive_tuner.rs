use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use anyhow::Result;

/// Adaptive performance tuner that automatically optimizes system parameters
pub struct AdaptiveTuner {
    /// Performance history for analysis
    performance_history: Arc<RwLock<VecDeque<PerformanceSnapshot>>>,
    /// Current tuning parameters
    current_params: Arc<RwLock<TuningParams>>,
    /// Tuning strategy
    strategy: TuningStrategy,
}

#[derive(Debug, Clone)]
struct PerformanceSnapshot {
    timestamp: Instant,
    latency_p50: f64,
    latency_p95: f64,
    throughput: f64,
    error_rate: f64,
    memory_usage: f64,
    cache_hit_rate: f64,
}

#[derive(Debug, Clone)]
pub struct TuningParams {
    /// Event channel buffer sizes
    pub event_buffer_size: usize,
    pub action_buffer_size: usize,
    /// Batch processing parameters
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    /// Cache parameters
    pub cache_size: usize,
    pub cache_ttl_seconds: u64,
    /// Connection pool parameters
    pub max_connections: usize,
    pub connection_timeout_ms: u64,
}

impl Default for TuningParams {
    fn default() -> Self {
        Self {
            event_buffer_size: 1024,
            action_buffer_size: 512,
            batch_size: 10,
            batch_timeout_ms: 10,
            cache_size: 10000,
            cache_ttl_seconds: 60,
            max_connections: 10,
            connection_timeout_ms: 5000,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TuningStrategy {
    /// Conservative tuning with small adjustments
    Conservative,
    /// Aggressive tuning for maximum performance
    Aggressive,
    /// Balanced approach
    Balanced,
}

impl AdaptiveTuner {
    pub fn new(strategy: TuningStrategy) -> Self {
        Self {
            performance_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            current_params: Arc::new(RwLock::new(TuningParams::default())),
            strategy,
        }
    }

    /// Record current performance metrics
    pub async fn record_performance(&self, snapshot: PerformanceSnapshot) {
        let mut history = self.performance_history.write().await;
        
        // Keep only recent history
        if history.len() >= 1000 {
            history.pop_front();
        }
        
        history.push_back(snapshot);
        
        // Trigger tuning if we have enough data
        if history.len() >= 10 {
            self.analyze_and_tune().await;
        }
    }

    async fn analyze_and_tune(&self) {
        let history = self.performance_history.read().await;
        let recent_snapshots: Vec<_> = history.iter().rev().take(10).collect();
        
        if recent_snapshots.len() < 5 {
            return;
        }

        let avg_latency = recent_snapshots.iter()
            .map(|s| s.latency_p95)
            .sum::<f64>() / recent_snapshots.len() as f64;
            
        let avg_throughput = recent_snapshots.iter()
            .map(|s| s.throughput)
            .sum::<f64>() / recent_snapshots.len() as f64;
            
        let avg_error_rate = recent_snapshots.iter()
            .map(|s| s.error_rate)
            .sum::<f64>() / recent_snapshots.len() as f64;

        let mut params = self.current_params.write().await;
        let mut changed = false;

        // Tune based on performance trends
        match self.strategy {
            TuningStrategy::Aggressive => {
                // High latency → increase buffer sizes
                if avg_latency > 100.0 {
                    params.event_buffer_size = (params.event_buffer_size * 2).min(8192);
                    params.action_buffer_size = (params.action_buffer_size * 2).min(4096);
                    changed = true;
                }
                
                // Low throughput → reduce batch timeout
                if avg_throughput < 100.0 {
                    params.batch_timeout_ms = (params.batch_timeout_ms / 2).max(1);
                    changed = true;
                }
                
                // High error rate → increase connection limits
                if avg_error_rate > 0.05 {
                    params.max_connections = (params.max_connections * 2).min(50);
                    params.connection_timeout_ms = (params.connection_timeout_ms * 2).min(30000);
                    changed = true;
                }
            }
            
            TuningStrategy::Conservative => {
                // Only make small adjustments
                if avg_latency > 200.0 {
                    params.event_buffer_size = (params.event_buffer_size * 11 / 10).min(4096);
                    changed = true;
                }
            }
            
            TuningStrategy::Balanced => {
                // Balanced approach between aggressive and conservative
                if avg_latency > 150.0 {
                    params.event_buffer_size = (params.event_buffer_size * 15 / 10).min(6144);
                    changed = true;
                }
                
                if avg_error_rate > 0.03 {
                    params.max_connections = (params.max_connections * 13 / 10).min(30);
                    changed = true;
                }
            }
        }

        if changed {
            tracing::info!("Auto-tuned parameters: latency={:.1}ms, throughput={:.1}, error_rate={:.3}%",
                avg_latency, avg_throughput, avg_error_rate * 100.0);
            
            metrics::counter!("artemis.adaptive_tuner.adjustments").increment(1);
            self.log_parameter_changes(&params).await;
        }
    }

    async fn log_parameter_changes(&self, params: &TuningParams) {
        metrics::gauge!("artemis.tuner.event_buffer_size").set(params.event_buffer_size as f64);
        metrics::gauge!("artemis.tuner.action_buffer_size").set(params.action_buffer_size as f64);
        metrics::gauge!("artemis.tuner.batch_size").set(params.batch_size as f64);
        metrics::gauge!("artemis.tuner.cache_size").set(params.cache_size as f64);
    }

    /// Get current tuning parameters
    pub async fn get_params(&self) -> TuningParams {
        self.current_params.read().await.clone()
    }

    /// Force parameter update (for manual tuning)
    pub async fn update_params(&self, params: TuningParams) {
        let mut current = self.current_params.write().await;
        *current = params;
        
        metrics::counter!("artemis.adaptive_tuner.manual_updates").increment(1);
    }

    /// Start background performance monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        let tuner = self.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Collect current performance metrics
                let snapshot = PerformanceSnapshot {
                    timestamp: Instant::now(),
                    latency_p50: Self::get_metric_value("artemis.engine.strategy_process_time", 0.5),
                    latency_p95: Self::get_metric_value("artemis.engine.strategy_process_time", 0.95),
                    throughput: Self::get_counter_rate("artemis.engine.actions_generated"),
                    error_rate: Self::get_counter_rate("artemis.engine.action_send_errors"),
                    memory_usage: Self::get_memory_usage(),
                    cache_hit_rate: Self::get_cache_hit_rate(),
                };
                
                tuner.record_performance(snapshot).await;
            }
        });
        
        Ok(())
    }

    fn get_metric_value(_metric: &str, _percentile: f64) -> f64 {
        // TODO: Implement actual metrics collection
        0.0
    }

    fn get_counter_rate(_metric: &str) -> f64 {
        // TODO: Implement rate calculation
        0.0
    }

    fn get_memory_usage() -> f64 {
        // TODO: Implement memory usage tracking
        0.0
    }

    fn get_cache_hit_rate() -> f64 {
        // TODO: Implement cache hit rate calculation
        0.0
    }
}

impl Clone for AdaptiveTuner {
    fn clone(&self) -> Self {
        Self {
            performance_history: Arc::clone(&self.performance_history),
            current_params: Arc::clone(&self.current_params),
            strategy: self.strategy.clone(),
        }
    }
}
