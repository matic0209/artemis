//! Core Abstractions for MEV Arbitrage System
//!
//! This module provides high-level abstractions that unify different arbitrage detection
//! and execution strategies, making it easy to compose, extend, and integrate various
//! components without tight coupling.

use alloy_primitives::{Address, U256, Bytes};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

/// Core arbitrage opportunity abstraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// Unique identifier
    pub id: String,
    /// Opportunity type and details
    pub opportunity_type: OpportunityType,
    /// Expected profit in wei
    pub expected_profit: U256,
    /// Estimated gas cost
    pub gas_cost: U256,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Execution deadline (not serialized - use timestamp instead if needed)
    #[serde(skip)]
    pub deadline: Option<Instant>,
    /// Required capital
    pub required_capital: U256,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Detection result with standardized format
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub opportunities: Vec<ArbitrageOpportunity>,
    pub detection_time: Duration,
    pub detector_id: String,
    pub confidence_threshold: f64,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Execution path abstraction
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub id: String,
    pub opportunity_id: String,
    pub steps: Vec<ExecutionStep>,
    pub estimated_gas: U256,
    pub estimated_profit: U256,
    pub execution_strategy: ExecutionStrategy,
    pub validation_results: Option<ValidationResult>,
}

/// Individual execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_type: StepType,
    pub contract_address: Address,
    pub call_data: Bytes,
    pub value: U256,
    pub gas_limit: u64,
    pub dependencies: Vec<String>, // IDs of previous steps this depends on
}

/// Types of execution steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepType {
    TokenSwap { token_in: Address, token_out: Address, amount: U256 },
    FlashLoan { asset: Address, amount: U256 },
    FlashLoanRepay { asset: Address, amount: U256 },
    TokenTransfer { token: Address, to: Address, amount: U256 },
    ContractCall { function_sig: String, params: serde_json::Value },
    StateCheck { condition: String },
}

/// Execution strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    Immediate,
    Delayed { delay: Duration },
    Conditional { condition: String },
    Bundle { bundle_strategy: BundleStrategy },
}

/// Bundle execution strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BundleStrategy {
    SingleBundle,
    MultipleBundles { count: usize },
    PrivateMempool,
}

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub confidence: f64,
    pub validation_types: Vec<ValidationType>,
    pub issues: Vec<ValidationIssue>,
    pub recommendations: Vec<String>,
}

/// Types of validation performed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationType {
    Symbolic,
    Concrete,
    Economic,
    Risk,
    Gas,
    Timing,
}

/// Validation issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub issue_type: String,
    pub description: String,
    pub suggested_fix: Option<String>,
}

/// Issue severity levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// Risk levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

// ============================================================================
// Core Traits for Abstraction
// ============================================================================

/// Arbitrage detector trait - unified interface for all detection methods
#[async_trait]
pub trait ArbitrageDetector: Send + Sync {
    /// Detect arbitrage opportunities
    async fn detect(&mut self, context: &DetectionContext) -> Result<DetectionResult>;

    /// Get detector configuration
    fn config(&self) -> &DetectorConfig;

    /// Update detector configuration
    fn update_config(&mut self, config: DetectorConfig) -> Result<()>;

    /// Get detector metadata
    fn metadata(&self) -> DetectorMetadata;

    /// Health check
    async fn health_check(&self) -> Result<HealthStatus>;
}

/// Path explorer trait - unified interface for execution path generation
#[async_trait]
pub trait PathExplorer: Send + Sync {
    /// Generate execution paths for an opportunity
    async fn explore_paths(&mut self, opportunity: &ArbitrageOpportunity, context: &ExplorationContext) -> Result<Vec<ExecutionPlan>>;

    /// Optimize existing execution plan
    async fn optimize_plan(&mut self, plan: &ExecutionPlan) -> Result<ExecutionPlan>;

    /// Get explorer configuration
    fn config(&self) -> &ExplorerConfig;
}

/// Validator trait - unified interface for validation
#[async_trait]
pub trait Validator: Send + Sync {
    /// Validate an execution plan
    async fn validate(&mut self, plan: &ExecutionPlan, context: &ValidationContext) -> Result<ValidationResult>;

    /// Get supported validation types
    fn supported_types(&self) -> Vec<ValidationType>;

    /// Get validator configuration
    fn config(&self) -> &ValidatorConfig;
}

/// Optimizer trait - unified interface for optimization
#[async_trait]
pub trait Optimizer: Send + Sync {
    /// Optimize execution parameters
    async fn optimize(&mut self, plan: &ExecutionPlan, context: &OptimizationContext) -> Result<OptimizationResult>;

