//! Liquidation Detector for Aave V2/V3, Compound, and other lending protocols
//!
//! This detector identifies underwater positions that can be liquidated for profit.

use alloy_primitives::{Address, U256};
use alloy_contract::SolCallBuilder;
use alloy_provider::Provider;
use std::collections::HashMap;
use anyhow::{Result, Context};
use async_trait::async_trait;

use crate::abstractions::{ArbitrageDetector, DetectionContext, DetectionResult, ArbitrageOpportunity, OpportunityType};
use crate::detectors::aave_contracts::{AaveAccountData, IAaveV2Pool, IAaveV3Pool};

/// Supported lending protocols
#[derive(Debug, Clone, PartialEq)]
pub enum LiquidationProtocol {
    AaveV2,
    AaveV3,
    Compound,
    MakerDAO,
}

/// Aave借贷仓位
#[derive(Debug, Clone)]
pub struct AavePosition {
    pub user: Address,
    pub collateral_token: Address,
    pub collateral_amount: U256,
    pub debt_token: Address,
    pub debt_amount: U256,
    pub health_factor: f64,
}

/// 清算机会
#[derive(Debug, Clone)]
pub struct LiquidationOpportunity {
    pub protocol: LiquidationProtocol,
    pub position: AavePosition,
    pub liquidation_reward: U256,
    pub collateral_to_seize: U256,
    pub debt_to_repay: U256,
    pub max_close_factor: f64, // 最大可清算比例 (通常50%)
}

/// Price impact estimate from liquidation
#[derive(Debug, Clone, Default)]
pub struct PriceImpact {
    /// Affected pools
    pub affected_pools: HashMap<Address, PoolStateChange>,
    /// Token price changes
    pub price_changes: HashMap<Address, PriceChange>,
    /// Gas used
    pub gas_used: u64,
}

#[derive(Debug, Clone)]
pub struct PoolStateChange {
    pub pool_address: Address,
    pub token0: Address,
    pub token1: Address,
    pub old_reserve0: U256,
    pub old_reserve1: U256,
    pub new_reserve0: U256,
    pub new_reserve1: U256,
}

#[derive(Debug, Clone)]
pub struct PriceChange {
    pub token: Address,
    pub old_price_in_weth: U256,
    pub new_price_in_weth: U256,
    pub change_percentage: f64,
}

/// Configuration for liquidation detector
#[derive(Debug, Clone)]
pub struct LiquidationDetectorConfig {
    /// Minimum health factor to trigger liquidation (usually 1.0)
    pub health_threshold: f64,

    /// Minimum profit to consider (in wei)
    pub min_profit_wei: U256,

    /// Aave V2 pool address
    pub aave_v2_pool: Address,

    /// Aave V3 pool address
    pub aave_v3_pool: Address,

    /// Enable Aave V2 detection
    pub enable_aave_v2: bool,

    /// Enable Aave V3 detection
    pub enable_aave_v3: bool,
}

impl Default for LiquidationDetectorConfig {
    fn default() -> Self {
        Self {
            health_threshold: 1.0,
            min_profit_wei: U256::from(10_000_000_000_000_000u64), // 0.01 ETH
            // Mainnet addresses
            aave_v2_pool: "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9"
                .parse()
                .unwrap(),
            aave_v3_pool: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"
                .parse()
                .unwrap(),
            enable_aave_v2: true,
            enable_aave_v3: true,
        }
    }
}

/// Liquidation detector
pub struct LiquidationDetector {
    config: LiquidationDetectorConfig,
}

impl LiquidationDetector {
    pub fn new(config: LiquidationDetectorConfig) -> Self {
        Self { config }
    }

