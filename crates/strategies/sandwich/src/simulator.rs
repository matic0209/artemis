use std::sync::Arc;
use anyhow::{anyhow, Result};
use tracing::{debug, info, warn};
use tokio::sync::Mutex;

use artemis_core::eth::{Provider, Address, U256};
use crate::types::{SandwichOpportunity, BlockInfo, TokenInventory, SandwichConfig};
use crate::revm_engine::{RevmEngine, RevmConfig};
use crate::transaction_executor::{TransactionExecutor, SandwichSimulationResult};
use alloy_consensus::transaction::Transaction as TransactionTrait;
use alloy_eips::eip2718::Encodable2718;

/// 高性能 Sandwich 模拟器 (集成 REVM)
pub struct SandwichSimulator {
    provider: Arc<Provider>,
    /// REVM 引擎
    revm_engine: Option<Arc<Mutex<RevmEngine>>>,
    /// 交易执行器
    transaction_executor: Option<TransactionExecutor>,
    /// 配置
    config: SandwichConfig,
}

impl SandwichSimulator {
    pub fn new(provider: Arc<Provider>, config: SandwichConfig) -> Self {
        Self { 
            provider,
            revm_engine: None,
            transaction_executor: None,
            config,
        }
    }

    pub async fn initialize(&mut self, block: &BlockInfo) -> Result<()> {
        info!("🧪 初始化 Sandwich 模拟器 - 区块: {}", block.number);
        
        // 创建 REVM 引擎
        let revm_config = RevmConfig::default();
        let mut revm_engine = RevmEngine::new(block, Some(revm_config))?;
        
        // 从链上同步状态
        revm_engine.sync_from_chain(self.provider.clone()).await?;
        
        let engine_arc = Arc::new(Mutex::new(revm_engine));
        
        // 创建交易执行器
        let transaction_executor = TransactionExecutor::new(
            engine_arc.clone(),
            self.config.clone(),
        );
        
        self.revm_engine = Some(engine_arc);
        self.transaction_executor = Some(transaction_executor);
        
        info!("✅ Sandwich 模拟器初始化完成");
        Ok(())
    }

    /// 模拟 sandwich 攻击，返回利润和最优输入
    pub async fn simulate_sandwich(
        &self,
        opportunity: &SandwichOpportunity,
        block: &BlockInfo,
    ) -> Result<(U256, U256)> {
        debug!("🧪 模拟 Sandwich 机会");
        
        // TODO: 完整的 revm 模拟实现
        // 1. 设置区块环境
        // 2. 创建前置交易（买入中间代币）
        // 3. 执行受害者交易
        // 4. 创建后置交易（卖出中间代币）
        // 5. 计算净利润

        // 暂时返回估算值
        let estimated_profit = self.estimate_profit_heuristic(opportunity, block).await?;
        let optimal_input = self.calculate_optimal_input(opportunity, block).await?;

        Ok((estimated_profit, optimal_input))
    }

    /// 启发式利润估算（快速）
    async fn estimate_profit_heuristic(
        &self,
        opportunity: &SandwichOpportunity,
        block: &BlockInfo,
    ) -> Result<U256> {
        // 基于交易价值和池子流动性的快速估算
        let victim_value = opportunity.victim_txs
            .iter()
            .map(|tx| tx.inner.value())
            .map(|val| val.to::<u128>())
            .sum::<u128>();

        // 估算滑点影响
        let estimated_slippage = victim_value as f64 / 1e18 * 0.003; // 0.3% 滑点
        let estimated_profit_eth = estimated_slippage * 0.5; // 捕获 50% 的滑点

        // 转换为 wei
        let profit_wei = (estimated_profit_eth * 1e18) as u128;
        
        Ok(U256::from(profit_wei))
    }

    /// 计算最优输入金额
    async fn calculate_optimal_input(
        &self,
        opportunity: &SandwichOpportunity,
        _block: &BlockInfo,
    ) -> Result<U256> {
        // 基于受害者交易价值计算最优输入
        let victim_value = opportunity.victim_txs
            .iter()
            .map(|tx| tx.inner.value())
            .map(|val| val.to::<u128>())
            .sum::<u128>();

        // 最优输入通常是受害者交易价值的 2-5 倍
        let optimal_input = victim_value * 3;
        
        Ok(U256::from(optimal_input))
    }

