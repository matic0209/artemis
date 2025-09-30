//! Unified Arbitrage Manager
//!
//! This module provides a high-level manager that coordinates all arbitrage
//! detection, path exploration, validation, optimization, and execution
//! components using the abstract interfaces.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};
use alloy_primitives::{Address, U256};

use crate::abstractions::*;
use crate::component_factory::ArbitrageComponentFactory;

/// Configuration for the unified arbitrage manager
#[derive(Debug, Clone)]
pub struct ManagerConfig {
    /// Enabled detector types and their configurations
    pub detectors: Vec<DetectorSpec>,
    /// Explorer configuration
    pub explorer: ExplorerSpec,
    /// Validator configurations
    pub validators: Vec<ValidatorSpec>,
    /// Optimizer configuration
    pub optimizer: OptimizerSpec,
    /// Executor configuration
    pub executor: ExecutorSpec,
    /// Global settings
    pub global: GlobalConfig,
}

/// Detector specification
#[derive(Debug, Clone)]
pub struct DetectorSpec {
    pub name: String,
    pub detector_type: String,
    pub config: DetectorConfig,
    pub weight: f64, // Weight for combining results
}

/// Explorer specification
#[derive(Debug, Clone)]
pub struct ExplorerSpec {
    pub explorer_type: String,
    pub config: ExplorerConfig,
}

/// Validator specification
#[derive(Debug, Clone)]
pub struct ValidatorSpec {
    pub name: String,
    pub validator_type: String,
    pub config: ValidatorConfig,
    pub required: bool, // Whether this validator must pass
}

/// Optimizer specification
#[derive(Debug, Clone)]
pub struct OptimizerSpec {
    pub optimizer_type: String,
    pub enabled: bool,
}

/// Executor specification
#[derive(Debug, Clone)]
pub struct ExecutorSpec {
    pub executor_type: String,
    pub enabled: bool,
}

/// Global configuration
#[derive(Debug, Clone)]
pub struct GlobalConfig {
    /// Maximum time for complete detection cycle
    pub max_detection_time: Duration,
    /// Maximum opportunities to process per cycle
    pub max_opportunities_per_cycle: usize,
    /// Minimum confidence threshold for processing
    pub min_confidence_threshold: f64,
    /// Enable parallel processing
    pub enable_parallel_processing: bool,
    /// Enable caching
    pub enable_caching: bool,
    /// Cache TTL
    pub cache_ttl: Duration,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            detectors: vec![
                DetectorSpec {
                    name: "enhanced_graph".to_string(),
                    detector_type: "enhanced_graph".to_string(),
                    config: DetectorConfig {
                        enabled: true,
                        confidence_threshold: 0.7,
                        max_opportunities: 10,
                        timeout: Duration::from_secs(30),
                        detector_specific: serde_json::json!({}),
                    },
                    weight: 1.0,
                },
            ],
            explorer: ExplorerSpec {
                explorer_type: "defi_analyzer".to_string(),
                config: ExplorerConfig {
                    max_depth: 5,
                    max_paths: 100,
                    enable_optimization: true,
                    timeout: Duration::from_secs(10),
                },
            },
            validators: vec![
                ValidatorSpec {
                    name: "economic".to_string(),
                    validator_type: "economic".to_string(),
                    config: ValidatorConfig {
                        enabled_types: vec![ValidationType::Economic],
                        strict_mode: false,
                        timeout: Duration::from_secs(5),
                    },
                    required: true,
                },
            ],
            optimizer: OptimizerSpec {
                optimizer_type: "z3".to_string(),
                enabled: true,
            },
            executor: ExecutorSpec {
                executor_type: "simulation".to_string(),
                enabled: true,
            },
            global: GlobalConfig {
                max_detection_time: Duration::from_secs(60),
                max_opportunities_per_cycle: 50,
                min_confidence_threshold: 0.6,
                enable_parallel_processing: true,
                enable_caching: true,
                cache_ttl: Duration::from_secs(30),
            },
        }
    }
}

/// Unified arbitrage manager
pub struct UnifiedArbitrageManager {
    /// Component factory
    factory: Arc<ArbitrageComponentFactory>,
    /// Active detectors
    detectors: Vec<(String, Box<dyn ArbitrageDetector>, f64)>, // name, detector, weight
    /// Path explorer
    explorer: Box<dyn PathExplorer>,
    /// Validators
    validators: Vec<(String, Box<dyn Validator>, bool)>, // name, validator, required
    /// Optimizer
    optimizer: Option<Box<dyn Optimizer>>,
    /// Executor
    executor: Option<Box<dyn Executor>>,
    /// Configuration
    config: ManagerConfig,
    /// Metrics
    metrics: Arc<RwLock<ManagerMetrics>>,
    /// Cache for opportunities
    opportunity_cache: Arc<RwLock<OpportunityCache>>,
}

