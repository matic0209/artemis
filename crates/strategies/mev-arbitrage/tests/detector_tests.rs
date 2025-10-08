//! Integration tests for detectors
//!
//! Tests FastArbitrageDetector and DeepArbitrageDetector implementations

#[cfg(feature = "full")]
mod detector_integration_tests {
    use mev_arbitrage::abstractions::{
        ArbitrageDetector, DetectionContext, DetectionParams, MarketData,
    };
    use mev_arbitrage::fast_arbitrage_detector::FastArbitrageDetector;
    use mev_arbitrage::enhanced_arbitrage_detector::DeepArbitrageDetector;
    use mev_arbitrage_graph::StateSnapshot;
    use alloy_primitives::{Address, U256};
    use std::collections::HashMap;
    use std::time::Duration;

    /// Helper function to create test detection context
    fn create_test_context() -> DetectionContext {
        DetectionContext {
            block_number: 18_000_000,
            timestamp: 1700000000,
            gas_price: U256::from(30_000_000_000u64), // 30 gwei
            state_snapshot: StateSnapshot {
                block_number: 18_000_000,
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
                min_profit_wei: U256::from(500_000_000_000_000u64), // 0.0005 ETH
                max_gas_cost: U256::from(1_000_000),
                min_confidence: 0.7,
                enable_flash_loans: true,
                target_tokens: vec![],
            },
        }
    }

    #[tokio::test]
    async fn test_fast_detector_creation() {
        use mev_arbitrage::fast_arbitrage_detector::FastDetectorConfig;
        let detector = FastArbitrageDetector::new(FastDetectorConfig::default());

        // Check metadata
        let metadata = detector.metadata();
        assert_eq!(metadata.name, "FastArbitrageDetector");
        assert_eq!(metadata.version, "1.0.0");
        assert!(metadata.supported_opportunity_types.contains(&"TwoHopArbitrage".to_string()));

        // Check config
        let config = detector.config();
        assert!(config.enabled);
        assert_eq!(config.confidence_threshold, 0.8);
        assert_eq!(config.max_opportunities, 100);
    }

    #[tokio::test]
    async fn test_fast_detector_health_check() {
        use mev_arbitrage::fast_arbitrage_detector::FastDetectorConfig;
        let detector = FastArbitrageDetector::new(FastDetectorConfig::default());

        let health = detector.health_check().await;
        assert!(health.is_ok(), "Health check should succeed");

        let status = health.unwrap();
        assert!(status.is_healthy);
        assert!(status.status_message.contains("operating normally"));
    }

    #[tokio::test]
    async fn test_fast_detector_detection() {
        use mev_arbitrage::fast_arbitrage_detector::FastDetectorConfig;
        let mut detector = FastArbitrageDetector::new(FastDetectorConfig::default());
        let context = create_test_context();

        // Run detection
        let result = detector.detect(&context).await;
        assert!(result.is_ok(), "Detection should not error");

        let detection_result = result.unwrap();
        assert_eq!(detection_result.detector_id, "fast_arbitrage_detector");
        assert_eq!(detection_result.confidence_threshold, 0.8);

        // Detection time should be reasonable
        assert!(detection_result.detection_time < Duration::from_secs(1),
                "Detection should complete quickly");
    }

    #[tokio::test]
    async fn test_fast_detector_config_update() {
        use mev_arbitrage::abstractions::DetectorConfig;
        use mev_arbitrage::fast_arbitrage_detector::FastDetectorConfig;

        let mut detector = FastArbitrageDetector::new(FastDetectorConfig::default());

        let new_config = DetectorConfig {
            enabled: true,
            confidence_threshold: 0.9,
            max_opportunities: 50,
            timeout: Duration::from_secs(3),
            detector_specific: serde_json::json!({
                "min_profit_threshold": "1000000000000000", // 0.001 ETH
                "max_gas_cost": "500000"
            }),
        };

        let result = detector.update_config(new_config);
        assert!(result.is_ok(), "Config update should succeed");
    }

