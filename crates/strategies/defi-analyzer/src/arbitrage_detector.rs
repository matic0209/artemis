//! Advanced Arbitrage Detection Algorithms

use alloy_primitives::{Address, U256};
use anyhow::Result;
use tracing::{info, debug, warn};
use std::collections::HashMap;

use crate::{
    types::{ArbitrageOpportunity, RiskLevel, AnalysisEvent},
    error::{DeFiResult, DeFiAnalyzerError},
};

/// Advanced arbitrage detector
pub struct ArbitrageDetector {
    /// Price history cache
    price_history: HashMap<Address, Vec<PricePoint>>,
    /// Liquidity pools cache
    liquidity_pools: HashMap<Address, LiquidityPool>,
    /// Market conditions
    market_conditions: MarketConditions,
    /// Detection algorithms
    algorithms: Vec<Box<dyn ArbitrageAlgorithm>>,
}

/// Price point for historical analysis
#[derive(Debug, Clone)]
pub struct PricePoint {
    pub timestamp: u64,
    pub price: U256,
    pub volume: U256,
    pub source: String,
}

/// Liquidity pool information
#[derive(Debug, Clone)]
pub struct LiquidityPool {
    pub address: Address,
    pub token_a: Address,
    pub token_b: Address,
    pub reserve_a: U256,
    pub reserve_b: U256,
    pub fee: u32,
    pub last_updated: u64,
}

/// Market conditions
#[derive(Debug, Clone)]
pub struct MarketConditions {
    pub gas_price: U256,
    pub network_congestion: f64,
    pub volatility: f64,
    pub liquidity_depth: f64,
}

/// Arbitrage algorithm trait
pub trait ArbitrageAlgorithm {
    fn name(&self) -> &str;
    fn detect_opportunities(&self, event: &AnalysisEvent, detector: &ArbitrageDetector) -> DeFiResult<Vec<ArbitrageOpportunity>>;
    fn priority(&self) -> u8;
}

/// Price arbitrage algorithm
pub struct PriceArbitrageAlgorithm;

impl ArbitrageAlgorithm for PriceArbitrageAlgorithm {
    fn name(&self) -> &str {
        "Price Arbitrage"
    }

    fn detect_opportunities(&self, event: &AnalysisEvent, detector: &ArbitrageDetector) -> DeFiResult<Vec<ArbitrageOpportunity>> {
        debug!("Running price arbitrage detection for contract: {}", event.contract_address);
        
        let mut opportunities = Vec::new();
        
        // Get price data for the contract
        if let Some(price_history) = detector.price_history.get(&event.contract_address) {
            if price_history.len() < 2 {
                return Ok(opportunities);
            }

            // Find price differences across different sources
            let latest_prices = detector.get_latest_prices(&event.contract_address)?;
            
            for (source_a, price_a) in &latest_prices {
                for (source_b, price_b) in &latest_prices {
                    if source_a != source_b {
                        let price_diff = if price_a > price_b {
                            price_a - price_b
                        } else {
                            price_b - price_a
                        };

                        // Calculate potential profit
                        let profit = detector.calculate_price_arbitrage_profit(
                            *price_a, *price_b, U256::from(1_000_000_000_000_000_000u64) // 1 ETH
                        )?;

                        if profit > detector.market_conditions.gas_price * U256::from(200_000u64) {
                            opportunities.push(ArbitrageOpportunity {
                                opportunity_id: format!("price_arb_{}_{}_{}", 
                                    event.block_number, source_a, source_b),
                                expected_profit: profit,
                                required_gas: 200_000,
                                success_probability: detector.calculate_success_probability(profit),
                                risk_level: detector.assess_price_arbitrage_risk(profit),
                                strategy_description: format!(
                                    "Buy from {} at {}, sell to {} at {}",
                                    source_b, price_b, source_a, price_a
                                ),
                            });
                        }
                    }
                }
            }
        }

        debug!("Found {} price arbitrage opportunities", opportunities.len());
        Ok(opportunities)
    }

    fn priority(&self) -> u8 {
        90
    }
}

