//! Integration Examples for DeFi Analyzer with Negative Cycle Arbitrage
//! 
//! This module provides complete examples of how to integrate the DeFi analyzer
//! with the negative cycle arbitrage algorithm in different scenarios.

use std::collections::HashMap;
use alloy_primitives::{Address, U256};
use tracing::{info, debug};
use anyhow::Result;

use crate::{
    DeFiAnalyzerStrategy,
    AnalyzerConfig,
    AnalysisEvent,
    EventType,
    NegativeCycleArbitrageEngine,
    NegativeCycleConfig,
    StateSnapshot,
    negative_cycle_arbitrage::{PoolInfo, PoolData, TokenInfo},
};

/// Complete integration example
pub struct IntegrationExample;

impl IntegrationExample {
    /// Example 1: Basic negative cycle arbitrage detection
    pub async fn basic_negative_cycle_example() -> Result<()> {
        info!("Running basic negative cycle arbitrage example");
        
        // Configure analyzer
        let analyzer_config = AnalyzerConfig {
            max_execution_paths: 1000,
            max_depth: 50,
            enable_arbitrage_detection: true,
            enable_consistency_checking: true,
            min_profit_threshold: U256::from(1_000_000_000_000_000u64), // 0.001 ETH
            ..Default::default()
        };
        
        // Configure negative cycle arbitrage
        let arbitrage_config = NegativeCycleConfig {
            target_revenue: U256::from(5_000_000_000_000_000u64), // 0.005 ETH
            max_cycles_per_iteration: 5,
            max_path_length: 4,
            gas_price: U256::from(30_000_000_000u64), // 30 gwei
            base_gas_cost: 200_000,
            supported_protocols: vec![
                "uniswap_v2".to_string(),
                "uniswap_v3".to_string(),
                "sushiswap".to_string(),
            ],
        };
        
        // Create strategy with negative cycle integration
        let mut strategy = DeFiAnalyzerStrategy::new(analyzer_config);
        strategy.initialize().await?;
        
        // Create mock state with triangular arbitrage opportunity
        let state = Self::create_triangular_arbitrage_state();
        
        // Process analysis event
        let event = AnalysisEvent {
            event_type: EventType::MempoolTransaction,
            contract_address: Address::from([2u8; 20]),
            tx_data: Some(vec![0xa9, 0x05, 0x9c, 0xbb]), // swapExactTokensForTokens selector
            transaction_data: Some(vec![0xa9, 0x05, 0x9c, 0xbb]),
            transaction_hash: [1u8; 32],
            event_data: serde_json::to_vec(&state).unwrap_or_default(),
            event_kind: "mempool_analysis".to_string(),
            block_number: 18500000,
            timestamp: 1700000000,
            metadata: HashMap::new(),
        };
        
        let actions = strategy.process_event(event).await;
        info!("Generated {} arbitrage actions", actions.len());
        
        Ok(())
    }
    
    /// Example 2: Multi-protocol arbitrage with complex cycles
    pub async fn multi_protocol_arbitrage_example() -> Result<()> {
        info!("Running multi-protocol arbitrage example");
        
        // Create state with cross-protocol arbitrage opportunities
        let state = Self::create_cross_protocol_arbitrage_state();
        
        let arbitrage_config = NegativeCycleConfig {
            target_revenue: U256::from(2_000_000_000_000_000u64), // 0.002 ETH
            max_cycles_per_iteration: 15,
            max_path_length: 6,
            supported_protocols: vec![
                "uniswap_v2".to_string(),
                "uniswap_v3".to_string(),
                "sushiswap".to_string(),
                "curve".to_string(),
                "balancer".to_string(),
            ],
            ..Default::default()
        };
        
        let mut arbitrage_engine = NegativeCycleArbitrageEngine::new(arbitrage_config);
        let total_revenue = arbitrage_engine.execute_arbitrage_algorithm(&state).await?;
        
        info!("Multi-protocol arbitrage revenue: {} ETH", total_revenue);
        Ok(())
    }
    