    /// 详细的 REVM 模拟（精确但较慢）
    pub async fn simulate_detailed(
        &self,
        opportunity: &SandwichOpportunity,
        block: &BlockInfo,
        inventory: &TokenInventory,
    ) -> Result<SandwichSimulationResult> {
        debug!("🧪 开始详细 REVM 模拟");
        
        // 检查是否已初始化
        let executor = self.transaction_executor.as_ref()
            .ok_or_else(|| anyhow!("模拟器未初始化，请先调用 initialize()"))?;
        
        // 设置搜索者账户初始状态
        if let Some(engine) = &self.revm_engine {
            let mut engine_guard = engine.lock().await;
            engine_guard.setup_searcher_account(
                inventory.searcher_address,
                inventory.weth_balance,
            )?;
        }
        
        // 执行完整的三阶段模拟
        let result = executor.execute_sandwich_simulation(
            opportunity,
            inventory.searcher_address,
            inventory,
        ).await?;
        
        debug!("🧪 REVM 模拟完成 - 成功: {}, 净利润: {:.4} ETH", 
               result.success, 
               result.net_profit.to::<u128>() as f64 / 1e18);
        
        Ok(result)
    }

    /// 快速盈利性检查（不使用 REVM）
    pub async fn quick_profitability_check(
        &self,
        opportunity: &SandwichOpportunity,
        block: &BlockInfo,
    ) -> Result<bool> {
        debug!("⚡ 快速盈利性检查");
        
        // 基于启发式算法快速评估
        let estimated_profit = self.estimate_profit_heuristic(opportunity, block).await?;
        let gas_cost = self.estimate_gas_cost(block).await?;
        
        let is_profitable = estimated_profit > gas_cost;
        debug!("💰 预估利润: {:.4} ETH, Gas成本: {:.4} ETH, 盈利: {}", 
               estimated_profit.to::<u128>() as f64 / 1e18,
               gas_cost.to::<u128>() as f64 / 1e18,
               is_profitable);
        
        Ok(is_profitable)
    }
    
    /// 估算 Gas 成本
    async fn estimate_gas_cost(&self, block: &BlockInfo) -> Result<U256> {
        // 估算三阶段交易的总 Gas 成本
        let total_gas = 150_000 + 100_000 + 120_000; // 前置 + 受害者 + 后置
        let gas_price = block.base_fee_per_gas * U256::from(12) / U256::from(10); // 1.2x base fee
        
        Ok(U256::from(total_gas) * gas_price)
    }

    /// 检查代币是否安全（Salmonella 检查）
    pub async fn check_token_safety(&self, token: Address) -> Result<bool> {
        // 实现 Salmonella 检查 - 检查代币的 transfer 函数是否安全
        debug!("🦠 检查代币安全性: {:?}", token);
        
        use alloy_provider::Provider as ProviderTrait;
        use crate::contracts::ERC20;
        
        // 1. 检查代币是否是合约
        let code = self.provider.get_code_at(token).await?;
        if code.is_empty() {
            return Ok(false); // 不是合约
        }
        
        // 2. 尝试调用基本的 ERC20 函数
        let erc20 = ERC20::new(token, &self.provider);
        
        // 检查 totalSupply（安全函数）
        if erc20.totalSupply().call().await.is_err() {
            debug!("代币 {:?} totalSupply 调用失败", token);
            return Ok(false);
        }
        
        // 检查 decimals
        if erc20.decimals().call().await.is_err() {
            debug!("代币 {:?} decimals 调用失败", token);
            return Ok(false);
        }
        
        // 3. 检查代币名称和符号（一些恶意代币会在这里做手脚）
        match erc20.symbol().call().await {
            Ok(symbol) => {
                // 检查符号是否包含异常字符
                if symbol.contains('\0') || symbol.len() > 20 {
                    debug!("代币 {:?} 符号异常: {}", token, symbol);
                    return Ok(false);
                }
            }
            Err(_) => {
                debug!("代币 {:?} symbol 调用失败", token);
                return Ok(false);
            }
        }
        
        // 4. 简化的 transfer 检查（实际应该用 revm 模拟）
        // 这里只做基本检查，避免已知的问题代币
        let known_bad_tokens = [
            "0x0000000000000000000000000000000000000000", // 零地址
            "0x000000000000000000000000000000000000dead", // 死地址
        ];
        
        for &bad_token in &known_bad_tokens {
            match bad_token.parse::<Address>() {
                Ok(parsed_token) => {
                    if token == parsed_token {
                        return Ok(false);
                    }
                }
                Err(e) => {
                    warn!("Failed to parse bad token address '{}': {}", bad_token, e);
                    // Continue checking other tokens
                }
            }
        }
        
        debug!("✅ 代币 {:?} 通过安全检查", token);
        Ok(true)
    }

