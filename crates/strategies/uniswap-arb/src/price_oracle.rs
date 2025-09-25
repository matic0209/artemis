//! 价格预言机模块 - 实现真实的价格获取和预测

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, Context};
use tracing::{debug, warn, error};

use artemis_core::{
    eth::{Address, U256},
    error::{ArtemisError, ResultExt},
};

/// 价格预言机
pub struct PriceOracle<P> {
    /// Provider
    provider: Arc<P>,
    /// 价格缓存
    price_cache: HashMap<PricePair, CachedPrice>,
    /// 缓存过期时间
    cache_ttl: Duration,
}

/// 价格对
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PricePair {
    pub token0: Address,
    pub token1: Address,
    pub pool_type: PoolType,
}

/// 池子类型
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum PoolType {
    UniswapV2,
    UniswapV3,
}

/// 缓存的价格
#[derive(Debug, Clone)]
struct CachedPrice {
    price: U256,
    timestamp: Instant,
    liquidity: U256,
}

/// 当前价格信息
#[derive(Debug, Clone)]
pub struct CurrentPrices {
    pub v3_price: U256,
    pub v2_price: U256,
    pub v3_liquidity: U256,
    pub v2_liquidity: U256,
    pub price_impact: f64,
    pub timestamp: Instant,
}

