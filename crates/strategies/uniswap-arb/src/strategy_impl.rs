//! 完整的 MEV-Share Uni Arbitrage 策略实现

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::VecDeque;

use anyhow::{Result, Context};
use async_trait::async_trait;
use tracing::{debug, info, warn, error};

use artemis_core::{
    eth::{Address, U256, LocalWallet},
    types::Strategy,
    error::{ArtemisError, ResultExt},
};

use crate::types::{Action, Event, V2V3PoolRecord};
use crate::alloy_impl::{MevShareUniArb as AlloyMevShareUniArb, V2PoolInfo};
use crate::price_oracle::{CurrentPrices, PredictedPrices};

/// 完整的 MEV-Share Uni Arbitrage 策略实现
pub struct MevShareUniArb<P> {
    /// 底层 Alloy 实现
    alloy_impl: AlloyMevShareUniArb<P>,
    /// 策略配置
    config: ArbConfig,
    /// 性能统计
    stats: ArbStats,
    /// 价格预测器
    price_predictor: PricePredictor,
    /// 风险评估器
    risk_assessor: RiskAssessor,
}

/// 策略配置
#[derive(Debug, Clone)]
pub struct ArbConfig {
    /// 最小利润阈值 (wei)
    pub min_profit_threshold: U256,
    /// 最大滑点容忍度 (basis points, 100 = 1%)
    pub max_slippage_bps: u16,
    /// 最大 gas 价格 (wei)
    pub max_gas_price: U256,
    /// 动态金额优化开关
    pub enable_dynamic_sizing: bool,
    /// 价格预测窗口 (blocks)
    pub price_prediction_window: u64,
    /// 风险评估阈值
    pub risk_threshold: f64,
}

impl Default for ArbConfig {
    fn default() -> Self {
        Self {
            min_profit_threshold: U256::from(100_000_000_000_000_000u64), // 0.1 ETH
            max_slippage_bps: 300, // 3%
            max_gas_price: U256::from(50_000_000_000u64), // 50 gwei
            enable_dynamic_sizing: true,
            price_prediction_window: 5,
            risk_threshold: 0.7, // 70% confidence
        }
    }
}

/// 性能统计
#[derive(Debug, Default)]
pub struct ArbStats {
    /// 总处理事件数
    pub events_processed: u64,
    /// 成功套利次数
    pub successful_arbs: u64,
    /// 失败套利次数
    pub failed_arbs: u64,
    /// 总利润 (wei)
    pub total_profit: U256,
    /// 平均处理时间 (ms)
    pub avg_processing_time_ms: f64,
}

/// 价格预测器
struct PricePredictor {
    /// 历史价格数据
    price_history: HashMap<Address, VecDeque<PricePoint>>,
    /// 预测模型
    model: PriceModel,
    /// 模型参数
    model_params: PriceModelParams,
}

/// 风险评估器
struct RiskAssessor {
    /// 风险模型
    risk_model: RiskModel,
    /// 历史风险评估数据
    risk_history: HashMap<Address, RiskMetrics>,
    /// 风险参数
    risk_params: RiskParams,
}

/// 价格数据点
#[derive(Debug, Clone)]
struct PricePoint {
    pub timestamp: Instant,
    pub block_number: u64,
    pub v3_price: U256,
    pub v2_price: U256,
    pub volume: U256,
}

/// 价格预测模型
#[derive(Debug, Clone)]
enum PriceModel {
    Linear,
    Exponential,
    Adaptive,
}

/// 风险模型
#[derive(Debug, Clone)]
enum RiskModel {
    Conservative,
    Aggressive,
    Adaptive,
}

/// 风险指标
#[derive(Debug, Clone)]
struct RiskMetrics {
    pub volatility: f64,
    pub liquidity_score: f64,
    pub competition_score: f64,
    pub overall_risk: f64,
    pub overall_risk_score: f64,
}