/// Liquidity arbitrage algorithm
pub struct LiquidityArbitrageAlgorithm;

impl ArbitrageAlgorithm for LiquidityArbitrageAlgorithm {
    fn name(&self) -> &str {
        "Liquidity Arbitrage"
    }

    fn detect_opportunities(&self, event: &AnalysisEvent, detector: &ArbitrageDetector) -> DeFiResult<Vec<ArbitrageOpportunity>> {
        debug!("Running liquidity arbitrage detection for contract: {}", event.contract_address);
        
        let mut opportunities = Vec::new();
        
        // Check for liquidity imbalances
        if let Some(pool) = detector.liquidity_pools.get(&event.contract_address) {
            let imbalance = detector.calculate_liquidity_imbalance(pool)?;
            
            if imbalance > U256::from(1_000_000_000_000_000_000u64) { // 1 ETH threshold
                let profit = detector.calculate_liquidity_arbitrage_profit(pool, imbalance)?;
                
                if profit > detector.market_conditions.gas_price * U256::from(300_000u64) {
                    opportunities.push(ArbitrageOpportunity {
                        opportunity_id: format!("liquidity_arb_{}", event.block_number),
                        expected_profit: profit,
                        required_gas: 300_000,
                        success_probability: detector.calculate_liquidity_success_probability(imbalance),
                        risk_level: detector.assess_liquidity_arbitrage_risk(imbalance),
                        strategy_description: format!(
                            "Rebalance liquidity pool {} with imbalance {}",
                            pool.address, imbalance
                        ),
                    });
                }
            }
        }

        debug!("Found {} liquidity arbitrage opportunities", opportunities.len());
        Ok(opportunities)
    }

    fn priority(&self) -> u8 {
        80
    }
}

/// Gas arbitrage algorithm
pub struct GasArbitrageAlgorithm;

impl ArbitrageAlgorithm for GasArbitrageAlgorithm {
    fn name(&self) -> &str {
        "Gas Arbitrage"
    }

    fn detect_opportunities(&self, event: &AnalysisEvent, detector: &ArbitrageDetector) -> DeFiResult<Vec<ArbitrageOpportunity>> {
        debug!("Running gas arbitrage detection for contract: {}", event.contract_address);
        
        let mut opportunities = Vec::new();
        
        // Analyze transaction data for gas optimization opportunities
        if let Some(tx_data) = &event.tx_data {
            let gas_optimization = detector.analyze_gas_optimization(tx_data)?;
            
            if gas_optimization.savings > U256::from(10_000_000_000_000_000u64) { // 0.01 ETH
                opportunities.push(ArbitrageOpportunity {
                    opportunity_id: format!("gas_arb_{}", event.block_number),
                    expected_profit: gas_optimization.savings,
                    required_gas: gas_optimization.optimization_gas,
                    success_probability: 0.95, // Gas optimization is usually reliable
                    risk_level: RiskLevel::Low,
                    strategy_description: format!(
                        "Optimize gas usage: save {} gas, cost {} gas",
                        gas_optimization.savings, gas_optimization.optimization_gas
                    ),
                });
            }
        }

        debug!("Found {} gas arbitrage opportunities", opportunities.len());
        Ok(opportunities)
    }

    fn priority(&self) -> u8 {
        70
    }
}

/// Gas optimization analysis result
#[derive(Debug)]
pub struct GasOptimization {
    pub savings: U256,
    pub optimization_gas: u64,
    pub original_gas: u64,
    pub optimized_gas: u64,
}

impl ArbitrageDetector {
    /// Create a new arbitrage detector
    pub fn new() -> Self {
        let mut detector = Self {
            price_history: HashMap::new(),
            liquidity_pools: HashMap::new(),
            market_conditions: MarketConditions {
                gas_price: U256::from(20_000_000_000u64), // 20 gwei
                network_congestion: 0.5,
                volatility: 0.02,
                liquidity_depth: 1.0,
            },
            algorithms: Vec::new(),
        };

        // Register algorithms
        detector.register_algorithm(Box::new(PriceArbitrageAlgorithm));
        detector.register_algorithm(Box::new(LiquidityArbitrageAlgorithm));
        detector.register_algorithm(Box::new(GasArbitrageAlgorithm));

        detector
    }