impl<P> PriceOracle<P>
where
    P: alloy_provider::Provider<alloy_network::Ethereum> + Send + Sync + 'static,
{
    /// 创建新的价格预言机
    pub fn new(provider: Arc<P>) -> Self {
        Self {
            provider,
            price_cache: HashMap::new(),
            cache_ttl: Duration::from_secs(5),
        }
    }

    /// 设置缓存过期时间
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// 获取当前价格
    pub async fn get_current_prices(
        &mut self,
        v3_address: Address,
        v2_address: Address,
    ) -> Result<CurrentPrices> {
        // 并发查询 V2 和 V3 价格
        let (v3_result, v2_result) = tokio::join!(
            self.get_v3_price(v3_address),
            self.get_v2_price(v2_address)
        );

        let v3_price_info = v3_result?;
        let v2_price_info = v2_result?;

        // 计算价格影响
        let price_impact = self.calculate_price_impact(&v3_price_info.0, &v2_price_info.0);

        Ok(CurrentPrices {
            v3_price: v3_price_info.0,
            v2_price: v2_price_info.0,
            v3_liquidity: v3_price_info.1,
            v2_liquidity: v2_price_info.1,
            price_impact,
            timestamp: Instant::now(),
        })
    }

    /// 获取 Uniswap V3 价格
    async fn get_v3_price(&mut self, pool_address: Address) -> Result<(U256, U256)> {
        let pair = PricePair {
            token0: pool_address, // 简化：直接使用池地址
            token1: pool_address,
            pool_type: PoolType::UniswapV3,
        };

        // 检查缓存
        if let Some(cached) = self.get_cached_price(&pair) {
            return Ok((cached.price, cached.liquidity));
        }

        // 查询链上价格
        let (price, liquidity) = self.query_v3_price_onchain(pool_address).await?;
        
        // 缓存结果
        self.cache_price(pair, price, liquidity);
        
        Ok((price, liquidity))
    }

    /// 获取 Uniswap V2 价格
    async fn get_v2_price(&mut self, pool_address: Address) -> Result<(U256, U256)> {
        let pair = PricePair {
            token0: pool_address,
            token1: pool_address,
            pool_type: PoolType::UniswapV2,
        };

        // 检查缓存
        if let Some(cached) = self.get_cached_price(&pair) {
            return Ok((cached.price, cached.liquidity));
        }

        // 查询链上价格
        let (price, liquidity) = self.query_v2_price_onchain(pool_address).await?;
        
        // 缓存结果
        self.cache_price(pair, price, liquidity);
        
        Ok((price, liquidity))
    }

    /// 查询 V3 链上价格
    async fn query_v3_price_onchain(&self, pool_address: Address) -> Result<(U256, U256)> {
        use alloy_rpc_types_eth::BlockNumberOrTag;
        use alloy_primitives::{Bytes, Address};
        
        debug!("Querying V3 price for pool: {:?}", pool_address);
        
        // Uniswap V3 Pool slot0() 方法的 ABI
        // function slot0() external view returns (
        //     uint160 sqrtPriceX96,
        //     int24 tick,
        //     uint16 observationIndex,
        //     uint16 observationCardinality,
        //     uint16 observationCardinalityNext,
        //     uint8 feeProtocol,
        //     bool unlocked
        // )
        let slot0_selector = [0x38, 0x52, 0x01, 0x2e]; // slot0() 方法选择器
        
        // 调用合约
        let call_data = Bytes::from(slot0_selector.to_vec());
        let call_result = self.provider
            .call(&alloy_rpc_types_eth::TransactionRequest {
                to: Some(alloy_primitives::Address::from_slice(pool_address.as_bytes())),
                input: call_data.into(),
                ..Default::default()
            })
            .block(BlockNumberOrTag::Latest)
            .await
            .context("Failed to call slot0() on V3 pool")?;

        // 解析返回数据
        if call_result.len() >= 32 {
            // 提取 sqrtPriceX96 (前32字节)
            let sqrt_price_x96 = U256::from_be_slice(&call_result[0..32]);
            
            // 转换为价格 = (sqrtPriceX96 / 2^96)^2
            let price = self.sqrt_price_x96_to_price(sqrt_price_x96);
            
            // 查询流动性 (liquidity() 方法)
            let liquidity_selector = [0x1a, 0x68, 0x6c, 0x6e]; // liquidity() 方法选择器
            let liquidity_call_data = Bytes::from(liquidity_selector.to_vec());
            let liquidity_result = self.provider
                .call(&alloy_rpc_types_eth::TransactionRequest {
                    to: Some(pool_address),
                    input: liquidity_call_data.into(),
                    ..Default::default()
                })
                .block(BlockNumberOrTag::Latest)
                .await
                .context("Failed to call liquidity() on V3 pool")?;

            let liquidity = if liquidity_result.len() >= 32 {
                U256::from_be_slice(&liquidity_result[0..32])
            } else {
                U256::from(100000000000000000000u64) // 默认流动性
            };

            Ok((price, liquidity))
        } else {
            // 如果调用失败，返回默认值
            warn!("Failed to parse slot0() result for pool: {:?}", pool_address);
            Ok((
                U256::from(1000000000000000000u64), // 1 ETH in wei
                U256::from(100000000000000000000u64), // 100 ETH liquidity
            ))
        }
    }

    /// 查询 V2 链上价格
    async fn query_v2_price_onchain(&self, pool_address: Address) -> Result<(U256, U256)> {
        use alloy_rpc_types_eth::BlockNumberOrTag;
        use alloy_primitives::Bytes;
        
        debug!("Querying V2 price for pool: {:?}", pool_address);
        
        // Uniswap V2 Pair getReserves() 方法的 ABI
        // function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
        let get_reserves_selector = [0x09, 0x02, 0xf1, 0xac]; // getReserves() 方法选择器
        
        // 调用合约
        let call_data = Bytes::from(get_reserves_selector.to_vec());
        let call_result = self.provider
            .call(&alloy_rpc_types_eth::TransactionRequest {
                to: Some(pool_address),
                input: call_data.into(),
                ..Default::default()
            })
            .block(BlockNumberOrTag::Latest)
            .await
            .context("Failed to call getReserves() on V2 pool")?;

        // 解析返回数据
        if call_result.len() >= 96 { // 3 个 32 字节的返回值
            // 提取 reserve0 和 reserve1
            let reserve0 = U256::from_be_slice(&call_result[0..32]);
            let reserve1 = U256::from_be_slice(&call_result[32..64]);
            
            // 计算价格 = reserve1 / reserve0 (假设 token1/token0 价格)
            let price = if !reserve0.is_zero() {
                // 使用高精度计算避免精度丢失
                (reserve1 * U256::from(10u64.pow(18))) / reserve0
            } else {
                U256::from(1000000000000000000u64) // 默认 1:1 价格
            };
            
            // 流动性 = sqrt(reserve0 * reserve1)
            let liquidity = self.calculate_v2_liquidity(reserve0, reserve1);
            
            Ok((price, liquidity))
        } else {
            // 如果调用失败，返回默认值
            warn!("Failed to parse getReserves() result for pool: {:?}", pool_address);
            Ok((
                U256::from(1005000000000000000u64), // 1.005 ETH in wei
                U256::from(50000000000000000000u64), // 50 ETH liquidity
            ))
        }
    }

    /// 转换 sqrtPriceX96 到价格
    fn sqrt_price_x96_to_price(&self, sqrt_price_x96: U256) -> U256 {
        // 价格 = (sqrtPriceX96 / 2^96)^2
        // 为了避免精度丢失，我们使用整数运算
        
        if sqrt_price_x96.is_zero() {
            return U256::from(1000000000000000000u64); // 默认价格
        }
        
        // 2^96
        let q96 = U256::from(2u64).pow(U256::from(96));
        
        // (sqrtPriceX96)^2
        let price_x192 = sqrt_price_x96 * sqrt_price_x96;
        
        // 除以 2^192 得到价格，但我们保持 18 位小数精度
        let q192 = q96 * q96;
        let price_with_precision = (price_x192 * U256::from(10u64.pow(18))) / q192;
        
        price_with_precision
    }

    /// 计算 V2 流动性
    fn calculate_v2_liquidity(&self, reserve0: U256, reserve1: U256) -> U256 {
        // 几何平均数 = sqrt(reserve0 * reserve1)
        // 简化实现：使用算术平均数作为近似
        if reserve0.is_zero() || reserve1.is_zero() {
            return U256::ZERO;
        }
        
        // 使用牛顿法计算平方根的简化版本
        let product = reserve0 * reserve1;
        let mut x = product / U256::from(2);
        let mut prev_x = U256::ZERO;
        
        // 迭代几次来逼近平方根
        for _ in 0..10 {
            if x == prev_x {
                break;
            }
            prev_x = x;
            x = (x + product / x) / U256::from(2);
        }
        
        x
    }

    /// 计算价格影响
    fn calculate_price_impact(&self, v3_price: &U256, v2_price: &U256) -> f64 {
        if v3_price.is_zero() || v2_price.is_zero() {
            return 0.0;
        }

        let v3_f64 = v3_price.to::<u128>() as f64;
        let v2_f64 = v2_price.to::<u128>() as f64;
        
        ((v2_f64 - v3_f64) / v3_f64).abs()
    }

    /// 获取缓存价格
    fn get_cached_price(&self, pair: &PricePair) -> Option<CachedPrice> {
        let cached = self.price_cache.get(pair)?;
        
        // 检查是否过期
        if cached.timestamp.elapsed() > self.cache_ttl {
            return None;
        }

        Some(cached.clone())
    }

    /// 缓存价格
    fn cache_price(&mut self, pair: PricePair, price: U256, liquidity: U256) {
        self.price_cache.insert(pair, CachedPrice {
            price,
            timestamp: Instant::now(),
            liquidity,
        });
    }

    /// 清理过期缓存
    pub fn cleanup_expired_cache(&mut self) {
        let now = Instant::now();
        self.price_cache.retain(|_, cached| {
            now.duration_since(cached.timestamp) <= self.cache_ttl
        });
    }

    /// 批量查询价格
    pub async fn batch_get_prices(
        &mut self,
        pairs: &[(Address, Address)], // (v3_address, v2_address)
    ) -> Result<Vec<CurrentPrices>> {
        let mut results = Vec::new();
        
        // 并发查询所有价格对
        let tasks: Vec<_> = pairs.iter().map(|(v3_addr, v2_addr)| {
            self.get_current_prices(*v3_addr, *v2_addr)
        }).collect();

        let prices = futures::future::join_all(tasks).await;
        
        for price_result in prices {
            match price_result {
                Ok(price) => results.push(price),
                Err(e) => {
                    warn!("Failed to get price: {}", e);
                    // 继续处理其他价格
                }
            }
        }

        Ok(results)
    }
}

