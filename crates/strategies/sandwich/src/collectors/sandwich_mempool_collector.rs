use std::collections::HashSet;
use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tracing::{debug, info};

use artemis_core::{
    eth::{Transaction, Provider, Address, U256},
    types::{Collector, CollectorStream},
};
use alloy_provider::Provider as ProviderTrait;
use alloy_consensus::transaction::Transaction as TransactionTrait;

/// 专门用于 Sandwich 策略的内存池收集器
/// 只收集可能可以被 sandwich 的交易
pub struct SandwichMempoolCollector {
    provider: Arc<Provider>,
    /// 监控的 DEX 路由器地址
    target_routers: HashSet<Address>,
    /// 监控的代币地址
    target_tokens: HashSet<Address>,
    /// 最小交易价值（避免处理小额交易）
    min_value_wei: u128,
    /// 缓冲区大小
    buffer_size: usize,
}

impl SandwichMempoolCollector {
    pub fn new(provider: Arc<Provider>) -> Self {
        // 预设的 DEX 路由器地址
        let mut target_routers = HashSet::new();
        
        // 安全解析地址的辅助函数
        fn parse_router_address(addr_str: &str) -> Address {
            addr_str.parse()
                .unwrap_or_else(|_| panic!("Invalid router address: {}", addr_str))
        }
        
        // Uniswap V2 Router
        target_routers.insert(parse_router_address("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"));
        // Uniswap V3 Router  
        target_routers.insert(parse_router_address("0xE592427A0AEce92De3Edee1F18E0157C05861564"));
        // Sushiswap Router
        target_routers.insert(parse_router_address("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F"));
        // 1inch Router
        target_routers.insert(parse_router_address("0x1111111254EEB25477B68fb85Ed929f73A960582"));

        Self {
            provider,
            target_routers,
            target_tokens: HashSet::new(),
            min_value_wei: 100_000_000_000_000_000, // 0.1 ETH
            buffer_size: 4096, // 大缓冲区用于高频 sandwich
        }
    }

    /// 添加要监控的代币
    pub fn with_target_tokens(mut self, tokens: HashSet<Address>) -> Self {
        self.target_tokens = tokens;
        self
    }

    /// 设置最小交易价值
    pub fn with_min_value(mut self, min_value_wei: u128) -> Self {
        self.min_value_wei = min_value_wei;
        self
    }

    /// 设置缓冲区大小
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// 检查交易是否可能被 sandwich
    fn is_sandwichable_tx(&self, tx: &Transaction) -> bool {
        // 检查交易价值
        let value = tx.inner.value();
        if value > U256::ZERO {
            if value < self.min_value_wei {
                return false;
            }
        }

        // 检查是否与目标路由器交互
        if let Some(to) = tx.inner.to() {
            if self.target_routers.contains(&to) {
                return true;
            }
        }

        // 检查是否与目标代币交互
        if !self.target_tokens.is_empty() {
            // 解析交易数据检查代币交互
            let input = tx.inner.input();
            if !input.is_empty() {
                let tokens = crate::utils::extract_tokens_from_calldata(input);
                for token in tokens {
                    if self.target_tokens.contains(&token) {
                        return true;
                    }
                }
            }
        }

        // 检查交易数据是否包含 DEX 相关的函数选择器
        let input = tx.inner.input();
        if !input.is_empty() {
            if input.len() >= 4 {
                let selector = &input[0..4];
                return self.is_dex_function_selector(selector);
            }
        }

        false
    }

    /// 检查函数选择器是否是 DEX 相关
    fn is_dex_function_selector(&self, selector: &[u8]) -> bool {
        match selector {
            // swapExactTokensForTokens
            [0x38, 0xed, 0x17, 0x39] => true,
            // swapTokensForExactTokens  
            [0x88, 0x03, 0xdb, 0xee] => true,
            // swapExactETHForTokens
            [0x7f, 0xf3, 0x6a, 0xb5] => true,
            // swapTokensForExactETH
            [0x47, 0x51, 0xb7, 0xb1] => true,
            // swapExactTokensForETH
            [0x18, 0xcb, 0xaf, 0xe5] => true,
            // swapETHForExactTokens
            [0xfb, 0x3b, 0xdb, 0x41] => true,
            // Uniswap V3 exactInputSingle
            [0x41, 0x4b, 0xf3, 0x89] => true,
            // Uniswap V3 exactOutputSingle
            [0xdb, 0x3e, 0x21, 0x98] => true,
            // 1inch swap
            [0x12, 0xaa, 0x3c, 0xaf] => true,
            _ => false,
        }
    }

    /// 获取交易的预估滑点影响
    fn estimate_slippage_impact(&self, tx: &Transaction) -> f64 {
        // 基于交易价值估算滑点影响
        let value = tx.inner.value();
        if value > U256::ZERO {
            let value_eth = value.to::<u128>() as f64 / 1e18;
            
            match value_eth {
                v if v >= 100.0 => 0.05,   // 大额交易，高滑点
                v if v >= 10.0 => 0.03,    // 中等交易
                v if v >= 1.0 => 0.01,     // 小额交易
                _ => 0.005,                // 微小交易
            }
        } else {
            0.01 // 默认滑点
        }
    }
}

