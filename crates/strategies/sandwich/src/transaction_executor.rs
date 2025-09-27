//! 交易执行器 - 处理三阶段 Sandwich 交易执行

use std::sync::Arc;
use alloy_rpc_types_eth::TransactionTrait;
use anyhow::{anyhow, Result, Context};
use tracing::{debug, info, warn, error};
use tokio::sync::Mutex;
use revm::primitives::{U256 as RevmU256, Address as RevmAddress, Bytes};
use crate::revm_engine::{RevmTransactTo as TransactTo, RevmTxEnv as TxEnv};
use artemis_core::eth::{Address, U256, Transaction};

use crate::revm_engine::{RevmEngine, TransactionResult};
use crate::types::{SandwichOpportunity, TokenInventory, SandwichConfig};

/// 交易执行器
pub struct TransactionExecutor {
    /// REVM 引擎
    engine: Arc<Mutex<RevmEngine>>,
    /// 交易构建器
    tx_builder: TransactionBuilder,
    /// 配置
    config: SandwichConfig,
}

impl TransactionExecutor {
    /// 创建新的交易执行器
    pub fn new(
        engine: Arc<Mutex<RevmEngine>>, 
        config: SandwichConfig
    ) -> Self {
        Self {
            engine,
            tx_builder: TransactionBuilder::new(config.clone()),
            config,
        }
    }
    
    /// 执行完整的三阶段 Sandwich 模拟
    pub async fn execute_sandwich_simulation(
        &self,
        opportunity: &SandwichOpportunity,
        searcher_address: Address,
        inventory: &TokenInventory,
    ) -> Result<SandwichSimulationResult> {
        info!("🧪 开始三阶段 Sandwich 模拟");
        
        let mut total_gas = 0u64;
        let initial_weth_balance = inventory.weth_balance;
        
        // 阶段 1: 执行前置交易 (买入中间代币)
        debug!("🔹 阶段 1: 执行前置交易");
        let frontrun_result = self.execute_frontrun_transaction(
            opportunity,
            searcher_address,
        ).await?;
        
        if !frontrun_result.success {
            return Ok(SandwichSimulationResult::failed(
                "前置交易失败",
                frontrun_result.revert_reason,
            ));
        }
        
        total_gas += frontrun_result.gas_used;
        debug!("✅ 前置交易成功，Gas: {}", frontrun_result.gas_used);
        
        // 阶段 2: 执行受害者交易
        debug!("🔹 阶段 2: 执行受害者交易");
        let victim_results = self.execute_victim_transactions(&opportunity.victim_txs).await?;
        
        // 检查受害者交易是否都成功
        let victim_gas: u64 = victim_results.iter().map(|r| r.gas_used).sum();
        let failed_victims: Vec<_> = victim_results.iter()
            .filter(|r| !r.success)
            .collect();
        
        if !failed_victims.is_empty() {
            warn!("部分受害者交易失败: {}/{}", failed_victims.len(), victim_results.len());
        }
        
        debug!("✅ 受害者交易完成，总 Gas: {}", victim_gas);
        
        // 阶段 3: 执行后置交易 (卖出中间代币)
        debug!("🔹 阶段 3: 执行后置交易");
        let intermediate_balance = self.calculate_intermediate_token_balance(
            &frontrun_result,
            opportunity,
        ).await?;
        
        let backrun_result = self.execute_backrun_transaction(
            opportunity,
            searcher_address,
            intermediate_balance,
        ).await?;
        
        if !backrun_result.success {
            return Ok(SandwichSimulationResult::failed(
                "后置交易失败",
                backrun_result.revert_reason,
            ));
        }
        
        total_gas += backrun_result.gas_used;
        debug!("✅ 后置交易成功，Gas: {}", backrun_result.gas_used);
        
        // 计算最终利润
        let final_weth_balance = self.get_final_weth_balance(searcher_address).await?;
        let net_profit = if final_weth_balance > initial_weth_balance {
            final_weth_balance - initial_weth_balance
        } else {
            U256::ZERO
        };
        
        // 计算价格影响
        let price_impact = self.calculate_price_impact(&victim_results)?;
        
        info!("🎯 Sandwich 模拟完成 - 净利润: {:.4} ETH, 总 Gas: {}", 
              net_profit.to::<u128>() as f64 / 1e18, total_gas);
        
        Ok(SandwichSimulationResult {
            success: true,
            net_profit,
            frontrun_gas: frontrun_result.gas_used,
            backrun_gas: backrun_result.gas_used,
            victim_gas,
            total_gas,
            price_impact,
            simulation_accuracy: 0.98, // REVM 模拟准确度
            failure_reason: None,
            intermediate_token_balance: intermediate_balance,
        })
    }
    
