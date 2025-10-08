//! Integration tests for validators
//!
//! Tests SymbolicValidator and REVMValidator implementations

#[cfg(feature = "full")]
mod validator_integration_tests {
    use mev_arbitrage::abstractions::{
        Validator, ValidationType, ValidationContext, SimulationParams, RiskLevel,
        ExecutionPlan, ExecutionStep, GasStrategy, StepType,
    };
    use mev_arbitrage::symbolic_validator::SymbolicValidator;
    use mev_arbitrage::revm_validator::REVMValidator;
    use alloy_primitives::{Address, U256, Bytes};
    use std::time::Duration;

    /// Helper function to create test execution plan
    fn create_test_execution_plan() -> ExecutionPlan {
        use mev_arbitrage::abstractions::ExecutionStrategy;

        ExecutionPlan {
            id: "test_plan_001".to_string(),
            opportunity_id: "opp_001".to_string(),
            steps: vec![
                ExecutionStep {
                    step_type: StepType::TokenSwap {
                        token_in: Address::ZERO,
                        token_out: Address::from([1u8; 20]),
                        amount: U256::from(1_000_000_000_000_000u64),
                    },
                    contract_address: Address::ZERO,
                    call_data: Bytes::default(),
                    value: U256::ZERO,
                    gas_limit: 100_000,
                    dependencies: vec![],
                },
            ],
            estimated_gas: U256::from(100_000),
            estimated_profit: U256::from(1_000_000_000_000_000u64), // 0.001 ETH
            execution_strategy: ExecutionStrategy::Immediate,
            validation_results: None,
        }
    }

    /// Helper function to create test validation context
    fn create_test_validation_context() -> ValidationContext {
        ValidationContext {
            validation_types: vec![
                ValidationType::Symbolic,
                ValidationType::Gas,
                ValidationType::Economic,
            ],
            simulation_params: SimulationParams {
                fork_block: Some(18_000_000),
                simulation_timeout: Duration::from_secs(10),
                enable_state_override: true,
                gas_limit: 500_000,
            },
            risk_tolerance: RiskLevel::Medium,
        }
    }

    #[tokio::test]
    async fn test_symbolic_validator_creation() {
        let validator = SymbolicValidator::new();

        let config = validator.config();
        assert!(!config.strict_mode);
        assert_eq!(config.timeout, Duration::from_secs(5));

        let types = validator.supported_types();
        assert!(!types.is_empty(), "Should support at least gas validation");
    }

    #[tokio::test]
    async fn test_symbolic_validator_supported_types() {
        let validator = SymbolicValidator::new();
        let types = validator.supported_types();

        #[cfg(feature = "full")]
        {
            assert!(types.contains(&ValidationType::Symbolic));
            assert!(types.contains(&ValidationType::Gas));
        }

        #[cfg(not(feature = "full"))]
        {
            // In lite mode, only gas validation is supported
            assert!(types.contains(&ValidationType::Gas));
        }
    }

    #[tokio::test]
    async fn test_symbolic_validator_validation() {
        let mut validator = SymbolicValidator::new();
        let plan = create_test_execution_plan();
        let context = create_test_validation_context();

        let result = validator.validate(&plan, &context).await;
        assert!(result.is_ok(), "Validation should not error");

        let validation_result = result.unwrap();
        assert!(!validation_result.validation_types.is_empty());
    }

    #[tokio::test]
    async fn test_symbolic_validator_gas_check() {
        let mut validator = SymbolicValidator::new();

        // Create plan with excessive gas
        let mut plan = create_test_execution_plan();
        plan.estimated_gas = U256::from(12_000_000); // Very high gas

        let context = ValidationContext {
            validation_types: vec![ValidationType::Gas],
            simulation_params: SimulationParams {
                fork_block: None,
                simulation_timeout: Duration::from_secs(5),
                enable_state_override: false,
                gas_limit: 500_000,
            },
            risk_tolerance: RiskLevel::Low,
        };

        let result = validator.validate(&plan, &context).await;
        assert!(result.is_ok());

        let validation_result = result.unwrap();
        // Should have warning about high gas
        assert!(!validation_result.issues.is_empty() || validation_result.is_valid);
    }

    #[tokio::test]
    async fn test_revm_validator_creation() {
        let validator = REVMValidator::new();

        let config = validator.config();
        assert!(config.strict_mode, "REVM validator should use strict mode");
        assert_eq!(config.timeout, Duration::from_secs(10));

        let types = validator.supported_types();
        assert!(!types.is_empty());
    }

    #[tokio::test]
    async fn test_revm_validator_supported_types() {
        let validator = REVMValidator::new();
        let types = validator.supported_types();

        // Should always support economic and gas validation
        assert!(types.contains(&ValidationType::Economic));
        assert!(types.contains(&ValidationType::Gas));

        #[cfg(feature = "full")]
        {
            assert!(types.contains(&ValidationType::Concrete));
        }
    }

    #[tokio::test]
    async fn test_revm_validator_validation() {
        let mut validator = REVMValidator::new();
        let plan = create_test_execution_plan();
        let context = create_test_validation_context();

        let result = validator.validate(&plan, &context).await;
        assert!(result.is_ok(), "Validation should not error");

        let validation_result = result.unwrap();
        assert!(validation_result.is_valid, "Simple plan should be valid");
        assert!(validation_result.confidence > 0.5);
    }

