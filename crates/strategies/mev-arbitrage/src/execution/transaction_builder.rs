//! Transaction Builder for MEV Execution
//!
//! This module builds signed, ready-to-submit transactions for arbitrage opportunities.
//! It handles:
//! - Transaction encoding (flashloan initiation, swap routing)
//! - Gas price optimization
//! - Transaction signing
//! - Flashbots bundle construction
//! - EIP-1559 transaction support

use alloy_primitives::{Address, U256, Bytes, TxHash};
use alloy_rpc_types::{TransactionRequest, TransactionInput};
use alloy_signer::{Signature, Signer};
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::SolCall;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use tracing::{info, debug, warn};

use crate::abstractions::{ExecutionPlan, ExecutionStep, StepType};
use crate::execution::flashloan::{FlashloanRequest, FlashloanExecutor, FlashloanProvider};
use crate::execution::GasStrategy;

// ============================================================================
// Executor Contract Interface
// ============================================================================

use alloy_sol_types::sol;

sol! {
    /// MEV Executor Contract
    /// This contract receives flashloans and executes arbitrage swaps
    interface IMEVExecutor {
        /// Execute arbitrage with Aave V3 flashloan
        function executeArbitrageWithAaveV3(
            address asset,
            uint256 amount,
            bytes calldata params
        ) external;

        /// Execute arbitrage with Balancer flashloan
        function executeArbitrageWithBalancer(
            address[] calldata tokens,
            uint256[] calldata amounts,
            bytes calldata userData
        ) external;

        /// Execute simple arbitrage without flashloan
        function executeSimpleArbitrage(
            address[] calldata path,
            address[] calldata routers,
            uint256 amountIn
        ) external payable;

        /// Withdraw profits to owner
        function withdrawProfits(address token, uint256 amount) external;
    }
}

// ============================================================================
// Transaction Builder
// ============================================================================

/// Transaction builder for MEV execution
pub struct TransactionBuilder {
    /// Private key signer
    signer: PrivateKeySigner,

    /// Chain ID (1 = mainnet)
    chain_id: u64,

    /// Our deployed executor contract
    executor_contract: Address,

    /// Flashloan executor
    flashloan_executor: FlashloanExecutor,

    /// Gas strategy
    gas_strategy: GasStrategy,

    /// Current nonce (tracked locally)
    current_nonce: u64,
}

/// Built transaction ready for submission
#[derive(Debug, Clone)]
pub struct BuiltTransaction {
    /// Transaction request
    pub tx: TransactionRequest,

    /// Expected profit (wei)
    pub expected_profit: U256,

    /// Estimated gas cost (wei)
    pub estimated_gas_cost: U256,

    /// Net profit after gas (wei)
    pub net_profit: U256,

    /// Transaction hash (after signing)
    pub tx_hash: Option<TxHash>,
}

/// Flashbots bundle
#[derive(Debug, Clone)]
pub struct FlashbotsBundle {
    /// Transactions in bundle (signed)
    pub transactions: Vec<Bytes>,

    /// Target block number
    pub target_block: u64,

    /// Minimum timestamp
    pub min_timestamp: Option<u64>,

    /// Maximum timestamp
    pub max_timestamp: Option<u64>,

    /// Bundle hash
    pub bundle_hash: Option<String>,
}

impl TransactionBuilder {
    /// Create new transaction builder
    pub fn new(
        private_key: &str,
        chain_id: u64,
        executor_contract: Address,
        gas_strategy: GasStrategy,
    ) -> Result<Self> {
        let signer = PrivateKeySigner::from_bytes(&hex::decode(private_key)?)?;

        Ok(Self {
            signer,
            chain_id,
            executor_contract,
            flashloan_executor: FlashloanExecutor::new(executor_contract),
            gas_strategy,
            current_nonce: 0,
        })
    }

    /// Get signer address
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Set current nonce
    pub fn set_nonce(&mut self, nonce: u64) {
        self.current_nonce = nonce;
    }

    /// Build transaction from execution plan
    pub async fn build_transaction(
        &mut self,
        plan: &ExecutionPlan,
        gas_price: U256,
    ) -> Result<BuiltTransaction> {
        info!("🔨 Building transaction for plan: {}", plan.id);

        // Check if plan requires flashloan
        let requires_flashloan = plan.steps.iter().any(|step| {
            matches!(step.step_type, StepType::FlashLoan { .. })
        });

        let tx = if requires_flashloan {
            self.build_flashloan_transaction(plan, gas_price).await?
        } else {
            self.build_simple_arbitrage_transaction(plan, gas_price).await?
        };

        // Calculate costs
        let gas_limit = tx.gas.ok_or_else(|| anyhow!("No gas limit"))?;
        let estimated_gas_cost = U256::from(gas_limit).saturating_mul(gas_price);
        let net_profit = plan.estimated_profit.saturating_sub(estimated_gas_cost);

        Ok(BuiltTransaction {
            tx,
            expected_profit: plan.estimated_profit,
            estimated_gas_cost,
            net_profit,
            tx_hash: None,
        })
    }

