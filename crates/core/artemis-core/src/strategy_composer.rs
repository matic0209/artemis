//! Advanced Strategy Composition System
//!
//! This module provides sophisticated strategy orchestration capabilities,
//! including strategy chaining, conditional execution, and dynamic optimization.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use serde::{Serialize, Deserialize};

use crate::types::Strategy;

/// Strategy composition modes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositionMode {
    /// Execute strategies in sequence
    Sequential,
    /// Execute strategies in parallel
    Parallel,
    /// Execute strategies conditionally based on rules
    Conditional,
    /// Execute strategies with voting mechanism
    Voting,
    /// Execute strategies with fallback chain
    Fallback,
}

/// Strategy execution context with rich metadata
#[derive(Debug, Clone)]
pub struct StrategyExecutionContext {
    /// Strategy identifier
    pub strategy_id: String,
    /// Execution priority
    pub priority: ExecutionPriority,
    /// Resource allocation
    pub resource_quota: ResourceQuota,
    /// Execution constraints
    pub constraints: ExecutionConstraints,
    /// Performance targets
    pub performance_targets: PerformanceTargets,
}

/// Execution priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExecutionPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

/// Resource allocation quotas
#[derive(Debug, Clone)]
pub struct ResourceQuota {
    /// Maximum CPU time allocation
    pub max_cpu_time: Duration,
    /// Maximum memory allocation in MB
    pub max_memory_mb: u64,
    /// Maximum gas budget
    pub max_gas: u64,
    /// Network bandwidth allocation
    pub max_bandwidth_mbps: u32,
}

/// Execution constraints
#[derive(Debug, Clone)]
pub struct ExecutionConstraints {
    /// Maximum execution time
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Minimum success rate required
    pub min_success_rate: f64,
    /// Market conditions required
    pub market_conditions: Vec<MarketCondition>,
}

/// Market condition filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCondition {
    /// Condition type
    pub condition_type: MarketConditionType,
    /// Value to compare against
    pub threshold: f64,
    /// Comparison operator
    pub operator: ComparisonOperator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketConditionType {
    /// Gas price in gwei
    GasPrice,
    /// ETH price in USD
    EthPrice,
    /// Market volatility index
    VolatilityIndex,
    /// Network congestion level
    NetworkCongestion,
    /// DEX TVL threshold
    DexTvl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equal,
    NotEqual,
}

/// Performance targets for strategies
#[derive(Debug, Clone)]
pub struct PerformanceTargets {
    /// Target latency
    pub target_latency: Duration,
    /// Target success rate
    pub target_success_rate: f64,
    /// Target profit margin
    pub target_profit_margin: f64,
    /// Target throughput (events/second)
    pub target_throughput: f64,
}

/// Strategy composition rule
#[derive(Debug, Clone)]
pub struct CompositionRule {
    /// Rule identifier
    pub rule_id: String,
    /// Condition to evaluate
    pub condition: RuleCondition,
    /// Action to take when condition is met
    pub action: RuleAction,
    /// Rule priority
    pub priority: u32,
}

/// Rule condition types
#[derive(Debug, Clone)]
pub enum RuleCondition {
    /// Always execute
    Always,
    /// Never execute
    Never,
    /// Execute based on strategy performance
    PerformanceBased {
        strategy_id: String,
        metric: PerformanceMetric,
        threshold: f64,
        operator: ComparisonOperator,
    },
    /// Execute based on market conditions
    MarketBased(MarketCondition),
    /// Execute based on time windows
    TimeBased {
        start_time: std::time::SystemTime,
        end_time: std::time::SystemTime,
        timezone: String,
    },
    /// Execute based on event characteristics
    EventBased {
        event_type: String,
        field: String,
        threshold: f64,
        operator: ComparisonOperator,
    },
    /// Complex logical conditions
    Logical {
        operator: LogicalOperator,
        conditions: Vec<Box<RuleCondition>>,
    },
}

#[derive(Debug, Clone)]
pub enum LogicalOperator {
    And,
    Or,
    Not,
}

/// Performance metrics for rule evaluation
#[derive(Debug, Clone)]
pub enum PerformanceMetric {
    SuccessRate,
    AverageLatency,
    TotalProfit,
    Throughput,
    ErrorRate,
}