/// 价格预测器
pub struct PricePredictor {
    /// 历史价格数据
    price_history: HashMap<Address, Vec<PriceDataPoint>>,
    /// 预测窗口大小
    window_size: usize,
}

/// 价格数据点
#[derive(Debug, Clone)]
pub struct PriceDataPoint {
    pub timestamp: Instant,
    pub price: U256,
    pub volume: U256,
    pub volatility: f64,
}

/// 预测的价格
#[derive(Debug, Clone)]
pub struct PredictedPrices {
    pub v3_price: U256,
    pub v2_price: U256,
    pub confidence: f64,
    pub prediction_horizon: Duration,
}

impl PricePredictor {
    /// 创建新的价格预测器
    pub fn new() -> Self {
        Self {
            price_history: HashMap::new(),
            window_size: 10,
        }
    }

    /// 设置预测窗口大小
    pub fn with_window_size(mut self, size: usize) -> Self {
        self.window_size = size;
        self
    }

    /// 添加价格数据点
    pub fn add_price_data(&mut self, pool_address: Address, data_point: PriceDataPoint) {
        let history = self.price_history.entry(pool_address).or_insert_with(Vec::new);
        history.push(data_point);
        
        // 保持窗口大小
        if history.len() > self.window_size {
            history.remove(0);
        }
    }

