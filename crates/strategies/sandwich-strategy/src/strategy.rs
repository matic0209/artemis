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
use crate::pools::{Pool, PoolDiscovery};
use alloy_provider::Provider as ProviderTrait;
use alloy_consensus::transaction::Transaction as TransactionTrait;

use crate::types::{
    Event, Action, SandwichConfig, SandwichOpportunity, SandwichBundle, 
    BlockInfo, TokenInventory, SandwichStats, PoolState,
};
use crate::simulator::SandwichSimulator;

/// 查询 Uniswap V2 池子的储备量
async fn query_v2_reserves(
    provider: Arc<Provider>,
    pool_address: Address,
) -> Result<(U256, U256, u32)> {
    use alloy_rpc_types_eth::BlockNumberOrTag;
    use alloy_primitives::Bytes;
    
    // getReserves() 方法选择器: 0x0902f1ac
    let get_reserves_selector = [0x09, 0x02, 0xf1, 0xac];
    let call_data = Bytes::from(get_reserves_selector.to_vec());
    
    let call_result = provider
        .call(&alloy_rpc_types_eth::TransactionRequest {
            to: Some(alloy_rpc_types_eth::TransactionKind::Call(pool_address)),
            data: Some(call_data),
            ..Default::default()
        })
        .block(BlockNumberOrTag::Latest)
        .await
        .map_err(|e| anyhow!("调用 getReserves() 失败: {}", e))?;

    if call_result.len() >= 96 { // 3 * 32 bytes
        let reserve0 = U256::from_big_endian(&call_result[0..32]);
        let reserve1 = U256::from_big_endian(&call_result[32..64]);
        let block_timestamp_last = u32::from_be_bytes([
            call_result[92], call_result[93], call_result[94], call_result[95]
        ]);
        
        Ok((reserve0, reserve1, block_timestamp_last))
    } else {
        Err(anyhow!("getReserves() 返回数据长度不足"))
    }
}

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
    bundle_builder: bundle_builder::BundleBuilder,
    /// 性能统计
    stats: SandwichStats,
    /// 当前区块信息
    current_block: BlockInfo,
}

/// 池子管理器
struct PoolManager {
    /// 池子发现器
    discovery: PoolDiscovery,
    /// 已知的池子缓存
    known_pools: Arc<std::sync::RwLock<HashMap<Address, Pool>>>,
    /// 提供者
    provider: Arc<Provider>,
}

impl PoolManager {
    pub fn new(provider: Arc<Provider>) -> Self {
        Self {
            discovery: PoolDiscovery::new(Arc::clone(&provider)),
            known_pools: Arc::new(std::sync::RwLock::new(HashMap::new())),
            provider,
        }
    }

    /// 获取所有已知池子
    pub fn get_all_pools(&self) -> HashMap<Address, Pool> {
        self.known_pools.read().unwrap().clone()
    }

    /// 根据地址获取池子
    pub fn get_pool(&self, address: &Address) -> Option<Pool> {
        self.known_pools.read().unwrap().get(address).cloned()
    }

    /// 添加新池子
    pub fn add_pool(&self, address: Address, pool: Pool) {
        self.known_pools.write().unwrap().insert(address, pool);
    }

    /// 发现新池子
    pub async fn discover_pools_for_token(&self, token: Address) -> Result<Vec<Pool>> {
        self.discovery.discover_pools_for_token(token).await
    }

    /// 更新池子状态
    pub async fn refresh_pool_states(&self) -> Result<()> {
        let pools = self.get_all_pools();
        
        for (address, mut pool) in pools {
            // 查询最新的储备量
            match query_v2_reserves(Arc::clone(&self.provider), address).await {
                Ok((reserve0, reserve1, timestamp)) => {
                    pool.reserve0 = reserve0;
                    pool.reserve1 = reserve1;
                    pool.last_updated = timestamp as u64;
                    
                    // 更新缓存
                    self.add_pool(address, pool);
                }
                Err(e) => {
                    debug!("更新池子 {:?} 状态失败: {:?}", address, e);
                }
            }
        }
        
        Ok(())
    }
}

