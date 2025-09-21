use std::sync::Arc;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tracing::{info, warn, error};

use artemis_core::{
    eth::Provider,
    types::Executor,
    executors::flashbots_alloy_executor::FlashbotsAlloyExecutor,
};
use alloy_mev::{EthMevProviderExt, Endpoints};

use crate::types::{SandwichBundle, SandwichResult, SandwichStats};

/// 专门用于 Sandwich 攻击的优化执行器
pub struct SandwichExecutor {
    /// Flashbots 执行器（备用）
    flashbots_executor: FlashbotsAlloyExecutor,
    /// rbuilder 执行器（主要）
    rbuilder_enabled: bool,
    /// 执行统计
    stats: SandwichStats,
    /// Provider
    provider: Arc<Provider>,
}

impl SandwichExecutor {
    pub fn new(provider: Arc<Provider>, enable_rbuilder: bool) -> Self {
        // 创建 Flashbots 执行器作为备用
        let flashbots_endpoints = Endpoints::flashbots();
        let flashbots_executor = FlashbotsAlloyExecutor::new(
            Arc::clone(&provider),
            flashbots_endpoints,
        );

        Self {
            flashbots_executor,
            rbuilder_enabled: enable_rbuilder,
            stats: SandwichStats::default(),
            provider,
        }
    }

    /// 智能执行策略：优先使用 rbuilder，失败时降级到 Flashbots
    async fn execute_with_fallback(&mut self, bundle: SandwichBundle) -> Result<SandwichResult> {
        let start_time = std::time::Instant::now();
        
        // 首先尝试 rbuilder（如果启用）
        if self.rbuilder_enabled {
            match self.execute_via_rbuilder(&bundle).await {
                Ok(result) => {
                    info!("🚀 rbuilder 执行成功: {:?}", result.bundle_hash);
                    metrics::counter!("artemis.sandwich.rbuilder_success").increment(1);
                    return Ok(result);
                }
                Err(e) => {
                    warn!("rbuilder 执行失败，降级到 Flashbots: {:?}", e);
                    metrics::counter!("artemis.sandwich.rbuilder_fallback").increment(1);
                }
            }
        }

        // 降级到 Flashbots 执行器
        let result = self.execute_via_flashbots(&bundle).await?;
        
        let execution_time = start_time.elapsed();
        metrics::histogram!("artemis.sandwich.execution_time")
            .record(execution_time.as_millis() as f64);
        
        Ok(result)
    }

    /// 通过 rbuilder 执行（本地优化）
    async fn execute_via_rbuilder(&self, bundle: &SandwichBundle) -> Result<SandwichResult> {
        // 使用 rbuilder 的本地区块构建优化
        
        // 1. 本地模拟验证
        let simulation_result = self.simulate_bundle_locally(bundle).await?;
        if !simulation_result.success {
            return Err(anyhow!("Bundle 模拟失败: {:?}", simulation_result.error));
        }

        // 2. 提交到 rbuilder 进行优化
        let bundle_hash = self.submit_to_rbuilder(bundle).await?;
        
        Ok(SandwichResult {
            bundle_hash,
            expected_profit: bundle.expected_revenue,
            actual_profit: None, // 稍后从链上获取
            gas_used: bundle.estimated_gas,
            success: true,
            error: None,
        })
    }

