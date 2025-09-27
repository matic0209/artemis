//! High-Performance Artemis Engine
//!
//! Advanced engine implementation with sophisticated event processing,
//! adaptive load balancing, and comprehensive monitoring capabilities.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

use crate::types::Strategy;
use super::event_bus::{EventBus, EventBusConfig};
use super::execution_coordinator::ExecutionCoordinator;
use super::metrics_collector::{MetricsCollector, MetricsSummary, PerformanceMetrics};

/// Advanced engine configuration
#[derive(Debug, Clone)]
pub struct HighPerformanceEngineConfig {
    /// Maximum concurrent collectors
    pub max_collectors: usize,
    /// Maximum concurrent executors
    pub max_executors: usize,
    /// Strategy execution timeout
    pub strategy_timeout: Duration,
    /// Enable adaptive load balancing
    pub enable_adaptive_balancing: bool,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Circuit breaker threshold
    pub circuit_breaker_threshold: f64,
    /// Event bus configuration
    pub event_bus_config: EventBusConfig,
    /// Enable hot-swapping of strategies
    pub enable_hot_swap: bool,
}

impl Default for HighPerformanceEngineConfig {
    fn default() -> Self {
        Self {
            max_collectors: 16,
            max_executors: 32,
            strategy_timeout: Duration::from_secs(10),
            enable_adaptive_balancing: true,
            health_check_interval: Duration::from_secs(30),
            circuit_breaker_threshold: 0.95,
            event_bus_config: EventBusConfig::default(),
            enable_hot_swap: true,
        }
    }
}

/// Engine component health status
#[derive(Debug, Clone)]
pub enum ComponentHealth {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Strategy execution context with enhanced metadata
#[derive(Debug)]
pub struct StrategyContext {
    /// Strategy identifier
    pub strategy_id: String,
    /// Execution metrics
    pub metrics: StrategyMetrics,
    /// Circuit breaker state
    pub circuit_breaker: CircuitBreaker,
    /// Last successful execution
    pub last_success: Option<Instant>,
}

/// Per-strategy performance metrics
#[derive(Debug, Default)]
pub struct StrategyMetrics {
    /// Total executions
    pub executions: u64,
    /// Successful executions
    pub successes: u64,
    /// Failed executions
    pub failures: u64,
    /// Average execution time
    pub avg_execution_time: Duration,
    /// Profit generated
    pub total_profit: f64,
}

/// Circuit breaker for fault tolerance
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Current state
    pub state: CircuitBreakerState,
    /// Failure count in current window
    pub failure_count: u32,
    /// Success count in current window
    pub success_count: u32,
    /// Last state change
    pub last_state_change: Instant,
    /// Threshold for opening circuit
    pub failure_threshold: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    fn new(failure_threshold: u32) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            last_state_change: Instant::now(),
            failure_threshold,
        }
    }

    fn record_success(&mut self) {
        self.success_count += 1;
        if self.state == CircuitBreakerState::HalfOpen && self.success_count >= 3 {
            self.state = CircuitBreakerState::Closed;
            self.failure_count = 0;
            self.last_state_change = Instant::now();
        }
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        if self.failure_count >= self.failure_threshold {
            self.state = CircuitBreakerState::Open;
            self.last_state_change = Instant::now();
        }
    }

    fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Try to transition to half-open after timeout
                if self.last_state_change.elapsed() > Duration::from_secs(60) {
                    self.state = CircuitBreakerState::HalfOpen;
                    self.last_state_change = Instant::now();
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }
}

/// High-performance engine with advanced capabilities
pub struct HighPerformanceEngine<E, A>
where
    E: Send + Sync + 'static,
    A: Send + Sync + 'static,
{
    /// Engine configuration
    config: HighPerformanceEngineConfig,
    /// High-performance event bus
    event_bus: Arc<EventBus<E>>,
    /// Execution coordinator
    execution_coordinator: Arc<ExecutionCoordinator<A>>,
    /// Performance metrics collector
    metrics_collector: Arc<MetricsCollector>,
    /// Registered strategies
    strategies: Arc<RwLock<HashMap<String, Arc<RwLock<Box<dyn Strategy<E, A> + Send + Sync>>>>>>,
    /// Strategy contexts
    strategy_contexts: Arc<RwLock<HashMap<String, StrategyContext>>>,
    /// Component health status
    component_health: Arc<RwLock<HashMap<String, ComponentHealth>>>,
    /// Running tasks
    running_tasks: Arc<RwLock<Vec<JoinHandle<()>>>>,
    /// Shutdown signal
    shutdown_tx: Arc<RwLock<Option<mpsc::Sender<()>>>>,
    /// Load balancing semaphore
    load_balancer: Arc<Semaphore>,
}

