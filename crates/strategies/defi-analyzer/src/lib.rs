//! DeFi Analyzer Strategy for Artemis
//!
//! This strategy provides comprehensive DeFi protocol analysis capabilities
//! including symbolic execution, arbitrage detection, and MEV opportunity
//! identification. It integrates with Artemis's MEV framework to deliver
//! production-ready DeFi analysis and arbitrage execution.

#[cfg(feature = "full")]
pub mod strategy;
#[cfg(feature = "full")]
pub mod analyzer;
#[cfg(feature = "full")]
pub mod types;
#[cfg(feature = "full")]
pub mod config;
#[cfg(feature = "full")]
pub mod error;
#[cfg(feature = "full")]
pub mod arbitrage_detector;
#[cfg(feature = "full")]
pub mod observability;
#[cfg(feature = "full")]
pub mod evm_interpreter;
#[cfg(feature = "full")]
pub mod abi_parser;
#[cfg(feature = "full")]
pub mod defi_feature_extractor;
#[cfg(feature = "full")]
pub mod path_explorer;
#[cfg(feature = "full")]
pub mod collectors;
#[cfg(feature = "full")]
pub mod executors;
#[cfg(feature = "full")]
pub mod negative_cycle_arbitrage;
#[cfg(feature = "full")]
pub mod jit_strategy_discovery;
#[cfg(feature = "full")]
pub mod integration_examples;
#[cfg(feature = "full")]
pub mod production_config;
#[cfg(feature = "full")]
pub mod production_error_handler;
#[cfg(feature = "full")]
pub mod production_monitoring;
#[cfg(feature = "full")]
pub mod production_security;
#[cfg(feature = "full")]
pub mod production_load_testing;
#[cfg(feature = "full")]
pub mod mev_arbitrage_bot;
#[cfg(feature = "full")]
pub mod mev_arbitrage_engine;
#[cfg(feature = "full")]
pub mod technical_integration_demo;
#[cfg(feature = "full")]
pub mod mev_defense_strategies;
#[cfg(feature = "full")]
pub mod artemis_integration_bridge;
#[cfg(feature = "full")]
pub mod artemis_mev_strategy;
#[cfg(feature = "full")]
pub mod tests;

#[cfg(feature = "full")]
pub use strategy::DeFiAnalyzerStrategy;
#[cfg(feature = "full")]
pub use analyzer::DeFiAnalyzer;
#[cfg(feature = "full")]
pub use types::{AnalysisEvent, AnalysisAction, AnalysisResult, EventType};
#[cfg(feature = "full")]
pub use config::AnalyzerConfig;
#[cfg(feature = "full")]
pub use error::{DeFiAnalyzerError, DeFiResult};
#[cfg(feature = "full")]
pub use arbitrage_detector::{ArbitrageDetector, ArbitrageAlgorithm};
#[cfg(feature = "full")]
pub use observability::{ObservabilityManager, ObservabilityConfig};
#[cfg(feature = "full")]
pub use evm_interpreter::{SEVM, SymbolicEVMInterpreter, EVMExecutionState, ExecutionPath, ExecutionPathList};
#[cfg(feature = "full")]
pub use abi_parser::{ABIParser, ABIElement};
#[cfg(feature = "full")]
pub use defi_feature_extractor::{DeFiFeatureExtractor, DeFiFeature};
#[cfg(feature = "full")]
pub use path_explorer::{PathExplorer, PathExplorerConfig, PathExplorerStats};
#[cfg(feature = "full")]
pub use collectors::{DeFiBlockCollector, DeFiLogCollector, DeFiMempoolCollector, DeFiCollectorConfig};
#[cfg(feature = "full")]
pub use executors::{DeFiMempoolExecutor, DeFiFlashbotsExecutor, DeFiExecutorConfig, DeFiExecutorFactory};
#[cfg(feature = "full")]
pub use negative_cycle_arbitrage::{NegativeCycleArbitrageEngine, NegativeCycleConfig, StateSnapshot, ArbitrageCycle, ArbitragePath};
#[cfg(feature = "full")]
pub use jit_strategy_discovery::{JITStrategyDiscoveryEngine, JITConfig, StrategyCandidate, StrategyType, DeFiAction};
#[cfg(feature = "full")]
pub use mev_arbitrage_bot::{MEVArbitrageBot, MEVEvent, BlockEvent, TransactionEvent, PriceEvent};
#[cfg(feature = "full")]
pub use mev_arbitrage_engine::{MEVArbitrageEngine, SymbolicStrategyDiscoverer, ConcreteExecutionValidator, ContractBehavior};
#[cfg(feature = "full")]
pub use artemis_mev_strategy::{CompleteMEVStrategy, CompleteMEVCollector, CompleteMEVExecutor, setup_complete_artemis_mev};

#[cfg(not(feature = "full"))]
mod stub;
#[cfg(not(feature = "full"))]
pub use stub::*;