    /// 通过 Flashbots 执行（标准方式）
    async fn execute_via_flashbots(&self, bundle: &SandwichBundle) -> Result<SandwichResult> {
        // 转换为 Flashbots bundle 格式
        let flashbots_bundle = bundle.clone().into();
        
        // 使用 alloy-mev 执行
        use alloy_mev::EthMevProviderExt;
        
        let bundle_txs = vec![
            bundle.frontrun_tx.clone(),
            bundle.victim_txs[0].clone(), // 简化：只处理单个受害者
            bundle.backrun_tx.clone(),
        ];
        
        match self.provider.send_bundle(&bundle_txs).await {
            Ok(bundle_hash) => {
                info!("✅ Flashbots 提交成功: {:?}", bundle_hash);
                
                Ok(SandwichResult {
                    bundle_hash: artemis_core::eth::Hash::from_slice(&bundle_hash.0),
                    expected_profit: bundle.expected_revenue,
                    actual_profit: None,
                    gas_used: bundle.estimated_gas,
                    success: true,
                    error: None,
                })
            }
            Err(e) => {
                error!("Flashbots 提交失败: {:?}", e);
                
                Ok(SandwichResult {
                    bundle_hash: artemis_core::eth::Hash::ZERO,
                    expected_profit: bundle.expected_revenue,
                    actual_profit: None,
                    gas_used: bundle.estimated_gas,
                    success: false,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// 本地模拟 bundle
    async fn simulate_bundle_locally(&self, bundle: &SandwichBundle) -> Result<SandwichResult> {
        // TODO: 使用 revm 进行本地模拟
        // 1. 设置区块环境
        // 2. 执行前置交易
        // 3. 执行受害者交易
        // 4. 执行后置交易
        // 5. 计算净利润和 gas 使用
        
        // 暂时返回成功的模拟结果
        Ok(SandwichResult {
            bundle_hash: artemis_core::eth::Hash::ZERO,
            expected_profit: bundle.expected_revenue,
            actual_profit: Some(bundle.expected_revenue),
            gas_used: bundle.estimated_gas,
            success: true,
            error: None,
        })
    }

    /// 提交到 rbuilder
    async fn submit_to_rbuilder(&self, bundle: &SandwichBundle) -> Result<artemis_core::eth::Hash> {
        // TODO: 实现 rbuilder JSON-RPC 调用
        // 使用 eth_sendBundle 方法
        
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendBundle",
            "params": [{
                "txs": [
                    bundle.frontrun_tx,
                    bundle.victim_txs[0], // 简化处理
                    bundle.backrun_tx
                ],
                "blockNumber": format!("0x{:x}", bundle.target_block.to::<u64>()),
            }],
            "id": 1
        });

        // TODO: 发送 HTTP 请求到 rbuilder
        // let response = reqwest::Client::new()
        //     .post("http://localhost:8645")
        //     .json(&payload)
        //     .send()
        //     .await?;

        // 暂时返回模拟的哈希
        Ok(artemis_core::eth::Hash::random())
    }

    /// 获取执行统计
    pub fn get_stats(&self) -> &SandwichStats {
        &self.stats
    }

    /// 重置统计
    pub fn reset_stats(&mut self) {
        self.stats = SandwichStats::default();
    }
}

#[async_trait]
impl Executor<SandwichBundle> for SandwichExecutor {
    async fn execute(&self, bundle: SandwichBundle) -> Result<()> {
        let start = std::time::Instant::now();
        
        info!("🥪 执行 Sandwich Bundle");
        info!("   - 目标区块: {}", bundle.target_block);
        info!("   - 预期收益: {:.4} ETH", bundle.expected_revenue.to::<u128>() as f64 / 1e18);
        info!("   - 估算 Gas: {}", bundle.estimated_gas);
        
        // 执行 bundle
        let mut executor = self.clone();
        let result = executor.execute_with_fallback(bundle).await?;
        
        let execution_time = start.elapsed();
        
        if result.success {
            info!("✅ Sandwich 执行成功!");
            info!("   - Bundle Hash: {:?}", result.bundle_hash);
            info!("   - 执行时间: {:.2}ms", execution_time.as_millis());
            
            metrics::counter!("artemis.sandwich.successful_executions").increment(1);
        } else {
            warn!("❌ Sandwich 执行失败: {:?}", result.error);
            metrics::counter!("artemis.sandwich.failed_executions").increment(1);
        }
        
        metrics::histogram!("artemis.sandwich.total_execution_time")
            .record(execution_time.as_millis() as f64);
        
        Ok(())
    }
}

impl Clone for SandwichExecutor {
    fn clone(&self) -> Self {
        Self {
            flashbots_executor: self.flashbots_executor.clone(),
            rbuilder_enabled: self.rbuilder_enabled,
            stats: self.stats.clone(),
            provider: Arc::clone(&self.provider),
        }
    }
}