/// Manager metrics
#[derive(Debug, Default, Clone)]
pub struct ManagerMetrics {
    pub total_detections: u64,
    pub successful_detections: u64,
    pub total_validations: u64,
    pub successful_validations: u64,
    pub total_optimizations: u64,
    pub successful_optimizations: u64,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub avg_detection_time: Duration,
    pub avg_validation_time: Duration,
    pub avg_optimization_time: Duration,
    pub avg_execution_time: Duration,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Opportunity cache
#[derive(Debug, Default)]
pub struct OpportunityCache {
    opportunities: HashMap<String, (ArbitrageOpportunity, Instant)>,
    execution_plans: HashMap<String, (ExecutionPlan, Instant)>,
}

/// Complete arbitrage result
#[derive(Debug, Clone)]
pub struct ArbitrageResult {
    pub opportunity: ArbitrageOpportunity,
    pub execution_plan: Option<ExecutionPlan>,
    pub validation_results: Vec<ValidationResult>,
    pub optimization_result: Option<OptimizationResult>,
    pub execution_result: Option<ExecutionResult>,
    pub overall_confidence: f64,
    pub processing_time: Duration,
    pub success: bool,
}

impl UnifiedArbitrageManager {
    /// Create new manager with configuration
    pub async fn new(config: ManagerConfig) -> Result<Self> {
        let factory = Arc::new(ArbitrageComponentFactory::new());

        // Create detectors
        let mut detectors = Vec::new();
        for spec in &config.detectors {
            if spec.config.enabled {
                let detector = factory.create_detector(&spec.detector_type, &spec.config)?;
                detectors.push((spec.name.clone(), detector, spec.weight));
            }
        }

        // Create explorer
        let explorer = factory.create_explorer(&config.explorer.explorer_type, &config.explorer.config)?;

        // Create validators
        let mut validators = Vec::new();
        for spec in &config.validators {
            if spec.config.enabled_types.iter().any(|_| true) { // Check if any types enabled
                let validator = factory.create_validator(&spec.validator_type, &spec.config)?;
                validators.push((spec.name.clone(), validator, spec.required));
            }
        }

        // Create optimizer
        let optimizer = if config.optimizer.enabled {
            Some(factory.create_optimizer(&config.optimizer.optimizer_type, "default")?)
        } else {
            None
        };

        // Create executor
        let executor = if config.executor.enabled {
            Some(factory.create_executor(&config.executor.executor_type, "default")?)
        } else {
            None
        };

        Ok(Self {
            factory,
            detectors,
            explorer,
            validators,
            optimizer,
            executor,
            config,
            metrics: Arc::new(RwLock::new(ManagerMetrics::default())),
            opportunity_cache: Arc::new(RwLock::new(OpportunityCache::default())),
        })
    }

    /// Main detection and processing cycle
    pub async fn process_arbitrage_cycle(&mut self, context: &DetectionContext) -> Result<Vec<ArbitrageResult>> {
        let cycle_start = Instant::now();
        // Pre-allocate based on max opportunities per cycle
        let mut results = Vec::with_capacity(self.config.global.max_opportunities_per_cycle);

        info!("Starting arbitrage detection cycle");

        // Phase 1: Detection
        let opportunities = self.detect_opportunities(context).await?;
        info!("Detected {} opportunities", opportunities.len());

        // Phase 2: Path exploration and processing
        for opportunity in opportunities.into_iter().take(self.config.global.max_opportunities_per_cycle) {
            if opportunity.confidence < self.config.global.min_confidence_threshold {
                debug!("Skipping opportunity {} due to low confidence: {}", opportunity.id, opportunity.confidence);
                continue;
            }

            let result = self.process_single_opportunity(opportunity, context).await?;
            results.push(result);

            // Check timeout
            if cycle_start.elapsed() > self.config.global.max_detection_time {
                warn!("Detection cycle timeout reached, stopping processing");
                break;
            }
        }

        info!("Completed arbitrage cycle in {:?} with {} results", cycle_start.elapsed(), results.len());
        Ok(results)
    }