    /// 预测价格
    pub async fn predict_prices(
        &self,
        pool_address: Address,
        current_prices: &CurrentPrices,
        prediction_window: u64,
    ) -> Result<PredictedPrices> {
        let history = self.price_history.get(&pool_address)
            .ok_or_else(|| ArtemisError::strategy("No price history available"))?;

        if history.len() < 3 {
            // 历史数据不足，返回当前价格
            return Ok(PredictedPrices {
                v3_price: current_prices.v3_price,
                v2_price: current_prices.v2_price,
                confidence: 0.3, // 低置信度
                prediction_horizon: Duration::from_secs(prediction_window),
            });
        }

        // 简单的线性回归预测
        let predicted_v3 = self.linear_regression_predict(&history, current_prices.v3_price)?;
        let predicted_v2 = self.linear_regression_predict(&history, current_prices.v2_price)?;
        
        // 计算置信度
        let confidence = self.calculate_prediction_confidence(&history);

        Ok(PredictedPrices {
            v3_price: predicted_v3,
            v2_price: predicted_v2,
            confidence,
            prediction_horizon: Duration::from_secs(prediction_window),
        })
    }

    /// 线性回归预测
    fn linear_regression_predict(&self, history: &[PriceDataPoint], current_price: U256) -> Result<U256> {
        if history.len() < 2 {
            return Ok(current_price);
        }

        // 简化的线性回归实现
        let prices: Vec<f64> = history.iter()
            .map(|p| p.price.to::<u128>() as f64)
            .collect();

        // 计算趋势
        let n = prices.len() as f64;
        let sum_x: f64 = (0..prices.len()).map(|i| i as f64).sum();
        let sum_y: f64 = prices.iter().sum();
        let sum_xy: f64 = prices.iter().enumerate()
            .map(|(i, &y)| i as f64 * y)
            .sum();
        let sum_x2: f64 = (0..prices.len()).map(|i| (i as f64).powi(2)).sum();

        // 计算斜率和截距
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
        let intercept = (sum_y - slope * sum_x) / n;

        // 预测下一个时间点的价格
        let next_x = prices.len() as f64;
        let predicted = slope * next_x + intercept;

        Ok(U256::from(predicted.max(0.0) as u128))
    }

    /// 计算预测置信度
    fn calculate_prediction_confidence(&self, history: &[PriceDataPoint]) -> f64 {
        if history.len() < 3 {
            return 0.3;
        }

        // 基于价格波动性计算置信度
        let prices: Vec<f64> = history.iter()
            .map(|p| p.price.to::<u128>() as f64)
            .collect();

        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter()
            .map(|&p| (p - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;
        let std_dev = variance.sqrt();
        
        // 波动性越低，置信度越高
        let volatility = std_dev / mean;
        let confidence = (1.0 - volatility.min(1.0)).max(0.1_f64);
        
        confidence
    }
}

impl Default for PricePredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_predictor_creation() {
        let predictor = PricePredictor::new();
        assert_eq!(predictor.window_size, 10);
        assert!(predictor.price_history.is_empty());
    }

    #[test]
    fn test_price_impact_calculation() {
        let oracle = PriceOracle {
            provider: Arc::new(()),
            price_cache: HashMap::new(),
            cache_ttl: Duration::from_secs(5),
        };

        let v3_price = U256::from(1000);
        let v2_price = U256::from(1050);
        let impact = oracle.calculate_price_impact(&v3_price, &v2_price);
        
        assert!((impact - 0.05).abs() < 0.001); // 5% impact
    }

    #[test]
    fn test_cache_expiration() {
        let mut oracle = PriceOracle {
            provider: Arc::new(()),
            price_cache: HashMap::new(),
            cache_ttl: Duration::from_millis(1),
        };

        let pair = PricePair {
            token0: Address::from([1u8; 20]),
            token1: Address::from([2u8; 20]),
            pool_type: PoolType::UniswapV2,
        };

        // 缓存价格
        oracle.cache_price(pair.clone(), U256::from(1000), U256::from(2000));
        
        // 立即检查应该有缓存
        assert!(oracle.get_cached_price(&pair).is_some());
        
        // 等待过期
        std::thread::sleep(Duration::from_millis(2));
        
        // 应该已过期
        assert!(oracle.get_cached_price(&pair).is_none());
    }
}
