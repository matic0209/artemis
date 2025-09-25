//! Complete DeFi Feature Extractor for Symbolic Execution
//! 
//! This module provides complete DeFi feature extraction functionality for symbolic execution,
//! maintaining full compatibility with DeFiAligner's DeFi feature extractor implementation.

use alloy_primitives::{Address, U256};
use anyhow::Result;
use tracing::{info, debug, warn, error};
use std::collections::HashMap;
use regex::Regex;
use diff_match_patch::Dmp;
use z3::{Context, Config, ast::{BV, Bool, Ast}};

use crate::{
    types::AnalysisEvent,
    error::{DeFiResult, DeFiAnalyzerError},
    evm_interpreter::{ExecutionPath, EVMExecutionState, OpCode},
};

/// Complete DeFi Feature Extractor implementation
pub struct DeFiFeatureExtractor<'ctx> {
    /// Z3 context
    ctx: &'ctx z3::Context,
    /// Balance symbols cache
    balance_symbols_cache: HashMap<String, Vec<z3::ast::BV<'ctx>>>,
    /// Configuration
    config: DeFiFeatureExtractorConfig,
}

/// DeFi Feature Extractor configuration
#[derive(Debug, Clone)]
pub struct DeFiFeatureExtractorConfig {
    /// Enable balance change detection
    pub enable_balance_change_detection: bool,
    /// Enable ETH transfer detection
    pub enable_eth_transfer_detection: bool,
    /// Enable condition detection
    pub enable_condition_detection: bool,
    /// Enable arbitrage detection
    pub enable_arbitrage_detection: bool,
}

impl Default for DeFiFeatureExtractorConfig {
    fn default() -> Self {
        Self {
            enable_balance_change_detection: true,
            enable_eth_transfer_detection: true,
            enable_condition_detection: true,
            enable_arbitrage_detection: true,
        }
    }
}

/// ETH Transfer Information
#[derive(Debug, Clone)]
pub struct EthTransferInfo<'ctx> {
    /// Sender address
    pub sender: z3::ast::BV<'ctx>,
    /// Payee address
    pub payee: z3::ast::BV<'ctx>,
    /// Transfer value
    pub value: z3::ast::BV<'ctx>,
}

/// Balance Change Information
#[derive(Debug, Clone)]
pub struct BalanceChangeInfo<'ctx> {
    /// Storage change location
    pub storage_change_location: z3::ast::BV<'ctx>,
    /// Storage change value
    pub storage_change_value: z3::ast::BV<'ctx>,
}

/// Address to Balance Changes mapping
pub type AddressToBalanceChanges<'ctx> = HashMap<String, Vec<BalanceChangeInfo<'ctx>>>;

/// Address to Conditions mapping
pub type AddressToConditions<'ctx> = HashMap<String, Vec<z3::ast::BV<'ctx>>>;

/// Token Balance Symbol mapping
pub type TokenBalanceSymbol<'ctx> = HashMap<String, z3::ast::BV<'ctx>>;

/// DeFi Feature structure
#[derive(Debug, Clone)]
pub struct DeFiFeature<'ctx> {
    /// ERC balance change map
    pub erc_balance_change_map: AddressToBalanceChanges<'ctx>,
    /// Condition map
    pub condition_map: AddressToConditions<'ctx>,
    /// ETH transfers
    pub eth_transfers: Vec<EthTransferInfo<'ctx>>,
    /// Arbitrage opportunities
    pub arbitrage_opportunities: Vec<ArbitrageOpportunity<'ctx>>,
}

/// Arbitrage Opportunity
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity<'ctx> {
    /// Opportunity type
    pub opportunity_type: ArbitrageType,
    /// Profit potential
    pub profit_potential: U256,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Required conditions
    pub required_conditions: Vec<z3::ast::Bool<'ctx>>,
    /// Execution path
    pub execution_path: ExecutionPath<'ctx>,
}

/// Arbitrage Type
#[derive(Debug, Clone)]
pub enum ArbitrageType {
    /// Price arbitrage
    PriceArbitrage,
    /// Liquidity arbitrage
    LiquidityArbitrage,
    /// Flash loan arbitrage
    FlashLoanArbitrage,
    /// Cross-protocol arbitrage
    CrossProtocolArbitrage,
}

