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
pub mod metrics;
pub mod arbitrage_detector;
pub mod symbolic_execution;
pub mod security;
pub mod observability;
pub mod evm_interpreter_complete;
pub mod abi_parser_complete;
pub mod defi_feature_extractor_complete;
pub mod path_explorer_complete;

// Re-export main components
pub use strategy::DeFiAnalyzerStrategy;
pub use analyzer::DeFiAnalyzer;
pub use types::{AnalysisEvent, AnalysisAction, AnalysisResult};
pub use config::AnalyzerConfig;
pub use error::{DeFiAnalyzerError, DeFiResult};
pub use metrics::{MetricsCollector, MetricsSummary};
pub use arbitrage_detector::{ArbitrageDetector, ArbitrageAlgorithm};
pub use symbolic_execution::{SymbolicExecutionEngine, ExecutionResult};
pub use security::{SecurityManager, SecurityConfig};
pub use observability::{ObservabilityManager, ObservabilityConfig};
pub use evm_interpreter_complete::{SEVM, SymbolicEVMInterpreter, EVMExecutionState, ExecutionPath, ExecutionPathList};
pub use abi_parser_complete::{ABIParser, ABIElement};
pub use defi_feature_extractor_complete::{DeFiFeatureExtractor, DeFiFeature};
pub use path_explorer_complete::{PathExplorer, PathExplorerConfig, PathExplorerStats};
