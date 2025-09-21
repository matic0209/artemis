use artemis_core::{
    collectors::block_collector::NewBlock,
    executors::flashbots_alloy_executor::FlashbotsAlloyBundle,
    eth::{Address, Hash, Transaction, U256, U64},
};
use cfmms::pool::Pool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 核心事件枚举
#[derive(Debug, Clone)]
pub enum Event {
    NewBlock(NewBlock),
    NewTransaction(Transaction),
}

/// 核心行动枚举  
#[derive(Debug, Clone)]
pub enum Action {
    SubmitSandwichBundle(SandwichBundle),
}

/// Sandwich 策略配置
#[derive(Debug, Clone)]
pub struct SandwichConfig {
    /// Sandwich 合约地址
    pub sandwich_contract: Address,
    /// 搜索者钱包地址
    pub searcher_address: Address,
    /// WETH 地址
    pub weth_address: Address,
    /// 最小利润阈值（wei）
    pub min_profit_threshold: U256,
    /// 最大 gas 价格
    pub max_gas_price: U256,
    /// 启用多肉 sandwich
    pub enable_multi_meat: bool,
}

impl Default for SandwichConfig {
    fn default() -> Self {
        Self {
            sandwich_contract: Address::ZERO,
            searcher_address: Address::ZERO,
            weth_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(),
            min_profit_threshold: U256::from(1_000_000_000_000_000u64), // 0.001 ETH
            max_gas_price: U256::from(100_000_000_000u64), // 100 gwei
            enable_multi_meat: true,
        }
    }
}

/// 区块信息
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockInfo {
    pub number: U64,
    pub base_fee_per_gas: U256,
    pub timestamp: U256,
    pub gas_used: Option<U256>,
    pub gas_limit: Option<U256>,
}

impl BlockInfo {
    /// 计算下一个区块的信息
    pub fn next_block(&self) -> BlockInfo {
        BlockInfo {
            number: self.number + U64::from(1),
            base_fee_per_gas: self.calculate_next_base_fee(),
            timestamp: self.timestamp + U256::from(12), // 12秒区块时间
            gas_used: None,
            gas_limit: None,
        }
    }

    /// 计算下一个区块的 base fee
    fn calculate_next_base_fee(&self) -> U256 {
        let current_base_fee = self.base_fee_per_gas;
        
        if let (Some(gas_used), Some(gas_limit)) = (self.gas_used, self.gas_limit) {
            let gas_target = gas_limit / U256::from(2);
            
            if gas_used == gas_target {
                current_base_fee
            } else if gas_used > gas_target {
                let gas_delta = gas_used - gas_target;
                let base_fee_delta = current_base_fee * gas_delta / gas_target / U256::from(8);
                current_base_fee + base_fee_delta
            } else {
                let gas_delta = gas_target - gas_used;
                let base_fee_delta = current_base_fee * gas_delta / gas_target / U256::from(8);
                current_base_fee.saturating_sub(base_fee_delta)
            }
        } else {
            current_base_fee
        }
    }
}

impl From<NewBlock> for BlockInfo {
    fn from(block: NewBlock) -> Self {
        Self {
            number: block.number,
            // 从 block 中获取实际数据（NewBlock 结构需要扩展）
            base_fee_per_gas: U256::from(20_000_000_000u64), // 20 gwei 默认
            timestamp: U256::from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            gas_used: Some(U256::from(15_000_000u64)), // 15M gas 默认
            gas_limit: Some(U256::from(30_000_000u64)), // 30M gas limit 默认
        }
    }
}

/// Sandwich 机会的原始数据
#[derive(Debug, Clone)]
pub struct SandwichOpportunity {
    /// 受害者交易
    pub victim_txs: Vec<Transaction>,
    /// 起始/结束代币（通常是 WETH）
    pub start_end_token: Address,
    /// 中间代币（受害者交易的目标代币）
    pub intermediary_token: Address,
    /// 目标池子
    pub target_pool: Pool,
    /// 预期利润
    pub estimated_profit: U256,
    /// 最优输入金额
    pub optimal_input: U256,
}

impl SandwichOpportunity {
    pub fn new(
        victim_txs: Vec<Transaction>,
        start_end_token: Address,
        intermediary_token: Address,
        target_pool: Pool,
        estimated_profit: U256,
        optimal_input: U256,
    ) -> Self {
        Self {
            victim_txs,
            start_end_token,
            intermediary_token,
            target_pool,
            estimated_profit,
            optimal_input,
        }
    }