    /// Get optimization capabilities
    fn capabilities(&self) -> OptimizerCapabilities;
}

/// Executor trait - unified interface for execution
#[async_trait]
pub trait Executor: Send + Sync {
    /// Execute an arbitrage plan
    async fn execute(&mut self, plan: &ExecutionPlan, context: &ExecutionContext) -> Result<ExecutionResult>;

    /// Estimate execution cost
    async fn estimate_cost(&self, plan: &ExecutionPlan) -> Result<CostEstimate>;

    /// Check execution feasibility
    async fn check_feasibility(&self, plan: &ExecutionPlan) -> Result<FeasibilityResult>;
}

// ============================================================================
// Context Types
// ============================================================================

/// Detection context
#[derive(Debug, Clone)]
pub struct DetectionContext {
    pub block_number: u64,
    pub timestamp: u64,
    pub gas_price: U256,
    pub state_snapshot: StateSnapshot,
    pub market_data: MarketData,
    pub detection_params: DetectionParams,
}

/// Exploration context
#[derive(Debug, Clone)]
pub struct ExplorationContext {
    pub max_gas_limit: u64,
    pub max_steps: usize,
    pub timeout: Duration,
    pub user_address: Address,
    pub available_capital: U256,
}

/// Validation context
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub validation_types: Vec<ValidationType>,
    pub simulation_params: SimulationParams,
    pub risk_tolerance: RiskLevel,
    // MEV优化：关键参数
    pub gas_price: U256,
    pub min_profit: U256,
    pub slippage_tolerance: f64,
}

/// Optimization context
#[derive(Debug, Clone)]
pub struct OptimizationContext {
    pub optimization_target: OptimizationTarget,
    pub constraints: Vec<OptimizationConstraint>,
    pub timeout: Duration,
}

/// Execution context
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub dry_run: bool,
    pub gas_strategy: GasStrategy,
    pub slippage_tolerance: f64,
    pub deadline: Option<Duration>,
    pub private_pool: bool,
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Detector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorConfig {
    pub enabled: bool,
    pub confidence_threshold: f64,
    pub max_opportunities: usize,
    pub timeout: Duration,
    pub detector_specific: serde_json::Value,
}

/// Explorer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorerConfig {
    pub max_depth: usize,
    pub max_paths: usize,
    pub enable_optimization: bool,
    pub timeout: Duration,
}

/// Validator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    pub enabled_types: Vec<ValidationType>,
    pub strict_mode: bool,
    pub timeout: Duration,
}

// ============================================================================
// Result Types
// ============================================================================

/// Optimization result
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub optimized_plan: ExecutionPlan,
    pub improvement_metrics: ImprovementMetrics,
    pub optimization_time: Duration,
}

/// Execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub transaction_hashes: Vec<String>,
    pub actual_profit: U256,
    pub actual_gas_used: u64,
    pub execution_time: Duration,
    pub error_message: Option<String>,
}

/// Cost estimate
#[derive(Debug, Clone)]
pub struct CostEstimate {
    pub gas_estimate: u64,
    pub gas_price_estimate: U256,
    pub total_cost_wei: U256,
    pub confidence: f64,
}

/// Feasibility result
#[derive(Debug, Clone)]
pub struct FeasibilityResult {
    pub is_feasible: bool,
    pub feasibility_score: f64,
    pub limiting_factors: Vec<String>,
    pub recommendations: Vec<String>,
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Detector metadata
#[derive(Debug, Clone)]
pub struct DetectorMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub supported_opportunity_types: Vec<String>,
    pub performance_metrics: PerformanceMetrics,
}

/// Health status
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub status_message: String,
    pub last_check: Instant,
    pub metrics: HashMap<String, f64>,
}

/// Market data
#[derive(Debug, Clone)]
pub struct MarketData {
    pub token_prices: HashMap<Address, U256>,
    pub pool_reserves: HashMap<Address, PoolReserves>,
    pub gas_price_history: Vec<GasPricePoint>,
    pub volume_data: HashMap<Address, VolumeData>,
}

impl Default for MarketData {
    fn default() -> Self {
        Self {
            token_prices: HashMap::new(),
            pool_reserves: HashMap::new(),
            gas_price_history: Vec::new(),
            volume_data: HashMap::new(),
        }
    }
}

/// Pool reserves
#[derive(Debug, Clone)]
pub struct PoolReserves {
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub last_update: u64,
}

