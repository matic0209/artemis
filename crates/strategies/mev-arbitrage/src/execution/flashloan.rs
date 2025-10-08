//! Flashloan Integration for Zero-Capital Arbitrage
//!
//! Supports multiple flashloan providers:
//! - Aave V2: Most liquid, 0.09% fee
//! - Aave V3: Multi-chain, 0.05% fee
//! - Uniswap V3: No upfront fee, pay from profit
//! - Balancer: 0% fee for complex strategies

use alloy_primitives::{Address, U256, Bytes};
use alloy_sol_types::sol;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use tracing::{info, debug, warn};

use crate::abstractions::ExecutionPlan;

// ============================================================================
// Aave V2 Flashloan Interface
// ============================================================================

sol! {
    /// Aave V2 Lending Pool
    interface IAaveV2LendingPool {
        function flashLoan(
            address receiverAddress,
            address[] calldata assets,
            uint256[] calldata amounts,
            uint256[] calldata modes,
            address onBehalfOf,
            bytes calldata params,
            uint16 referralCode
        ) external;
    }

    /// Aave V2 Flashloan Receiver
    interface IFlashLoanReceiverV2 {
        function executeOperation(
            address[] calldata assets,
            uint256[] calldata amounts,
            uint256[] calldata premiums,
            address initiator,
            bytes calldata params
        ) external returns (bool);
    }
}

// ============================================================================
// Aave V3 Flashloan Interface
// ============================================================================

sol! {
    /// Aave V3 Pool
    interface IAaveV3Pool {
        function flashLoan(
            address receiverAddress,
            address[] calldata assets,
            uint256[] calldata amounts,
            uint256[] calldata interestRateModes,
            address onBehalfOf,
            bytes calldata params,
            uint16 referralCode
        ) external;

        function flashLoanSimple(
            address receiverAddress,
            address asset,
            uint256 amount,
            bytes calldata params,
            uint16 referralCode
        ) external;
    }

    /// Aave V3 Flashloan Receiver
    interface IFlashLoanReceiverV3 {
        function executeOperation(
            address[] calldata assets,
            uint256[] calldata amounts,
            uint256[] calldata premiums,
            address initiator,
            bytes calldata params
        ) external returns (bool);
    }
}

// ============================================================================
// Uniswap V3 Flash Callback Interface
// ============================================================================

sol! {
    /// Uniswap V3 Pool (for flash swaps)
    interface IUniswapV3Pool {
        function flash(
            address recipient,
            uint256 amount0,
            uint256 amount1,
            bytes calldata data
        ) external;
    }

    /// Uniswap V3 Flash Callback
    interface IUniswapV3FlashCallback {
        function uniswapV3FlashCallback(
            uint256 fee0,
            uint256 fee1,
            bytes calldata data
        ) external;
    }
}

// ============================================================================
// Balancer Flash Loan Interface
// ============================================================================

sol! {
    /// Balancer Vault
    interface IBalancerVault {
        function flashLoan(
            address recipient,
            address[] calldata tokens,
            uint256[] calldata amounts,
            bytes calldata userData
        ) external;
    }

    /// Balancer Flash Loan Receiver
    interface IFlashLoanRecipient {
        function receiveFlashLoan(
            address[] calldata tokens,
            uint256[] calldata amounts,
            uint256[] calldata feeAmounts,
            bytes calldata userData
        ) external;
    }
}

// ============================================================================
// Flashloan Provider Configuration
// ============================================================================

/// Flashloan provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashloanProvider {
    AaveV2,
    AaveV3,
    UniswapV3,
    Balancer,
}

impl FlashloanProvider {
    /// Get fee in basis points (bps)
    pub fn fee_bps(&self) -> u64 {
        match self {
            FlashloanProvider::AaveV2 => 9,   // 0.09%
            FlashloanProvider::AaveV3 => 5,   // 0.05%
            FlashloanProvider::UniswapV3 => 0, // Pay from profit
            FlashloanProvider::Balancer => 0,  // 0% fee
        }
    }

