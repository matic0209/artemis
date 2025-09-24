//! Type definitions for DeFi Analyzer Strategy

use std::collections::HashMap;
use alloy_primitives::{Address, U256, Bytes};
use serde::{Deserialize, Serialize};
use defi_aligner_rs::Result as DeFiResult;

/// Analysis event from Artemis collectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisEvent {
    /// Event type
    pub event_type: EventType,
    /// Contract address being analyzed
    pub contract_address: Address,
    /// Transaction data
    pub tx_data: Option<Bytes>,
    /// Block number
    pub block_number: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Types of events that can trigger analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    /// New transaction in mempool
    MempoolTransaction,
    /// New block with DeFi activity
    BlockWithDeFiActivity,
    /// Contract deployment
    ContractDeployment,
    /// MEV-Share event
    MevShareEvent,
    /// Custom analysis trigger
    CustomAnalysis,
}

/// Analysis action to be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisAction {
    /// Action type
    pub action_type: ActionType,
    /// Target contract
    pub contract_address: Address,
    /// Analysis parameters
    pub parameters: AnalysisParameters,
    /// Priority (higher = more urgent)
    pub priority: u8,
}

/// Types of analysis actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    /// Run symbolic execution analysis
    SymbolicExecution,
    /// Extract DeFi features
    FeatureExtraction,
    /// Compare with documentation
    DocumentationComparison,
    /// Generate analysis report
    GenerateReport,
}

/// Analysis parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisParameters {
    /// ABI JSON string
    pub abi_json: Option<String>,
    /// Function name to analyze
    pub function_name: Option<String>,
    /// Analysis depth
    pub depth: u32,
    /// Timeout in seconds
    pub timeout_seconds: u64,
    /// Additional configuration
    pub config: HashMap<String, String>,
}

/// Analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Analysis ID
    pub analysis_id: String,
    /// Contract address
    pub contract_address: Address,
    /// Analysis status
    pub status: AnalysisStatus,
    /// Results
    pub results: AnalysisResults,
    /// Execution time (ms)
    pub execution_time_ms: u64,
    /// Timestamp
    pub timestamp: u64,
}

/// Analysis status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisStatus {
    /// Analysis completed successfully
    Completed,
    /// Analysis failed
    Failed,
    /// Analysis in progress
    InProgress,
    /// Analysis timed out
    Timeout,
}

/// Detailed analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResults {
    /// DeFi features found
    pub defi_features: DeFiFeatures,
    /// Inconsistencies detected
    pub inconsistencies: Vec<Inconsistency>,
    /// Potential arbitrage opportunities
    pub arbitrage_opportunities: Vec<ArbitrageOpportunity>,
    /// Risk assessment
    pub risk_assessment: RiskAssessment,
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// DeFi features extracted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeFiFeatures {
    /// Balance changes detected
    pub balance_changes: u32,
    /// Conditional constraints found
    pub conditional_constraints: u32,
    /// ETH transfers detected
    pub eth_transfers: u32,
    /// Token operations
    pub token_operations: u32,
    /// Liquidity operations
    pub liquidity_operations: u32,
}

/// Inconsistency found
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inconsistency {
    /// Inconsistency type
    pub inconsistency_type: InconsistencyType,
    /// Description
    pub description: String,
    /// Severity level
    pub severity: SeverityLevel,
    /// Location in code
    pub location: Option<String>,
    /// Suggested fix
    pub suggested_fix: Option<String>,
}

/// Types of inconsistencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InconsistencyType {
    /// Documentation mismatch
    DocumentationMismatch,
    /// Logic error
    LogicError,
    /// Security vulnerability
    SecurityVulnerability,
    /// Gas optimization issue
    GasOptimizationIssue,
    /// State inconsistency
    StateInconsistency,
}

/// Severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeverityLevel {
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical severity
    Critical,
}

/// Arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// Opportunity ID
    pub opportunity_id: String,
    /// Expected profit (wei)
    pub expected_profit: U256,
    /// Required gas
    pub required_gas: u64,
    /// Success probability
    pub success_probability: f64,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Strategy description
    pub strategy_description: String,
}

/// Risk levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Low risk
    Low,
    /// Medium risk
    Medium,
    /// High risk
    High,
}

/// Risk assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    /// Overall risk score (0-100)
    pub overall_risk_score: u8,
    /// Risk factors
    pub risk_factors: Vec<RiskFactor>,
    /// Mitigation strategies
    pub mitigation_strategies: Vec<String>,
}

/// Risk factor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    /// Factor name
    pub factor_name: String,
    /// Risk score (0-100)
    pub risk_score: u8,
    /// Description
    pub description: String,
}