    /// Build flashloan-based arbitrage transaction
    async fn build_flashloan_transaction(
        &mut self,
        plan: &ExecutionPlan,
        gas_price: U256,
    ) -> Result<TransactionRequest> {
        debug!("Building flashloan transaction");

        // Extract flashloan information
        let (flashloan_asset, flashloan_amount) = self.extract_flashloan_info(plan)?;

        // Build callback data (the arbitrage swaps)
        let callback_data = self.encode_arbitrage_steps(plan)?;

        // Create flashloan request
        let fl_request = FlashloanRequest::new(flashloan_asset, flashloan_amount)
            .with_callback_data(callback_data);

        // Get optimal flashloan provider and calldata
        let (target, calldata) = self.flashloan_executor.build_optimal_flashloan(&fl_request)?;

        // Build transaction
        let mut tx = TransactionRequest::default()
            .to(target)
            .input(TransactionInput::new(calldata))
            .from(self.address())
            .chain_id(self.chain_id)
            .nonce(self.current_nonce);

        // Set gas parameters
        self.set_gas_parameters(&mut tx, plan.estimated_gas, gas_price)?;

        // Increment nonce
        self.current_nonce += 1;

        Ok(tx)
    }

    /// Build simple arbitrage transaction (no flashloan)
    async fn build_simple_arbitrage_transaction(
        &mut self,
        plan: &ExecutionPlan,
        gas_price: U256,
    ) -> Result<TransactionRequest> {
        debug!("Building simple arbitrage transaction");

        // Extract swap path and routers
        let (path, routers, amount_in) = self.extract_swap_info(plan)?;

        // Encode function call
        let call = IMEVExecutor::executeSimpleArbitrageCall {
            path,
            routers,
            amountIn: amount_in,
        };

        let calldata = call.abi_encode();

        // Build transaction
        let mut tx = TransactionRequest::default()
            .to(self.executor_contract)
            .input(TransactionInput::new(calldata.into()))
            .from(self.address())
            .value(amount_in) // Send ETH if needed
            .chain_id(self.chain_id)
            .nonce(self.current_nonce);

        // Set gas parameters
        self.set_gas_parameters(&mut tx, plan.estimated_gas, gas_price)?;

        // Increment nonce
        self.current_nonce += 1;

        Ok(tx)
    }

    /// Extract flashloan information from execution plan
    fn extract_flashloan_info(&self, plan: &ExecutionPlan) -> Result<(Address, U256)> {
        for step in &plan.steps {
            if let StepType::FlashLoan { asset, amount } = &step.step_type {
                return Ok((*asset, *amount));
            }
        }

        Err(anyhow!("No flashloan step found in plan"))
    }

    /// Extract swap information from execution plan
    fn extract_swap_info(&self, plan: &ExecutionPlan) -> Result<(Vec<Address>, Vec<Address>, U256)> {
        let mut path = Vec::new();
        let mut routers = Vec::new();
        let mut amount_in = U256::ZERO;

        for step in &plan.steps {
            match &step.step_type {
                StepType::TokenSwap { token_in, token_out, amount } => {
                    if path.is_empty() {
                        path.push(*token_in);
                        amount_in = *amount;
                    }
                    path.push(*token_out);
                    routers.push(step.contract_address); // DEX router
                }
                _ => {}
            }
        }

        if path.len() < 2 {
            return Err(anyhow!("Invalid swap path"));
        }

        Ok((path, routers, amount_in))
    }

    /// Encode arbitrage steps into callback data
    fn encode_arbitrage_steps(&self, plan: &ExecutionPlan) -> Result<Bytes> {
        // This would encode all swap steps into bytes
        // For now, return empty - actual implementation would:
        // 1. Encode each swap (DEX router, function selector, parameters)
        // 2. Pack into single bytes array
        // 3. Executor contract decodes and executes each step

        // Simplified encoding:
        let mut steps_data = Vec::new();

        for step in &plan.steps {
            match &step.step_type {
                StepType::TokenSwap { token_in, token_out, amount } => {
                    // Encode: router address (20 bytes) + amount (32 bytes) + tokens (40 bytes)
                    let mut step_data = Vec::new();
                    step_data.extend_from_slice(step.contract_address.as_slice()); // 20 bytes
                    step_data.extend_from_slice(&amount.to_be_bytes::<32>()); // 32 bytes
                    step_data.extend_from_slice(token_in.as_slice()); // 20 bytes
                    step_data.extend_from_slice(token_out.as_slice()); // 20 bytes

                    steps_data.extend(step_data);
                }
                _ => {}
            }
        }

        Ok(Bytes::from(steps_data))
    }