    /// 估算交易的滑点影响
    pub fn estimate_slippage_impact(
        &self,
        amount: U256,
        pool_liquidity: U256,
    ) -> f64 {
        if pool_liquidity.is_zero() {
            return 0.0;
        }

        // 简化的滑点计算：amount / liquidity
        let impact = amount.to::<u128>() as f64 / pool_liquidity.to::<u128>() as f64;
        
        // 限制在合理范围内
        impact.min(0.1).max(0.0001) // 0.01% - 10%
    }
}

/// Bundle 构建器
pub mod bundle_builder {
    use super::*;
    use crate::types::{SandwichBundle, SandwichConfig};

    pub struct BundleBuilder {
        config: SandwichConfig,
        provider: Option<Arc<Provider>>,
    }

    impl BundleBuilder {
        pub fn new(config: SandwichConfig) -> Self {
            Self {
                config,
                provider: None,
            }
        }

        pub async fn initialize(&mut self, provider: Arc<Provider>) -> Result<()> {
            self.provider = Some(provider);
            Ok(())
        }

        /// 构建完整的 sandwich bundle
        pub async fn build_sandwich_bundle(
            &self,
            opportunity: &SandwichOpportunity,
            block: &BlockInfo,
            inventory: &TokenInventory,
        ) -> Result<SandwichBundle> {
            info!("🔨 构建 Sandwich Bundle");
            
            // 1. 构建前置交易（买入中间代币）
            let frontrun_tx = self.build_frontrun_transaction(
                opportunity,
                block,
                inventory,
            ).await?;

            // 2. 获取受害者交易（已经是 RLP 格式）
            let victim_txs = opportunity.victim_txs
                .iter()
                .map(|tx| format!("0x{}", hex::encode(Encodable2718::encoded_2718(&tx.inner))))
                .collect();

            // 3. 构建后置交易（卖出中间代币）
            let backrun_tx = self.build_backrun_transaction(
                opportunity,
                block,
                inventory,
            ).await?;

            Ok(SandwichBundle {
                frontrun_tx,
                victim_txs,
                backrun_tx,
                target_block: block.number,
                expected_revenue: opportunity.estimated_profit,
                estimated_gas: 300_000, // 估算总 gas
            })
        }

        /// 构建前置交易
        async fn build_frontrun_transaction(
            &self,
            opportunity: &SandwichOpportunity,
            block: &BlockInfo,
            _inventory: &TokenInventory,
        ) -> Result<String> {
            // 构建实际的前置交易
            debug!("🔨 构建前置交易，输入: {:.4} ETH", 
                opportunity.optimal_input.to::<u128>() as f64 / 1e18);

            // 1. 计算 gas 价格（略高于受害者交易以确保优先执行）
            let victim_gas_price = opportunity.victim_txs
                .iter()
                .filter_map(|tx| Some(tx.inner.max_fee_per_gas()))
                .max()
                .unwrap_or(block.base_fee_per_gas.to::<u128>());
            
            let frontrun_gas_price = victim_gas_price + 1_000_000_000; // +1 gwei
            
            // 2. 构建交易数据（调用 sandwich 合约）
            let tx_data = self.build_sandwich_call_data(
                opportunity.intermediary_token,
                opportunity.optimal_input,
                true, // is_frontrun
            );
            
            // 3. 创建交易请求
            use artemis_core::eth::TxRequest;
            let tx_request = TxRequest {
                to: Some(self.config.sandwich_contract.into()),
                value: Some(opportunity.optimal_input),
                gas: Some(200_000), // 估算 gas limit
                gas_price: Some(frontrun_gas_price),
                input: alloy_primitives::Bytes::from(tx_data).into(),
                nonce: None, // 由 Provider 自动填充
                ..Default::default()
            };
            
            // 4. 签名并编码（简化实现）
            // 实际应该使用钱包签名
            Ok(format!("0x{}", hex::encode(serde_json::to_vec(&tx_request)?)))
        }