    /// Get provider name
    pub fn name(&self) -> &'static str {
        match self {
            FlashloanProvider::AaveV2 => "Aave V2",
            FlashloanProvider::AaveV3 => "Aave V3",
            FlashloanProvider::UniswapV3 => "Uniswap V3",
            FlashloanProvider::Balancer => "Balancer",
        }
    }

    /// Check if provider supports multi-asset flashloans
    pub fn supports_multi_asset(&self) -> bool {
        match self {
            FlashloanProvider::AaveV2 => true,
            FlashloanProvider::AaveV3 => true,
            FlashloanProvider::UniswapV3 => false, // Only single pool
            FlashloanProvider::Balancer => true,
        }
    }
}

/// Flashloan provider addresses
pub struct FlashloanAddresses {
    pub aave_v2_lending_pool: Address,
    pub aave_v3_pool: Address,
    pub balancer_vault: Address,
    // Uniswap V3 uses pool addresses directly
}

impl FlashloanAddresses {
    /// Mainnet addresses
    pub fn mainnet() -> Self {
        Self {
            // Aave V2 Ethereum Mainnet
            aave_v2_lending_pool: "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9"
                .parse()
                .unwrap(),

            // Aave V3 Ethereum Mainnet
            aave_v3_pool: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"
                .parse()
                .unwrap(),

            // Balancer V2 Vault
            balancer_vault: "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
                .parse()
                .unwrap(),
        }
    }

    /// Get address for provider
    pub fn get_address(&self, provider: FlashloanProvider) -> Address {
        match provider {
            FlashloanProvider::AaveV2 => self.aave_v2_lending_pool,
            FlashloanProvider::AaveV3 => self.aave_v3_pool,
            FlashloanProvider::Balancer => self.balancer_vault,
            FlashloanProvider::UniswapV3 => Address::ZERO, // Use pool address
        }
    }
}

// ============================================================================
// Flashloan Request Builder
// ============================================================================

/// Flashloan request
#[derive(Debug, Clone)]
pub struct FlashloanRequest {
    /// Assets to borrow
    pub assets: Vec<Address>,
    /// Amounts to borrow
    pub amounts: Vec<U256>,
    /// Preferred provider
    pub provider: FlashloanProvider,
    /// Calldata to execute during flashloan callback
    pub callback_data: Bytes,
}

impl FlashloanRequest {
    /// Create new flashloan request
    pub fn new(asset: Address, amount: U256) -> Self {
        Self {
            assets: vec![asset],
            amounts: vec![amount],
            provider: FlashloanProvider::AaveV3, // Default to cheapest
            callback_data: Bytes::new(),
        }
    }

    /// Add another asset
    pub fn with_asset(mut self, asset: Address, amount: U256) -> Self {
        self.assets.push(asset);
        self.amounts.push(amount);
        self
    }

    /// Set provider
    pub fn with_provider(mut self, provider: FlashloanProvider) -> Self {
        self.provider = provider;
        self
    }

    /// Set callback data
    pub fn with_callback_data(mut self, data: Bytes) -> Self {
        self.callback_data = data;
        self
    }

    /// Calculate total fee
    pub fn calculate_fee(&self) -> Result<Vec<U256>> {
        let fee_bps = self.provider.fee_bps();
        let mut fees = Vec::new();

        for amount in &self.amounts {
            let fee = amount
                .checked_mul(U256::from(fee_bps))
                .ok_or_else(|| anyhow!("Fee calculation overflow"))?
                .checked_div(U256::from(10000))
                .ok_or_else(|| anyhow!("Fee division error"))?;
            fees.push(fee);
        }

        Ok(fees)
    }

    /// Calculate total repayment amount
    pub fn calculate_repayment(&self) -> Result<Vec<U256>> {
        let fees = self.calculate_fee()?;
        let mut repayments = Vec::new();

        for (amount, fee) in self.amounts.iter().zip(fees.iter()) {
            let repayment = amount
                .checked_add(*fee)
                .ok_or_else(|| anyhow!("Repayment calculation overflow"))?;
            repayments.push(repayment);
        }

        Ok(repayments)
    }