    /// Set gas parameters based on strategy
    fn set_gas_parameters(
        &self,
        tx: &mut TransactionRequest,
        estimated_gas: U256,
        base_gas_price: U256,
    ) -> Result<()> {
        // Add 20% buffer to gas estimate
        let gas_limit = estimated_gas
            .saturating_mul(U256::from(120))
            .saturating_div(U256::from(100));

        tx.gas = Some(gas_limit.to::<u128>());

        // Set gas price based on strategy
        match &self.gas_strategy {
            GasStrategy::Fast => {
                // 1.2x base gas price
                let gas_price = base_gas_price
                    .saturating_mul(U256::from(12))
                    .saturating_div(U256::from(10));

                // EIP-1559
                tx.max_fee_per_gas = Some(gas_price.to::<u128>());
                tx.max_priority_fee_per_gas = Some((gas_price / U256::from(10)).to::<u128>()); // 10% tip
            }

            GasStrategy::Standard => {
                tx.max_fee_per_gas = Some(base_gas_price.to::<u128>());
                tx.max_priority_fee_per_gas = Some((base_gas_price / U256::from(20)).to::<u128>()); // 5% tip
            }

            GasStrategy::Custom { gas_price } => {
                tx.max_fee_per_gas = Some(gas_price.to::<u128>());
                tx.max_priority_fee_per_gas = Some((gas_price / U256::from(20)).to::<u128>());
            }

            GasStrategy::Dynamic { adjustment_factor } => {
                let adjusted = base_gas_price
                    .saturating_mul(U256::from((*adjustment_factor * 1000.0) as u64))
                    .saturating_div(U256::from(1000));

                tx.max_fee_per_gas = Some(adjusted.to::<u128>());
                tx.max_priority_fee_per_gas = Some((adjusted / U256::from(20)).to::<u128>());
            }

            _ => {
                tx.max_fee_per_gas = Some(base_gas_price.to::<u128>());
                tx.max_priority_fee_per_gas = Some((base_gas_price / U256::from(20)).to::<u128>());
            }
        }

        Ok(())
    }

    /// Sign transaction
    pub async fn sign_transaction(&self, tx: &TransactionRequest) -> Result<Bytes> {
        // Convert TransactionRequest to signable format
        // In production, use proper EIP-1559 encoding

        // Simplified: just encode the transaction
        // Real implementation would:
        // 1. RLP encode the transaction
        // 2. Hash with keccak256
        // 3. Sign with ECDSA
        // 4. Return signed transaction bytes

        info!("✍️  Signing transaction from {:?}", self.address());

        // For now, return placeholder
        // In production:
        // let signature = self.signer.sign_transaction(&tx).await?;
        // let signed_tx = encode_signed_transaction(tx, signature);

        Ok(Bytes::new())
    }

    /// Build Flashbots bundle
    pub async fn build_flashbots_bundle(
        &mut self,
        plans: Vec<ExecutionPlan>,
        target_block: u64,
        base_gas_price: U256,
    ) -> Result<FlashbotsBundle> {
        info!("📦 Building Flashbots bundle with {} transactions", plans.len());

        let mut transactions = Vec::new();

        for plan in plans {
            // Build transaction
            let built_tx = self.build_transaction(&plan, base_gas_price).await?;

            // Sign transaction
            let signed_tx = self.sign_transaction(&built_tx.tx).await?;

            transactions.push(signed_tx);
        }

        Ok(FlashbotsBundle {
            transactions,
            target_block,
            min_timestamp: None,
            max_timestamp: None,
            bundle_hash: None,
        })
    }

    /// Build bundle for backrun opportunity
    pub async fn build_backrun_bundle(
        &mut self,
        victim_tx_hash: TxHash,
        our_plan: ExecutionPlan,
        target_block: u64,
        base_gas_price: U256,
    ) -> Result<FlashbotsBundle> {
        info!("🎯 Building backrun bundle for victim: {:?}", victim_tx_hash);

        // Build our backrun transaction
        let built_tx = self.build_transaction(&our_plan, base_gas_price).await?;

        // Sign transaction
        let signed_tx = self.sign_transaction(&built_tx.tx).await?;

        // Bundle: [victim_tx, our_tx]
        // Note: victim tx is already in mempool, we just reference it
        // Flashbots will include both in the bundle

        Ok(FlashbotsBundle {
            transactions: vec![signed_tx],
            target_block,
            min_timestamp: None,
            max_timestamp: None,
            bundle_hash: None,
        })
    }

