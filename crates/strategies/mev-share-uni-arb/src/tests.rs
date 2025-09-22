//! 策略测试模块

use std::sync::Arc;
use std::collections::HashMap;

use anyhow::Result;
use artemis_core::{
    eth::{Address, U256, LocalWallet},
    error::ArtemisError,
};

use crate::{
    MevShareUniArb, ArbConfig, ArbStats, StrategyConfig,
    types::{Action, Event, V2V3PoolRecord},
};

/// 模拟 Provider 用于测试
#[derive(Debug, Clone)]
struct MockProvider {
    gas_price: U256,
    block_number: u64,
    chain_id: u64,
}

impl MockProvider {
    fn new() -> Self {
        Self {
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            block_number: 18000000,
            chain_id: 1,
        }
    }
}

#[async_trait::async_trait]
impl alloy_provider::Provider<alloy_network::Ethereum> for MockProvider {
    async fn get_gas_price(&self) -> Result<U256, alloy_provider::ProviderError> {
        Ok(self.gas_price)
    }
    
    async fn get_block_number(&self) -> Result<u64, alloy_provider::ProviderError> {
        Ok(self.block_number)
    }
    
    async fn get_chain_id(&self) -> Result<u64, alloy_provider::ProviderError> {
        Ok(self.chain_id)
    }
    
    async fn get_transaction_count(&self, _address: Address) -> Result<u64, alloy_provider::ProviderError> {
        Ok(42) // Mock nonce
    }
}

/// 测试套件
#[cfg(test)]
mod strategy_tests {
    use super::*;
    use mev_share::sse::Event as MevShareEvent;
    use primitive_types::{H160, H256};
    
    /// 创建测试策略实例
    fn create_test_strategy() -> MevShareUniArb<MockProvider> {
        let provider = Arc::new(MockProvider::new());
        let wallet = LocalWallet::from_str("0x0000000000000000000000000000000000000000000000000000000000000001")
            .expect("Failed to create test wallet");
        let arb_contract = Address::from([1u8; 20]);
        
        let config = ArbConfig {
            min_profit_threshold: U256::from(100_000_000_000_000_000u64), // 0.1 ETH
            max_slippage_bps: 300, // 3%
            max_gas_price: U256::from(50_000_000_000u64), // 50 gwei
            enable_dynamic_sizing: true,
            price_prediction_window: 5,
            risk_threshold: 0.7,
        };
        
        MevShareUniArb::new(provider, wallet, arb_contract, Some(config))
    }
    
    /// 创建测试 MEV-Share 事件
    fn create_test_mev_event() -> MevShareEvent {
        MevShareEvent {
            hash: H256::from([1u8; 32]),
            logs: vec![mev_share::sse::Log {
                address: H160::from([2u8; 20]),
                topics: vec![],
                data: vec![],
            }],
            txs: vec![],
            bundle_transactions: vec![],
            mev_commit_hash: None,
        }
    }
    
    #[tokio::test]
    async fn test_strategy_initialization() {
        let strategy = create_test_strategy();
        let stats = strategy.get_stats();
        
        assert_eq!(stats.events_processed, 0);
        assert_eq!(stats.successful_arbs, 0);
        assert_eq!(stats.failed_arbs, 0);
        assert_eq!(stats.total_profit, U256::zero());
        assert_eq!(stats.avg_processing_time_ms, 0.0);
    }
    
    #[tokio::test]
    async fn test_process_mev_share_event_no_logs() {
        let mut strategy = create_test_strategy();
        
        let mut event = create_test_mev_event();
        event.logs = vec![]; // 空日志
        
        let mev_event = Event::MEVShareEvent(event);
        let actions = strategy.process_event(mev_event).await;
        
        assert!(actions.is_empty());
    }
    
    #[tokio::test]
    async fn test_process_mev_share_event_unknown_pool() {
        let mut strategy = create_test_strategy();
        
        let event = create_test_mev_event();
        let mev_event = Event::MEVShareEvent(event);
        let actions = strategy.process_event(mev_event).await;
        
        // 由于池子不在监控列表中，应该返回空
        assert!(actions.is_empty());
    }
    
    #[tokio::test]
    async fn test_stats_update() {
        let mut strategy = create_test_strategy();
        
        // 处理一些事件来更新统计
        let event = create_test_mev_event();
        let mev_event = Event::MEVShareEvent(event);
        strategy.process_event(mev_event).await;
        
        let stats = strategy.get_stats();
        assert_eq!(stats.events_processed, 1);
    }
}

/// 辅助函数
fn create_test_strategy() -> MevShareUniArb<MockProvider> {
    let provider = Arc::new(MockProvider::new());
    let wallet = LocalWallet::from_str("0x0000000000000000000000000000000000000000000000000000000000000001")
        .expect("Failed to create test wallet");
    let arb_contract = Address::from([1u8; 20]);
    
    MevShareUniArb::new(provider, wallet, arb_contract, None)
}

fn create_test_mev_event() -> mev_share::sse::Event {
    mev_share::sse::Event {
        hash: primitive_types::H256::from([1u8; 32]),
        logs: vec![mev_share::sse::Log {
            address: primitive_types::H160::from([2u8; 20]),
            topics: vec![],
            data: vec![],
        }],
        txs: vec![],
        bundle_transactions: vec![],
        mev_commit_hash: None,
    }
}