    /// Detect opportunities using all configured detectors
    async fn detect_opportunities(&mut self, context: &DetectionContext) -> Result<Vec<ArbitrageOpportunity>> {
        // Pre-allocate with expected capacity for better performance
        let detector_count = self.detectors.len();
        let mut all_opportunities = Vec::with_capacity(detector_count * 10); // Estimate 10 opportunities per detector
        let mut detector_results = Vec::with_capacity(detector_count);

        // Run detectors (in parallel if enabled)
        if self.config.global.enable_parallel_processing {
            // Parallel detection (would need to refactor for actual parallel execution)
            for (name, detector, weight) in &mut self.detectors {
                let detection_start = Instant::now();
                match detector.detect(context).await {
                    Ok(detection_result) => {
                        info!("Detector {} found {} opportunities in {:?}",
                              name, detection_result.opportunities.len(), detection_start.elapsed());
                        detector_results.push((detection_result, *weight));
                    }
                    Err(e) => {
                        error!("Detector {} failed: {}", name, e);
                    }
                }
            }
        } else {
            // Sequential detection
            for (name, detector, weight) in &mut self.detectors {
                let detection_start = Instant::now();
                match detector.detect(context).await {
                    Ok(detection_result) => {
                        info!("Detector {} found {} opportunities in {:?}",
                              name, detection_result.opportunities.len(), detection_start.elapsed());
                        detector_results.push((detection_result, *weight));
                    }
                    Err(e) => {
                        error!("Detector {} failed: {}", name, e);
                    }
                }
            }
        }

        // Combine and deduplicate opportunities
        for (detection_result, weight) in detector_results {
            for mut opportunity in detection_result.opportunities {
                // Apply detector weight to confidence
                opportunity.confidence = (opportunity.confidence * weight).min(1.0);
                all_opportunities.push(opportunity);
            }
        }

        // Deduplicate and rank opportunities
        all_opportunities = self.deduplicate_opportunities(all_opportunities);
        all_opportunities.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        Ok(all_opportunities)
    }

    /// Process a single opportunity through the complete pipeline
    async fn process_single_opportunity(&mut self, opportunity: ArbitrageOpportunity, context: &DetectionContext) -> Result<ArbitrageResult> {
        let processing_start = Instant::now();
        let mut result = ArbitrageResult {
            opportunity: opportunity.clone(),
            execution_plan: None,
            validation_results: Vec::new(),
            optimization_result: None,
            execution_result: None,
            overall_confidence: opportunity.confidence,
            processing_time: Duration::default(),
            success: false,
        };

        // Phase 1: Path exploration
        let exploration_context = ExplorationContext {
            max_gas_limit: 500_000,
            max_steps: 10,
            timeout: Duration::from_secs(5),
            user_address: Address::ZERO, // Would be configured
            available_capital: U256::from(1000000000000000000u64), // 1 ETH
        };

        let execution_plans = match self.explorer.explore_paths(&opportunity, &exploration_context).await {
            Ok(plans) => plans,
            Err(e) => {
                error!("Path exploration failed for opportunity {}: {}", opportunity.id, e);
                result.processing_time = processing_start.elapsed();
                return Ok(result);
            }
        };

        if execution_plans.is_empty() {
            debug!("No execution paths found for opportunity {}", opportunity.id);
            result.processing_time = processing_start.elapsed();
            return Ok(result);
        }

        // Take the best execution plan
        let mut best_plan = execution_plans[0].clone();
        result.execution_plan = Some(best_plan.clone());

        // Phase 2: Validation
        let validation_context = ValidationContext {
            validation_types: vec![ValidationType::Economic, ValidationType::Risk],
            simulation_params: SimulationParams {
                fork_block: Some(context.block_number),
                simulation_timeout: Duration::from_secs(10),
                enable_state_override: true,
                gas_limit: 500_000,
            },
            risk_tolerance: RiskLevel::Medium,
        };

        let mut all_validations_passed = true;
        for (name, validator, required) in &mut self.validators {
            match validator.validate(&best_plan, &validation_context).await {
                Ok(validation_result) => {
                    info!("Validator {} result: valid={}, confidence={}",
                          name, validation_result.is_valid, validation_result.confidence);

                    if *required && !validation_result.is_valid {
                        all_validations_passed = false;
                    }

                    result.validation_results.push(validation_result);
                }
                Err(e) => {
                    error!("Validator {} failed: {}", name, e);
                    if *required {
                        all_validations_passed = false;
                    }
                }
            }
        }

        if !all_validations_passed {
            debug!("Required validations failed for opportunity {}", opportunity.id);
            result.processing_time = processing_start.elapsed();
            return Ok(result);
        }

        // Phase 3: Optimization
        if let Some(optimizer) = &mut self.optimizer {
            let optimization_context = OptimizationContext {
                optimization_target: OptimizationTarget::MaximizeProfit,
                constraints: vec![
                    OptimizationConstraint::MaxGasCost(U256::from(300_000)),
                    OptimizationConstraint::MaxSlippage(0.05),
                ],
                timeout: Duration::from_secs(10),
            };

            match optimizer.optimize(&best_plan, &optimization_context).await {
                Ok(optimization_result) => {
                    info!("Optimization improved plan by {:.2}%",
                          optimization_result.improvement_metrics.overall_score * 100.0);
                    best_plan = optimization_result.optimized_plan.clone();
                    result.execution_plan = Some(best_plan.clone());
                    result.optimization_result = Some(optimization_result);
                }
                Err(e) => {
                    warn!("Optimization failed for opportunity {}: {}", opportunity.id, e);
                }
            }
        }

        // Phase 4: Execution (if enabled and not dry run)
        if let Some(executor) = &mut self.executor {
            let execution_context = ExecutionContext {
                dry_run: true, // Default to simulation
                gas_strategy: GasStrategy::Standard,
                slippage_tolerance: 0.01,
                deadline: Some(Duration::from_secs(60)),
                private_pool: false,
            };

            match executor.execute(&best_plan, &execution_context).await {
                Ok(execution_result) => {
                    info!("Execution result: success={}, profit={}",
                          execution_result.success, execution_result.actual_profit);
                    result.execution_result = Some(execution_result.clone());
                    result.success = execution_result.success;
                }
                Err(e) => {
                    error!("Execution failed for opportunity {}: {}", opportunity.id, e);
                }
            }
        } else {
            // Mark as successful if validation passed and no execution attempted
            result.success = all_validations_passed;
        }

        // Calculate overall confidence
        result.overall_confidence = self.calculate_overall_confidence(&result);
        result.processing_time = processing_start.elapsed();

        Ok(result)
    }

