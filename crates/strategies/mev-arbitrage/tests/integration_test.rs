//! Integration tests for MEV Arbitrage system
//!
//! Tests the core functionality of the unified arbitrage system.

#[cfg(feature = "full")]
mod unified_manager_tests {
    use mev_arbitrage::*;
    use mev_arbitrage::abstractions::{
        DetectionContext, MarketData, DetectionParams, OpportunityType, RiskLevel,
        DetectorConfig, ExplorerConfig
    };
    use mev_arbitrage::unified_arbitrage_manager::{
        DetectorSpec, ExplorerSpec, OptimizerSpec, ExecutorSpec, GlobalConfig
    };
    use std::time::Duration;
    use alloy_primitives::{Address, U256};

    /// Test that we can create a UnifiedArbitrageManager with default config
    #[tokio::test]
    async fn test_create_manager_with_default_config() {
        let config = ManagerConfig::default();
        let result = UnifiedArbitrageManager::new(config).await;

        assert!(result.is_ok(), "Should be able to create manager with default config");

        let manager = result.unwrap();
        let metrics = manager.get_metrics().await;

        // Check initial metrics are zero
        assert_eq!(metrics.total_detections, 0);
        assert_eq!(metrics.successful_detections, 0);
    }

    /// Test that we can create a custom configuration
    #[test]
    fn test_custom_manager_config() {
        let config = ManagerConfig {
            detectors: vec![
                DetectorSpec {
                    name: "test_detector".to_string(),
                    detector_type: "enhanced_graph".to_string(),
                    config: DetectorConfig {
                        enabled: true,
                        confidence_threshold: 0.8,
                        max_opportunities: 5,
                        timeout: Duration::from_secs(10),
                        detector_specific: serde_json::json!({}),
                    },
                    weight: 1.0,
                },
            ],
            explorer: ExplorerSpec {
                explorer_type: "defi_analyzer".to_string(),
                config: ExplorerConfig {
                    max_depth: 3,
                    max_paths: 50,
                    enable_optimization: true,
                    timeout: Duration::from_secs(5),
                },
            },
            validators: vec![],
            optimizer: OptimizerSpec {
                optimizer_type: "z3".to_string(),
                enabled: false, // Disabled for testing
            },
            executor: ExecutorSpec {
                executor_type: "simulation".to_string(),
                enabled: false, // Disabled for testing
            },
            global: GlobalConfig {
                max_detection_time: Duration::from_secs(30),
                max_opportunities_per_cycle: 20,
                min_confidence_threshold: 0.7,
                enable_parallel_processing: false,
                enable_caching: true,
                cache_ttl: Duration::from_secs(10),
            },
        };

        // Just verify the config can be created
        assert_eq!(config.detectors.len(), 1);
        assert_eq!(config.detectors[0].name, "test_detector");
    }

    /// Test basic detection context creation
    #[test]
    fn test_create_detection_context() {
        use mev_arbitrage_graph::StateSnapshot;
        use std::collections::HashMap;

        let context = DetectionContext {
            block_number: 12345,
            timestamp: 1234567890,
            gas_price: U256::from(20_000_000_000u64),
            state_snapshot: StateSnapshot {
                block_number: 12345,
                token_reserves: HashMap::new(),
                spot_prices: HashMap::new(),
                pools: HashMap::new(),
                tokens: HashMap::new(),
            },
            market_data: MarketData {
                token_prices: HashMap::new(),
                pool_reserves: HashMap::new(),
                gas_price_history: Vec::new(),
                volume_data: HashMap::new(),
            },
            detection_params: DetectionParams {
                min_profit_wei: U256::from(1000000000000000u64), // 0.001 ETH
                max_gas_cost: U256::from(300000),
                min_confidence: 0.6,
                enable_flash_loans: true,
                target_tokens: vec![],
            },
        };

        assert_eq!(context.block_number, 12345);
        assert_eq!(context.timestamp, 1234567890);
    }

    /// Test ArbitrageOpportunity creation
    #[test]
    fn test_create_arbitrage_opportunity() {
        let token_a = Address::ZERO;
        let token_b = Address::repeat_byte(1);

        let opportunity = ArbitrageOpportunity {
            id: "test_opp_001".to_string(),
            opportunity_type: OpportunityType::SimpleArbitrage {
                token_a,
                token_b,
                path: vec![token_a, token_b, token_a],
            },
            expected_profit: U256::from(1000000000000000000u64), // 1 ETH
            gas_cost: U256::from(200000),
            confidence: 0.85,
            risk_level: RiskLevel::Low,
            deadline: None,
            required_capital: U256::from(100000000000000000u64), // 0.1 ETH
            metadata: serde_json::json!({
                "detection_method": "test",
            }),
        };

        assert_eq!(opportunity.id, "test_opp_001");
        assert_eq!(opportunity.confidence, 0.85);

        // Test serialization
        let serialized = serde_json::to_string(&opportunity);
        assert!(serialized.is_ok(), "Should be able to serialize opportunity");
    }
}

/// Basic compilation test - always runs
#[test]
fn test_crate_compiles() {
    // If this test runs, the crate compiled successfully
    assert!(true);
}