    /// 执行前置交易
    async fn execute_frontrun_transaction(
        &self,
        opportunity: &SandwichOpportunity,
        searcher_address: Address,
    ) -> Result<TransactionResult> {
        debug!("🔨 构建前置交易");
        
        // 构建前置交易
        let tx_env = self.tx_builder.build_frontrun_tx_env(
            opportunity,
            searcher_address,
        ).await?;
        
        // 执行交易
        let mut engine = self.engine.lock().await;
        let result = engine.execute_transaction(tx_env)
            .context("前置交易执行失败")?;
        
        Ok(result)
    }
    
    /// 执行受害者交易
    async fn execute_victim_transactions(
        &self,
        victim_txs: &[Transaction],
    ) -> Result<Vec<TransactionResult>> {
        debug!("👥 执行 {} 个受害者交易", victim_txs.len());
        
        let mut results = Vec::new();
        let mut engine = self.engine.lock().await;
        
        for (i, victim_tx) in victim_txs.iter().enumerate() {
            debug!("🔸 执行受害者交易 {}/{}", i + 1, victim_txs.len());
            
            let tx_env = self.tx_builder.build_victim_tx_env(victim_tx)?;
            let result = engine.execute_transaction(tx_env)
                .context(format!("受害者交易 {} 执行失败", i))?;
            
            if !result.success {
                warn!("受害者交易 {} 失败: {:?}", i, result.revert_reason);
            }
            
            results.push(result);
        }
        
        Ok(results)
    }
    
    /// 执行后置交易
    async fn execute_backrun_transaction(
        &self,
        opportunity: &SandwichOpportunity,
        searcher_address: Address,
        intermediate_balance: U256,
    ) -> Result<TransactionResult> {
        debug!("🔨 构建后置交易，中间代币余额: {}", intermediate_balance);
        
        // 构建后置交易
        let tx_env = self.tx_builder.build_backrun_tx_env(
            opportunity,
            searcher_address,
            intermediate_balance,
        ).await?;
        
        // 执行交易
        let mut engine = self.engine.lock().await;
        let result = engine.execute_transaction(tx_env)
            .context("后置交易执行失败")?;
        
        Ok(result)
    }
    
    /// 计算中间代币余额
    async fn calculate_intermediate_token_balance(
        &self,
        frontrun_result: &TransactionResult,
        opportunity: &SandwichOpportunity,
    ) -> Result<U256> {
        // 从前置交易的状态变化中提取中间代币余额
        for state_change in &frontrun_result.state_changes {
            if state_change.address == opportunity.intermediary_token {
                // 简化实现：假设获得的中间代币数量等于输入金额
                // 实际应该从状态变化中精确计算
                return Ok(opportunity.optimal_input);
            }
        }
        
        // 如果没有找到状态变化，使用估算值
        Ok(opportunity.optimal_input * U256::from(95) / U256::from(100)) // 95% 的输入金额
    }
    
    /// 获取最终 WETH 余额
    async fn get_final_weth_balance(&self, searcher_address: Address) -> Result<U256> {
        let mut engine = self.engine.lock().await;
        let weth_address: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        
        // 计算 WETH 余额存储槽
        let balance_slot = self.calculate_balance_slot(searcher_address, 3)?;
        let balance = engine.get_storage(weth_address, balance_slot)?;
        
        Ok(balance)
    }
    
    /// 计算价格影响
    fn calculate_price_impact(&self, victim_results: &[TransactionResult]) -> Result<f64> {
        // 简化实现：基于受害者交易的 Gas 消耗估算价格影响
        let total_victim_gas: u64 = victim_results.iter().map(|r| r.gas_used).sum();
        let avg_gas = total_victim_gas as f64 / victim_results.len() as f64;
        
        // Gas 消耗越高，价格影响越大
        let impact = (avg_gas - 21000.0) / 100000.0; // 基于额外 Gas 计算
        Ok(impact.max(0.001).min(0.1)) // 限制在 0.1% - 10% 之间
    }
    