    /// Register an arbitrage algorithm
    pub fn register_algorithm(&mut self, algorithm: Box<dyn ArbitrageAlgorithm>) {
        self.algorithms.push(algorithm);
        // Sort by priority (highest first)
        self.algorithms.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }

    /// Detect arbitrage opportunities
    pub fn detect_opportunities(&self, event: &AnalysisEvent) -> DeFiResult<Vec<ArbitrageOpportunity>> {
        info!("Detecting arbitrage opportunities for contract: {}", event.contract_address);
        
        let mut all_opportunities = Vec::new();
        
        for algorithm in &self.algorithms {
            match algorithm.detect_opportunities(event, self) {
                Ok(opportunities) => {
                    debug!("Algorithm {} found {} opportunities", algorithm.name(), opportunities.len());
                    all_opportunities.extend(opportunities);
                },
                Err(e) => {
                    warn!("Algorithm {} failed: {}", algorithm.name(), e);
                    // Continue with other algorithms
                }
            }
        }

        // Remove duplicates and sort by profit
        all_opportunities.sort_by(|a, b| b.expected_profit.cmp(&a.expected_profit));
        all_opportunities.dedup_by(|a, b| a.opportunity_id == b.opportunity_id);

        info!("Found {} total arbitrage opportunities", all_opportunities.len());
        Ok(all_opportunities)
    }

    /// Get latest prices for a contract
    fn get_latest_prices(&self, contract_address: &Address) -> DeFiResult<HashMap<String, U256>> {
        let mut prices = HashMap::new();
        
        if let Some(history) = self.price_history.get(contract_address) {
            if let Some(latest) = history.last() {
                prices.insert(latest.source.clone(), latest.price);
            }
        }
        
        Ok(prices)
    }

    /// Calculate price arbitrage profit
    fn calculate_price_arbitrage_profit(&self, price_a: U256, price_b: U256, amount: U256) -> DeFiResult<U256> {
        let price_diff = if price_a > price_b {
            price_a - price_b
        } else {
            price_b - price_a
        };
        
        let gross_profit = (price_diff * amount) / U256::from(10).pow(U256::from(18));
        let gas_cost = self.market_conditions.gas_price * U256::from(200_000u64);
        
        Ok(gross_profit.saturating_sub(gas_cost))
    }

    /// Calculate success probability
    fn calculate_success_probability(&self, profit: U256) -> f64 {
        // Higher profit = higher success probability
        let profit_eth = profit.to::<u128>() as f64 / 1e18;
        (profit_eth * 0.1).min(0.95) // Cap at 95%
    }