    /// 检测健康度低于阈值的仓位
    pub async fn detect_underwater_positions<P>(
        &self,
        provider: &P,
        context: &DetectionContext,
    ) -> Result<Vec<LiquidationOpportunity>>
    where
        P: Provider + Clone,
    {
        let mut opportunities = vec![];

        // Detect Aave V2 liquidations
        if self.config.enable_aave_v2 {
            let aave_v2_opps = self
                .detect_aave_v2_liquidations(provider, context)
                .await?;
            opportunities.extend(aave_v2_opps);
        }

        // Detect Aave V3 liquidations
        if self.config.enable_aave_v3 {
            let aave_v3_opps = self
                .detect_aave_v3_liquidations(provider, context)
                .await?;
            opportunities.extend(aave_v3_opps);
        }

        // Filter by minimum profit
        opportunities.retain(|opp| opp.liquidation_reward >= self.config.min_profit_wei);

        Ok(opportunities)
    }

    /// Detect Aave V2 liquidations
    async fn detect_aave_v2_liquidations<P>(
        &self,
        provider: &P,
        context: &DetectionContext,
    ) -> Result<Vec<LiquidationOpportunity>>
    where
        P: Provider + Clone,
    {
        let mut opportunities = vec![];

        // In a real implementation, we would:
        // 1. Subscribe to Aave events or query a subgraph for active positions
        // 2. Monitor price feeds for collateral value changes
        // 3. Maintain a cache of positions to check

        // For now, we'll demonstrate checking a list of known users
        let users_to_check = self.get_users_to_check(context);

        for user_address in users_to_check {
            // Query user account data from Aave V2 Pool
            let account_data = self
                .query_aave_v2_account(provider, user_address)
                .await?;

            // Check if liquidatable
            if !account_data.is_liquidatable() {
                continue;
            }

            // Calculate liquidation opportunity details
            if let Some(opp) = self.calculate_aave_v2_liquidation(user_address, &account_data).await? {
                opportunities.push(opp);
            }
        }

        Ok(opportunities)
    }

    /// Query Aave V2 account data for a user
    async fn query_aave_v2_account<P>(
        &self,
        provider: &P,
        user: Address,
    ) -> Result<AaveAccountData>
    where
        P: Provider + Clone,
    {
        // Create contract instance
        let pool = IAaveV2Pool::new(self.config.aave_v2_pool, provider.clone());

        // Call getUserAccountData
        let result = pool
            .getUserAccountData(user)
            .call()
            .await
            .context("Failed to query Aave V2 getUserAccountData")?;

        Ok(AaveAccountData {
            total_collateral_eth: result.totalCollateralETH,
            total_debt_eth: result.totalDebtETH,
            available_borrows_eth: result.availableBorrowsETH,
            current_liquidation_threshold: result.currentLiquidationThreshold,
            ltv: result.ltv,
            health_factor: result.healthFactor,
        })
    }

    /// Calculate Aave V2 liquidation opportunity details
    async fn calculate_aave_v2_liquidation(
        &self,
        user: Address,
        account_data: &AaveAccountData,
    ) -> Result<Option<LiquidationOpportunity>> {
        // For Aave V2, liquidation bonus is typically 5%
        let liquidation_bonus_bps = 500u64; // 5% = 500 basis points

        // Maximum close factor is 50% for Aave V2
        let max_close_factor = 0.5;

        // Calculate debt to repay (50% of total debt)
        let debt_to_repay = account_data.total_debt_eth
            .saturating_mul(U256::from(50))
            / U256::from(100);

        // Calculate collateral to seize with bonus
        // collateral_to_seize = debt_to_repay * (1 + bonus)
        let collateral_to_seize = debt_to_repay
            .saturating_mul(U256::from(10000 + liquidation_bonus_bps))
            / U256::from(10000);

        // Calculate liquidation reward (the bonus)
        let liquidation_reward = collateral_to_seize.saturating_sub(debt_to_repay);

        // Check if profitable
        if liquidation_reward < self.config.min_profit_wei {
            return Ok(None);
        }

        // Create a simplified position (in real implementation, we'd query specific reserves)
        let position = AavePosition {
            user,
            collateral_token: Address::ZERO, // Would query actual collateral token
            collateral_amount: account_data.total_collateral_eth,
            debt_token: Address::ZERO, // Would query actual debt token
            debt_amount: account_data.total_debt_eth,
            health_factor: account_data.health_factor_as_f64(),
        };

        Ok(Some(LiquidationOpportunity {
            protocol: LiquidationProtocol::AaveV2,
            position,
            liquidation_reward,
            collateral_to_seize,
            debt_to_repay,
            max_close_factor,
        }))
    }

