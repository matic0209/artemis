//! DeFi Analyzer Strategy for Artemis
//! 
//! This strategy integrates DeFiAligner's symbolic execution capabilities
//! with Artemis's MEV framework to analyze DeFi protocols for inconsistencies
//! and potential arbitrage opportunities.

pub mod strategy;
pub mod analyzer;
pub mod types;
pub mod config;

// Re-export main components
pub use strategy::DeFiAnalyzerStrategy;
pub use analyzer::DeFiAnalyzer;
pub use types::{AnalysisEvent, AnalysisAction, AnalysisResult};
pub use config::AnalyzerConfig;