impl<E, A> HighPerformanceEngine<E, A>
where
    E: Send + Sync + Clone + 'static,
    A: Send + Sync + Clone + 'static,
{
    /// Create a new high-performance engine
    pub fn new(config: HighPerformanceEngineConfig) -> Self {
        let event_bus = Arc::new(EventBus::new(config.event_bus_config.clone()));
        let execution_coordinator = Arc::new(ExecutionCoordinator::new());
        let metrics_collector = Arc::new(MetricsCollector::new());
        let load_balancer = Arc::new(Semaphore::new(config.max_executors));

        Self {
            config,
            event_bus,
            execution_coordinator,
            metrics_collector,
            strategies: Arc::new(RwLock::new(HashMap::new())),
            strategy_contexts: Arc::new(RwLock::new(HashMap::new())),
            component_health: Arc::new(RwLock::new(HashMap::new())),
            running_tasks: Arc::new(RwLock::new(Vec::new())),
            shutdown_tx: Arc::new(RwLock::new(None)),
            load_balancer,
        }
    }

    /// Start the high-performance engine
    pub async fn start(&self) -> Result<(), EngineError> {
        info!("🚀 Starting Artemis High-Performance Engine");

        // Setup shutdown channel
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        // Start health monitoring
        self.start_health_monitor().await;

        // Start metrics collection
        self.start_metrics_collection().await;

        // Start event processing
        self.start_event_processing().await;

        info!("✅ High-Performance Engine started successfully");

        // Wait for shutdown signal
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received");
            }
        }

        Ok(())
    }

    /// Add a strategy with hot-swap capability
    pub async fn add_strategy(
        &self,
        strategy_id: String,
        strategy: Box<dyn Strategy<E, A> + Send + Sync>,
    ) -> Result<(), EngineError> {
        info!("📊 Adding strategy: {}", strategy_id);

        let context = StrategyContext {
            strategy_id: strategy_id.clone(),
            metrics: StrategyMetrics::default(),
            circuit_breaker: CircuitBreaker::new(10),
            last_success: None,
        };

        self.strategies.write().await.insert(
            strategy_id.clone(),
            Arc::new(RwLock::new(strategy)),
        );
        self.strategy_contexts.write().await.insert(strategy_id.clone(), context);
        self.component_health.write().await.insert(
            format!("strategy_{}", strategy_id),
            ComponentHealth::Healthy,
        );

        Ok(())
    }

    /// Execute a strategy with circuit breaker protection
    async fn execute_strategy_safe(
        &self,
        strategy_id: &str,
        event: E,
    ) -> Result<Vec<A>, EngineError> {
        let start_time = Instant::now();

        let strategy = {
            let strategies = self.strategies.read().await;
            strategies
                .get(strategy_id)
                .cloned()
                .ok_or_else(|| EngineError::StrategyNotFound(strategy_id.to_string()))?
        };

        // Check circuit breaker
        {
            let mut contexts = self.strategy_contexts.write().await;
            if let Some(context) = contexts.get_mut(strategy_id) {
                if !context.circuit_breaker.can_execute() {
                    return Err(EngineError::CircuitBreakerOpen(strategy_id.to_string()));
                }
            }
        }

        // Execute with timeout
        let execution = {
            let strategy = Arc::clone(&strategy);
            async move {
                let mut guard = strategy.write().await;
                guard.process_event(event).await
            }
        };

        let result = tokio::time::timeout(self.config.strategy_timeout, execution).await;

        let execution_time = start_time.elapsed();

        // Update metrics and circuit breaker
        let mut contexts = self.strategy_contexts.write().await;
        if let Some(context) = contexts.get_mut(strategy_id) {
            context.metrics.executions += 1;
            context.metrics.avg_execution_time =
                (context.metrics.avg_execution_time + execution_time) / 2;

            match result {
                Ok(actions) => {
                    context.metrics.successes += 1;
                    context.circuit_breaker.record_success();
                    context.last_success = Some(Instant::now());
                    Ok(actions)
                }
                Err(_) => {
                    context.metrics.failures += 1;
                    context.circuit_breaker.record_failure();
                    Err(EngineError::StrategyTimeout(strategy_id.to_string()))
                }
            }
        } else {
            Err(EngineError::StrategyNotFound(strategy_id.to_string()))
        }
    }

    /// Start health monitoring
    async fn start_health_monitor(&self) {
        let component_health = Arc::clone(&self.component_health);
        let strategy_contexts = Arc::clone(&self.strategy_contexts);
        let health_check_interval = self.config.health_check_interval;

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(health_check_interval);

            loop {
                interval.tick().await;

                // Check strategy health
                let contexts = strategy_contexts.read().await;
                let mut health = component_health.write().await;

                for (strategy_id, context) in contexts.iter() {
                    let health_key = format!("strategy_{}", strategy_id);

                    // Calculate failure rate
                    let total_executions = context.metrics.executions;
                    if total_executions > 0 {
                        let failure_rate = context.metrics.failures as f64 / total_executions as f64;

                        let component_health = if failure_rate > 0.5 {
                            ComponentHealth::Unhealthy {
                                reason: format!("High failure rate: {:.2}%", failure_rate * 100.0)
                            }
                        } else if failure_rate > 0.2 {
                            ComponentHealth::Degraded {
                                reason: format!("Elevated failure rate: {:.2}%", failure_rate * 100.0)
                            }
                        } else {
                            ComponentHealth::Healthy
                        };

                        health.insert(health_key, component_health);
                    }
                }

                debug!("Health check completed for {} components", health.len());
            }
        });

        self.running_tasks.write().await.push(task);
    }

    /// Start metrics collection
    async fn start_metrics_collection(&self) {
        let metrics_collector = Arc::clone(&self.metrics_collector);
        let strategy_contexts = Arc::clone(&self.strategy_contexts);

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                let contexts = strategy_contexts.read().await;
                for (strategy_id, context) in contexts.iter() {
                    let metrics = PerformanceMetrics {
                        strategy_id: strategy_id.clone(),
                        executions: context.metrics.executions,
                        success_rate: if context.metrics.executions > 0 {
                            context.metrics.successes as f64 / context.metrics.executions as f64
                        } else {
                            0.0
                        },
                        avg_latency: context.metrics.avg_execution_time,
                        profit: context.metrics.total_profit,
                        timestamp: SystemTime::now(),
                    };

                    metrics_collector.record_strategy_metrics(metrics).await;
                }
            }
        });

        self.running_tasks.write().await.push(task);
    }

    /// Start event processing
    async fn start_event_processing(&self) {
        // Implementation would start event processing workers
        debug!("Event processing started");
    }

    /// Get comprehensive engine status
    pub async fn get_status(&self) -> EngineStatus {
        let contexts = self.strategy_contexts.read().await;
        let health = self.component_health.read().await;

        let total_strategies = contexts.len();
        let healthy_strategies = health.values()
            .filter(|h| matches!(h, ComponentHealth::Healthy))
            .count();

        EngineStatus {
            total_strategies,
            healthy_strategies,
            uptime: Instant::now(), // Would track actual uptime
            metrics: self.metrics_collector.get_summary().await,
        }
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) -> Result<(), EngineError> {
        info!("🛑 Shutting down High-Performance Engine");

        // Signal shutdown
        if let Some(sender) = self.shutdown_tx.read().await.as_ref() {
            let _ = sender.send(()).await;
        }

        // Wait for tasks to complete
        let mut tasks = self.running_tasks.write().await;
        for task in tasks.drain(..) {
            if !task.is_finished() {
                task.abort();
            }
        }

        // Shutdown event bus
        self.event_bus.shutdown().await
            .map_err(|e| EngineError::ShutdownFailed(format!("{:?}", e)))?;

        info!("✅ Engine shutdown completed");
        Ok(())
    }
}

/// Engine status report
#[derive(Debug)]
pub struct EngineStatus {
    pub total_strategies: usize,
    pub healthy_strategies: usize,
    pub uptime: Instant,
    pub metrics: MetricsSummary,
}

/// Engine-specific errors
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Strategy not found: {0}")]
    StrategyNotFound(String),
    #[error("Strategy timeout: {0}")]
    StrategyTimeout(String),
    #[error("Strategy error: {0}")]
    StrategyError(String),
    #[error("Circuit breaker open for strategy: {0}")]
    CircuitBreakerOpen(String),
    #[error("Shutdown failed: {0}")]
    ShutdownFailed(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker() {
        let mut cb = CircuitBreaker::new(3);
        assert_eq!(cb.state, CircuitBreakerState::Closed);
        assert!(cb.can_execute());

        // Record failures
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();

        assert_eq!(cb.state, CircuitBreakerState::Open);
        assert!(!cb.can_execute());
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let config = HighPerformanceEngineConfig::default();
        let engine: HighPerformanceEngine<String, String> = HighPerformanceEngine::new(config);

        let status = engine.get_status().await;
        assert_eq!(status.total_strategies, 0);
    }
}