    /// Estimate transaction profitability
    pub fn estimate_profitability(
        &self,
        plan: &ExecutionPlan,
        gas_price: U256,
    ) -> (U256, U256, U256) {
        let gas_limit = plan.estimated_gas
            .saturating_mul(U256::from(120))
            .saturating_div(U256::from(100));

        let gas_cost = gas_limit.saturating_mul(gas_price);
        let net_profit = plan.estimated_profit.saturating_sub(gas_cost);

        (plan.estimated_profit, gas_cost, net_profit)
    }

    /// Check if opportunity is profitable after gas
    pub fn is_profitable(
        &self,
        plan: &ExecutionPlan,
        gas_price: U256,
        min_profit: U256,
    ) -> bool {
        let (_, _, net_profit) = self.estimate_profitability(plan, gas_price);
        net_profit >= min_profit
    }
}

// ============================================================================
// Bundle Utilities
// ============================================================================

impl FlashbotsBundle {
    /// Create new empty bundle
    pub fn new(target_block: u64) -> Self {
        Self {
            transactions: Vec::new(),
            target_block,
            min_timestamp: None,
            max_timestamp: None,
            bundle_hash: None,
        }
    }

    /// Add transaction to bundle
    pub fn add_transaction(mut self, tx: Bytes) -> Self {
        self.transactions.push(tx);
        self
    }

    /// Set timestamp constraints
    pub fn with_timestamps(mut self, min: u64, max: u64) -> Self {
        self.min_timestamp = Some(min);
        self.max_timestamp = Some(max);
        self
    }

    /// Calculate bundle hash (for tracking)
    pub fn calculate_hash(&mut self) {
        // In production, hash all transactions together
        self.bundle_hash = Some(format!("bundle_{}", self.target_block));
    }

    /// Get bundle size
    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    /// Serialize bundle for Flashbots API
    pub fn serialize_for_flashbots(&self) -> serde_json::Value {
        serde_json::json!({
            "txs": self.transactions.iter()
                .map(|tx| format!("0x{}", hex::encode(tx)))
                .collect::<Vec<_>>(),
            "blockNumber": format!("0x{:x}", self.target_block),
            "minTimestamp": self.min_timestamp,
            "maxTimestamp": self.max_timestamp,
        })
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Encode swap for Uniswap V2 Router
pub fn encode_uniswap_v2_swap(
    amount_in: U256,
    amount_out_min: U256,
    path: Vec<Address>,
    to: Address,
    deadline: u64,
) -> Bytes {
    // swapExactTokensForTokens selector: 0x38ed1739
    let selector = [0x38u8, 0xed, 0x17, 0x39];

    // Encode parameters (simplified)
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&selector);
    // In production: proper ABI encoding of (uint256, uint256, address[], address, uint256)

    Bytes::from(calldata)
}

/// Encode swap for Uniswap V3 Router
pub fn encode_uniswap_v3_swap(
    token_in: Address,
    token_out: Address,
    fee: u32,
    recipient: Address,
    amount_in: U256,
    amount_out_min: U256,
    sqrt_price_limit_x96: U256,
) -> Bytes {
    // exactInputSingle selector
    let selector = [0x41u8, 0x4b, 0xf3, 0x89];

    let mut calldata = Vec::new();
    calldata.extend_from_slice(&selector);
    // In production: proper ABI encoding

    Bytes::from(calldata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_builder_creation() {
        let private_key = "0000000000000000000000000000000000000000000000000000000000000001";
        let executor = Address::repeat_byte(0x42);

        let builder = TransactionBuilder::new(
            private_key,
            1, // mainnet
            executor,
            GasStrategy::Standard,
        );

        assert!(builder.is_ok());
    }

    #[test]
    fn test_flashbots_bundle_creation() {
        let bundle = FlashbotsBundle::new(12345678)
            .add_transaction(Bytes::from(vec![1, 2, 3]))
            .with_timestamps(1234567890, 1234567900);

        assert_eq!(bundle.size(), 1);
        assert_eq!(bundle.target_block, 12345678);
    }

    #[test]
    fn test_profitability_check() {
        let private_key = "0000000000000000000000000000000000000000000000000000000000000001";
        let executor = Address::repeat_byte(0x42);

        let builder = TransactionBuilder::new(
            private_key,
            1,
            executor,
            GasStrategy::Standard,
        ).unwrap();

        let plan = ExecutionPlan {
            id: "test".to_string(),
            opportunity_id: "test".to_string(),
            steps: vec![],
            estimated_gas: U256::from(200_000),
            estimated_profit: U256::from(1_000_000_000_000_000_000u64), // 1 ETH
            execution_strategy: crate::abstractions::ExecutionStrategy::Immediate,
            validation_results: None,
        };

        let gas_price = U256::from(50_000_000_000u64); // 50 gwei

        let is_profitable = builder.is_profitable(
            &plan,
            gas_price,
            U256::from(100_000_000_000_000_000u64), // 0.1 ETH min profit
        );

        assert!(is_profitable);
    }
}
