#![cfg(feature = "full")]
//! Unit tests for FastArbitrageDetector

use mev_arbitrage::detectors::fast::{FastArbitrageDetector, FastDetectorConfig};
use mev_arbitrage::abstractions::*;
use mev_arbitrage_graph::StateSnapshot;
use alloy_primitives::{U256, Address};
use std::collections::HashMap;

/// Test that the fast detector can be created with default config
#[test]
fn test_fast_detector_creation() {
    let config = FastDetectorConfig::default();
    let detector = FastArbitrageDetector::new(config);

    // Should be able to get config
    let detector_config = detector.config();
    assert!(detector_config.enabled);
    assert_eq!(detector_config.confidence_threshold, 0.8);
}

/// Test custom configuration
#[test]
fn test_fast_detector_custom_config() {
    let config = FastDetectorConfig {
        min_two_hop_profit: U256::from(100_000_000_000_000u64),
        min_three_hop_profit: U256::from(200_000_000_000_000u64),
        max_slippage: 0.03,
        cache_size: 500,
        hot_pair_threshold: 5,
        price_deviation_threshold: 0.01,
    };

    let detector = FastArbitrageDetector::new(config.clone());
    assert!(detector.config().enabled);
}

/// Test detector metadata
#[test]
fn test_detector_metadata() {
    let config = FastDetectorConfig::default();
    let detector = FastArbitrageDetector::new(config);

    let metadata = detector.metadata();
    assert_eq!(metadata.name, "FastArbitrageDetector");
    assert_eq!(metadata.version, "1.0.0");
    assert!(metadata.supported_opportunity_types.contains(&"TwoHopArbitrage".to_string()));
    assert!(metadata.supported_opportunity_types.contains(&"TriangleArbitrage".to_string()));
}

/// Test health check
#[tokio::test]
async fn test_detector_health_check() {
    let config = FastDetectorConfig::default();
    let detector = FastArbitrageDetector::new(config);

    let health = detector.health_check().await;
    assert!(health.is_ok());

    let health_status = health.unwrap();
    assert!(health_status.is_healthy);
    assert!(health_status.metrics.contains_key("cache_hit_rate"));
}

/// Test basic detection with empty state
#[tokio::test]
async fn test_fast_detector_empty_state() {
    let config = FastDetectorConfig::default();
    let mut detector = FastArbitrageDetector::new(config);

    // Create empty detection context
    let context = create_empty_detection_context();

    // Test detection - should not panic
    let result = detector.detect(&context).await;
    assert!(result.is_ok());

    let detection_result = result.unwrap();
    assert_eq!(detection_result.detector_id, "fast_arbitrage_detector");
    assert_eq!(detection_result.opportunities.len(), 0); // No opportunities in empty state
}

/// Test detection with mock state containing opportunities
#[tokio::test]
async fn test_fast_detector_with_mock_state() {
    let config = FastDetectorConfig {
        min_two_hop_profit: U256::from(100u64), // Very low threshold for testing
        min_three_hop_profit: U256::from(100u64),
        max_slippage: 0.05,
        cache_size: 1000,
        hot_pair_threshold: 1,
        price_deviation_threshold: 0.001, // 0.1%
    };

    let mut detector = FastArbitrageDetector::new(config);

    // Create mock state with price differences
    let context = create_mock_detection_context();

    let result = detector.detect(&context).await;
    assert!(result.is_ok());

    let detection_result = result.unwrap();
    assert!(detection_result.detection_time.as_millis() < 1000); // Should be fast
}

/// Test config update
#[test]
fn test_detector_config_update() {
    let config = FastDetectorConfig::default();
    let mut detector = FastArbitrageDetector::new(config);

    let new_config = DetectorConfig {
        enabled: true,
        confidence_threshold: 0.9,
        max_opportunities: 50,
        timeout: std::time::Duration::from_secs(10),
        detector_specific: serde_json::json!({
            "min_profit_threshold": "1000000000000000",
        }),
    };

    let update_result = detector.update_config(new_config);
    assert!(update_result.is_ok());
}

