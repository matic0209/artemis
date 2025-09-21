use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tracing::{debug, info, warn, error};
use rayon::prelude::*;

use artemis_core::{
    eth::{Address, Provider, U256},
    types::Strategy,
    state_manager::StateManager,
};
use cfmms::{
    pool::Pool,
    sync,
};
use alloy_provider::Provider as ProviderTrait;
use alloy_consensus::transaction::Transaction as TransactionTrait;

use crate::types::{
    Event, Action, SandwichConfig, SandwichOpportunity, SandwichBundle, 
    BlockInfo, TokenInventory, SandwichStats, PoolState,
};
use crate::simulator::{SandwichSimulator, bundle_builder::BundleBuilder};

/// 高性能 Sandwich 攻击策略
pub struct SandwichStrategy {
    /// Alloy provider
    provider: Arc<Provider>,
    /// 策略配置
    config: SandwichConfig,
    /// 池子管理器
    pool_manager: PoolManager,
    /// 代币库存管理
    inventory: TokenInventory,
    /// 状态管理器（缓存优化）
    state_manager: Arc<StateManager>,
    /// Sandwich 模拟器
    simulator: SandwichSimulator,
    /// Bundle 构建器
    bundle_builder: BundleBuilder,
    /// 性能统计
    stats: SandwichStats,
    /// 当前区块信息
    current_block: BlockInfo,
}

/// 池子管理器
struct PoolManager {
    /// 所有监控的池子
    pools: HashMap<Address, Pool>,
    /// 池子状态缓存
    pool_states: HashMap<Address, PoolState>,
    /// 代币对到池子的映射
    token_pair_to_pool: HashMap<(Address, Address), Address>,
}

impl SandwichStrategy {
    pub fn new(
        provider: Arc<Provider>,
        config: SandwichConfig,
        state_manager: Arc<StateManager>,
    ) -> Self {
        Self {
            simulator: SandwichSimulator::new(Arc::clone(&provider)),
            bundle_builder: BundleBuilder::new(config.clone()),
            pool_manager: PoolManager::new(),
            inventory: TokenInventory::new(),
            stats: SandwichStats::default(),
            current_block: BlockInfo::default(),
            provider,
            config,
            state_manager,
        }
    }

    /// 处理新的交易，寻找 sandwich 机会
    async fn process_transaction(&mut self, tx: artemis_core::eth::Transaction) -> Option<Action> {
        let start_time = std::time::Instant::now();
        
        // 1. 快速预检查
        if !self.quick_precheck(&tx) {
            return None;
        }

        // 2. 解析交易，确定涉及的池子
        let affected_pools = match self.get_affected_pools(&tx).await {
            Ok(pools) => pools,
            Err(e) => {
                debug!("无法解析交易影响的池子: {:?}", e);
                return None;
            }
        };

        if affected_pools.is_empty() {
            return None;
        }

        // 3. 并行检查每个池子的 sandwich 机会
        let opportunities: Vec<_> = affected_pools
            .into_par_iter()
            .filter_map(|pool| {
                // 检查是否是 WETH 池子（我们只 sandwich WETH 池子）
                if !self.is_weth_pool(&pool) {
                    return None;
                }

                // 创建 sandwich 机会
                let (token_a, token_b) = self.get_pool_tokens(&pool);
                let intermediary_token = if token_a == self.config.weth_address {
                    token_b
                } else {
                    token_a
                };

                Some(SandwichOpportunity::new(
                    vec![tx.clone()],
                    self.config.weth_address,
                    intermediary_token,
                    pool,
                    U256::ZERO, // 稍后计算
                    U256::ZERO, // 稍后计算
                ))
            })
            .collect();

        if opportunities.is_empty() {
            return None;
        }

        // 4. 模拟每个机会，找到最优的
        let mut best_opportunity: Option<SandwichOpportunity> = None;
        let mut max_profit = U256::ZERO;

        for mut opportunity in opportunities {
            match self.simulator.simulate_sandwich(&opportunity, &self.current_block).await {
                Ok((profit, optimal_input)) => {
                    if profit > max_profit && profit >= self.config.min_profit_threshold {
                        opportunity.estimated_profit = profit;
                        opportunity.optimal_input = optimal_input;
                        best_opportunity = Some(opportunity);
                        max_profit = profit;
                    }
                }
                Err(e) => {
                    debug!("Sandwich 模拟失败: {:?}", e);
                }
            }
        }

        // 5. 如果找到有利可图的机会，构建 bundle
        if let Some(opportunity) = best_opportunity {
            match self.bundle_builder.build_sandwich_bundle(
                &opportunity, 
                &self.current_block,
                &self.inventory
            ).await {
                Ok(bundle) => {
                    let processing_time = start_time.elapsed();
                    
                    info!("🥪 发现 Sandwich 机会! 利润: {:.4} ETH, 处理时间: {:.2}ms",
                        max_profit.to::<u128>() as f64 / 1e18,
                        processing_time.as_millis());
                    
                    // 更新统计
                    self.stats.update_opportunity(true);
                    
                    // 记录 metrics
                    metrics::counter!("artemis.sandwich.opportunities_found").increment(1);
                    metrics::histogram!("artemis.sandwich.processing_time")
                        .record(processing_time.as_millis() as f64);
                    metrics::histogram!("artemis.sandwich.estimated_profit")
                        .record(max_profit.to::<u128>() as f64 / 1e18);
                    
                    return Some(Action::SubmitSandwichBundle(bundle));
                }
                Err(e) => {
                    error!("构建 Sandwich bundle 失败: {:?}", e);
                }
            }
        }

        // 更新统计（未找到机会）
        self.stats.update_opportunity(false);
        None
    }

