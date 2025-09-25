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