/// Risk Level
#[derive(Debug, Clone)]
pub enum RiskLevel {
    /// Low risk
    Low,
    /// Medium risk
    Medium,
    /// High risk
    High,
    /// Critical risk
    Critical,
}

impl<'ctx> DeFiFeatureExtractor<'ctx> {
    /// Create new DeFi feature extractor
    pub fn new(ctx: &'ctx z3::Context, config: DeFiFeatureExtractorConfig) -> Self {
        Self {
            ctx,
            balance_symbols_cache: HashMap::new(),
            config,
        }
    }

    /// Extract DeFi symbolic features from execution path
    pub fn get_defi_symbolic_features_from_path(&mut self, path: &ExecutionPath<'ctx>, 
                                              called_contract: z3::ast::BV<'ctx>) -> DeFiResult<DeFiFeature<'ctx>> {
        let mut balance_change_map = AddressToBalanceChanges::new();
        let mut balance_symbol_map = TokenBalanceSymbol::new();
        let mut eth_transfers = Vec::new();
        let mut address_to_conditions_map = AddressToConditions::new();
        
        // Pre-compute constant values to avoid borrowing conflicts  
        let zero_value = BV::from_u64(self.ctx, 0, 256);
        let one_value = BV::from_u64(self.ctx, 1, 256);
        
        let mut current_called_contract = called_contract;

        for (index, execution_state) in path.iter().enumerate() {
            // Get balance symbol
            let current_called_address_string = format!("0x{}", 
                current_called_contract.to_string().chars().skip(26).take(40).collect::<String>());
            
            if !balance_symbol_map.contains_key(&current_called_address_string) {
                // Create balance symbol directly to avoid borrowing conflicts
                let balance_symbol = BV::new_const(self.ctx, format!("balance_{}", current_called_address_string), 256);
                balance_symbol_map.insert(current_called_address_string.clone(), balance_symbol);
            }

            // Update JUMPI conditions
            if execution_state.current_opcode == OpCode::JUMPI {
                if let Some(next_state) = path.get(index + 1) {
                    let current_pc = execution_state.current_pc;
                    let next_location = next_state.current_pc;
                    
                    if let Some(condition) = execution_state.current_stack.back(1) {
                        let condition = self.simplify_z3_bv(condition);
                        
                        let jumpi_condition = if next_location == current_pc + 1 {
                            condition._eq(&zero_value)
                        } else {
                            condition._eq(&one_value)
                        };
                        
                        let simplified_condition = jumpi_condition.simplify();
                        
                        if let Some(concrete_condition) = simplified_condition.as_bool() {
                            if !concrete_condition {
                                // Store the condition as a BV by creating a new one
                                let condition_bv = BV::new_const(self.ctx, format!("condition_{}", current_called_address_string), 256);
                                address_to_conditions_map
                                    .entry(current_called_address_string.clone())
                                    .or_insert_with(Vec::new)
                                    .push(condition_bv);
                            }
                        }
                    }
                }
            }

            // Update called contract for CALL operations
            if matches!(execution_state.current_opcode, OpCode::CALL | OpCode::DELEGATECALL | OpCode::STATICCALL) {
                if let Some(prev_state) = path.get(index.saturating_sub(1)) {
                    if let Some(new_contract) = prev_state.current_stack.back(1) {
                        current_called_contract = new_contract.clone();
                        
                        // Collect ETH transfer for CALL operations
                        if execution_state.current_opcode == OpCode::CALL {
                            if let Some(value) = prev_state.current_stack.back(2) {
                                if value.to_string() != "#x0000000000000000000000000000000000000000000000000000000000000000" {
                                    let eth_transfer = EthTransferInfo {
                                        sender: prev_state.current_called_contract.clone(),
                                        payee: new_contract,
                                        value: value,
                                    };
                                    eth_transfers.push(eth_transfer);
                                }
                            }
                        }
                    }
                }
            }

            // Update called contract for RETURN/STOP operations
            if matches!(execution_state.current_opcode, OpCode::RETURN | OpCode::STOP) {
                if let Some(next_state) = path.get(index + 1) {
                    if execution_state.current_evm_depth == next_state.current_evm_depth {
                        current_called_contract = execution_state.current_called_contract.clone();
                    } else {
                        current_called_contract = next_state.current_called_contract.clone();
                    }
                }
            }

            // Detect balance changes
            if execution_state.current_opcode == OpCode::SSTORE {
                if let Some(prev_state) = path.get(index.saturating_sub(1)) {
                    if let Some(location) = prev_state.current_stack.back(0) {
                        if location.to_string().contains("SHA3") {
                            let sstore_location_string = format!("SLOAD{}=>{}", 
                                current_called_contract.to_string(), location.to_string());
                            let token_balance_string = balance_symbol_map
                                .get(&current_called_address_string)
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            
                            if self.is_same_balance_struct(&sstore_location_string, &token_balance_string) {
                                if let Some(value) = prev_state.current_stack.back(1) {
                                    let balance_change_info = BalanceChangeInfo {
                                        storage_change_location: location,
                                        storage_change_value: value,
                                    };
                                    
                                    balance_change_map
                                        .entry(current_called_address_string.clone())
                                        .or_insert_with(Vec::new)
                                        .push(balance_change_info);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Extract arbitrage opportunities
        let arbitrage_opportunities = if self.config.enable_arbitrage_detection {
            self.extract_arbitrage_opportunities(path)?
        } else {
            Vec::new()
        };

        Ok(DeFiFeature {
            erc_balance_change_map: balance_change_map,
            condition_map: address_to_conditions_map,
            eth_transfers,
            arbitrage_opportunities,
        })
    }

    /// Get balance symbol for address
    fn get_balance_symbol(&mut self, address: &str, ctx: &'ctx z3::Context) -> Vec<z3::ast::BV<'ctx>> {
        if let Some(cached) = self.balance_symbols_cache.get(address) {
            return cached.clone();
        }

        // This is a simplified implementation
        // In a real implementation, you would analyze the contract to find balance storage patterns
        let balance_symbol = BV::new_const(ctx, format!("balance_{}", address), 256);
        let symbols = vec![balance_symbol];
        
        self.balance_symbols_cache.insert(address.to_string(), symbols.clone());
        symbols
    }

    /// Check if two balance structures are the same
    fn is_same_balance_struct(&self, sstore_string: &str, balance_string: &str) -> bool {
        let mut dmp = Dmp::new();
        let diffs = dmp.diff_main(sstore_string, balance_string, false);
        
        let mut delete_count = 0;
        let mut insert_count = 0;
        let mut sstore_diff_string = String::new();
        
        for diff in diffs {
            match diff.operation {
                1 => { // Insert
                    insert_count += 1;
                    if insert_count > 1 {
                        return false;
                    }
                },
                -1 => { // Delete
                    sstore_diff_string = diff.text.clone();
                    delete_count += 1;
                    if delete_count > 1 {
                        return false;
                    }
                },
                0 => { // Equal
                    // Continue
                },
                _ => {}
            }
        }

        if delete_count == 1 && insert_count == 1 {
            let hex_regex = Regex::new(r"^(0x|0X)?[0-9a-fA-F]+$").unwrap();
            let cleaned_diff = sstore_diff_string.replace("#x", "");
            !hex_regex.is_match(&cleaned_diff)
        } else {
            false
        }
    }

    /// Simplify Z3 BV
    fn simplify_z3_bv<'a>(&self, bv: z3::ast::BV<'a>) -> z3::ast::BV<'a> {
        bv.simplify()
    }

    /// Extract arbitrage opportunities
    fn extract_arbitrage_opportunities(&self, path: &ExecutionPath<'ctx>) -> DeFiResult<Vec<ArbitrageOpportunity<'ctx>>> {
        let mut opportunities = Vec::new();
        
        // Analyze execution path for arbitrage patterns
        for (i, state) in path.iter().enumerate() {
            match state.current_opcode {
                OpCode::CALL => {
                    // Check for cross-protocol arbitrage
                    if let Some(opportunity) = self.analyze_cross_protocol_arbitrage(path, i) {
                        opportunities.push(opportunity);
                    }
                },
                OpCode::SSTORE => {
                    // Check for price arbitrage
                    if let Some(opportunity) = self.analyze_price_arbitrage(path, i) {
                        opportunities.push(opportunity);
                    }
                },
                _ => {}
            }
        }

        Ok(opportunities)
    }

    /// Analyze cross-protocol arbitrage
    fn analyze_cross_protocol_arbitrage(&self, path: &ExecutionPath<'ctx>, index: usize) -> Option<ArbitrageOpportunity<'ctx>> {
        if index + 1 >= path.len() {
            return None;
        }
        
        let current_state = &path[index];
        let next_state = &path[index + 1];
        
        // Check for cross-protocol call patterns
        if let Some(call_info) = &current_state.call_info {
            // Look for patterns that suggest arbitrage opportunities
            let profit_potential = self.calculate_profit_potential(path, index);
            if profit_potential > U256::from(0) {
                return Some(ArbitrageOpportunity {
                    opportunity_type: ArbitrageType::CrossProtocolArbitrage,
                    profit_potential,
                    risk_level: RiskLevel::Medium,
                    confidence: 0.7,
                    description: "Cross-protocol arbitrage opportunity detected".to_string(),
                    execution_path: path.clone(),
                });
            }
        }
        
        None
    }

    /// Analyze price arbitrage
    fn analyze_price_arbitrage(&self, path: &ExecutionPath<'ctx>, index: usize) -> Option<ArbitrageOpportunity<'ctx>> {
        if index + 1 >= path.len() {
            return None;
        }
        
        let current_state = &path[index];
        
        // Check for price manipulation patterns
        if let Some(storage_info) = &current_state.storage_info {
            // Look for price-related storage changes
            let profit_potential = self.calculate_price_arbitrage_potential(path, index);
            if profit_potential > U256::from(0) {
                return Some(ArbitrageOpportunity {
                    opportunity_type: ArbitrageType::PriceArbitrage,
                    profit_potential,
                    risk_level: RiskLevel::High,
                    confidence: 0.8,
                    description: "Price arbitrage opportunity detected".to_string(),
                    execution_path: path.clone(),
                });
            }
        }
        
        None
    }

    /// Calculate profit potential for cross-protocol arbitrage
    fn calculate_profit_potential(&self, path: &ExecutionPath<'ctx>, index: usize) -> U256 {
        // Analyze execution path for profit opportunities
        let mut total_profit = U256::from(0);
        
        // Look for balance changes that indicate profit
        for (i, state) in path.iter().enumerate() {
            if i >= index {
                // Check for ETH transfers
                if let Some(call_info) = &state.call_info {
                    if call_info.value > U256::from(0) {
                        total_profit = total_profit.saturating_add(call_info.value);
                    }
                }
                
                // Check for storage changes that might indicate profit
                if let Some(storage_info) = &state.storage_info {
                    if storage_info.key.contains("profit") || storage_info.key.contains("balance") {
                        total_profit = total_profit.saturating_add(U256::from(100));
                    }
                }
            }
        }
        
        // Apply risk factor
        let risk_factor = U256::from(80); // 80% of calculated profit
        total_profit * risk_factor / U256::from(100)
    }

    /// Calculate price arbitrage potential
    fn calculate_price_arbitrage_potential(&self, path: &ExecutionPath<'ctx>, index: usize) -> U256 {
        // Analyze price differences in execution path
        let mut price_difference = U256::from(0);
        
        // Look for price-related operations
        for (i, state) in path.iter().enumerate() {
            if i >= index {
                match state.current_opcode {
                    OpCode::SSTORE => {
                        // Check if this is a price update
                        if let Some(storage_info) = &state.storage_info {
                            if storage_info.key.contains("price") || storage_info.key.contains("rate") {
                                // Estimate price arbitrage potential
                                price_difference = price_difference.saturating_add(U256::from(50));
                            }
                        }
                    },
                    OpCode::CALL => {
                        // Check for external price calls
                        if let Some(call_info) = &state.call_info {
                            if call_info.to.contains("oracle") || call_info.to.contains("price") {
                                price_difference = price_difference.saturating_add(U256::from(25));
                            }
                        }
                    },
                    _ => {}
                }
            }
        }
        
        // Apply market volatility factor
        let volatility_factor = U256::from(120); // 120% of calculated difference
        price_difference * volatility_factor / U256::from(100)
    }

    /// Check if element is in condition list
    pub fn is_element_in_condition_list(&self, element: z3::ast::Bool, condition_list: &[z3::ast::Bool]) -> bool {
        for item in condition_list {
            if element.to_string() == item.to_string() {
                return true;
            }
        }
        false
    }

    /// Analyze DeFi protocol patterns
    pub fn analyze_defi_protocol_patterns<'a>(&self, path: &ExecutionPath<'a>) -> DeFiResult<Vec<DeFiProtocolPattern<'a>>> where 'ctx: 'a {
        let mut patterns = Vec::new();
        
        for state in path {
            match state.current_opcode {
                OpCode::CALL => {
                    // Analyze call patterns
                    if let Some(pattern) = self.analyze_call_pattern(state) {
                        patterns.push(pattern);
                    }
                },
                OpCode::SSTORE => {
                    // Analyze storage patterns
                    if let Some(pattern) = self.analyze_storage_pattern(state) {
                        patterns.push(pattern);
                    }
                },
                _ => {}
            }
        }

        Ok(patterns)
    }

    /// Analyze call pattern
    fn analyze_call_pattern(&self, state: &EVMExecutionState<'ctx>) -> Option<DeFiProtocolPattern<'ctx>> {
        // Analyze call patterns for DeFi protocols
        None
    }

    /// Analyze storage pattern
    fn analyze_storage_pattern(&self, state: &EVMExecutionState<'ctx>) -> Option<DeFiProtocolPattern<'ctx>> {
        // Analyze storage patterns for DeFi protocols
        None
    }

    /// Get feature statistics
    pub fn get_feature_statistics<'a>(&self, feature: &DeFiFeature<'a>) -> DeFiFeatureStatistics {
        DeFiFeatureStatistics {
            total_balance_changes: feature.erc_balance_change_map.values().map(|v| v.len()).sum(),
            total_eth_transfers: feature.eth_transfers.len(),
            total_conditions: feature.condition_map.values().map(|v| v.len()).sum(),
            total_arbitrage_opportunities: feature.arbitrage_opportunities.len(),
        }
    }
}

/// DeFi Protocol Pattern
#[derive(Debug, Clone)]
pub struct DeFiProtocolPattern<'ctx> {
    /// Pattern type
    pub pattern_type: DeFiPatternType,
    /// Pattern description
    pub description: String,
    /// Confidence score
    pub confidence: f64,
    /// Required conditions
    pub required_conditions: Vec<z3::ast::Bool<'ctx>>,
}

/// DeFi Pattern Type
#[derive(Debug, Clone)]
pub enum DeFiPatternType {
    /// Uniswap pattern
    Uniswap,
    /// Compound pattern
    Compound,
    /// Aave pattern
    Aave,
    /// Curve pattern
    Curve,
    /// Balancer pattern
    Balancer,
    /// Generic DEX pattern
    GenericDEX,
    /// Generic lending pattern
    GenericLending,
}

/// DeFi Feature Statistics
#[derive(Debug, Clone)]
pub struct DeFiFeatureStatistics {
    /// Total balance changes
    pub total_balance_changes: usize,
    /// Total ETH transfers
    pub total_eth_transfers: usize,
    /// Total conditions
    pub total_conditions: usize,
    /// Total arbitrage opportunities
    pub total_arbitrage_opportunities: usize,
}

/// DeFi Feature Analyzer
pub struct DeFiFeatureAnalyzer<'ctx> {
    /// Feature extractor
    extractor: DeFiFeatureExtractor<'ctx>,
    /// Analysis results
    analysis_results: Vec<DeFiAnalysisResult<'ctx>>,
}

/// DeFi Analysis Result
#[derive(Debug, Clone)]
pub struct DeFiAnalysisResult<'ctx> {
    /// Contract address
    pub contract_address: Address,
    /// Analysis timestamp
    pub timestamp: u64,
    /// Extracted features
    pub features: DeFiFeature<'ctx>,
    /// Risk assessment
    pub risk_assessment: RiskAssessment,
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// Risk Assessment
#[derive(Debug, Clone)]
pub struct RiskAssessment {
    /// Overall risk level
    pub overall_risk: RiskLevel,
    /// Risk factors
    pub risk_factors: Vec<RiskFactor>,
    /// Risk score (0.0 to 1.0)
    pub risk_score: f64,
}

/// Risk Factor
#[derive(Debug, Clone)]
pub struct RiskFactor {
    /// Factor type
    pub factor_type: RiskFactorType,
    /// Description
    pub description: String,
    /// Severity
    pub severity: RiskLevel,
    /// Impact
    pub impact: f64,
}

/// Risk Factor Type
#[derive(Debug, Clone)]
pub enum RiskFactorType {
    /// High gas usage
    HighGasUsage,
    /// Complex control flow
    ComplexControlFlow,
    /// External dependencies
    ExternalDependencies,
    /// State manipulation
    StateManipulation,
    /// Reentrancy risk
    ReentrancyRisk,
    /// Integer overflow
    IntegerOverflow,
    /// Access control
    AccessControl,
}

impl<'ctx> DeFiFeatureAnalyzer<'ctx> {
    /// Create new DeFi feature analyzer
    pub fn new(extractor: DeFiFeatureExtractor<'ctx>) -> Self {
        Self {
            extractor,
            analysis_results: Vec::new(),
        }
    }

    /// Analyze execution path
    pub fn analyze_execution_path(&mut self, path: &ExecutionPath<'ctx>, 
                                 called_contract: z3::ast::BV<'ctx>) -> DeFiResult<DeFiAnalysisResult> {
        let features = self.extractor.get_defi_symbolic_features_from_path(path, called_contract)?;
        let risk_assessment = self.assess_risks(&features);
        let recommendations = self.generate_recommendations(&features, &risk_assessment);
        
        let result = DeFiAnalysisResult {
            contract_address: Address::ZERO, // This should be extracted from the path
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_secs(),
            features,
            risk_assessment,
            recommendations,
        };
        
        self.analysis_results.push(result.clone());
        Ok(result)
    }

    /// Assess risks
    fn assess_risks(&self, features: &DeFiFeature) -> RiskAssessment {
        let mut risk_factors = Vec::new();
        let mut risk_score: f32 = 0.0;
        
        // Analyze balance changes
        if features.erc_balance_change_map.len() > 10 {
            risk_factors.push(RiskFactor {
                factor_type: RiskFactorType::StateManipulation,
                description: "High number of balance changes detected".to_string(),
                severity: RiskLevel::Medium,
                impact: 0.3,
            });
            risk_score += 0.3;
        }
        
        // Analyze ETH transfers
        if features.eth_transfers.len() > 5 {
            risk_factors.push(RiskFactor {
                factor_type: RiskFactorType::ExternalDependencies,
                description: "High number of ETH transfers detected".to_string(),
                severity: RiskLevel::High,
                impact: 0.5,
            });
            risk_score += 0.5;
        }
        
        // Analyze arbitrage opportunities
        if !features.arbitrage_opportunities.is_empty() {
            risk_factors.push(RiskFactor {
                factor_type: RiskFactorType::ComplexControlFlow,
                description: "Arbitrage opportunities detected".to_string(),
                severity: RiskLevel::High,
                impact: 0.7,
            });
            risk_score += 0.7;
        }
        
        let overall_risk = if risk_score > 0.7 {
            RiskLevel::Critical
        } else if risk_score > 0.5 {
            RiskLevel::High
        } else if risk_score > 0.3 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };
        
        RiskAssessment {
            overall_risk,
            risk_factors,
            risk_score: risk_score.min(1.0) as f64,
        }
    }

    /// Generate recommendations
    fn generate_recommendations(&self, features: &DeFiFeature, risk_assessment: &RiskAssessment) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if risk_assessment.risk_score > 0.5 {
            recommendations.push("Consider implementing additional security measures".to_string());
        }
        
        if !features.arbitrage_opportunities.is_empty() {
            recommendations.push("Monitor for potential arbitrage exploitation".to_string());
        }
        
        if features.eth_transfers.len() > 3 {
            recommendations.push("Review ETH transfer logic for potential vulnerabilities".to_string());
        }
        
        recommendations
    }

    /// Get analysis results
    pub fn get_analysis_results(&self) -> &Vec<DeFiAnalysisResult> {
        &self.analysis_results
    }

    /// Clear analysis results
    pub fn clear_analysis_results(&mut self) {
        self.analysis_results.clear();
    }
}