/// Test performance under load
#[tokio::test]
async fn test_fast_detector_performance() {
    let config = FastDetectorConfig::default();
    let mut detector = FastArbitrageDetector::new(config);

    let context = create_mock_detection_context();

    // Run detection 10 times and measure average time
    let mut total_time = std::time::Duration::ZERO;

    for _ in 0..10 {
        let start = std::time::Instant::now();
        let _ = detector.detect(&context).await;
        total_time += start.elapsed();
    }

    let avg_time = total_time / 10;
    // Should be fast - under 200ms
    assert!(avg_time.as_millis() < 200, "Average detection time too slow: {:?}", avg_time);
}

/// Test that detector respects confidence threshold
#[tokio::test]
async fn test_confidence_filtering() {
    let config = FastDetectorConfig::default();
    let mut detector = FastArbitrageDetector::new(config);

    let context = create_mock_detection_context();
    let result = detector.detect(&context).await.unwrap();

    // All opportunities should meet the confidence threshold
    for opp in &result.opportunities {
        assert!(opp.confidence >= result.confidence_threshold);
    }
}

// Helper functions to create test contexts

fn create_empty_detection_context() -> DetectionContext {
    use mev_arbitrage::abstractions::MarketData;

    DetectionContext {
        block_number: 1000,
        timestamp: 1234567890,
        gas_price: U256::from(20_000_000_000u64), // 20 gwei
        state_snapshot: StateSnapshot {
            tokens: HashMap::new(),
            pools: HashMap::new(),
            spot_prices: HashMap::new(),
            block_number: 1000,
            timestamp: 1234567890,
        },
        market_data: MarketData {
            token_prices: HashMap::new(),
            pool_reserves: HashMap::new(),
            gas_price_history: vec![],
            volume_data: HashMap::new(),
        },
        detection_params: DetectionParams {
            min_profit_wei: U256::from(1_000_000_000_000_000u64),
            max_gas_cost: U256::from(500_000),
            min_confidence: 0.8,
            enable_flash_loans: true,
            target_tokens: vec![],
        },
    }
}

fn create_mock_detection_context() -> DetectionContext {
    use mev_arbitrage::abstractions::MarketData;
    use mev_arbitrage_graph::TokenInfo;

    let mut state = StateSnapshot {
        tokens: HashMap::new(),
        pools: HashMap::new(),
        spot_prices: HashMap::new(),
        block_number: 1000,
        timestamp: 1234567890,
    };

    // Add some mock tokens
    let token_a = "WETH".to_string();
    let token_b = "USDC".to_string();
    let token_c = "DAI".to_string();

    state.tokens.insert(token_a.clone(), TokenInfo {
        address: Address::ZERO,
        symbol: "WETH".to_string(),
        decimals: 18,
    });
    state.tokens.insert(token_b.clone(), TokenInfo {
        address: Address::ZERO,
        symbol: "USDC".to_string(),
        decimals: 6,
    });
    state.tokens.insert(token_c.clone(), TokenInfo {
        address: Address::ZERO,
        symbol: "DAI".to_string(),
        decimals: 18,
    });

    // Add mock prices that could create arbitrage
    state.spot_prices.insert((token_a.clone(), token_b.clone()), 2000.0);
    state.spot_prices.insert((token_b.clone(), token_c.clone()), 1.0);
    state.spot_prices.insert((token_c.clone(), token_a.clone()), 0.0005);

    DetectionContext {
        block_number: 1000,
        timestamp: 1234567890,
        gas_price: U256::from(20_000_000_000u64),
        state_snapshot: state,
        market_data: MarketData {
            token_prices: HashMap::new(),
            pool_reserves: HashMap::new(),
            gas_price_history: vec![],
            volume_data: HashMap::new(),
        },
        detection_params: DetectionParams {
            min_profit_wei: U256::from(100u64),
            max_gas_cost: U256::from(500_000),
            min_confidence: 0.5,
            enable_flash_loans: true,
            target_tokens: vec![],
        },
    }
}