    /// Example 3: Real-time arbitrage monitoring
    pub async fn realtime_arbitrage_monitoring_example() -> Result<()> {
        info!("Starting real-time arbitrage monitoring");
        
        let config = AnalyzerConfig::default();
        let mut strategy = DeFiAnalyzerStrategy::new(config);
        strategy.initialize().await?;
        
        // Simulate stream of events
        for i in 0..10 {
            let event = AnalysisEvent {
                event_type: EventType::BlockAnalysis,
                contract_address: Address::from([i as u8; 20]),
                tx_data: Some(vec![0xa9, 0x05, 0x9c, 0xbb]), // Mock swap data
                transaction_data: Some(vec![0xa9, 0x05, 0x9c, 0xbb]),
                transaction_hash: [i as u8; 32],
                event_data: vec![],
                event_kind: "block_analysis".to_string(),
                block_number: 18500000 + i,
                timestamp: 1700000000 + i * 12,
                metadata: HashMap::new(),
            };
            
            let actions = strategy.process_event(event).await;
            if !actions.is_empty() {
                info!("Block {}: Found {} arbitrage opportunities", 18500000 + i, actions.len());
            }
            
            // Simulate 12-second block time
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Print final statistics
        let metrics = strategy.get_metrics_summary();
        info!("Final metrics: {}", metrics);
        
        Ok(())
    }
    
    /// Create state snapshot with triangular arbitrage opportunity
    fn create_triangular_arbitrage_state() -> StateSnapshot {
        let mut state = StateSnapshot::new();
        state.block_number = 18500000;
        
        // Add tokens
        let mut tokens = HashMap::new();
        tokens.insert("WETH".to_string(), TokenInfo {
            address: Address::from([1u8; 20]),
            symbol: "WETH".to_string(),
            decimals: 18,
        });
        tokens.insert("USDC".to_string(), TokenInfo {
            address: Address::from([2u8; 20]),
            symbol: "USDC".to_string(),
            decimals: 6,
        });
        tokens.insert("DAI".to_string(), TokenInfo {
            address: Address::from([3u8; 20]),
            symbol: "DAI".to_string(),
            decimals: 18,
        });
        state.tokens = tokens;
        
        // Add pools with arbitrage opportunity
        let mut pools = HashMap::new();
        
        // WETH/USDC pool (Uniswap V2)
        pools.insert("weth_usdc_v2".to_string(), PoolInfo {
            address: Address::from([10u8; 20]),
            protocol: "uniswap_v2".to_string(),
            token_a: "WETH".to_string(),
            token_b: "USDC".to_string(),
            fee: 3000,
            pool_data: PoolData::UniswapV2 {
                reserve_a: U256::from(1000) * U256::from(10u64.pow(18)), // 1000 WETH
                reserve_b: U256::from(2000000) * U256::from(10u64.pow(6)), // 2M USDC (rate: 2000)
            },
        });
        
        // USDC/DAI pool (Uniswap V3)
        pools.insert("usdc_dai_v3".to_string(), PoolInfo {
            address: Address::from([11u8; 20]),
            protocol: "uniswap_v3".to_string(),
            token_a: "USDC".to_string(),
            token_b: "DAI".to_string(),
            fee: 500,
            pool_data: PoolData::UniswapV3 {
                liquidity: U256::from(10000000) * U256::from(10u64.pow(18)),
                sqrt_price: U256::from(79228162514264337593543950336u128), // ~1.0 price
                tick: 0,
            },
        });
        
        // DAI/WETH pool (SushiSwap) - Mispriced for arbitrage
        pools.insert("dai_weth_sushi".to_string(), PoolInfo {
            address: Address::from([12u8; 20]),
            protocol: "sushiswap".to_string(),
            token_a: "DAI".to_string(),
            token_b: "WETH".to_string(),
            fee: 3000,
            pool_data: PoolData::UniswapV2 {
                reserve_a: U256::from(1800000) * U256::from(10u64.pow(18)), // 1.8M DAI
                reserve_b: U256::from(1000) * U256::from(10u64.pow(18)), // 1000 WETH (rate: 1800)
            },
        });
        
        state.pools = pools;
        
        // Add spot prices (with arbitrage opportunity)
        let mut spot_prices = HashMap::new();
        spot_prices.insert(("WETH".to_string(), "USDC".to_string()), 2000.0);
        spot_prices.insert(("USDC".to_string(), "WETH".to_string()), 1.0 / 2000.0);
        spot_prices.insert(("USDC".to_string(), "DAI".to_string()), 1.0);
        spot_prices.insert(("DAI".to_string(), "USDC".to_string()), 1.0);
        spot_prices.insert(("DAI".to_string(), "WETH".to_string()), 1.0 / 1800.0); // Mispriced
        spot_prices.insert(("WETH".to_string(), "DAI".to_string()), 1800.0); // Mispriced
        state.spot_prices = spot_prices;
        
        state
    }
    
    /// Create state snapshot with cross-protocol arbitrage opportunities
    fn create_cross_protocol_arbitrage_state() -> StateSnapshot {
        let mut state = Self::create_triangular_arbitrage_state();
        
        // Add more tokens
        state.tokens.insert("USDT".to_string(), TokenInfo {
            address: Address::from([4u8; 20]),
            symbol: "USDT".to_string(),
            decimals: 6,
        });
        state.tokens.insert("WBTC".to_string(), TokenInfo {
            address: Address::from([5u8; 20]),
            symbol: "WBTC".to_string(),
            decimals: 8,
        });
        
        // Add Curve stable pool
        state.pools.insert("curve_3pool".to_string(), PoolInfo {
            address: Address::from([20u8; 20]),
            protocol: "curve".to_string(),
            token_a: "USDC".to_string(),
            token_b: "USDT".to_string(),
            fee: 400,
            pool_data: PoolData::Curve {
                balances: vec![
                    U256::from(10000000) * U256::from(10u64.pow(6)), // 10M USDC
                    U256::from(10000000) * U256::from(10u64.pow(6)), // 10M USDT
                    U256::from(10000000) * U256::from(10u64.pow(18)), // 10M DAI
                ],
                amplification: U256::from(2000),
            },
        });
        
        // Add WBTC/WETH pool
        state.pools.insert("wbtc_weth_v3".to_string(), PoolInfo {
            address: Address::from([21u8; 20]),
            protocol: "uniswap_v3".to_string(),
            token_a: "WBTC".to_string(),
            token_b: "WETH".to_string(),
            fee: 3000,
            pool_data: PoolData::UniswapV3 {
                liquidity: U256::from(5000) * U256::from(10u64.pow(18)),
                sqrt_price: U256::from(396140812571321687967719759744u128), // ~15.5 price (WBTC/WETH)
                tick: 54000,
            },
        });
        
        // Add more spot prices for cross-protocol opportunities
        state.spot_prices.insert(("USDC".to_string(), "USDT".to_string()), 0.9999);
        state.spot_prices.insert(("USDT".to_string(), "USDC".to_string()), 1.0001);
        state.spot_prices.insert(("WBTC".to_string(), "WETH".to_string()), 15.5);
        state.spot_prices.insert(("WETH".to_string(), "WBTC".to_string()), 1.0 / 15.5);
        
        state
    }
    
    /// Comprehensive performance test
    pub async fn performance_test_example() -> Result<()> {
        info!("Running performance test");
        
        let start_time = std::time::Instant::now();
        let mut total_opportunities = 0;
        
        for i in 0..100 {
            let state = if i % 2 == 0 {
                Self::create_triangular_arbitrage_state()
            } else {
                Self::create_cross_protocol_arbitrage_state()
            };
            
            let config = NegativeCycleConfig {
                target_revenue: U256::from(1_000_000_000_000_000u64),
                max_cycles_per_iteration: 5,
                ..Default::default()
            };
            
            let mut engine = NegativeCycleArbitrageEngine::new(config);
            let revenue = engine.execute_arbitrage_algorithm(&state).await?;
            
            if revenue > U256::ZERO {
                total_opportunities += 1;
            }
            
            if i % 20 == 0 {
                debug!("Processed {} states, found {} opportunities", i + 1, total_opportunities);
            }
        }
        
        let elapsed = start_time.elapsed();
        info!("Performance test completed:");
        info!("  - Processed 100 states in {:?}", elapsed);
        info!("  - Found {} arbitrage opportunities", total_opportunities);
        info!("  - Average time per state: {:?}", elapsed / 100);
        
        Ok(())
    }
}

/// Example usage functions
pub async fn run_all_examples() -> Result<()> {
    info!("Starting DeFi Analyzer integration examples");
    
    // Run basic example
    IntegrationExample::basic_negative_cycle_example().await?;
    
    // Run multi-protocol example
    IntegrationExample::multi_protocol_arbitrage_example().await?;
    
    // Run real-time monitoring example
    IntegrationExample::realtime_arbitrage_monitoring_example().await?;
    
    // Run performance test
    IntegrationExample::performance_test_example().await?;
    
    info!("All integration examples completed successfully");
    Ok(())
}