    /// Validate request
    pub fn validate(&self) -> Result<()> {
        if self.assets.is_empty() {
            return Err(anyhow!("No assets specified"));
        }

        if self.assets.len() != self.amounts.len() {
            return Err(anyhow!("Assets and amounts length mismatch"));
        }

        if !self.provider.supports_multi_asset() && self.assets.len() > 1 {
            return Err(anyhow!(
                "{} does not support multi-asset flashloans",
                self.provider.name()
            ));
        }

        for amount in &self.amounts {
            if amount.is_zero() {
                return Err(anyhow!("Flashloan amount cannot be zero"));
            }
        }

        Ok(())
    }
}

// ============================================================================
// Flashloan Executor
// ============================================================================

/// Flashloan executor
pub struct FlashloanExecutor {
    /// Provider addresses
    addresses: FlashloanAddresses,
    /// Executor contract address (our deployed contract)
    executor_contract: Address,
}

impl FlashloanExecutor {
    /// Create new flashloan executor
    pub fn new(executor_contract: Address) -> Self {
        Self {
            addresses: FlashloanAddresses::mainnet(),
            executor_contract,
        }
    }

    /// Build Aave V2 flashloan calldata
    pub fn build_aave_v2_calldata(&self, request: &FlashloanRequest) -> Result<Bytes> {
        request.validate()?;

        let assets = request.assets.clone();
        let amounts = request.amounts.clone();
        let modes = vec![U256::ZERO; assets.len()]; // 0 = no debt, must repay in same tx

        let call = IAaveV2LendingPool::flashLoanCall {
            receiverAddress: self.executor_contract,
            assets,
            amounts,
            modes,
            onBehalfOf: self.executor_contract,
            params: request.callback_data.clone(),
            referralCode: 0,
        };

        Ok(call.abi_encode().into())
    }

    /// Build Aave V3 flashloan calldata
    pub fn build_aave_v3_calldata(&self, request: &FlashloanRequest) -> Result<Bytes> {
        request.validate()?;

        // Use simple flashloan for single asset
        if request.assets.len() == 1 {
            let call = IAaveV3Pool::flashLoanSimpleCall {
                receiverAddress: self.executor_contract,
                asset: request.assets[0],
                amount: request.amounts[0],
                params: request.callback_data.clone(),
                referralCode: 0,
            };

            Ok(call.abi_encode().into())
        } else {
            // Multi-asset flashloan
            let call = IAaveV3Pool::flashLoanCall {
                receiverAddress: self.executor_contract,
                assets: request.assets.clone(),
                amounts: request.amounts.clone(),
                interestRateModes: vec![U256::ZERO; request.assets.len()],
                onBehalfOf: self.executor_contract,
                params: request.callback_data.clone(),
                referralCode: 0,
            };

            Ok(call.abi_encode().into())
        }
    }

    /// Build Uniswap V3 flash calldata
    pub fn build_uniswap_v3_flash_calldata(
        &self,
        pool: Address,
        amount0: U256,
        amount1: U256,
        data: Bytes,
    ) -> Result<Bytes> {
        let call = IUniswapV3Pool::flashCall {
            recipient: self.executor_contract,
            amount0,
            amount1,
            data,
        };

        Ok(call.abi_encode().into())
    }

    /// Build Balancer flashloan calldata
    pub fn build_balancer_calldata(&self, request: &FlashloanRequest) -> Result<Bytes> {
        request.validate()?;

        let call = IBalancerVault::flashLoanCall {
            recipient: self.executor_contract,
            tokens: request.assets.clone(),
            amounts: request.amounts.clone(),
            userData: request.callback_data.clone(),
        };

        Ok(call.abi_encode().into())
    }

    /// Select optimal provider for a flashloan request
    pub fn select_optimal_provider(
        &self,
        request: &FlashloanRequest,
    ) -> Result<FlashloanProvider> {
        // Multi-asset: prefer Balancer (0% fee) > Aave V3 > Aave V2
        if request.assets.len() > 1 {
            return Ok(FlashloanProvider::Balancer);
        }

        // Single asset: prefer Balancer > Aave V3 > Aave V2 > Uniswap V3
        // In practice, check liquidity availability
        Ok(FlashloanProvider::AaveV3)
    }

