//! DeFi Analyzer Strategy for Artemis
//! 
//! This strategy integrates DeFiAligner's symbolic execution capabilities
//! with Artemis's MEV framework to analyze DeFi protocols for inconsistencies
//! and potential arbitrage opportunities.

pub mod strategy;
pub mod analyzer;
pub mod types;
pub mod config;
pub mod error;
pub mod arbitrage_detector;
pub mod observability;
pub mod evm_interpreter;
pub mod abi_parser;
pub mod defi_feature_extractor;
pub mod path_explorer;
pub mod collectors;
pub mod executors;
pub mod negative_cycle_arbitrage;
pub mod jit_strategy_discovery;
pub mod integration_examples;
pub mod production_config;
pub mod production_error_handler;
pub mod production_monitoring;
pub mod production_security;
pub mod production_load_testing;
pub mod mev_arbitrage_bot;
pub mod mev_arbitrage_engine;
pub mod technical_integration_demo;
pub mod mev_defense_strategies;
pub mod artemis_integration_bridge;
pub mod tests;

// Re-export main components
pub use strategy::DeFiAnalyzerStrategy;
pub use analyzer::DeFiAnalyzer;
pub use types::{AnalysisEvent, AnalysisAction, AnalysisResult};
pub use config::AnalyzerConfig;
pub use error::{DeFiAnalyzerError, DeFiResult};
pub use arbitrage_detector::{ArbitrageDetector, ArbitrageAlgorithm};
pub use observability::{ObservabilityManager, ObservabilityConfig};
pub use evm_interpreter::{SEVM, SymbolicEVMInterpreter, EVMExecutionState, ExecutionPath, ExecutionPathList};
pub use abi_parser::{ABIParser, ABIElement};
pub use defi_feature_extractor::{DeFiFeatureExtractor, DeFiFeature};
pub use path_explorer::{PathExplorer, PathExplorerConfig, PathExplorerStats};
pub use collectors::{DeFiBlockCollector, DeFiLogCollector, DeFiMempoolCollector, DeFiCollectorConfig};
pub use executors::{DeFiMempoolExecutor, DeFiFlashbotsExecutor, DeFiExecutorConfig, DeFiExecutorFactory};
pub use negative_cycle_arbitrage::{NegativeCycleArbitrageEngine, NegativeCycleConfig, StateSnapshot, ArbitrageCycle, ArbitragePath};
pub use jit_strategy_discovery::{JITStrategyDiscoveryEngine, JITConfig, StrategyCandidate, StrategyType, DeFiAction};
pub use mev_arbitrage_bot::{MEVArbitrageBot, MEVEvent, BlockEvent, TransactionEvent, PriceEvent};
pub use mev_arbitrage_engine::{MEVArbitrageEngine, SymbolicStrategyDiscoverer, ConcreteExecutionValidator, ContractBehavior};
