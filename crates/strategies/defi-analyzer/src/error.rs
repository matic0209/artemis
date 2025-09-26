//! DeFi Analyzer Error Handling

use thiserror::Error;

/// DeFi Analyzer specific errors
#[derive(Error, Debug)]
pub enum DeFiAnalyzerError {
    /// Analysis timeout
    #[error("Analysis timeout after {timeout_ms}ms")]
    AnalysisTimeout { timeout_ms: u64 },

    /// Symbolic execution failed
    #[error("Symbolic execution failed: {reason}")]
    SymbolicExecutionFailed { reason: String },

    /// Feature extraction failed
    #[error("Feature extraction failed: {reason}")]
    FeatureExtractionFailed { reason: String },

    /// Documentation comparison failed
    #[error("Documentation comparison failed: {reason}")]
    DocumentationComparisonFailed { reason: String },

    /// Risk assessment failed
    #[error("Risk assessment failed: {reason}")]
    RiskAssessmentFailed { reason: String },

    /// Cache operation failed
    #[error("Cache operation failed: {reason}")]
    CacheOperationFailed { reason: String },

    /// Configuration error
    #[error("Configuration error: {reason}")]
    ConfigurationError { reason: String },

    /// Network error
    #[error("Network error: {reason}")]
    NetworkError { reason: String },

    /// Insufficient resources
    #[error("Insufficient resources: {resource_type}")]
    InsufficientResources { resource_type: String },

    /// Analysis depth exceeded
    #[error("Analysis depth {depth} exceeded maximum {max_depth}")]
    AnalysisDepthExceeded { depth: u32, max_depth: u32 },

    /// Invalid input
    #[error("Invalid input: {reason}")]
    InvalidInput { reason: String },

    /// Computation error
    #[error("Computation error: {reason}")]
    ComputationError { reason: String },

    /// ABI fetch error
    #[error("ABI fetch error: {reason}")]
    ABIFetchError { reason: String },

    /// Circuit breaker open
    #[error("Circuit breaker open: {reason}")]
    CircuitBreakerOpen { reason: String },

    /// Execution error
    #[error("Execution error: {reason}")]
    ExecutionError { reason: String },

    /// Z3 error
    #[error("Z3 error: {reason}")]
    Z3Error { reason: String },

    /// Contract call error
    #[error("Contract call error: {reason}")]
    ContractCallError { reason: String },
}

/// Result type for DeFi Analyzer operations
pub type DeFiResult<T> = Result<T, DeFiAnalyzerError>;

/// Error recovery strategies
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Retry with exponential backoff
    RetryWithBackoff { max_retries: u32, base_delay_ms: u64 },
    /// Use cached result
    UseCachedResult,
    /// Skip analysis
    SkipAnalysis,
    /// Reduce analysis depth
    ReduceAnalysisDepth,
    /// Use simplified analysis
    UseSimplifiedAnalysis,
}

impl DeFiAnalyzerError {
    /// Get recovery strategy for this error
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            DeFiAnalyzerError::AnalysisTimeout { .. } => {
                RecoveryStrategy::RetryWithBackoff { max_retries: 3, base_delay_ms: 100 }
            },
            DeFiAnalyzerError::SymbolicExecutionFailed { .. } => {
                RecoveryStrategy::UseSimplifiedAnalysis
            },
            DeFiAnalyzerError::FeatureExtractionFailed { .. } => {
                RecoveryStrategy::UseCachedResult
            },
            DeFiAnalyzerError::DocumentationComparisonFailed { .. } => {
                RecoveryStrategy::SkipAnalysis
            },
            DeFiAnalyzerError::RiskAssessmentFailed { .. } => {
                RecoveryStrategy::UseCachedResult
            },
            DeFiAnalyzerError::CacheOperationFailed { .. } => {
                RecoveryStrategy::SkipAnalysis
            },
            DeFiAnalyzerError::ConfigurationError { .. } => {
                RecoveryStrategy::SkipAnalysis
            },
            DeFiAnalyzerError::NetworkError { .. } => {
                RecoveryStrategy::RetryWithBackoff { max_retries: 5, base_delay_ms: 200 }
            },
            DeFiAnalyzerError::InsufficientResources { .. } => {
                RecoveryStrategy::ReduceAnalysisDepth
            },
            DeFiAnalyzerError::AnalysisDepthExceeded { .. } => {
                RecoveryStrategy::ReduceAnalysisDepth
            },
        }
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            DeFiAnalyzerError::AnalysisTimeout { .. } => true,
            DeFiAnalyzerError::SymbolicExecutionFailed { .. } => true,
            DeFiAnalyzerError::FeatureExtractionFailed { .. } => true,
            DeFiAnalyzerError::DocumentationComparisonFailed { .. } => true,
            DeFiAnalyzerError::RiskAssessmentFailed { .. } => true,
            DeFiAnalyzerError::CacheOperationFailed { .. } => true,
            DeFiAnalyzerError::NetworkError { .. } => true,
            DeFiAnalyzerError::InsufficientResources { .. } => true,
            DeFiAnalyzerError::AnalysisDepthExceeded { .. } => true,
            DeFiAnalyzerError::InvalidInput { .. } => false,
            DeFiAnalyzerError::ComputationError { .. } => true,
            DeFiAnalyzerError::ABIFetchError { .. } => true,
            DeFiAnalyzerError::CircuitBreakerOpen { .. } => true,
            DeFiAnalyzerError::ExecutionError { .. } => true,
            DeFiAnalyzerError::Z3Error { .. } => true,
            DeFiAnalyzerError::ContractCallError { .. } => true,
            DeFiAnalyzerError::ConfigurationError { .. } => false,
        }
    }
}