    /// 获取受害者交易哈希（用于日志）
    pub fn victim_hashes(&self) -> Vec<Hash> {
        use alloy_consensus::transaction::Transaction as TransactionTrait;
        self.victim_txs.iter().map(|tx| TransactionTrait::hash(&tx.inner)).collect()
    }

    /// 检查是否是有效的 sandwich 机会
    pub fn is_profitable(&self, min_threshold: U256) -> bool {
        self.estimated_profit >= min_threshold
    }
}

/// 完整的 Sandwich Bundle
#[derive(Debug, Clone)]
pub struct SandwichBundle {
    /// 前置交易（买入）
    pub frontrun_tx: String, // RLP 编码的交易
    /// 受害者交易
    pub victim_txs: Vec<String>, // RLP 编码的交易
    /// 后置交易（卖出）
    pub backrun_tx: String, // RLP 编码的交易
    /// 目标区块
    pub target_block: U64,
    /// 预期收益
    pub expected_revenue: U256,
    /// Gas 使用估算
    pub estimated_gas: u64,
}

impl From<SandwichBundle> for FlashbotsAlloyBundle {
    fn from(sandwich: SandwichBundle) -> Self {
        // 暂时返回简化的 bundle，实际使用时需要完整实现
        FlashbotsAlloyBundle {
            txs: vec![
                sandwich.frontrun_tx,
                sandwich.backrun_tx, // 简化：跳过受害者交易处理
            ],
            target_block: Some(sandwich.target_block.to()),
            min_timestamp: None,
            max_timestamp: None,
            replacement_uuid: None,
            reverting_hashes: vec![], // Sandwich 不允许回滚
        }
    }
}

/// 池子状态缓存
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub fee: u32, // V3 池子的费率
    pub last_updated: u64, // 区块号
}

/// 代币余额缓存
#[derive(Debug, Clone)]
pub struct TokenInventory {
    /// WETH 余额
    pub weth_balance: U256,
    /// 其他代币余额
    pub token_balances: HashMap<Address, U256>,
    /// 上次更新时间
    pub last_updated: u64,
}

impl TokenInventory {
    pub fn new() -> Self {
        Self {
            weth_balance: U256::ZERO,
            token_balances: HashMap::new(),
            last_updated: 0,
        }
    }

    pub fn update_weth_balance(&mut self, balance: U256, block_number: u64) {
        self.weth_balance = balance;
        self.last_updated = block_number;
    }

    pub fn update_token_balance(&mut self, token: Address, balance: U256) {
        self.token_balances.insert(token, balance);
    }

    pub fn get_weth_balance(&self) -> U256 {
        self.weth_balance
    }

    pub fn get_token_balance(&self, token: &Address) -> U256 {
        self.token_balances.get(token).copied().unwrap_or(U256::ZERO)
    }
}

/// Sandwich 执行结果
#[derive(Debug, Clone)]
pub struct SandwichResult {
    pub bundle_hash: Hash,
    pub expected_profit: U256,
    pub actual_profit: Option<U256>,
    pub gas_used: u64,
    pub success: bool,
    pub error: Option<String>,
}

/// 性能统计
#[derive(Debug, Default, Clone)]
pub struct SandwichStats {
    pub total_opportunities: u64,
    pub profitable_opportunities: u64,
    pub successful_sandwiches: u64,
    pub total_profit: U256,
    pub average_profit: U256,
    pub success_rate: f64,
}

impl SandwichStats {
    pub fn update_opportunity(&mut self, profitable: bool) {
        self.total_opportunities += 1;
        if profitable {
            self.profitable_opportunities += 1;
        }
    }

    pub fn update_result(&mut self, result: &SandwichResult) {
        if result.success {
            self.successful_sandwiches += 1;
            if let Some(actual_profit) = result.actual_profit {
                self.total_profit += actual_profit;
                self.average_profit = self.total_profit / U256::from(self.successful_sandwiches);
            }
        }
        
        self.success_rate = if self.total_opportunities > 0 {
            self.successful_sandwiches as f64 / self.total_opportunities as f64
        } else {
            0.0
        };
    }
}