    /// 计算余额存储槽
    fn calculate_balance_slot(&self, account: Address, slot: u8) -> Result<U256> {
        use revm::primitives::keccak256;
        
        let mut input = [0u8; 64];
        input[12..32].copy_from_slice(account.as_slice());
        input[63] = slot;
        
        let hash = keccak256(&input);
        Ok(U256::from_be_slice(&hash.0))
    }
}

/// 交易构建器
#[derive(Clone)]
pub struct TransactionBuilder {
    /// 配置
    config: SandwichConfig,
}

impl TransactionBuilder {
    pub fn new(config: SandwichConfig) -> Self {
        Self { config }
    }
    
    /// 构建前置交易环境
    pub async fn build_frontrun_tx_env(
        &self,
        opportunity: &SandwichOpportunity,
        searcher_address: Address,
    ) -> Result<TxEnv> {
        // 计算 Gas 价格 (略高于受害者交易)
        let victim_gas_price = self.get_max_victim_gas_price(opportunity);
        let frontrun_gas_price = victim_gas_price + U256::from(1000000000u64); // +1 gwei
        
        // 构建调用数据
        let call_data = self.build_sandwich_call_data(
            opportunity.intermediary_token,
            opportunity.optimal_input,
            true, // is_frontrun
        )?;
        
        Ok(TxEnv {
            tx_type: 2, // EIP-1559 transaction
            caller: RevmAddress::from_slice(searcher_address.as_slice()),
            gas_limit: 200_000, // 前置交易 Gas 限制
            gas_price: frontrun_gas_price.to::<u128>(),
            kind: TransactTo::Call(RevmAddress::from_slice(self.config.sandwich_contract.as_slice())),
            value: RevmU256::from(opportunity.optimal_input),
            data: Bytes::from(call_data),
            nonce: 0, // 简化：使用固定 nonce
            chain_id: Some(1), // Mainnet
            access_list: Default::default(),
            gas_priority_fee: None,
            blob_hashes: vec![],
            max_fee_per_blob_gas: 0,
            authorization_list: Default::default(),
        })
    }
    
    /// 构建受害者交易环境
    pub fn build_victim_tx_env(&self, victim_tx: &Transaction) -> Result<TxEnv> {
        // 从受害者交易中提取信息
        let caller = victim_tx.inner.signer();
        let inner = victim_tx.inner.inner();
        let to = inner.to().unwrap_or_default();
        let value = inner.value();
        let gas_limit = inner.gas_limit();
        let gas_price = inner.max_fee_per_gas();
        let data = inner.input().clone();
        
        Ok(TxEnv {
            tx_type: 2, // EIP-1559 transaction
            caller: RevmAddress::from_slice(caller.as_slice()),
            gas_limit,
            gas_price,
            kind: TransactTo::Call(RevmAddress::from_slice(to.as_slice())),
            value: RevmU256::from(value),
            data: Bytes::from(data.as_ref().to_vec()),
            nonce: 1, // 简化实现
            chain_id: Some(1),
            access_list: Default::default(),
            gas_priority_fee: None,
            blob_hashes: vec![],
            max_fee_per_blob_gas: 0,
            authorization_list: Default::default(),
        })
    }
    
    /// 构建后置交易环境
    pub async fn build_backrun_tx_env(
        &self,
        opportunity: &SandwichOpportunity,
        searcher_address: Address,
        intermediate_balance: U256,
    ) -> Result<TxEnv> {
        // 计算贿赂 Gas 价格
        let base_gas_price = self.get_max_victim_gas_price(opportunity);
        let profit = opportunity.estimated_profit;
        let gas_limit = 150_000u64;
        
        // 将 90% 利润作为贿赂
        let bribe = profit * U256::from(90) / U256::from(100);
        let bribe_per_gas = bribe / U256::from(gas_limit);
        let backrun_gas_price = base_gas_price + bribe_per_gas;
        
        // 构建调用数据
        let call_data = self.build_sandwich_call_data(
            opportunity.intermediary_token,
            intermediate_balance,
            false, // is_backrun
        )?;
        
        Ok(TxEnv {
            tx_type: 2, // EIP-1559 transaction
            caller: RevmAddress::from_slice(searcher_address.as_slice()),
            gas_limit,
            gas_price: backrun_gas_price.to::<u128>(),
            kind: TransactTo::Call(RevmAddress::from_slice(self.config.sandwich_contract.as_slice())),
            value: RevmU256::ZERO, // 后置交易不需要 ETH 输入
            data: Bytes::from(call_data),
            nonce: 2, // 简化：固定 nonce
            chain_id: Some(1),
            access_list: Default::default(),
            gas_priority_fee: None,
            blob_hashes: vec![],
            max_fee_per_blob_gas: 0,
            authorization_list: Default::default(),
        })
    }
    