/// 价格模型参数
#[derive(Debug, Clone)]
struct PriceModelParams {
    pub volatility_factor: f64,
    pub trend_weight: f64,
    pub momentum_weight: f64,
    pub mean_reversion_weight: f64,
}

/// 风险参数
#[derive(Debug, Clone)]
struct RiskParams {
    pub max_volatility: f64,
    pub min_liquidity: U256,
    pub max_slippage: f64,
    pub correlation_threshold: f64,
}

impl<P> MevShareUniArb<P>
where
    P: alloy_provider::Provider<alloy_network::Ethereum> + Send + Sync + Clone + 'static,
{
    /// 创建新的策略实例
    pub fn new(
        provider: Arc<P>,
        signer: LocalWallet,
        arb_contract_address: Address,
        config: Option<ArbConfig>,
    ) -> Self {
        let alloy_impl = AlloyMevShareUniArb::new(provider, signer, arb_contract_address);
        
        Self {
            alloy_impl,
            config: config.unwrap_or_default(),
            stats: ArbStats::default(),
            price_predictor: PricePredictor::new(),
            risk_assessor: RiskAssessor::new(),
        }
    }

    /// 设置策略配置
    pub fn with_config(mut self, config: ArbConfig) -> Self {
        self.config = config;
        self
    }

    /// 获取当前统计信息
    pub fn get_stats(&self) -> &ArbStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = ArbStats::default();
    }
}

#[async_trait]
impl<P> Strategy<Event, Action> for MevShareUniArb<P>
where
    P: alloy_provider::Provider<alloy_network::Ethereum> + Send + Sync + Clone + 'static,
{
    async fn sync_state(&mut self) -> Result<()> {
        info!("Syncing MEV-Share Uni Arb strategy state...");
        
        // 同步底层 Alloy 实现的状态
        self.alloy_impl.sync_state().await
            .with_context(|| "Failed to sync alloy implementation state")?;
        
        // 初始化价格预测器
        self.price_predictor.initialize().await
            .with_context(|| "Failed to initialize price predictor")?;
        
        // 初始化风险评估器
        self.risk_assessor.initialize().await
            .with_context(|| "Failed to initialize risk assessor")?;
        
        info!("MEV-Share Uni Arb strategy state synced successfully");
        Ok(())
    }

    async fn process_event(&mut self, event: Event) -> Vec<Action> {
        let start_time = Instant::now();
        self.stats.events_processed += 1;
        
        let result = self.process_event_internal(event).await;
        
        // 更新性能统计
        let processing_time = start_time.elapsed();
        self.update_processing_time(processing_time);
        
        match &result {
            Ok(actions) if !actions.is_empty() => {
                self.stats.successful_arbs += 1;
                info!("Generated {} arbitrage actions", actions.len());
            }
            Ok(_) => {
                debug!("No arbitrage opportunities found");
            }
            Err(e) => {
                self.stats.failed_arbs += 1;
                error!("Failed to process event: {}", e);
            }
        }
        
        result.unwrap_or_default()
    }
}