    /// Get list of users to check for liquidation
    /// In production, this would come from:
    /// - Event logs (Borrow, Deposit, Withdraw events)
    /// - Subgraph queries
    /// - Cached list of active borrowers
    fn get_users_to_check(&self, _context: &DetectionContext) -> Vec<Address> {
        // TODO: Implement user discovery mechanism
        // For now, return empty list
        vec![]
    }

    /// Detect Aave V3 liquidations
    async fn detect_aave_v3_liquidations<P>(
        &self,
        provider: &P,
        context: &DetectionContext,
    ) -> Result<Vec<LiquidationOpportunity>>
    where
        P: Provider + Clone,
    {
        let mut opportunities = vec![];

        // Get users to check (same mechanism as V2)
        let users_to_check = self.get_users_to_check(context);

        for user_address in users_to_check {
            // Query user account data from Aave V3 Pool
            let account_data = self
                .query_aave_v3_account(provider, user_address)
                .await?;

            // Check if liquidatable
            if !account_data.is_liquidatable() {
                continue;
            }

            // Calculate liquidation opportunity details
            if let Some(opp) = self.calculate_aave_v3_liquidation(user_address, &account_data).await? {
                opportunities.push(opp);
            }
        }

        Ok(opportunities)
    }

    /// Query Aave V3 account data for a user
    async fn query_aave_v3_account<P>(
        &self,
        provider: &P,
        user: Address,
    ) -> Result<AaveAccountData>
    where
        P: Provider + Clone,
    {
        // Create contract instance
        let pool = IAaveV3Pool::new(self.config.aave_v3_pool, provider.clone());

        // Call getUserAccountData
        let result = pool
            .getUserAccountData(user)
            .call()
            .await
            .context("Failed to query Aave V3 getUserAccountData")?;

        Ok(AaveAccountData {
            total_collateral_eth: result.totalCollateralBase,
            total_debt_eth: result.totalDebtBase,
            available_borrows_eth: result.availableBorrowsBase,
            current_liquidation_threshold: result.currentLiquidationThreshold,
            ltv: result.ltv,
            health_factor: result.healthFactor,
        })
    }

    /// Calculate Aave V3 liquidation opportunity details
    async fn calculate_aave_v3_liquidation(
        &self,
        user: Address,
        account_data: &AaveAccountData,
    ) -> Result<Option<LiquidationOpportunity>> {
        // For Aave V3, liquidation bonus varies by asset but typically 5-10%
        let liquidation_bonus_bps = 500u64; // 5% conservative estimate

        // Maximum close factor is 50% for Aave V3
        let max_close_factor = 0.5;

        // Calculate debt to repay (50% of total debt)
        let debt_to_repay = account_data.total_debt_eth
            .saturating_mul(U256::from(50))
            / U256::from(100);

        // Calculate collateral to seize with bonus
        let collateral_to_seize = debt_to_repay
            .saturating_mul(U256::from(10000 + liquidation_bonus_bps))
            / U256::from(10000);

        // Calculate liquidation reward
        let liquidation_reward = collateral_to_seize.saturating_sub(debt_to_repay);

        // Check if profitable
        if liquidation_reward < self.config.min_profit_wei {
            return Ok(None);
        }

        // Create position
        let position = AavePosition {
            user,
            collateral_token: Address::ZERO, // Would query actual collateral token
            collateral_amount: account_data.total_collateral_eth,
            debt_token: Address::ZERO, // Would query actual debt token
            debt_amount: account_data.total_debt_eth,
            health_factor: account_data.health_factor_as_f64(),
        };

        Ok(Some(LiquidationOpportunity {
            protocol: LiquidationProtocol::AaveV3,
            position,
            liquidation_reward,
            collateral_to_seize,
            debt_to_repay,
            max_close_factor,
        }))
    }