impl SandwichStrategy {
    pub fn new(
        provider: Arc<Provider>,
        config: SandwichConfig,
        state_manager: Arc<StateManager>,
    ) -> Self {
        Self {
            simulator: SandwichSimulator::new(Arc::clone(&provider), config.clone()),
            bundle_builder: bundle_builder::BundleBuilder::new(config.clone()),
            pool_manager: PoolManager::new(Arc::clone(&provider)),
            inventory: TokenInventory::new(config.searcher_address),
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
            // 首先进行快速盈利性检查
            match self.simulator.quick_profitability_check(&opportunity, &self.current_block).await {
                Ok(is_potentially_profitable) => {
                    if !is_potentially_profitable {
                        debug!("机会快速检查不盈利，跳过详细模拟");
                        continue;
                    }
                }
                Err(e) => {
                    debug!("快速盈利性检查失败: {:?}", e);
                    continue;
                }
            }

            // 进行详细的 REVM 模拟
            match self.simulator.simulate_detailed(&opportunity, &self.current_block, &self.inventory).await {
                Ok(simulation_result) => {
                    if simulation_result.success && 
                       simulation_result.net_profit > max_profit && 
                       simulation_result.net_profit >= self.config.min_profit_threshold {
                        
                        opportunity.estimated_profit = simulation_result.net_profit;
                        opportunity.optimal_input = U256::from(1000000000000000000u64); // 1 ETH 默认，后续优化
                        best_opportunity = Some(opportunity);
                        max_profit = simulation_result.net_profit;
                        
                        info!("🧪 REVM 模拟成功 - 净利润: {:.6} ETH, Gas: {}, ROI: {:.2}%",
                              simulation_result.net_profit.as_u128() as f64 / 1e18,
                              simulation_result.total_gas,
                              simulation_result.calculate_roi(U256::from(1000000000000000000u64)));
                    }
                }
                Err(e) => {
                    debug!("REVM 模拟失败: {:?}", e);
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
            if let Some(pool) = self.pool_manager.get_all_pools().get(&to) {
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
                    
                    if let Some(pool) = self.pool_manager.find_pool_for_tokens(token_a, token_b) {
                        pools.push(pool.clone());
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
        pool.tokens()
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
                let state_manager = Arc::clone(&self.state_manager);
                let task = tokio::spawn(async move {
                    // 查询 Uniswap V2 池子的储备量
                    match query_v2_reserves(provider, pool_addr).await {
                        Ok((reserve0, reserve1, block_timestamp_last)) => {
                            (pool_addr, reserve0, reserve1, Some(block_timestamp_last))
                        }
                        Err(e) => {
                            debug!("查询池子 {:?} 储备量失败: {:?}", pool_addr, e);
                            (pool_addr, U256::ZERO, U256::ZERO, None)
                        }
                    }
                });
                tasks.push(task);
            }
            
            // 等待所有查询完成
            for task in tasks {
                if let Ok((pool_addr, reserve0, reserve1, timestamp)) = task.await.unwrap() {
                    if !reserve0.is_zero() && !reserve1.is_zero() {
                        // 更新池子状态缓存
                        let pool_state = PoolState {
                            address: pool_addr,
                            reserve0,
                            reserve1,
                            last_update: timestamp.unwrap_or(0),
                            liquidity: reserve0 * reserve1, // 简化的流动性计算
                        };
                        
                        // 通过状态管理器缓存池子状态
                        if let Err(e) = self.state_manager.update_pool_state(pool_addr, pool_state).await {
                            debug!("更新池子状态缓存失败: {:?}", e);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}

impl PoolManager {
    fn new(provider: Arc<Provider>) -> Self {
        Self {
            discovery: PoolDiscovery::new(provider),
        }
    }

    /// 初始化池子管理器
    async fn setup(&mut self) -> Result<()> {
        info!("🏊 初始化池子管理器...");
        
        // 发现 WETH 池子
        let pools = self.discovery.discover_weth_pools().await?;
        
        info!("✅ 发现了 {} 个 WETH 池子", pools.len());
        
        Ok(())
    }

    /// 根据代币对查找池子
    fn find_pool_for_tokens(&self, token_a: Address, token_b: Address) -> Option<&Pool> {
        self.discovery.find_pool_for_tokens(token_a, token_b)
    }

    /// 获取所有池子
    fn get_all_pools(&self) -> &std::collections::HashMap<Address, Pool> {
        self.discovery.get_all_pools()
    }
}

#[async_trait]
impl Strategy<Event, Action> for SandwichStrategy {
    async fn sync_state(&mut self) -> Result<()> {
        info!("🔄 同步 Sandwich 策略状态...");
        
        // 1. 初始化池子管理器
        self.pool_manager.setup().await?;
        
        // 2. 获取当前区块信息
        let block_number = ProviderTrait::get_block_number(&*self.provider).await?;
        self.current_block.number = artemis_core::eth::U64::from(block_number);
        
        // 3. 初始化代币库存
        let weth_balance = self.state_manager
            .get_balance(self.config.searcher_address)
            .await?;
        self.inventory.update_weth_balance(weth_balance, block_number);
        
        // 4. 初始化模拟器和构建器
        self.simulator.initialize(self.pool_manager.get_all_pools()).await?;
        self.bundle_builder.initialize(Arc::clone(&self.provider)).await?;
        
        info!("✅ Sandwich 策略同步完成");
        info!("   - 监控池子: {} 个", self.pool_manager.get_all_pools().len());
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
        
        let block_num = block.number.to::<u64>();
        
        // 初始化或更新 REVM 模拟器（每个新区块）
        if let Err(e) = self.simulator.initialize(&self.current_block).await {
            warn!("初始化 REVM 模拟器失败: {:?}", e);
        } else {
            debug!("🧪 REVM 模拟器已为区块 {} 初始化", block_num);
        }
        
        // 定期更新池子状态（每10个区块）
        if block_num % 10 == 0 {
            if let Err(e) = self.update_pool_states().await {
                warn!("更新池子状态失败: {:?}", e);
            }
        }
        
        // 定期更新代币库存（每5个区块）
        if block_num % 5 == 0 {
            if let Err(e) = self.update_inventory().await {
                warn!("更新代币库存失败: {:?}", e);
            }
        }
        
        // 记录统计信息
        metrics::gauge!("artemis.sandwich.current_block").set(block_num as f64);
        metrics::gauge!("artemis.sandwich.weth_balance")
            .set(self.inventory.get_weth_balance().to::<u128>() as f64 / 1e18);
        metrics::gauge!("artemis.sandwich.revm_initialized").set(1.0);
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
        self.pool_manager.get_all_pools().len()
    }
}

// 注意：Sandwich 模拟器现在从 simulator.rs 模块导入，
// 该模块包含完整的 REVM 集成实现

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
            info!("🔨 构建 Sandwich Bundle - 利润: {:.6} ETH", 
                  opportunity.estimated_profit.as_u128() as f64 / 1e18);

            // 1. 构建前置交易（买入）
            let frontrun_tx = self.build_frontrun_transaction(
                opportunity, 
                block, 
                inventory
            ).await.map_err(|e| anyhow!("构建前置交易失败: {}", e))?;

            // 2. 编码受害者交易
            let victim_txs = self.encode_victim_transactions(&opportunity.victim_txs)
                .await.map_err(|e| anyhow!("编码受害者交易失败: {}", e))?;

            // 3. 构建后置交易（卖出）
            let backrun_tx = self.build_backrun_transaction(
                opportunity, 
                block, 
                inventory
            ).await.map_err(|e| anyhow!("构建后置交易失败: {}", e))?;

            // 4. 计算总 gas 使用量
            let estimated_gas = self.calculate_total_gas_limit(opportunity);

            Ok(SandwichBundle {
                frontrun_tx,
                victim_txs,
                backrun_tx,
                target_block: block.number,
                expected_revenue: opportunity.estimated_profit,
                estimated_gas,
            })
        }

        /// 构建前置交易（买入操作）
        async fn build_frontrun_transaction(
            &self,
            opportunity: &SandwichOpportunity,
            block: &BlockInfo,
            inventory: &TokenInventory,
        ) -> Result<String> {
            // 暂时返回模拟交易，实际实现需要完整的交易构建
            let mock_tx_data = format!(
                "0x7ff36ab5{:064x}{:064x}{:040x}{:064x}",
                opportunity.optimal_input.as_u128(),  // amountIn
                0u128,                                // amountOutMin
                inventory.searcher_address.as_u128(), // to
                block.timestamp + 300                 // deadline
            );
            
            debug!("前置交易构建完成: 输入 {:.6} ETH", 
                   opportunity.optimal_input.as_u128() as f64 / 1e18);
            
            Ok(mock_tx_data)
        }

        /// 构建后置交易（卖出操作）
        async fn build_backrun_transaction(
            &self,
            opportunity: &SandwichOpportunity,
            block: &BlockInfo,
            inventory: &TokenInventory,
        ) -> Result<String> {
            // 估算从前置交易获得的代币数量
            let estimated_token_amount = opportunity.optimal_input * U256::from(95) / U256::from(100); // 假设 5% 滑点
            
            let mock_tx_data = format!(
                "0x18cbafe5{:064x}{:064x}{:040x}{:064x}",
                estimated_token_amount.as_u128(),     // amountIn
                0u128,                                // amountOutMin  
                inventory.searcher_address.as_u128(), // to
                block.timestamp + 300                 // deadline
            );
            
            debug!("后置交易构建完成: 卖出 {:.6} tokens", 
                   estimated_token_amount.as_u128() as f64 / 1e18);
            
            Ok(mock_tx_data)
        }

        /// 编码受害者交易
        async fn encode_victim_transactions(
            &self,
            victim_txs: &[artemis_core::eth::Transaction],
        ) -> Result<Vec<String>> {
            let mut encoded_txs = Vec::new();
            
            for (i, tx) in victim_txs.iter().enumerate() {
                // 暂时返回交易哈希，实际需要完整的 RLP 编码
                let tx_hash = format!("0x{:064x}", i);
                encoded_txs.push(tx_hash);
                
                debug!("受害者交易 {} 编码完成", i);
            }
            
            Ok(encoded_txs)
        }

        /// 计算总 gas 限制
        fn calculate_total_gas_limit(&self, opportunity: &SandwichOpportunity) -> u64 {
            let frontrun_gas = 200_000u64;
            let backrun_gas = 200_000u64;
            let victim_gas: u64 = opportunity.victim_txs.iter()
                .map(|tx| tx.gas_limit.unwrap_or(150_000))
                .sum();
            
            let total = frontrun_gas + backrun_gas + victim_gas + 50_000; // 安全缓冲
            
            debug!("总 Gas 估算: {} (前置: {}, 后置: {}, 受害者: {})", 
                   total, frontrun_gas, backrun_gas, victim_gas);
            
            total
        }
    }
}