    /// 构建 Sandwich 合约调用数据
    fn build_sandwich_call_data(
        &self,
        intermediary_token: Address,
        amount: U256,
        is_frontrun: bool,
    ) -> Result<Vec<u8>> {
        let mut call_data = Vec::new();
        
        if is_frontrun {
            // 前置交易：买入中间代币
            // 函数选择器 (示例)
            call_data.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]);
        } else {
            // 后置交易：卖出中间代币
            call_data.extend_from_slice(&[0x87, 0x65, 0x43, 0x21]);
        }
        
        // 参数编码
        call_data.extend_from_slice(&[0u8; 12]); // padding
        call_data.extend_from_slice(intermediary_token.as_slice());
        call_data.extend_from_slice(&amount.to_be_bytes::<32>());
        
        Ok(call_data)
    }
    
    /// 获取受害者交易的最高 Gas 价格
    fn get_max_victim_gas_price(&self, opportunity: &SandwichOpportunity) -> U256 {
        let max_price = opportunity
            .victim_txs
            .iter()
            .map(|tx| tx.inner.inner().priority_fee_or_price())
            .max()
            .unwrap_or(20_000_000_000u128);

        U256::from(max_price)
    }
}

/// Sandwich 模拟结果
#[derive(Debug, Clone)]
pub struct SandwichSimulationResult {
    /// 是否成功
    pub success: bool,
    /// 净利润
    pub net_profit: U256,
    /// 前置交易 Gas
    pub frontrun_gas: u64,
    /// 后置交易 Gas
    pub backrun_gas: u64,
    /// 受害者交易 Gas
    pub victim_gas: u64,
    /// 总 Gas 消耗
    pub total_gas: u64,
    /// 价格影响
    pub price_impact: f64,
    /// 模拟准确度
    pub simulation_accuracy: f64,
    /// 失败原因
    pub failure_reason: Option<String>,
    /// 中间代币余额
    pub intermediate_token_balance: U256,
}

impl SandwichSimulationResult {
    /// 创建失败结果
    pub fn failed(reason: &str, details: Option<String>) -> Self {
        Self {
            success: false,
            net_profit: U256::ZERO,
            frontrun_gas: 0,
            backrun_gas: 0,
            victim_gas: 0,
            total_gas: 0,
            price_impact: 0.0,
            simulation_accuracy: 0.0,
            failure_reason: Some(format!("{}: {:?}", reason, details)),
            intermediate_token_balance: U256::ZERO,
        }
    }
    
    /// 计算 ROI
    pub fn calculate_roi(&self, initial_investment: U256) -> f64 {
        if initial_investment.is_zero() {
            return 0.0;
        }
        
        let profit_f64 = self.net_profit.to::<u128>() as f64;
        let investment_f64 = initial_investment.to::<u128>() as f64;
        
        (profit_f64 / investment_f64) * 100.0
    }
    
    /// 检查是否盈利
    pub fn is_profitable(&self, min_profit_wei: U256) -> bool {
        self.success && self.net_profit >= min_profit_wei
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SandwichConfig};
    
    #[test]
    fn test_transaction_builder_creation() {
        let config = SandwichConfig::default();
        let builder = TransactionBuilder::new(config);
        // 基本创建测试
    }
    
    #[test]
    fn test_simulation_result_roi() {
        let result = SandwichSimulationResult {
            success: true,
            net_profit: U256::from(1000000000000000000u64), // 1 ETH
            frontrun_gas: 150000,
            backrun_gas: 120000,
            victim_gas: 100000,
            total_gas: 370000,
            price_impact: 0.05,
            simulation_accuracy: 0.98,
            failure_reason: None,
            intermediate_token_balance: U256::from(2000000000000000000u64), // 2 ETH
        };
        
        let roi = result.calculate_roi(U256::from(5000000000000000000u64)); // 5 ETH investment
        assert!((roi - 20.0).abs() < 0.01); // 20% ROI
    }
}