    /// Convert LiquidationOpportunity to ArbitrageOpportunity
    fn convert_to_arbitrage_opportunity(
        &self,
        liq: &LiquidationOpportunity,
    ) -> ArbitrageOpportunity {
        use uuid::Uuid;

        let protocol_name = match liq.protocol {
            LiquidationProtocol::AaveV2 => "AaveV2",
            LiquidationProtocol::AaveV3 => "AaveV3",
            LiquidationProtocol::Compound => "Compound",
            LiquidationProtocol::MakerDAO => "MakerDAO",
        };

        ArbitrageOpportunity {
            id: Uuid::new_v4().to_string(),
            opportunity_type: OpportunityType::Liquidation {
                protocol: protocol_name.to_string(),
                user: liq.position.user,
                collateral_token: liq.position.collateral_token,
                debt_token: liq.position.debt_token,
                debt_to_repay: liq.debt_to_repay,
                collateral_to_seize: liq.collateral_to_seize,
            },
            expected_profit: liq.liquidation_reward,
            gas_cost: U256::from(500_000u64).saturating_mul(U256::from(20_000_000_000u64)), // 500k gas * 20 gwei
            confidence: 0.9,
            risk_level: crate::abstractions::RiskLevel::Medium,
            deadline: None,
            required_capital: liq.debt_to_repay,
            metadata: serde_json::json!({
                "protocol": protocol_name,
                "health_factor": liq.position.health_factor,
                "max_close_factor": liq.max_close_factor,
            }),
        }
    }
}

#[async_trait]
impl ArbitrageDetector for LiquidationDetector {
    async fn detect(&mut self, _context: &DetectionContext) -> Result<DetectionResult> {
        use std::time::Duration;

        // TODO: Implement when we have provider access in context
        // For now, return empty result since we need a Provider instance
        // which isn't available in DetectionContext yet

        Ok(DetectionResult {
            opportunities: vec![],
            detection_time: Duration::from_millis(0),
            detector_id: "liquidation_detector".to_string(),
            confidence_threshold: 0.7,
            metadata: HashMap::new(),
        })
    }

    fn config(&self) -> &crate::abstractions::DetectorConfig {
        // TODO: Convert LiquidationDetectorConfig to DetectorConfig
        // For now, use a static reference
        unimplemented!("config() not yet implemented for LiquidationDetector")
    }

    fn update_config(&mut self, _config: crate::abstractions::DetectorConfig) -> Result<()> {
        // TODO: Update configuration
        unimplemented!("update_config() not yet implemented for LiquidationDetector")
    }

    fn metadata(&self) -> crate::abstractions::DetectorMetadata {
        use crate::abstractions::{DetectorMetadata, PerformanceMetrics};

        DetectorMetadata {
            name: "Liquidation Detector".to_string(),
            version: "0.1.0".to_string(),
            description: "Detects liquidation opportunities on Aave V2/V3 and other lending protocols".to_string(),
            supported_opportunity_types: vec![
                "Liquidation".to_string(),
            ],
            performance_metrics: PerformanceMetrics {
                avg_detection_time: std::time::Duration::from_millis(100),
                success_rate: 0.85,
                total_detections: 0,
                avg_profit_accuracy: 0.80,
            },
        }
    }

    async fn health_check(&self) -> Result<crate::abstractions::HealthStatus> {
        use crate::abstractions::HealthStatus;

        Ok(HealthStatus {
            is_healthy: true,
            status_message: "Liquidation detector operational".to_string(),
            last_check: std::time::Instant::now(),
            metrics: HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidation_detector_creation() {
        let config = LiquidationDetectorConfig::default();
        let detector = LiquidationDetector::new(config);

        assert_eq!(detector.config.health_threshold, 1.0);
        assert!(detector.config.enable_aave_v2);
        assert!(detector.config.enable_aave_v3);
    }

    #[test]
    fn test_config_default() {
        let config = LiquidationDetectorConfig::default();

        assert_eq!(config.health_threshold, 1.0);
        assert!(config.min_profit_wei > U256::ZERO);
    }
}