/// Rule actions
#[derive(Debug, Clone)]
pub enum RuleAction {
    /// Enable strategy execution
    EnableStrategy(String),
    /// Disable strategy execution
    DisableStrategy(String),
    /// Adjust strategy priority
    AdjustPriority {
        strategy_id: String,
        new_priority: ExecutionPriority,
    },
    /// Modify resource allocation
    AdjustResources {
        strategy_id: String,
        new_quota: ResourceQuota,
    },
    /// Trigger alert
    TriggerAlert {
        level: AlertLevel,
        message: String,
    },
    /// Execute custom action
    Custom {
        action_type: String,
        parameters: HashMap<String, String>,
    },
}

#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

/// Strategy voting configuration
#[derive(Debug, Clone)]
pub struct VotingConfig {
    /// Minimum number of votes required
    pub min_votes: u32,
    /// Voting threshold (percentage)
    pub threshold: f64,
    /// Voting timeout
    pub timeout: Duration,
    /// Weight assignments for strategies
    pub weights: HashMap<String, f64>,
}

/// Composite strategy that orchestrates multiple strategies
pub struct StrategyComposer<E, A>
where
    E: Send + Sync + 'static,
    A: Send + Sync + 'static,
{
    /// Registered strategies
    strategies: Arc<RwLock<HashMap<String, Arc<RwLock<Box<dyn Strategy<E, A> + Send + Sync>>>>>>,
    /// Strategy execution contexts
    contexts: Arc<RwLock<HashMap<String, StrategyExecutionContext>>>,
    /// Composition rules
    rules: Arc<RwLock<Vec<CompositionRule>>>,
    /// Performance history
    performance_history: Arc<RwLock<HashMap<String, VecDeque<StrategyPerformance>>>>,
    /// Active market conditions
    market_state: Arc<RwLock<MarketState>>,
    /// Composer configuration
    config: ComposerConfig,
}

/// Strategy performance record
#[derive(Debug, Clone)]
pub struct StrategyPerformance {
    /// Timestamp of record
    pub timestamp: Instant,
    /// Execution latency
    pub latency: Duration,
    /// Success status
    pub success: bool,
    /// Profit generated
    pub profit: f64,
    /// Gas used
    pub gas_used: u64,
    /// Error message if failed
    pub error: Option<String>,
}

/// Current market state
#[derive(Debug, Clone)]
pub struct MarketState {
    /// Current gas price in gwei
    pub gas_price_gwei: f64,
    /// ETH price in USD
    pub eth_price_usd: f64,
    /// Market volatility index
    pub volatility_index: f64,
    /// Network congestion level (0.0 to 1.0)
    pub network_congestion: f64,
    /// Total DEX TVL
    pub dex_tvl: f64,
    /// Last update timestamp
    pub last_updated: Instant,
}

impl Default for MarketState {
    fn default() -> Self {
        Self {
            gas_price_gwei: 0.0,
            eth_price_usd: 0.0,
            volatility_index: 0.0,
            network_congestion: 0.0,
            dex_tvl: 0.0,
            last_updated: Instant::now(),
        }
    }
}

/// Composer configuration
#[derive(Debug, Clone)]
pub struct ComposerConfig {
    /// Default composition mode
    pub default_mode: CompositionMode,
    /// Maximum parallel executions
    pub max_parallel_executions: usize,
    /// Strategy timeout
    pub strategy_timeout: Duration,
    /// Performance window size
    pub performance_window_size: usize,
    /// Market state update interval
    pub market_update_interval: Duration,
    /// Enable adaptive optimization
    pub enable_adaptive_optimization: bool,
}

impl Default for ComposerConfig {
    fn default() -> Self {
        Self {
            default_mode: CompositionMode::Parallel,
            max_parallel_executions: 10,
            strategy_timeout: Duration::from_secs(30),
            performance_window_size: 1000,
            market_update_interval: Duration::from_secs(60),
            enable_adaptive_optimization: true,
        }
    }
}