/// Gas price point
#[derive(Debug, Clone)]
pub struct GasPricePoint {
    pub timestamp: u64,
    pub gas_price: U256,
    pub block_number: u64,
}

/// Volume data
#[derive(Debug, Clone)]
pub struct VolumeData {
    pub volume_24h: U256,
    pub trades_24h: u64,
    pub last_trade: u64,
}

/// Detection parameters
#[derive(Debug, Clone)]
pub struct DetectionParams {
    pub min_profit_wei: U256,
    pub max_gas_cost: U256,
    pub min_confidence: f64,
    pub enable_flash_loans: bool,
    pub target_tokens: Vec<Address>,
}

impl Default for DetectionParams {
    fn default() -> Self {
        Self {
            min_profit_wei: U256::from(100_000_000_000_000_000u64), // 0.1 ETH
            max_gas_cost: U256::from(10_000_000_000_000_000u64),     // 0.01 ETH
            min_confidence: 0.7,
            enable_flash_loans: true,
            target_tokens: Vec::new(),
        }
    }
}

/// Simulation parameters
#[derive(Debug, Clone)]
pub struct SimulationParams {
    pub fork_block: Option<u64>,
    pub simulation_timeout: Duration,
    pub enable_state_override: bool,
    pub gas_limit: u64,
}

/// Optimization target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationTarget {
    MaximizeProfit,
    MinimizeGas,
    MinimizeRisk,
    MaximizeSpeed,
    Balanced { weights: OptimizationWeights },
}

/// Optimization weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationWeights {
    pub profit: f64,
    pub gas: f64,
    pub risk: f64,
    pub speed: f64,
}

/// Optimization constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationConstraint {
    MaxGasCost(U256),
    MaxSlippage(f64),
    MaxRisk(RiskLevel),
    MinProfit(U256),
    MaxExecutionTime(Duration),
}

/// Gas strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GasStrategy {
    Fast,
    Standard,
    Slow,
    Custom { gas_price: U256 },
    Dynamic { adjustment_factor: f64 },
}

/// Improvement metrics
#[derive(Debug, Clone)]
pub struct ImprovementMetrics {
    pub profit_improvement: f64,
    pub gas_improvement: f64,
    pub risk_improvement: f64,
    pub overall_score: f64,
}

/// Performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub avg_detection_time: Duration,
    pub success_rate: f64,
    pub total_detections: u64,
    pub avg_profit_accuracy: f64,
}

/// Optimizer capabilities
#[derive(Debug, Clone)]
pub struct OptimizerCapabilities {
    pub supported_targets: Vec<OptimizationTarget>,
    pub supported_constraints: Vec<String>,
    pub max_optimization_time: Duration,
    pub parallel_optimization: bool,
}

// ============================================================================
// State Snapshot (reuse from sub-packages)
// ============================================================================

pub use mev_arbitrage_graph::StateSnapshot;

// ============================================================================
// Opportunity Types (defined here since it's used across modules)
// ============================================================================

/// Types of arbitrage opportunities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpportunityType {
    /// Simple A -> B -> A arbitrage
    SimpleArbitrage {
        token_a: Address,
        token_b: Address,
        path: Vec<Address>,
    },
    /// Triangular A -> B -> C -> A arbitrage
    TriangularArbitrage {
        tokens: Vec<Address>,
        pools: Vec<Address>,
    },
    /// Flash loan arbitrage
    FlashLoanArbitrage {
        loan_asset: Address,
        loan_amount: U256,
        repay_amount: U256,
        arbitrage_path: Vec<Address>,
    },
    /// Cross-DEX arbitrage
    CrossDexArbitrage {
        dex_a: Address,
        dex_b: Address,
        token_pair: (Address, Address),
    },
    /// Sandwich attack opportunity
    Sandwich {
        victim_tx: String,
        front_run: Address,
        back_run: Address,
    },
    /// JIT liquidity provision
    JustInTime {
        pool: Address,
        target_tx: String,
        liquidity_amount: U256,
    },
    /// Cross-protocol arbitrage (e.g., Uniswap V2 vs V3)
    CrossProtocol {
        protocol_a: String,
        protocol_b: String,
        token_path: Vec<Address>,
    },
    /// Lending protocol liquidation (Aave, Compound, etc.)
    Liquidation {
        protocol: String,
        user: Address,
        collateral_token: Address,
        debt_token: Address,
        debt_to_repay: U256,
        collateral_to_seize: U256,
    },
}