    #[tokio::test]
    async fn test_deep_detector_creation() {
        let detector = DeepArbitrageDetector::new();

        // Check metadata
        let metadata = detector.metadata();
        assert_eq!(metadata.name, "DeepArbitrageDetector");
        assert!(metadata.supported_opportunity_types.contains(&"NegativeCycleArbitrage".to_string()));

        // Check config
        let config = detector.config();
        assert!(config.enabled);
        assert_eq!(config.confidence_threshold, 0.7);
        assert_eq!(config.max_opportunities, 50);
    }

    #[tokio::test]
    async fn test_deep_detector_health_check() {
        let detector = DeepArbitrageDetector::new();

        let health = detector.health_check().await;
        assert!(health.is_ok(), "Health check should succeed");

        let status = health.unwrap();
        assert!(status.is_healthy);
        assert!(status.metrics.contains_key("cycle_detection_rate"));
    }

    #[tokio::test]
    async fn test_deep_detector_detection() {
        let mut detector = DeepArbitrageDetector::new();
        let context = create_test_context();

        // Run detection
        let result = detector.detect(&context).await;
        assert!(result.is_ok(), "Detection should not error");

        let detection_result = result.unwrap();
        assert_eq!(detection_result.detector_id, "deep_arbitrage_detector");
        assert_eq!(detection_result.confidence_threshold, 0.7);
    }

    #[tokio::test]
    async fn test_deep_detector_config_update() {
        use mev_arbitrage::abstractions::DetectorConfig;

        let mut detector = DeepArbitrageDetector::new();

        let new_config = DetectorConfig {
            enabled: true,
            confidence_threshold: 0.75,
            max_opportunities: 30,
            timeout: Duration::from_secs(15),
            detector_specific: serde_json::json!({
                "max_path_length": 7,
                "min_cycle_profit": "2000000000000000"
            }),
        };

        let result = detector.update_config(new_config);
        assert!(result.is_ok(), "Config update should succeed");
    }

    #[tokio::test]
    async fn test_detectors_with_empty_state() {
        use mev_arbitrage::fast_arbitrage_detector::FastDetectorConfig;
        let mut fast_detector = FastArbitrageDetector::new(FastDetectorConfig::default());
        let mut deep_detector = DeepArbitrageDetector::new();
        let context = create_test_context();

        // Both should handle empty state gracefully
        let fast_result = fast_detector.detect(&context).await;
        assert!(fast_result.is_ok(), "Fast detector should handle empty state");

        let deep_result = deep_detector.detect(&context).await;
        assert!(deep_result.is_ok(), "Deep detector should handle empty state");
    }

    #[tokio::test]
    async fn test_detector_performance_metrics() {
        use mev_arbitrage::fast_arbitrage_detector::FastDetectorConfig;
        let detector = FastArbitrageDetector::new(FastDetectorConfig::default());
        let metadata = detector.metadata();

        // Check performance metrics exist
        let metrics = metadata.performance_metrics;
        assert!(metrics.avg_detection_time < Duration::from_millis(100),
                "Fast detector should be quick");
        assert!(metrics.success_rate > 0.9, "Should have high success rate");
        assert!(metrics.avg_profit_accuracy > 0.8, "Should have good accuracy");
    }

    #[tokio::test]
    async fn test_detector_opportunity_types() {
        use mev_arbitrage::fast_arbitrage_detector::FastDetectorConfig;
        let fast_detector = FastArbitrageDetector::new(FastDetectorConfig::default());
        let deep_detector = DeepArbitrageDetector::new();

        let fast_types = fast_detector.metadata().supported_opportunity_types;
        let deep_types = deep_detector.metadata().supported_opportunity_types;

        // Fast detector should support simple arbitrage types
        assert!(fast_types.contains(&"TwoHopArbitrage".to_string()));
        assert!(fast_types.contains(&"TriangleArbitrage".to_string()));

        // Deep detector should support complex types
        assert!(deep_types.contains(&"NegativeCycleArbitrage".to_string()));
        assert!(deep_types.contains(&"MultiHopArbitrage".to_string()));
    }
}

/// Basic compilation test - always runs
#[test]
fn test_detector_module_compiles() {
    // If this test runs, the detectors compiled successfully
    assert!(true);
}