        fn build_sandwich_call_data(
            &self,
            intermediary_token: Address,
            amount_in: U256,
            is_frontrun: bool,
        ) -> Vec<u8> {
            // 构建 sandwich 合约调用数据
            // 这里应该根据池子类型（V2/V3）构建不同的调用
            
            if is_frontrun {
                // 前置交易：买入中间代币
                // sandwichV2(address target_pool, address intermediary_token, uint256 amount_in, uint256 amount_out_min, bool is_weth_input)
                let mut call_data = Vec::new();
                call_data.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]); // 函数选择器（示例）
                call_data.extend_from_slice(&[0u8; 12]); // padding
                call_data.extend_from_slice(intermediary_token.as_slice());
                call_data.extend_from_slice(&amount_in.to_be_bytes::<32>());
                call_data
            } else {
                // 后置交易：卖出中间代币
                let mut call_data = Vec::new();
                call_data.extend_from_slice(&[0x87, 0x65, 0x43, 0x21]); // 函数选择器（示例）
                call_data.extend_from_slice(&[0u8; 12]); // padding
                call_data.extend_from_slice(intermediary_token.as_slice());
                call_data.extend_from_slice(&amount_in.to_be_bytes::<32>());
                call_data
            }
        }

        /// 构建后置交易
        async fn build_backrun_transaction(
            &self,
            opportunity: &SandwichOpportunity,
            block: &BlockInfo,
            _inventory: &TokenInventory,
        ) -> Result<String> {
            // 构建实际的后置交易
            debug!("🔨 构建后置交易，预期收益: {:.4} ETH", 
                opportunity.estimated_profit.to::<u128>() as f64 / 1e18);

            // 1. 计算最优的卖出金额（所有获得的中间代币）
            let intermediate_token_amount = opportunity.optimal_input; // 简化：假设 1:1
            
            // 2. 计算 gas 价格（包含给矿工的贿赂）
            let base_fee = block.base_fee_per_gas;
            let profit = opportunity.estimated_profit;
            let gas_used = 150_000u64; // 估算后置交易 gas
            
            // 贿赂计算：将 90% 的利润作为贿赂给矿工
            let bribe_amount = profit * U256::from(90) / U256::from(100);
            let gas_price = base_fee + (bribe_amount / U256::from(gas_used));
            
            // 3. 构建交易数据
            let tx_data = self.build_sandwich_call_data(
                opportunity.intermediary_token,
                intermediate_token_amount,
                false, // is_backrun
            );
            
            // 4. 创建交易请求
            use artemis_core::eth::TxRequest;
            let tx_request = TxRequest {
                to: Some(self.config.sandwich_contract.into()),
                value: Some(U256::ZERO), // 后置交易不需要 ETH 输入
                gas: Some(gas_used),
                gas_price: Some(gas_price.to::<u128>()),
                input: alloy_primitives::Bytes::from(tx_data).into(),
                nonce: None, // 自动填充（前置 + 受害者数量 + 1）
                ..Default::default()
            };
            
            // 5. 签名并编码（简化实现）
            Ok(format!("0x{}", hex::encode(serde_json::to_vec(&tx_request)?)))
        }

        /// 计算最优的 gas 价格
        fn calculate_optimal_gas_price(
            &self,
            victim_tx_gas_price: U256,
            base_fee: U256,
            profit: U256,
            gas_used: u64,
        ) -> U256 {
            // 前置交易：略高于受害者交易
            let frontrun_gas_price = victim_tx_gas_price + U256::from(1_000_000_000u64); // +1 gwei

            // 后置交易：包含给矿工的贿赂
            let max_bribe = profit / U256::from(2); // 最多贿赂 50% 利润
            let bribe_per_gas = max_bribe / U256::from(gas_used);
            let backrun_gas_price = base_fee + bribe_per_gas;

            frontrun_gas_price.max(backrun_gas_price)
        }
    }
}