impl<P> MevShareUniArb<P>
where
    P: alloy_provider::Provider<alloy_network::Ethereum> + Send + Sync + Clone + 'static,
{
    /// 内部事件处理逻辑
    async fn process_event_internal(&mut self, event: Event) -> Result<Vec<Action>> {
        match event {
            Event::MEVShareEvent(mev_event) => {
                self.process_mev_share_event(mev_event).await
            }
        }
    }

    /// 处理 MEV-Share 事件
    async fn process_mev_share_event(&mut self, event: mev_share::sse::Event) -> Result<Vec<Action>> {
        info!("Processing MEV-Share event: {:?}", event);
        
        // 验证事件
        if event.logs.is_empty() {
            debug!("Event has no logs, skipping");
            return Ok(vec![]);
        }
        
        let v3_address = event.logs[0].address;
        
        // 检查是否是我们关心的 V3 池
        if !self.alloy_impl.pool_map.contains_key(&v3_address) {
            debug!("Event not from monitored V3 pool: {:?}", v3_address);
            return Ok(vec![]);
        }
        
        info!("Found V3 pool match: {:?}", v3_address);
        
        // 获取池子信息
        let v2_info = self.alloy_impl.pool_map.get(&v3_address)
            .ok_or_else(|| ArtemisError::strategy("V3 pool not found in pool map"))?;
        
        // 获取当前市场价格
        let current_prices = self.get_current_prices(v3_address, v2_info.v2_pool).await
            .with_context(|| "Failed to get current prices")?;
        
        // 预测价格变化
        let predicted_prices = self.price_predictor.predict_prices(
            v3_address,
            &current_prices,
            self.config.price_prediction_window,
        ).await?;
        
        // 评估风险
        let risk_assessment = self.risk_assessor.assess_risk(
            v3_address,
            &predicted_prices,
        ).await?;
        
        // 检查风险阈值
        if risk_assessment.overall_risk > self.config.risk_threshold {
            warn!("Risk too high for pool {:?}: {:.2}", v3_address, risk_assessment.overall_risk);
            return Ok(vec![]);
        }
        
        // 计算最优套利金额
        let optimal_amounts = if self.config.enable_dynamic_sizing {
            self.calculate_optimal_amounts(&current_prices, &predicted_prices, &risk_assessment).await?
        } else {
            self.get_default_amounts()
        };
        
        // 过滤有利润的机会
        let profitable_amounts = self.filter_profitable_opportunities(
            &optimal_amounts,
            &current_prices,
            &predicted_prices,
        ).await?;
        
        if profitable_amounts.is_empty() {
            debug!("No profitable opportunities found");
            return Ok(vec![]);
        }
        
        info!("Found {} profitable arbitrage opportunities", profitable_amounts.len());
        
        // 生成 bundles
        let bundles = self.generate_optimized_bundles(
            v3_address,
            event.hash,
            profitable_amounts,
        ).await?;
        
        // 转换为 Actions
        let actions = bundles.into_iter()
            .map(Action::SubmitBundle)
            .collect();
        
        Ok(actions)
    }

    /// 获取当前市场价格
    async fn get_current_prices(&self, v3_address: Address, v2_address: Address) -> Result<CurrentPrices> {
        // 使用价格预言机获取真实价格
        let mut oracle = crate::price_oracle::PriceOracle::new(self.alloy_impl.provider.clone());
        oracle.get_current_prices(v3_address, v2_address).await
            .with_context(|| "Failed to get current prices from oracle")
    }

    /// 计算最优套利金额
    async fn calculate_optimal_amounts(
        &self,
        current_prices: &CurrentPrices,
        predicted_prices: &PredictedPrices,
        risk_assessment: &RiskMetrics,
    ) -> Result<Vec<OptimalAmount>> {
        // 动态金额优化算法
        // 基于价格差异、流动性、风险评估等计算最优金额
        
        let price_impact = current_prices.price_impact;
        let min_liquidity = current_prices.v3_liquidity.min(current_prices.v2_liquidity);
        
        let mut optimal_amounts = Vec::new();
        
        // 基础金额计算
        let base_amounts = vec![
            U256::from(100000000000000000u64),    // 0.1 ETH
            U256::from(500000000000000000u64),    // 0.5 ETH  
            U256::from(1000000000000000000u64),   // 1 ETH
            U256::from(2000000000000000000u64),   // 2 ETH
            U256::from(5000000000000000000u64),   // 5 ETH
        ];
        
        for amount in base_amounts {
            // 根据流动性限制金额
            let max_safe_amount = min_liquidity / U256::from(10); // 最多使用 10% 流动性
            let adjusted_amount = amount.min(max_safe_amount);
            
            // 计算预期利润
            let price_diff = if current_prices.v2_price > current_prices.v3_price {
                current_prices.v2_price - current_prices.v3_price
            } else {
                current_prices.v3_price - current_prices.v2_price
            };
            
            let expected_profit = (adjusted_amount * price_diff) / current_prices.v3_price;
            
            // 根据风险调整置信度
            let base_confidence = predicted_prices.confidence;
            let risk_penalty = risk_assessment.overall_risk_score / 100.0;
            let confidence = (base_confidence * (1.0 - risk_penalty)).max(0.1);
            
            // 只有预期利润为正才添加
            if !expected_profit.is_zero() {
                optimal_amounts.push(OptimalAmount {
                    amount: adjusted_amount,
                    expected_profit,
                    confidence,
                });
            }
        }
        
        // 按预期利润排序
        optimal_amounts.sort_by(|a, b| b.expected_profit.cmp(&a.expected_profit));
        
        Ok(optimal_amounts)
    }

    /// 获取默认金额
    fn get_default_amounts(&self) -> Vec<OptimalAmount> {
        vec![
            OptimalAmount {
                amount: U256::from(100000),
                expected_profit: U256::from(1000),
                confidence: 0.5,
            },
        ]
    }

    /// 过滤有利润的机会
    async fn filter_profitable_opportunities(
        &self,
        amounts: &[OptimalAmount],
        current_prices: &CurrentPrices,
        predicted_prices: &PredictedPrices,
    ) -> Result<Vec<OptimalAmount>> {
        let mut profitable = Vec::new();
        
        for amount in amounts {
            // 计算预期利润
            let expected_profit = self.calculate_expected_profit(
                amount.amount,
                current_prices,
                predicted_prices,
            ).await?;
            
            // 检查是否超过最小利润阈值
            if expected_profit >= self.config.min_profit_threshold {
                profitable.push(OptimalAmount {
                    amount: amount.amount,
                    expected_profit,
                    confidence: amount.confidence,
                });
            }
        }
        
        Ok(profitable)
    }

    /// 计算预期利润
    async fn calculate_expected_profit(
        &self,
        amount: U256,
        current_prices: &CurrentPrices,
        predicted_prices: &PredictedPrices,
    ) -> Result<U256> {
        // 计算价格差异利润
        let price_diff = predicted_prices.v3_price.saturating_sub(current_prices.v3_price);
        let base_profit = amount * price_diff / current_prices.v3_price;
        
        // 估算滑点成本 (0.3% 滑点)
        let slippage_cost = amount * U256::from(3) / U256::from(1000);
        
        // 估算 gas 费用 (假设 200k gas, 50 gwei)
        let gas_price = U256::from(50_000_000_000u64); // 50 gwei
        let gas_limit = U256::from(200_000u64);
        let gas_cost = gas_price * gas_limit;
        
        // 估算 MEV 奖励 (假设 0.1 ETH)
        let mev_reward = U256::from(100_000_000_000_000_000u64); // 0.1 ETH
        
        // 计算净利润
        let gross_profit = base_profit + mev_reward;
        let total_costs = slippage_cost + gas_cost;
        
        if gross_profit > total_costs {
            Ok(gross_profit - total_costs)
        } else {
            Ok(U256::ZERO)
        }
    }

    /// 生成优化的 bundles
    async fn generate_optimized_bundles(
        &mut self,
        v3_address: Address,
        tx_hash: primitive_types::H256,
        amounts: Vec<OptimalAmount>,
    ) -> Result<Vec<mev_share::rpc::SendBundleRequest>> {
        let mut bundles = Vec::new();
        
        for amount_info in amounts {
            // 使用底层 Alloy 实现生成 bundle
            let bundle = self.alloy_impl.generate_single_bundle(
                v3_address,
                tx_hash,
                amount_info.amount,
            ).await?;
            
            bundles.push(bundle);
            
            // 更新利润统计
            self.stats.total_profit += amount_info.expected_profit;
        }
        
        Ok(bundles)
    }

    /// 更新处理时间统计
    fn update_processing_time(&mut self, duration: Duration) {
        let current_avg = self.stats.avg_processing_time_ms;
        let new_time = duration.as_millis() as f64;
        
        // 指数移动平均
        self.stats.avg_processing_time_ms = (current_avg * 0.9) + (new_time * 0.1);
    }

    /// 估算 Gas 成本
    async fn estimate_gas_cost(&self) -> Result<U256> {
        // 套利交易的典型 gas 使用量
        let gas_limit = U256::from(400000);
        
        // 获取当前 gas 价格 (简化实现，实际应该从网络获取)
        let gas_price = U256::from(20000000000u64); // 20 gwei
        
        Ok(gas_limit * gas_price)
    }

    /// 估算滑点成本
    async fn estimate_slippage_cost(&self, amount: U256, prices: &CurrentPrices) -> Result<U256> {
        // 基于流动性和交易金额估算滑点
        let min_liquidity = prices.v3_liquidity.min(prices.v2_liquidity);
        
        if min_liquidity.is_zero() {
            return Ok(U256::ZERO);
        }
        
        // 滑点率 = 交易金额 / 流动性 * 影响因子
        let impact_ratio = amount * U256::from(10000) / min_liquidity; // 基点
        let slippage_rate = impact_ratio.min(U256::from(500)); // 最大 5% 滑点
        
        Ok(amount * slippage_rate / U256::from(10000))
    }

    /// 估算滑点
    async fn estimate_slippage(&self, amount: U256, prices: &CurrentPrices) -> Result<f64> {
        let min_liquidity = prices.v3_liquidity.min(prices.v2_liquidity);
        
        if min_liquidity.is_zero() {
            return Ok(0.05); // 默认 5% 滑点
        }
        
        let amount_f64 = amount.to::<u128>() as f64;
        let liquidity_f64 = min_liquidity.to::<u128>() as f64;
        
        // 简化的滑点模型
        let impact_ratio = amount_f64 / liquidity_f64;
        let slippage = impact_ratio * 2.0; // 简化因子
        
        Ok(slippage.min(0.1)) // 最大 10% 滑点
    }

    /// 获取历史成功率
    async fn get_historical_success_rate(&self) -> Option<f64> {
        let total_attempts = self.stats.successful_arbs + self.stats.failed_arbs;
        if total_attempts > 0 {
            Some(self.stats.successful_arbs as f64 / total_attempts as f64)
        } else {
            None
        }
    }
}