    /// 快速预检查交易是否值得进一步处理
    fn quick_precheck(&self, tx: &artemis_core::eth::Transaction) -> bool {
        // 检查 gas 价格是否合理
        if let Some(max_fee) = TransactionTrait::max_fee_per_gas(&tx.inner) {
            if max_fee > self.config.max_gas_price.to::<u128>() {
                return false;
            }
        }

        // 检查是否能在下一个区块执行
        let next_block_base_fee = self.current_block.next_block().base_fee_per_gas;
        if let Some(max_fee) = TransactionTrait::max_fee_per_gas(&tx.inner) {
            if U256::from(max_fee) < next_block_base_fee {
                return false;
            }
        }

        true
    }

    /// 获取交易影响的池子
    async fn get_affected_pools(&self, tx: &artemis_core::eth::Transaction) -> Result<Vec<Pool>> {
        // 这里需要解析交易的 calldata 来确定涉及的池子
        // 简化实现：基于 to 地址查找相关池子
        
        if let Some(to) = TransactionTrait::to(&tx.inner) {
            // 如果是路由器调用，需要解析 calldata
            if self.is_router_address(&to) {
                return self.parse_router_call(tx).await;
            }
            
            // 如果直接调用池子
            if let Some(pool) = self.pool_manager.pools.get(&to) {
                return Ok(vec![pool.clone()]);
            }
        }

        Ok(vec![])
    }

    fn is_router_address(&self, address: &Address) -> bool {
        // 检查是否是已知的 DEX 路由器
        let uniswap_v2: Address = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap();
        let uniswap_v3: Address = "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap();
        let sushiswap: Address = "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap();
        
        *address == uniswap_v2 || *address == uniswap_v3 || *address == sushiswap
    }

    async fn parse_router_call(&self, tx: &artemis_core::eth::Transaction) -> Result<Vec<Pool>> {
        // 实现路由器调用解析
        if let Some(input) = TransactionTrait::input(&tx.inner) {
            if input.len() >= 4 {
                let selector = &input[0..4];
                let params = &input[4..];
                
                // 根据函数选择器解析参数
                let token_addresses = match selector {
                    // swapExactTokensForTokens 等函数
                    [0x38, 0xed, 0x17, 0x39] | [0x88, 0x03, 0xdb, 0xee] | [0x18, 0xcb, 0xaf, 0xe5] => {
                        crate::utils::extract_tokens_from_calldata(input)
                    }
                    // swapExactETHForTokens 等 ETH 相关函数
                    [0x7f, 0xf3, 0x6a, 0xb5] | [0xfb, 0x3b, 0xdb, 0x41] | [0x47, 0x51, 0xb7, 0xb1] => {
                        crate::utils::extract_tokens_from_calldata(input)
                    }
                    _ => vec![],
                };
                
                // 根据代币地址查找对应的池子
                let mut pools = Vec::new();
                for i in 0..token_addresses.len().saturating_sub(1) {
                    let token_a = token_addresses[i];
                    let token_b = token_addresses[i + 1];
                    
                    if let Some(&pool_addr) = self.pool_manager.token_pair_to_pool.get(&(token_a, token_b)) {
                        if let Some(pool) = self.pool_manager.pools.get(&pool_addr) {
                            pools.push(pool.clone());
                        }
                    }
                }
                
                return Ok(pools);
            }
        }
        
        Ok(vec![])
    }

