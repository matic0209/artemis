//! MEV Arbitrage Strategy - Full Feature Implementation
//!
//! This module provides the complete MEV arbitrage system with all features enabled:
//! - Graph-based arbitrage detection (Bellman-Ford algorithm)
//! - Symbolic execution (Z3 + EVM interpreter)
//! - REVM validation (concrete execution verification)
//! - MEV defense strategies
//! - Unified abstraction architecture


// Re-export from sub-packages
pub use mev_arbitrage_graph::{
    NegativeCycleArbitrageEngine,
    NegativeCycleConfig,
    StateSnapshot,
    ArbitrageCycle,
    ArbitragePath,
};

pub use mev_arbitrage_symbolic::{
    SymbolicEVMInterpreter,
    SEVM,
    ExecutionPath,
    EVMExecutionState,
};

pub use mev_arbitrage_revm::{
    MEVArbitrageEngine as RevmMEVEngine,
    SymbolicStrategyDiscoverer,
    ConcreteExecutionValidator,
};

pub use mev_arbitrage_defense::{
    MEVDefenseEngine,
    DefenseConfig,
    SandwichDetector,
    FrontrunProtector,
    UserTransactionProtector,
};

// Re-export from local modules (these are declared in lib.rs when using cfg feature)
// We use super:: because these modules are siblings, not children
pub use super::abstractions;
pub use super::component_factory;
pub use super::unified_arbitrage_manager;
pub use super::event_system;
pub use super::gas_strategy;
pub use super::z3_optimizer;
// TODO: Temporarily disabled - need to fix dependencies
// pub use super::mev_orchestrator;
// pub use super::mev_pipeline;
// pub use super::hybrid_arbitrage_engine;

// Key type re-exports for convenience
pub use abstractions::{
    ArbitrageDetector, PathExplorer, Validator, Optimizer, Executor,
    ArbitrageOpportunity, ExecutionPlan, DetectionResult, ValidationResult,
    DetectionContext, ExplorationContext, ValidationContext,
    OptimizationContext, ExecutionContext,
};

pub use component_factory::ArbitrageComponentFactory;
pub use unified_arbitrage_manager::{UnifiedArbitrageManager, ManagerConfig, ArbitrageResult};

// Event system re-exports
pub use event_system::{
    MEVEvent, MEVAction, ExecutionPriority, EventPriority,
    ExecutionStep, EventRouter,
};

// Gas strategy re-exports
pub use gas_strategy::{
    GasStrategy, GasPrice, NetworkConditions,
    GasEstimator, GasOptimizer,
};

// Z3 optimizer re-exports
pub use z3_optimizer::{
    Z3ArbitrageOptimizer, Z3OptimizerConfig, Z3OptimizerStats,
    SimpleOptimizer,
};

// Artemis core type aliases
pub use artemis_core::types::{Strategy, Collector, Executor as ArtemisExecutor};

/// MEV Arbitrage Event type
pub type MEVArbitrageEvent = mev_arbitrage_graph::AnalysisEvent;

/// MEV Arbitrage Action type
pub type MEVArbitrageAction = mev_arbitrage_graph::AnalysisAction;

/// Configuration for the complete MEV arbitrage system
#[derive(Debug, Clone)]
pub struct MEVArbitrageConfig {
    /// Graph-based detection config
    pub graph_config: NegativeCycleConfig,
    /// Defense config
    pub defense_config: DefenseConfig,
    /// Manager config for unified architecture
    pub manager_config: ManagerConfig,
}

impl Default for MEVArbitrageConfig {
    fn default() -> Self {
        Self {
            graph_config: NegativeCycleConfig::default(),
            defense_config: DefenseConfig::default(),
            manager_config: ManagerConfig::default(),
        }
    }
}