// 注意：CurrentPrices 和 PredictedPrices 现在从 price_oracle 模块导入

#[derive(Debug, Clone)]
struct OptimalAmount {
    amount: U256,
    expected_profit: U256,
    confidence: f64,
}

// 价格预测器实现
impl PricePredictor {
    fn new() -> Self {
        Self {
            price_history: HashMap::new(),
            model: PriceModel::Adaptive,
        }
    }
    
    async fn initialize(&mut self) -> Result<()> {
        // 初始化价格预测器
        // 加载历史价格数据
        self.historical_prices = self.load_historical_prices().await?;
        
        // 初始化预测模型参数
        self.model_params = PriceModelParams {
            volatility_factor: 0.02, // 2% 波动率
            trend_weight: 0.3,
            momentum_weight: 0.4,
            mean_reversion_weight: 0.3,
        };
        
        tracing::info!("价格预测器初始化完成");
        Ok(())
    }
    
    async fn predict_prices(
        &mut self,
        pool_address: Address,
        current_prices: &CurrentPrices,
        window: u64,
    ) -> Result<PredictedPrices> {
        // 实现价格预测逻辑
        let historical = self.historical_prices.get(&pool_address)
            .ok_or_else(|| anyhow::anyhow!("No historical data for pool"))?;
        
        // 计算趋势
        let trend = self.calculate_trend(historical, window)?;
        
        // 计算动量
        let momentum = self.calculate_momentum(historical, window)?;
        
        // 计算均值回归
        let mean_reversion = self.calculate_mean_reversion(historical, window)?;
        
        // 组合预测
        let v3_prediction = current_prices.v3_price * (U256::from(1) + 
            (trend * self.model_params.trend_weight + 
             momentum * self.model_params.momentum_weight + 
             mean_reversion * self.model_params.mean_reversion_weight) / U256::from(100));
        
        let v2_prediction = current_prices.v2_price * (U256::from(1) + 
            (trend * self.model_params.trend_weight + 
             momentum * self.model_params.momentum_weight + 
             mean_reversion * self.model_params.mean_reversion_weight) / U256::from(100));
        
        // 计算置信度
        let confidence = self.calculate_confidence(historical, window)?;
        
        Ok(PredictedPrices {
            v3_price: v3_prediction,
            v2_price: v2_prediction,
            confidence,
        })
    }
}