    fn is_weth_pool(&self, pool: &Pool) -> bool {
        let (token_a, token_b) = self.get_pool_tokens(pool);
        token_a == self.config.weth_address || token_b == self.config.weth_address
    }

    fn get_pool_tokens(&self, pool: &Pool) -> (Address, Address) {
        match pool {
            Pool::UniswapV2(p) => (p.token_a, p.token_b),
            Pool::UniswapV3(p) => (p.token_a, p.token_b),
            _ => (Address::ZERO, Address::ZERO),
        }
    }

    /// 更新池子状态
    async fn update_pool_states(&mut self) -> Result<()> {
        info!("🔄 更新池子状态...");
        
        let pool_addresses: Vec<_> = self.pool_manager.pools.keys().copied().collect();
        
        // 批量获取池子状态
        for chunk in pool_addresses.chunks(50) { // 批量处理
            let mut tasks = Vec::new();
            
            for &pool_addr in chunk {
                let provider = Arc::clone(&self.provider);
                let task = tokio::spawn(async move {
                    // 获取池子的储备量等状态
                    // TODO: 实现具体的状态查询逻辑
                    (pool_addr, U256::ZERO, U256::ZERO)
                });
                tasks.push(task);
            }
            
            // 等待所有查询完成
            for task in tasks {
                if let Ok((pool_addr, reserve0, reserve1)) = task.await.unwrap() {
                    // 更新池子状态缓存
                    // TODO: 更新 pool_states
                }
            }
        }
        
        Ok(())
    }
}

impl PoolManager {
    fn new() -> Self {
        Self {
            pools: HashMap::new(),
            pool_states: HashMap::new(),
            token_pair_to_pool: HashMap::new(),
        }
    }

    /// 初始化池子管理器
    async fn setup(&mut self, provider: Arc<Provider>) -> Result<()> {
        info!("🏊 初始化池子管理器...");
        
        // 同步所有 Uniswap V2/V3 池子
        let pools = sync::sync_pairs(
            vec![], // DEX 配置
            provider.clone(),
            None, // 不使用检查点
        ).await.map_err(|e| anyhow!("同步池子失败: {:?}", e))?;
        
        info!("✅ 同步了 {} 个池子", pools.len());
        
        // 只保留 WETH 池子
        let weth_address: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        
        for pool in pools {
            let (token_a, token_b) = match &pool {
                Pool::UniswapV2(p) => (
                    Address::from(p.token_a.0), 
                    Address::from(p.token_b.0)
                ),
                Pool::UniswapV3(p) => (
                    Address::from(p.token_a.0), 
                    Address::from(p.token_b.0)
                ),
                _ => continue,
            };
            
            // 只关注 WETH 池子
            if token_a == weth_address || token_b == weth_address {
                let pool_address = Address::from(pool.address().0);
                self.pools.insert(pool_address, pool);
                self.token_pair_to_pool.insert((token_a, token_b), pool_address);
                self.token_pair_to_pool.insert((token_b, token_a), pool_address);
            }
        }
        
        info!("✅ 筛选出 {} 个 WETH 池子", self.pools.len());
        
        Ok(())
    }
}

#[async_trait]
impl Strategy<Event, Action> for SandwichStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        info!("🔄 同步 Sandwich 策略状态...");
        
        // 1. 初始化池子管理器
        self.pool_manager.setup(Arc::clone(&self.provider)).await?;
        
        // 2. 获取当前区块信息
        let block_number = ProviderTrait::get_block_number(&*self.provider).await?;
        self.current_block.number = artemis_core::eth::U64::from(block_number);
        
        // 3. 初始化代币库存
        let weth_balance = self.state_manager
            .get_balance(self.config.searcher_address)
            .await?;
        self.inventory.update_weth_balance(weth_balance, block_number);
        
        // 4. 初始化模拟器和构建器
        self.simulator.initialize(&self.pool_manager.pools).await?;
        self.bundle_builder.initialize(Arc::clone(&self.provider)).await?;
        
        info!("✅ Sandwich 策略同步完成");
        info!("   - 监控池子: {} 个", self.pool_manager.pools.len());
        info!("   - WETH 余额: {:.4} ETH", weth_balance.to::<u128>() as f64 / 1e18);
        info!("   - 当前区块: {}", block_number);
        
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::NewBlock(block) => {
                self.process_new_block(block).await;
                vec![]
            }
            Event::NewTransaction(tx) => {
                if let Some(action) = self.process_transaction(tx).await {
                    vec![action]
                } else {
                    vec![]
                }
            }
        }
    }
}