#[async_trait]
impl Collector<Transaction> for SandwichMempoolCollector {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Transaction>> {
        let (tx, rx) = mpsc::channel::<Transaction>(self.buffer_size);
        let provider = self.provider.clone();
        let target_routers = self.target_routers.clone();
        let min_value_wei = self.min_value_wei;
        
        tokio::spawn(async move {
            info!("🎯 启动 Sandwich 内存池收集器");
            
            if let Ok(stream) = ProviderTrait::subscribe_pending_transactions(&*provider).await {
                let mut stream = stream.into_stream();
                let mut processed_count = 0;
                let mut filtered_count = 0;
                
                while let Some(tx_hash) = stream.next().await {
                    // 获取完整交易详情
                    if let Ok(Some(txn)) = ProviderTrait::get_transaction_by_hash(&*provider, tx_hash).await {
                        processed_count += 1;
                        
                        // 快速预过滤
                        let is_potential_target = {
                            // 检查交易价值
                            let value_ok = txn.inner.value() >= min_value_wei;
                            
                            // 检查是否与目标路由器交互
                            let router_ok = txn.inner.to()
                                .map_or(false, |to| target_routers.contains(&to));
                            
                            // 检查是否有足够的 gas（避免失败的交易）
                            let gas_ok = txn.inner.gas_limit() > 100_000;
                            
                            value_ok && router_ok && gas_ok
                        };
                        
                        if is_potential_target {
                            if tx.send(txn).await.is_err() {
                                break;
                            }
                            
                            filtered_count += 1;
                            
                            // 每 100 个交易记录一次统计
                            if filtered_count % 100 == 0 {
                                debug!("Sandwich 收集器统计: 处理 {}, 过滤 {}, 命中率 {:.2}%", 
                                    processed_count, filtered_count, 
                                    (filtered_count as f64 / processed_count as f64) * 100.0);
                            }
                        }
                        
                        // 记录 metrics
                        if processed_count % 1000 == 0 {
                            metrics::counter!("artemis.sandwich.collector.transactions_processed")
                                .increment(1000);
                            metrics::counter!("artemis.sandwich.collector.transactions_filtered")
                                .increment(filtered_count as u64);
                            metrics::gauge!("artemis.sandwich.collector.filter_rate")
                                .set((filtered_count as f64 / processed_count as f64) * 100.0);
                        }
                    }
                }
            }
        });
        
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}

/// 智能交易过滤器
pub struct TransactionFilter {
    /// 已知的 MEV 机器人地址（避免 sandwich MEV 机器人）
    known_bots: HashSet<Address>,
    /// 代币黑名单（避免有问题的代币）
    token_blacklist: HashSet<Address>,
    /// 最大 gas 价格（避免 gas 战争）
    max_gas_price: U256,
}

impl TransactionFilter {
    pub fn new() -> Self {
        let mut known_bots = HashSet::new();
        
        // 安全解析地址的辅助函数
        fn parse_bot_address(addr_str: &str) -> Address {
            addr_str.parse()
                .unwrap_or_else(|_| panic!("Invalid bot address: {}", addr_str))
        }
        
        // 添加一些已知的 MEV 机器人地址
        known_bots.insert(parse_bot_address("0x5050e08626c499411b5d0e0b5af0e83d3fd82edf")); // Flashbots
        known_bots.insert(parse_bot_address("0x00000000003b3cc22aF3aE1EAc0440BcEe416B40")); // MEV Bot
        
        Self {
            known_bots,
            token_blacklist: HashSet::new(),
            max_gas_price: U256::from(200_000_000_000u64), // 200 gwei
        }
    }

    /// 检查交易是否应该被过滤掉
    pub fn should_filter(&self, tx: &Transaction) -> bool {
        // 过滤 MEV 机器人交易
        let from_addr = tx.inner.signer();
        if !from_addr.is_zero() {
            if self.known_bots.contains(&from_addr) {
                return true;
            }
        }

        // 过滤过高的 gas 价格
        let max_fee = tx.inner.max_fee_per_gas();
        if max_fee > 0 {
            if max_fee > self.max_gas_price.to::<u128>() {
                return true;
            }
        }

        // 过滤黑名单代币
        let input = tx.inner.input();
        if !input.is_empty() {
            let tokens = crate::utils::extract_tokens_from_calldata(input);
            for token in tokens {
                if self.token_blacklist.contains(&token) {
                    return true; // 过滤掉包含黑名单代币的交易
                }
            }
        }

        false
    }

    /// 添加代币到黑名单
    pub fn add_token_to_blacklist(&mut self, token: Address) {
        self.token_blacklist.insert(token);
    }

    /// 添加机器人地址到已知列表
    pub fn add_known_bot(&mut self, bot: Address) {
        self.known_bots.insert(bot);
    }
}