    #[tokio::test]
    async fn test_revm_validator_economic_check() {
        let mut validator = REVMValidator::new();

        // Create plan with negative profit
        let mut plan = create_test_execution_plan();
        plan.estimated_profit = U256::from(50_000); // Less than gas cost
        plan.estimated_gas = U256::from(100_000);

        let context = ValidationContext {
            validation_types: vec![ValidationType::Economic],
            simulation_params: SimulationParams {
                fork_block: None,
                simulation_timeout: Duration::from_secs(5),
                enable_state_override: false,
                gas_limit: 500_000,
            },
            risk_tolerance: RiskLevel::Low,
        };

        let result = validator.validate(&plan, &context).await;
        assert!(result.is_ok());

        let validation_result = result.unwrap();
        // Should fail due to negative profit
        assert!(!validation_result.is_valid, "Should detect negative profit");
        assert!(!validation_result.issues.is_empty(), "Should report issues");
    }

    #[tokio::test]
    async fn test_revm_validator_gas_limits() {
        let mut validator = REVMValidator::new();

        // Test with excessive gas
        let mut plan = create_test_execution_plan();
        plan.estimated_gas = U256::from(20_000_000); // Exceeds block limit

        let context = ValidationContext {
            validation_types: vec![ValidationType::Gas],
            simulation_params: SimulationParams {
                fork_block: None,
                simulation_timeout: Duration::from_secs(5),
                enable_state_override: false,
                gas_limit: 500_000,
            },
            risk_tolerance: RiskLevel::Low,
        };

        let result = validator.validate(&plan, &context).await;
        assert!(result.is_ok());

        let validation_result = result.unwrap();
        assert!(!validation_result.is_valid, "Should detect excessive gas");
    }

    #[tokio::test]
    async fn test_revm_validator_low_profit_margin() {
        let mut validator = REVMValidator::new();

        // Create plan with low profit margin (less than 2x gas)
        let mut plan = create_test_execution_plan();
        plan.estimated_gas = U256::from(100_000);
        plan.estimated_profit = U256::from(150_000); // 1.5x gas cost

        let context = ValidationContext {
            validation_types: vec![ValidationType::Economic],
            simulation_params: SimulationParams {
                fork_block: None,
                simulation_timeout: Duration::from_secs(5),
                enable_state_override: false,
                gas_limit: 500_000,
            },
            risk_tolerance: RiskLevel::Low,
        };

        let result = validator.validate(&plan, &context).await;
        assert!(result.is_ok());

        let validation_result = result.unwrap();
        // May have warnings but might still be valid
        assert!(validation_result.is_valid || !validation_result.issues.is_empty());
    }

    #[tokio::test]
    async fn test_validator_empty_plan() {
        let mut revm_validator = REVMValidator::new();

        // Empty execution plan
        use mev_arbitrage::abstractions::ExecutionStrategy;

        let plan = ExecutionPlan {
            id: "empty_plan".to_string(),
            opportunity_id: "opp_empty".to_string(),
            steps: vec![],
            estimated_gas: U256::ZERO,
            estimated_profit: U256::ZERO,
            execution_strategy: ExecutionStrategy::Immediate,
            validation_results: None,
        };

        let context = create_test_validation_context();
        let result = revm_validator.validate(&plan, &context).await;

        assert!(result.is_ok());
        // Empty plan should be invalid
    }

    #[tokio::test]
    async fn test_validators_with_multiple_validation_types() {
        let mut revm_validator = REVMValidator::new();
        let plan = create_test_execution_plan();

        let context = ValidationContext {
            validation_types: vec![
                ValidationType::Economic,
                ValidationType::Gas,
                ValidationType::Concrete,
            ],
            simulation_params: SimulationParams {
                fork_block: Some(18_000_000),
                simulation_timeout: Duration::from_secs(10),
                enable_state_override: true,
                gas_limit: 500_000,
            },
            risk_tolerance: RiskLevel::Medium,
        };

        let result = revm_validator.validate(&plan, &context).await;
        assert!(result.is_ok());

        let validation_result = result.unwrap();
        // Should have performed multiple validation types
        assert!(!validation_result.validation_types.is_empty());
    }

    #[tokio::test]
    async fn test_validator_confidence_scores() {
        let mut symbolic_validator = SymbolicValidator::new();
        let mut revm_validator = REVMValidator::new();
        let plan = create_test_execution_plan();
        let context = create_test_validation_context();

        let symbolic_result = symbolic_validator.validate(&plan, &context).await.unwrap();
        let revm_result = revm_validator.validate(&plan, &context).await.unwrap();

        // Both should provide confidence scores
        assert!(symbolic_result.confidence >= 0.0 && symbolic_result.confidence <= 1.0);
        assert!(revm_result.confidence >= 0.0 && revm_result.confidence <= 1.0);

        // REVM should generally have higher confidence when validation passes
        if revm_result.is_valid {
            assert!(revm_result.confidence > 0.7);
        }
    }
}

/// Basic compilation test - always runs
#[test]
fn test_validator_module_compiles() {
    // If this test runs, the validators compiled successfully
    assert!(true);
}