    /// Build optimal flashloan calldata
    pub fn build_optimal_flashloan(&self, request: &FlashloanRequest) -> Result<(Address, Bytes)> {
        let provider = self.select_optimal_provider(request)?;

        info!(
            "🏦 Building flashloan: {} on {}, amount: {} wei",
            request.assets.len(),
            provider.name(),
            request.amounts[0]
        );

        let calldata = match provider {
            FlashloanProvider::AaveV2 => self.build_aave_v2_calldata(request)?,
            FlashloanProvider::AaveV3 => self.build_aave_v3_calldata(request)?,
            FlashloanProvider::Balancer => self.build_balancer_calldata(request)?,
            FlashloanProvider::UniswapV3 => {
                return Err(anyhow!("Uniswap V3 flash requires specific pool address"));
            }
        };

        let target = self.addresses.get_address(provider);

        Ok((target, calldata))
    }
}

// ============================================================================
// Flashloan Strategy Integration
// ============================================================================

/// Convert ExecutionPlan to FlashloanRequest
pub fn execution_plan_to_flashloan(plan: &ExecutionPlan) -> Result<Option<FlashloanRequest>> {
    use crate::abstractions::StepType;

    // Check if plan starts with flashloan
    if plan.steps.is_empty() {
        return Ok(None);
    }

    match &plan.steps[0].step_type {
        StepType::FlashLoan { asset, amount } => {
            // Extract arbitrage steps (everything between flashloan and repayment)
            let callback_data = encode_arbitrage_steps(plan)?;

            let request = FlashloanRequest::new(*asset, *amount)
                .with_callback_data(callback_data);

            Ok(Some(request))
        }
        _ => Ok(None),
    }
}

/// Encode arbitrage steps into callback data
fn encode_arbitrage_steps(_plan: &ExecutionPlan) -> Result<Bytes> {
    // TODO: Encode the actual swap steps
    // This would include:
    // - Swap routes
    // - DEX routers
    // - Amounts
    // - Slippage limits

    Ok(Bytes::new())
}

/// Check if execution plan requires flashloan
pub fn requires_flashloan(plan: &ExecutionPlan) -> bool {
    use crate::abstractions::StepType;

    plan.steps.iter().any(|step| {
        matches!(step.step_type, StepType::FlashLoan { .. })
    })
}

/// Estimate profit after flashloan fees
pub fn estimate_profit_after_fees(
    gross_profit: U256,
    loan_amount: U256,
    provider: FlashloanProvider,
) -> U256 {
    let fee_bps = U256::from(provider.fee_bps());
    let fee = loan_amount
        .saturating_mul(fee_bps)
        .saturating_div(U256::from(10000));

    gross_profit.saturating_sub(fee)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flashloan_fee_calculation() {
        let weth = Address::repeat_byte(0x1);
        let amount = U256::from(1_000_000_000_000_000_000u64); // 1 ETH

        let request = FlashloanRequest::new(weth, amount)
            .with_provider(FlashloanProvider::AaveV3);

        let fees = request.calculate_fee().unwrap();
        assert_eq!(fees[0], U256::from(500_000_000_000_000u64)); // 0.0005 ETH (0.05%)

        let repayment = request.calculate_repayment().unwrap();
        assert_eq!(repayment[0], U256::from(1_000_500_000_000_000_000u64));
    }

    #[test]
    fn test_multi_asset_validation() {
        let weth = Address::repeat_byte(0x1);
        let usdc = Address::repeat_byte(0x2);

        let request = FlashloanRequest::new(weth, U256::from(1000))
            .with_asset(usdc, U256::from(2000))
            .with_provider(FlashloanProvider::UniswapV3);

        assert!(request.validate().is_err()); // Uniswap V3 doesn't support multi-asset
    }

    #[test]
    fn test_provider_selection() {
        let executor = FlashloanExecutor::new(Address::repeat_byte(0x99));

        // Single asset should prefer Aave V3
        let single = FlashloanRequest::new(Address::ZERO, U256::from(1000));
        let provider = executor.select_optimal_provider(&single).unwrap();
        assert_eq!(provider, FlashloanProvider::AaveV3);

        // Multi-asset should prefer Balancer
        let multi = single.with_asset(Address::repeat_byte(0x2), U256::from(2000));
        let provider = executor.select_optimal_provider(&multi).unwrap();
        assert_eq!(provider, FlashloanProvider::Balancer);
    }
}
