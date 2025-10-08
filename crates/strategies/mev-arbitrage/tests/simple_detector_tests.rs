//! Simplified integration tests for detectors and validators

#[cfg(feature = "full")]
#[tokio::test]
async fn test_detector_trait_implementations() {
    // This test verifies that our detectors implement the ArbitrageDetector trait
    // More detailed tests would require setting up proper state and contexts

    use mev_arbitrage::enhanced_arbitrage_detector::DeepArbitrageDetector;
    use mev_arbitrage::abstractions::ArbitrageDetector;

    let detector = DeepArbitrageDetector::new();
    let metadata = detector.metadata();

    assert_eq!(metadata.name, "DeepArbitrageDetector");
    assert!(!metadata.supported_opportunity_types.is_empty());
}

#[cfg(feature = "full")]
#[tokio::test]
async fn test_validator_trait_implementations() {
    use mev_arbitrage::symbolic_validator::SymbolicValidator;
    use mev_arbitrage::revm_validator::REVMValidator;
    use mev_arbitrage::abstractions::Validator;

    let symbolic_validator = SymbolicValidator::new();
    let revm_validator = REVMValidator::new();

    let symbolic_types = symbolic_validator.supported_types();
    let revm_types = revm_validator.supported_types();

    assert!(!symbolic_types.is_empty(), "Symbolic validator should support validation types");
    assert!(!revm_types.is_empty(), "REVM validator should support validation types");
}

#[test]
fn test_basic_compilation() {
    // If this test runs, the package compiled successfully
    assert!(true);
}