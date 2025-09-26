//! DeFi Analyzer Types

use alloy_primitives::{Address, U256};
use std::collections::HashMap;

/// Analysis event
#[derive(Debug, Clone)]
pub struct AnalysisEvent {
    /// Event type
    pub event_type: EventType,
    /// Contract address being analyzed
    pub contract_address: Address,
    /// Transaction data
    pub tx_data: Option<Vec<u8>>,
    /// Transaction data (for compatibility)
    pub transaction_data: Option<Vec<u8>>,
    /// Transaction hash
    pub transaction_hash: [u8; 32],
    /// Event data
    pub event_data: Vec<u8>,
    /// Event kind
    pub event_kind: String,
    /// Block number
    pub block_number: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Event types
#[derive(Debug, Clone)]
pub enum EventType {
    /// Mempool transaction
    MempoolTransaction,
    /// Block with DeFi activity
    BlockWithDeFiActivity,
    /// Contract deployment
    ContractDeployment,
    /// MEV-Share event
    MevShareEvent,
    /// Custom analysis
    CustomAnalysis,
}

/// Analysis action
#[derive(Debug, Clone)]
pub struct AnalysisAction {
    /// Action type
    pub action_type: ActionType,
    /// Target contract
    pub contract_address: Address,
    /// Target address (for compatibility)
    pub target_address: Address,
    /// Analysis parameters
    pub parameters: AnalysisParameters,
    /// Priority (0-100)
    pub priority: u8,
    /// Action ID (for compatibility)
    pub action_id: String,
    /// Calldata
    pub calldata: Vec<u8>,
    /// Value
    pub value: U256,
    /// Gas limit
    pub gas_limit: U256,
    /// Gas price
    pub gas_price: U256,
    /// Nonce
    pub nonce: u64,
    /// Chain ID
    pub chain_id: u64,
    /// Target block
    pub target_block: u64,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Expected profit
    pub expected_profit: U256,
    /// Min timestamp
    pub min_timestamp: u64,
    /// Max timestamp
    pub max_timestamp: u64,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Risk level
#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Action types
#[derive(Debug, Clone)]
pub enum ActionType {
    /// Symbolic execution
    SymbolicExecution,
    /// Feature extraction
    FeatureExtraction,
    /// Documentation comparison
    DocumentationComparison,
    /// Generate report
    GenerateReport,
    /// Arbitrage execution
    ArbitrageExecution,
    /// Liquidity provision
    LiquidityProvision,
    /// Risk management
    RiskManagement,
    /// Monitoring
    Monitoring,
}

/// Analysis parameters
#[derive(Debug, Clone)]
pub struct AnalysisParameters {
    /// ABI JSON
    pub abi_json: Option<String>,
    /// Function name
    pub function_name: Option<String>,
    /// Analysis depth
    pub depth: u32,
    /// Timeout in seconds
    pub timeout_seconds: u64,
    /// Additional configuration
    pub config: HashMap<String, String>,
}

/// Analysis result
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Analysis ID
    pub analysis_id: String,
    /// Contract address
    pub contract_address: Address,
    /// Analysis status
    pub status: AnalysisStatus,
    /// Analysis results
    pub results: AnalysisResults,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Timestamp
    pub timestamp: u64,
}

/// Analysis status
#[derive(Debug, Clone)]
pub enum AnalysisStatus {
    /// Pending
    Pending,
    /// In progress
    InProgress,
    /// Completed
    Completed,
    /// Failed
    Failed,
}

/// Analysis results
#[derive(Debug, Clone)]
pub struct AnalysisResults {
    /// DeFi features
    pub defi_features: DeFiFeatures,
    /// Inconsistencies
    pub inconsistencies: Vec<Inconsistency>,
    /// Arbitrage opportunities
    pub arbitrage_opportunities: Vec<ArbitrageOpportunity>,
    /// Risk assessment
    pub risk_assessment: RiskAssessment,
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// DeFi features
#[derive(Debug, Clone)]
pub struct DeFiFeatures {
    /// Balance changes
    pub balance_changes: u32,
    /// Conditional constraints
    pub conditional_constraints: u32,
    /// ETH transfers
    pub eth_transfers: u32,
    /// Token operations
    pub token_operations: u32,
    /// Liquidity operations
    pub liquidity_operations: u32,
}

/// Inconsistency
#[derive(Debug, Clone)]
pub struct Inconsistency {
    /// Inconsistency type
    pub inconsistency_type: InconsistencyType,
    /// Description
    pub description: String,
    /// Severity level
    pub severity: SeverityLevel,
    /// Location
    pub location: Option<String>,
    /// Suggested fix
    pub suggested_fix: Option<String>,
}

/// Inconsistency types
#[derive(Debug, Clone)]
pub enum InconsistencyType {
    /// Gas optimization issue
    GasOptimizationIssue,
    /// Logic inconsistency
    LogicInconsistency,
    /// State inconsistency
    StateInconsistency,
    /// Documentation mismatch
    DocumentationMismatch,
}

/// Severity levels
#[derive(Debug, Clone, PartialEq, PartialOrd)]
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
#[derive(Debug, Clone)]
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


/// Risk assessment
#[derive(Debug, Clone)]
pub struct RiskAssessment {
    /// Overall risk score (0-100)
    pub overall_risk_score: u8,
    /// Risk factors
    pub risk_factors: Vec<RiskFactor>,
    /// Mitigation strategies
    pub mitigation_strategies: Vec<String>,
}

/// Risk factor
#[derive(Debug, Clone)]
pub struct RiskFactor {
    /// Factor name
    pub factor_name: String,
    /// Risk score (0-100)
    pub risk_score: u8,
    /// Description
    pub description: String,
}