impl<E, A> StrategyComposer<E, A>
where
    E: Send + Sync + Clone + 'static,
    A: Send + Sync + Clone + 'static,
{
    /// Create a new strategy composer
    pub fn new(config: ComposerConfig) -> Self {
        Self {
            strategies: Arc::new(RwLock::new(HashMap::new())),
            contexts: Arc::new(RwLock::new(HashMap::new())),
            rules: Arc::new(RwLock::new(Vec::new())),
            performance_history: Arc::new(RwLock::new(HashMap::new())),
            market_state: Arc::new(RwLock::new(MarketState::default())),
            config,
        }
    }

    /// Register a strategy with the composer
    pub async fn register_strategy(
        &self,
        strategy_id: String,
        strategy: Box<dyn Strategy<E, A> + Send + Sync>,
        context: StrategyExecutionContext,
    ) -> Result<(), ComposerError> {
        info!("Registering strategy: {}", strategy_id);

        // Add strategy
        {
            let mut strategies = self.strategies.write().await;
            strategies.insert(strategy_id.clone(), Arc::new(RwLock::new(strategy)));
        }

        // Add context
        {
            let mut contexts = self.contexts.write().await;
            contexts.insert(strategy_id.clone(), context);
        }

        // Initialize performance history
        {
            let mut history = self.performance_history.write().await;
            history.insert(strategy_id, VecDeque::new());
        }

        Ok(())
    }

    /// Add a composition rule
    pub async fn add_rule(&self, rule: CompositionRule) {
        let mut rules = self.rules.write().await;
        rules.push(rule);

        // Sort rules by priority
        rules.sort_by(|a, b| a.priority.cmp(&b.priority));
    }

    /// Execute strategies using the specified composition mode
    pub async fn execute_composed(
        &self,
        event: E,
        mode: Option<CompositionMode>,
    ) -> Result<ComposedExecutionResult<A>, ComposerError> {
        let execution_mode = mode.unwrap_or(self.config.default_mode.clone());

        debug!("Executing composed strategies with mode: {:?}", execution_mode);

        // Evaluate rules to determine active strategies
        let active_strategies = self.evaluate_rules(&event).await?;

        if active_strategies.is_empty() {
            return Ok(ComposedExecutionResult {
                mode: execution_mode,
                total_duration: Duration::default(),
                strategy_results: HashMap::new(),
                combined_actions: Vec::new(),
                success: true,
                errors: Vec::new(),
            });
        }

        let start_time = Instant::now();

        // Execute based on composition mode
        let result = match execution_mode {
            CompositionMode::Sequential => self.execute_sequential(event, active_strategies).await?,
            CompositionMode::Parallel => self.execute_parallel(event, active_strategies).await?,
            CompositionMode::Conditional => self.execute_conditional(event, active_strategies).await?,
            CompositionMode::Voting => self.execute_voting(event, active_strategies).await?,
            CompositionMode::Fallback => self.execute_fallback(event, active_strategies).await?,
        };

        let total_duration = start_time.elapsed();

        // Record performance for all executed strategies
        for (strategy_id, strategy_result) in &result.strategy_results {
            self.record_performance(strategy_id, strategy_result).await;
        }

        Ok(ComposedExecutionResult {
            mode: execution_mode,
            total_duration,
            strategy_results: result.strategy_results,
            combined_actions: result.combined_actions,
            success: result.success,
            errors: result.errors,
        })
    }

    /// Execute strategies sequentially
    async fn execute_sequential(
        &self,
        event: E,
        strategy_ids: Vec<String>,
    ) -> Result<ComposedExecutionResult<A>, ComposerError> {
        let mut strategy_results = HashMap::new();
        let mut combined_actions = Vec::new();
        let mut errors = Vec::new();

        for strategy_id in strategy_ids {
            match self.execute_single_strategy(&strategy_id, event.clone()).await {
                Ok(result) => {
                    combined_actions.extend(result.actions.clone());
                    strategy_results.insert(strategy_id, result);
                }
                Err(e) => {
                    errors.push(format!("Strategy {} failed: {:?}", strategy_id, e));
                }
            }
        }

        Ok(ComposedExecutionResult {
            mode: CompositionMode::Sequential,
            total_duration: Duration::default(),
            strategy_results,
            combined_actions,
            success: errors.is_empty(),
            errors,
        })
    }

    /// Execute strategies in parallel
    async fn execute_parallel(
        &self,
        event: E,
        strategy_ids: Vec<String>,
    ) -> Result<ComposedExecutionResult<A>, ComposerError> {
        let mut tasks = Vec::new();

        // Spawn tasks for each strategy
        for strategy_id in strategy_ids {
            let event_clone = event.clone();
            let composer_clone = self.clone_for_task();

            let task = tokio::spawn(async move {
                composer_clone.execute_single_strategy(&strategy_id, event_clone).await
                    .map(|result| (strategy_id, result))
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        let mut strategy_results = HashMap::new();
        let mut combined_actions = Vec::new();
        let mut errors = Vec::new();

        for task in tasks {
            match task.await {
                Ok(Ok((strategy_id, result))) => {
                    combined_actions.extend(result.actions.clone());
                    strategy_results.insert(strategy_id, result);
                }
                Ok(Err(e)) => {
                    errors.push(format!("Strategy execution failed: {:?}", e));
                }
                Err(e) => {
                    errors.push(format!("Task join failed: {:?}", e));
                }
            }
        }

        Ok(ComposedExecutionResult {
            mode: CompositionMode::Parallel,
            total_duration: Duration::default(),
            strategy_results,
            combined_actions,
            success: errors.is_empty(),
            errors,
        })
    }

    /// Execute a single strategy with performance tracking
    async fn execute_single_strategy(
        &self,
        strategy_id: &str,
        event: E,
    ) -> Result<StrategyResult<A>, ComposerError> {
        let start_time = Instant::now();

        // Get strategy and context
        let (strategy, context) = {
            let strategies = self.strategies.read().await;
            let contexts = self.contexts.read().await;

            let strategy = strategies.get(strategy_id)
                .ok_or_else(|| ComposerError::StrategyNotFound(strategy_id.to_string()))?
                .clone();

            let context = contexts.get(strategy_id)
                .ok_or_else(|| ComposerError::ContextNotFound(strategy_id.to_string()))?
                .clone();

            (strategy, context)
        };

        // Execute with timeout
        let execution = {
            let strategy = Arc::clone(&strategy);
            async move {
                let mut guard = strategy.write().await;
                guard.process_event(event).await
            }
        };

        let result = tokio::time::timeout(context.constraints.timeout, execution).await;

        let duration = start_time.elapsed();

        match result {
            Ok(actions) => {
                Ok(StrategyResult {
                    strategy_id: strategy_id.to_string(),
                    success: true,
                    duration,
                    actions,
                    error: None,
                    gas_used: 0, // Would be calculated from actual execution
                    profit: 0.0, // Would be calculated from actual results
                })
            }
            Err(_) => {
                Err(ComposerError::StrategyTimeout(strategy_id.to_string()))
            }
        }
    }

    /// Execute strategies with conditional logic (placeholder)
    async fn execute_conditional(
        &self,
        event: E,
        strategy_ids: Vec<String>,
    ) -> Result<ComposedExecutionResult<A>, ComposerError> {
        // For now, fall back to sequential execution
        // Real implementation would evaluate complex conditions
        self.execute_sequential(event, strategy_ids).await
    }

    /// Execute strategies with voting mechanism (placeholder)
    async fn execute_voting(
        &self,
        event: E,
        strategy_ids: Vec<String>,
    ) -> Result<ComposedExecutionResult<A>, ComposerError> {
        // For now, fall back to parallel execution
        // Real implementation would collect votes and execute based on consensus
        self.execute_parallel(event, strategy_ids).await
    }

    /// Execute strategies with fallback chain (placeholder)
    async fn execute_fallback(
        &self,
        event: E,
        strategy_ids: Vec<String>,
    ) -> Result<ComposedExecutionResult<A>, ComposerError> {
        // For now, fall back to sequential execution
        // Real implementation would try strategies in order until one succeeds
        self.execute_sequential(event, strategy_ids).await
    }

    /// Evaluate rules to determine which strategies should execute
    async fn evaluate_rules(&self, _event: &E) -> Result<Vec<String>, ComposerError> {
        let strategies = self.strategies.read().await;
        let strategy_ids: Vec<String> = strategies.keys().cloned().collect();

        // For now, return all strategies
        // Real implementation would evaluate complex rules
        Ok(strategy_ids)
    }

    /// Record performance metrics for a strategy
    async fn record_performance(&self, strategy_id: &str, result: &StrategyResult<A>) {
        let performance = StrategyPerformance {
            timestamp: Instant::now(),
            latency: result.duration,
            success: result.success,
            profit: result.profit,
            gas_used: result.gas_used,
            error: result.error.clone(),
        };

        let mut history = self.performance_history.write().await;
        if let Some(strategy_history) = history.get_mut(strategy_id) {
            strategy_history.push_back(performance);

            // Keep only recent performance data
            if strategy_history.len() > self.config.performance_window_size {
                strategy_history.pop_front();
            }
        }
    }

    /// Update market state
    pub async fn update_market_state(&self, new_state: MarketState) {
        let mut market_state = self.market_state.write().await;
        *market_state = new_state;
    }

    /// Get performance statistics for a strategy
    pub async fn get_strategy_performance(&self, strategy_id: &str) -> Option<StrategyPerformanceStats> {
        let history = self.performance_history.read().await;
        let strategy_history = history.get(strategy_id)?;

        if strategy_history.is_empty() {
            return None;
        }

        let total_executions = strategy_history.len();
        let successful_executions = strategy_history.iter().filter(|p| p.success).count();
        let success_rate = successful_executions as f64 / total_executions as f64;

        let avg_latency = strategy_history.iter()
            .map(|p| p.latency)
            .sum::<Duration>() / total_executions as u32;

        let total_profit = strategy_history.iter().map(|p| p.profit).sum();

        Some(StrategyPerformanceStats {
            strategy_id: strategy_id.to_string(),
            total_executions,
            successful_executions,
            success_rate,
            avg_latency,
            total_profit,
        })
    }

    /// Clone the composer for use in async tasks
    fn clone_for_task(&self) -> Self {
        Self {
            strategies: Arc::clone(&self.strategies),
            contexts: Arc::clone(&self.contexts),
            rules: Arc::clone(&self.rules),
            performance_history: Arc::clone(&self.performance_history),
            market_state: Arc::clone(&self.market_state),
            config: self.config.clone(),
        }
    }
}

/// Result of a composed strategy execution
#[derive(Debug)]
pub struct ComposedExecutionResult<A> {
    /// Composition mode used
    pub mode: CompositionMode,
    /// Total execution duration
    pub total_duration: Duration,
    /// Results from individual strategies
    pub strategy_results: HashMap<String, StrategyResult<A>>,
    /// Combined actions from all strategies
    pub combined_actions: Vec<A>,
    /// Overall success status
    pub success: bool,
    /// Error messages
    pub errors: Vec<String>,
}

/// Result from a single strategy execution
#[derive(Debug, Clone)]
pub struct StrategyResult<A> {
    /// Strategy identifier
    pub strategy_id: String,
    /// Success status
    pub success: bool,
    /// Execution duration
    pub duration: Duration,
    /// Actions produced
    pub actions: Vec<A>,
    /// Error message if failed
    pub error: Option<String>,
    /// Gas used in execution
    pub gas_used: u64,
    /// Profit generated
    pub profit: f64,
}

/// Performance statistics for a strategy
#[derive(Debug, Clone)]
pub struct StrategyPerformanceStats {
    /// Strategy identifier
    pub strategy_id: String,
    /// Total number of executions
    pub total_executions: usize,
    /// Number of successful executions
    pub successful_executions: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average execution latency
    pub avg_latency: Duration,
    /// Total profit generated
    pub total_profit: f64,
}

/// Strategy composer errors
#[derive(Debug, thiserror::Error)]
pub enum ComposerError {
    #[error("Strategy not found: {0}")]
    StrategyNotFound(String),
    #[error("Context not found: {0}")]
    ContextNotFound(String),
    #[error("Strategy timeout: {0}")]
    StrategyTimeout(String),
    #[error("Rule evaluation failed: {0}")]
    RuleEvaluationFailed(String),
    #[error("Market state unavailable")]
    MarketStateUnavailable,
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use anyhow::Result;

    // Mock strategy for testing
    struct MockStrategy {
        id: String,
    }

    #[async_trait]
    impl Strategy<String, String> for MockStrategy {
        async fn sync_state(&mut self) -> Result<()> {
            Ok(())
        }

        async fn process_event(&mut self, _event: String) -> Vec<String> {
            vec![format!("action_from_{}", self.id)]
        }
    }

    #[tokio::test]
    async fn test_composer_creation() {
        let config = ComposerConfig::default();
        let composer: StrategyComposer<String, String> = StrategyComposer::new(config);

        // Test that composer is created successfully
        assert_eq!(composer.strategies.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_strategy_registration() {
        let config = ComposerConfig::default();
        let composer: StrategyComposer<String, String> = StrategyComposer::new(config);

        let strategy: Box<dyn Strategy<String, String> + Send + Sync> = Box::new(MockStrategy {
            id: "test_strategy".to_string(),
        });

        let context = StrategyExecutionContext {
            strategy_id: "test_strategy".to_string(),
            priority: ExecutionPriority::Medium,
            resource_quota: ResourceQuota {
                max_cpu_time: Duration::from_secs(10),
                max_memory_mb: 100,
                max_gas: 1000000,
                max_bandwidth_mbps: 10,
            },
            constraints: ExecutionConstraints {
                timeout: Duration::from_secs(30),
                max_retries: 3,
                min_success_rate: 0.8,
                market_conditions: Vec::new(),
            },
            performance_targets: PerformanceTargets {
                target_latency: Duration::from_millis(100),
                target_success_rate: 0.95,
                target_profit_margin: 0.1,
                target_throughput: 100.0,
            },
        };

        let result = composer.register_strategy("test_strategy".to_string(), strategy, context).await;
        assert!(result.is_ok());

        assert_eq!(composer.strategies.read().await.len(), 1);
    }
}