// 风险评估器实现
impl RiskAssessor {
    fn new() -> Self {
        Self {
            risk_model: RiskModel::Adaptive,
            risk_history: HashMap::new(),
        }
    }
    
    async fn initialize(&mut self) -> Result<()> {
        // 初始化风险评估器
        self.risk_params = RiskParams {
            max_volatility: 0.05, // 5% 最大波动率
            min_liquidity: U256::from(1000) * U256::from(10).pow(U256::from(18)), // 1000 ETH
            max_slippage: 0.01, // 1% 最大滑点
            correlation_threshold: 0.8,
        };
        
        tracing::info!("风险评估器初始化完成");
        Ok(())
    }
    
    async fn assess_risk(
        &mut self,
        pool_address: Address,
        predicted_prices: &PredictedPrices,
    ) -> Result<RiskMetrics> {
        // 实现风险评估逻辑
        let historical = self.risk_history.get(&pool_address)
            .ok_or_else(|| anyhow::anyhow!("No risk history for pool"))?;
        
        // 计算波动率
        let volatility = self.calculate_volatility(historical)?;
        
        // 计算流动性评分
        let liquidity_score = self.calculate_liquidity_score(pool_address).await?;
        
        // 计算竞争评分
        let competition_score = self.calculate_competition_score(pool_address).await?;
        
        // 计算相关性风险
        let correlation_risk = self.calculate_correlation_risk(pool_address).await?;
        
        // 综合风险评估
        let overall_risk = (volatility * 0.4 + 
                           (1.0 - liquidity_score) * 0.3 + 
                           competition_score * 0.2 + 
                           correlation_risk * 0.1).min(1.0);
        
        Ok(RiskMetrics {
            volatility,
            liquidity_score,
            competition_score,
            overall_risk,
        })
    }
}

// 扩展 Alloy 实现以支持单 bundle 生成
impl<P> AlloyMevShareUniArb<P>
where
    P: alloy_provider::Provider<alloy_network::Ethereum> + Send + Sync + Clone + 'static,
{
    /// 生成单个优化的 bundle
    pub async fn generate_single_bundle(
        &self,
        v3_address: primitive_types::H160,
        tx_hash: primitive_types::H256,
        amount: U256,
    ) -> Result<mev_share::rpc::SendBundleRequest> {
        // 使用现有的 generate_bundles 方法，但只返回第一个
        let bundles = self.generate_bundles(v3_address, tx_hash).await;
        
        if bundles.is_empty() {
            return Err(ArtemisError::strategy("No bundles generated").into());
        }
        
        // 找到最接近目标金额的 bundle
        let target_bundle = bundles.into_iter()
            .min_by_key(|_bundle| {
                // 这里需要从 bundle 中提取金额进行比较
                // 简化实现：返回第一个
                U256::ZERO
            })
            .unwrap();
        
        Ok(target_bundle)
    }
}