impl SandwichStrategy {
    /// 处理新区块
    async fn process_new_block(&mut self, block: artemis_core::collectors::block_collector::NewBlock) {
        debug!("📦 新区块: {}", block.number);
        
        // 更新当前区块信息
        self.current_block = BlockInfo::from(block);
        
        // 定期更新池子状态（每10个区块）
        if block.number.to::<u64>() % 10 == 0 {
            if let Err(e) = self.update_pool_states().await {
                warn!("更新池子状态失败: {:?}", e);
            }
        }
        
        // 定期更新代币库存（每5个区块）
        if block.number.to::<u64>() % 5 == 0 {
            if let Err(e) = self.update_inventory().await {
                warn!("更新代币库存失败: {:?}", e);
            }
        }
        
        // 记录统计信息
        metrics::gauge!("artemis.sandwich.current_block").set(block.number.to::<u64>() as f64);
        metrics::gauge!("artemis.sandwich.weth_balance")
            .set(self.inventory.get_weth_balance().to::<u128>() as f64 / 1e18);
    }


    /// 更新代币库存
    async fn update_inventory(&mut self) -> Result<()> {
        let weth_balance = self.state_manager
            .get_balance(self.config.searcher_address)
            .await?;
        
        self.inventory.update_weth_balance(
            weth_balance, 
            self.current_block.number.to::<u64>()
        );
        
        Ok(())
    }

    /// 获取策略统计信息
    pub fn get_stats(&self) -> &SandwichStats {
        &self.stats
    }

    /// 获取当前监控的池子数量
    pub fn get_pool_count(&self) -> usize {
        self.pool_manager.pools.len()
    }
}

/// Sandwich 模拟器
mod simulator {
    use super::*;
    
    pub struct SandwichSimulator {
        provider: Arc<Provider>,
        // TODO: 添加 revm 模拟器
    }
    
    impl SandwichSimulator {
        pub fn new(provider: Arc<Provider>) -> Self {
            Self { provider }
        }
        
        pub async fn initialize(&mut self, pools: &HashMap<Address, Pool>) -> Result<()> {
            info!("🧪 初始化 Sandwich 模拟器");
            // TODO: 设置 revm 环境
            Ok(())
        }
        
        /// 模拟 sandwich 攻击，返回利润和最优输入
        pub async fn simulate_sandwich(
            &self,
            opportunity: &SandwichOpportunity,
            block: &BlockInfo,
        ) -> Result<(U256, U256)> {
            // TODO: 实现 revm 模拟
            // 1. 设置区块环境
            // 2. 模拟前置交易
            // 3. 模拟受害者交易
            // 4. 模拟后置交易
            // 5. 计算净利润
            
            // 暂时返回模拟结果
            let estimated_profit = U256::from(1_000_000_000_000_000u64); // 0.001 ETH
            let optimal_input = U256::from(10_000_000_000_000_000_000u64); // 10 ETH
            
            Ok((estimated_profit, optimal_input))
        }
    }
}

/// Bundle 构建器
mod bundle_builder {
    use super::*;
    
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
            // TODO: 实现完整的 bundle 构建逻辑
            // 1. 构建前置交易（买入）
            // 2. 包含受害者交易
            // 3. 构建后置交易（卖出）
            // 4. 计算 gas 和费用
            
            Ok(SandwichBundle {
                frontrun_tx: "0x".to_string(), // TODO: 实际的 RLP 编码
                victim_txs: vec!["0x".to_string()], // TODO: 受害者交易 RLP
                backrun_tx: "0x".to_string(), // TODO: 实际的 RLP 编码
                target_block: block.number,
                expected_revenue: opportunity.estimated_profit,
                estimated_gas: 300_000, // 估算的总 gas 使用
            })
        }
    }
}