    /// Assess price arbitrage risk
    fn assess_price_arbitrage_risk(&self, profit: U256) -> RiskLevel {
        let profit_eth = profit.to::<u128>() as f64 / 1e18;
        
        if profit_eth > 1.0 {
            RiskLevel::Low
        } else if profit_eth > 0.1 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        }
    }

    /// Calculate liquidity imbalance
    fn calculate_liquidity_imbalance(&self, pool: &LiquidityPool) -> DeFiResult<U256> {
        // Enhanced imbalance calculation with market analysis
        let expected_ratio = U256::from(1);
        let actual_ratio = if pool.reserve_b > U256::from(0) {
            pool.reserve_a / pool.reserve_b
        } else {
            U256::from(0)
        };
        
        // Calculate percentage deviation
        let deviation = if actual_ratio > expected_ratio {
            (actual_ratio - expected_ratio) * U256::from(100) / expected_ratio
        } else if expected_ratio > U256::from(0) {
            (expected_ratio - actual_ratio) * U256::from(100) / expected_ratio
        } else {
            U256::from(0)
        };
        
        // Apply market volatility factor
        let volatility_factor = self.market_conditions.volatility_index;
        let adjusted_imbalance = deviation * volatility_factor / U256::from(100);
        
        Ok(adjusted_imbalance)
    }

    /// Calculate liquidity arbitrage profit
    fn calculate_liquidity_arbitrage_profit(&self, pool: &LiquidityPool, imbalance: U256) -> DeFiResult<U256> {
        // Enhanced profit calculation with market conditions
        let base_profit = imbalance * U256::from(1_000_000_000_000_000_000u64) / U256::from(10).pow(U256::from(18));
        
        // Apply market efficiency factor
        let efficiency_factor = U256::from(85); // 85% efficiency
        let adjusted_profit = base_profit * efficiency_factor / U256::from(100);
        
        // Calculate gas costs with current market conditions
        let gas_cost = self.market_conditions.gas_price * U256::from(300_000u64);
        
        // Apply slippage factor
        let slippage_factor = U256::from(95); // 5% slippage
        let final_profit = adjusted_profit * slippage_factor / U256::from(100);
        
        Ok(final_profit.saturating_sub(gas_cost))
    }

    /// Calculate liquidity success probability
    fn calculate_liquidity_success_probability(&self, imbalance: U256) -> f64 {
        let imbalance_eth = imbalance.to::<u128>() as f64 / 1e18;
        (imbalance_eth * 0.2).min(0.9) // Cap at 90%
    }

    /// Assess liquidity arbitrage risk
    fn assess_liquidity_arbitrage_risk(&self, imbalance: U256) -> RiskLevel {
        let imbalance_eth = imbalance.to::<u128>() as f64 / 1e18;
        
        if imbalance_eth > 10.0 {
            RiskLevel::Low
        } else if imbalance_eth > 1.0 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        }
    }

    /// Analyze gas optimization
    fn analyze_gas_optimization(&self, tx_data: &[u8]) -> DeFiResult<GasOptimization> {
        // Enhanced gas optimization analysis
        let base_gas = 21_000; // Base transaction cost
        let data_gas = tx_data.len() as u64 * 16; // 16 gas per byte
        let original_gas = base_gas + data_gas;
        
        // Analyze transaction structure for optimization opportunities
        let mut optimization_factor = 100;
        
        // Check for batch operations
        if tx_data.len() > 1000 {
            optimization_factor -= 10;
        }
        
        // Check for storage optimization
        let storage_ops = tx_data.windows(4).filter(|window| {
            matches!(window, [0x55, 0x60, 0x60, 0x52] | [0x54, 0x60, 0x60, 0x52]) // SSTORE/SLOAD patterns
        }).count();
        
        if storage_ops > 5 {
            optimization_factor -= 15;
        }
        
        // Check for loop optimization
        let loop_ops = tx_data.windows(4).filter(|window| {
            matches!(window, [0x5b, 0x60, 0x60, 0x52]) // JUMPDEST patterns
        }).count();
        
        if loop_ops > 3 {
            optimization_factor -= 10;
        }
        
        // Check for external call optimization
        let call_ops = tx_data.windows(4).filter(|window| {
            matches!(window, [0xf1, 0xf2, 0xf4, 0xfa]) // CALL/STATICCALL/DELEGATECALL patterns
        }).count();
        
        if call_ops > 2 {
            optimization_factor -= 8;
        }
        
        let optimized_gas = (original_gas * optimization_factor as u64) / 100;
        let savings = U256::from(original_gas - optimized_gas) * self.market_conditions.gas_price;
        
        Ok(GasOptimization {
            savings,
            optimization_gas: 50_000, // Gas needed for optimization
            original_gas,
            optimized_gas,
        })
    }

    /// Update price history
    pub fn update_price_history(&mut self, contract_address: Address, price_point: PricePoint) {
        self.price_history
            .entry(contract_address)
            .or_insert_with(Vec::new)
            .push(price_point);
    }

    /// Update liquidity pool
    pub fn update_liquidity_pool(&mut self, pool: LiquidityPool) {
        self.liquidity_pools.insert(pool.address, pool);
    }

    /// Update market conditions
    pub fn update_market_conditions(&mut self, conditions: MarketConditions) {
        self.market_conditions = conditions;
    }
}