    /// Deduplicate similar opportunities
    fn deduplicate_opportunities(&self, opportunities: Vec<ArbitrageOpportunity>) -> Vec<ArbitrageOpportunity> {
        // Simple deduplication based on opportunity type and tokens
        // In practice, would use more sophisticated similarity detection
        let mut unique_opportunities = Vec::new();
        let mut seen_signatures = std::collections::HashSet::new();

        for opportunity in opportunities {
            let signature = self.generate_opportunity_signature(&opportunity);
            if !seen_signatures.contains(&signature) {
                seen_signatures.insert(signature);
                unique_opportunities.push(opportunity);
            }
        }

        unique_opportunities
    }

    /// Generate a signature for opportunity deduplication
    fn generate_opportunity_signature(&self, opportunity: &ArbitrageOpportunity) -> String {
        match &opportunity.opportunity_type {
            OpportunityType::SimpleArbitrage { token_a, token_b, .. } => {
                format!("simple_{}_{}", token_a, token_b)
            }
            OpportunityType::TriangularArbitrage { tokens, .. } => {
                format!("triangular_{}_{}_{}", tokens[0], tokens[1], tokens[2])
            }
            _ => opportunity.id.clone(),
        }
    }

    /// Calculate overall confidence score
    fn calculate_overall_confidence(&self, result: &ArbitrageResult) -> f64 {
        let mut confidence = result.opportunity.confidence;

        // Factor in validation results
        if !result.validation_results.is_empty() {
            let avg_validation_confidence = result.validation_results.iter()
                .map(|v| v.confidence)
                .sum::<f64>() / result.validation_results.len() as f64;
            confidence = (confidence + avg_validation_confidence) / 2.0;
        }

        // Factor in optimization improvement
        if let Some(opt_result) = &result.optimization_result {
            confidence += opt_result.improvement_metrics.overall_score * 0.1;
        }

        // Factor in execution success
        if let Some(exec_result) = &result.execution_result {
            if exec_result.success {
                confidence = (confidence + 0.9) / 2.0;
            } else {
                confidence *= 0.5;
            }
        }

        confidence.min(1.0)
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> ManagerMetrics {
        (*self.metrics.read().await).clone()
    }

    /// Update configuration
    pub async fn update_config(&mut self, new_config: ManagerConfig) -> Result<()> {
        // This would involve recreating components with new configurations
        // For now, just update the stored config
        self.config = new_config;
        Ok(())
    }

    /// Health check for all components
    pub async fn health_check(&mut self) -> Result<HashMap<String, HealthStatus>> {
        let mut health_results = HashMap::new();

        // Check detectors
        for (name, detector, _) in &self.detectors {
            match detector.health_check().await {
                Ok(health) => {
                    health_results.insert(format!("detector_{}", name), health);
                }
                Err(e) => {
                    health_results.insert(format!("detector_{}", name), HealthStatus {
                        is_healthy: false,
                        status_message: format!("Health check failed: {}", e),
                        last_check: Instant::now(),
                        metrics: HashMap::new(),
                    });
                }
            }
        }

        // Add other component health checks as needed

        Ok(health_results)
    }
}