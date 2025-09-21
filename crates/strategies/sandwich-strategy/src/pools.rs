use artemis_core::eth::{Address, U256};
use serde::{Deserialize, Serialize};

/// 简化的池子类型（替代 cfmms）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Pool {
    UniswapV2(UniswapV2Pool),
    UniswapV3(UniswapV3Pool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniswapV2Pool {
    pub address: Address,
    pub token_a: Address,
    pub token_b: Address,
    pub reserve_a: U256,
    pub reserve_b: U256,
    pub fee: u32, // 通常是 30 (0.3%)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniswapV3Pool {
    pub address: Address,
    pub token_a: Address,
    pub token_b: Address,
    pub fee: u32, // 500, 3000, 10000
    pub sqrt_price_x96: U256,
    pub liquidity: u128,
    pub tick: i32,
}

impl Pool {
    pub fn address(&self) -> Address {
        match self {
            Pool::UniswapV2(p) => p.address,
            Pool::UniswapV3(p) => p.address,
        }
    }

    pub fn tokens(&self) -> (Address, Address) {
        match self {
            Pool::UniswapV2(p) => (p.token_a, p.token_b),
            Pool::UniswapV3(p) => (p.token_a, p.token_b),
        }
    }

    pub fn is_weth_pool(&self, weth_address: Address) -> bool {
        let (token_a, token_b) = self.tokens();
        token_a == weth_address || token_b == weth_address
    }

    /// 计算给定输入的输出金额
    pub fn calculate_amount_out(&self, amount_in: U256, token_in: Address) -> U256 {
        match self {
            Pool::UniswapV2(pool) => {
                let (reserve_in, reserve_out) = if token_in == pool.token_a {
                    (pool.reserve_a, pool.reserve_b)
                } else {
                    (pool.reserve_b, pool.reserve_a)
                };
                
                crate::utils::calculate_v2_amount_out(amount_in, reserve_in, reserve_out, pool.fee)
            }
            Pool::UniswapV3(_pool) => {
                // V3 计算更复杂，这里简化处理
                // 实际应该使用 sqrt price 和 tick 计算
                amount_in * U256::from(997) / U256::from(1000) // 简化的 0.3% 费用
            }
        }
    }

    /// 估算池子的流动性（用于滑点计算）
    pub fn estimate_liquidity(&self) -> U256 {
        match self {
            Pool::UniswapV2(pool) => {
                // V2 池子的流动性 = sqrt(reserve_a * reserve_b)
                let product = pool.reserve_a * pool.reserve_b;
                // 简化的平方根计算
                U256::from(self.sqrt_u256(product))
            }
            Pool::UniswapV3(pool) => {
                // V3 池子直接使用 liquidity 字段
                U256::from(pool.liquidity)
            }
        }
    }

    /// 简化的平方根计算
    fn sqrt_u256(&self, value: U256) -> u128 {
        if value.is_zero() {
            return 0;
        }
        
        // 使用牛顿法计算平方根（简化版本）
        let mut x = value.to::<u128>();
        let mut y = (x + 1) / 2;
        
        while y < x {
            x = y;
            y = (x + value.to::<u128>() / x) / 2;
        }
        
        x
    }
}

/// 池子发现和管理
pub struct PoolDiscovery {
    provider: std::sync::Arc<artemis_core::eth::Provider>,
    known_pools: std::collections::HashMap<Address, Pool>,
}

impl PoolDiscovery {
    pub fn new(provider: std::sync::Arc<artemis_core::eth::Provider>) -> Self {
        Self {
            provider,
            known_pools: std::collections::HashMap::new(),
        }
    }

    /// 发现并缓存常用的 WETH 池子
    pub async fn discover_weth_pools(&mut self) -> anyhow::Result<Vec<Pool>> {
        use crate::contracts::addresses::*;
        
        let mut pools = Vec::new();
        
        // 添加一些知名的 WETH 池子
        let known_weth_pools = vec![
            // WETH/USDC
            ("0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640", *WETH, "0xA0b86a33E6441c0F80A5B0e5e47C5C8c3Bb4e0D0".parse().unwrap(), 500u32),
            // WETH/USDT  
            ("0x4e68Ccd3E89f51C3074ca5072bbAC773960dFa36", *WETH, "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap(), 3000u32),
            // WETH/DAI
            ("0xC2e9F25Be6257c210d7Adf0D4Cd6E3E881ba25f8", *WETH, "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap(), 3000u32),
        ];

        for (pool_addr, token_a, token_b, fee) in known_weth_pools {
            let pool_address: Address = pool_addr.parse()?;
            
            // 创建 V3 池子（大多数主要池子都是 V3）
            let pool = Pool::UniswapV3(UniswapV3Pool {
                address: pool_address,
                token_a,
                token_b,
                fee,
                sqrt_price_x96: U256::from(1u64 << 96), // 1:1 价格
                liquidity: 1_000_000_000_000_000_000u128, // 1000 ETH 等值流动性
                tick: 0,
            });
            
            self.known_pools.insert(pool_address, pool.clone());
            pools.push(pool);
        }
        
        tracing::info!("✅ 发现 {} 个 WETH 池子", pools.len());
        
        Ok(pools)
    }

    /// 根据代币对查找池子
    pub fn find_pool_for_tokens(&self, token_a: Address, token_b: Address) -> Option<&Pool> {
        for pool in self.known_pools.values() {
            let (pool_token_a, pool_token_b) = pool.tokens();
            if (pool_token_a == token_a && pool_token_b == token_b) ||
               (pool_token_a == token_b && pool_token_b == token_a) {
                return Some(pool);
            }
        }
        None
    }

    /// 获取所有池子
    pub fn get_all_pools(&self) -> &std::collections::HashMap<Address, Pool> {
        &self.known_pools
    }

    /// 更新池子状态
    pub async fn update_pool_state(&mut self, pool_address: Address) -> anyhow::Result<()> {
        use alloy_provider::Provider as ProviderTrait;
        use crate::contracts::UniswapV2Pool;
        
        if let Some(pool) = self.known_pools.get_mut(&pool_address) {
            match pool {
                Pool::UniswapV2(v2_pool) => {
                    // 更新 V2 池子储备量
                    let contract = UniswapV2Pool::new(pool_address, &self.provider);
                    if let Ok((reserve0, reserve1, _)) = contract.getReserves().call().await {
                        v2_pool.reserve_a = U256::from(reserve0);
                        v2_pool.reserve_b = U256::from(reserve1);
                    }
                }
                Pool::UniswapV3(v3_pool) => {
                    // 更新 V3 池子状态
                    let contract = crate::contracts::UniswapV3Pool::new(pool_address, &self.provider);
                    if let Ok((sqrt_price_x96, tick, ..)) = contract.slot0().call().await {
                        v3_pool.sqrt_price_x96 = U256::from(sqrt_price_x96);
                        v3_pool.tick = tick;
                    }
                    
                    if let Ok(liquidity) = contract.liquidity().call().await {
                        v3_pool.liquidity = liquidity;
                    }
                }
            }
        }
        
        Ok(())
    }
}
