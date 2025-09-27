//! Execution Coordinator
//!
//! Sophisticated execution planning and coordination system with
//! dependency resolution, resource management, and optimal scheduling.

use std::collections::{HashMap, VecDeque, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error};
use serde::{Serialize, Deserialize};

/// Execution plan with dependency graph and resource requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan<A> {
    /// Plan identifier
    pub plan_id: String,
    /// Execution steps in dependency order
    pub steps: Vec<ExecutionStep<A>>,
    /// Resource requirements
    pub resource_requirements: ResourceRequirements,
    /// Estimated execution time
    pub estimated_duration: Duration,
    /// Priority level
    pub priority: ExecutionPriority,
    /// Maximum allowed latency
    pub max_latency: Duration,
}

/// Individual execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep<A> {
    /// Step identifier
    pub step_id: String,
    /// Action to execute
    pub action: A,
    /// Dependencies (other step IDs that must complete first)
    pub dependencies: Vec<String>,
    /// Resource requirements for this step
    pub resources: ResourceRequirements,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Timeout for this step
    pub timeout: Duration,
}

/// Resource requirements specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Gas limit required
    pub gas_limit: u64,
    /// Memory requirement in MB
    pub memory_mb: u64,
    /// CPU cores required
    pub cpu_cores: f32,
    /// Network bandwidth requirement
    pub network_bandwidth: u64,
    /// Exclusive resource locks needed
    pub exclusive_locks: Vec<String>,
}

/// Execution priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExecutionPriority {
    Urgent = 0,    // Critical time-sensitive operations
    High = 1,      // High-value opportunities
    Normal = 2,    // Standard operations
    Low = 3,       // Background tasks
}

/// Retry configuration for failed executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Base delay between retries
    pub base_delay: Duration,
    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
    /// Maximum retry delay
    pub max_delay: Duration,
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            max_delay: Duration::from_secs(30),
            jitter_factor: 0.1,
        }
    }
}

/// Execution result with comprehensive metrics
#[derive(Debug, Clone)]
pub struct ExecutionResult<A> {
    /// Plan that was executed
    pub plan_id: String,
    /// Overall success status
    pub success: bool,
    /// Results from individual steps
    pub step_results: HashMap<String, StepResult<A>>,
    /// Total execution time
    pub total_duration: Duration,
    /// Resources consumed
    pub resources_used: ResourceRequirements,
    /// Error information if failed
    pub error: Option<ExecutionError>,
}

/// Result of an individual execution step
#[derive(Debug, Clone)]
pub struct StepResult<A> {
    /// Step identifier
    pub step_id: String,
    /// Success status
    pub success: bool,
    /// Execution duration
    pub duration: Duration,
    /// Retry attempts made
    pub retry_attempts: u32,
    /// Output action (if modified)
    pub output_action: Option<A>,
    /// Error if failed
    pub error: Option<String>,
}

/// Sophisticated execution coordinator
pub struct ExecutionCoordinator<A> {
    /// Pending execution plans
    pending_plans: Arc<RwLock<VecDeque<ExecutionPlan<A>>>>,
    /// Currently executing plans
    executing_plans: Arc<RwLock<HashMap<String, ExecutionContext<A>>>>,
    /// Completed executions (for analysis)
    completed_executions: Arc<RwLock<VecDeque<ExecutionResult<A>>>>,
    /// Resource manager
    resource_manager: Arc<ResourceManager>,
    /// Dependency resolver
    dependency_resolver: Arc<DependencyResolver>,
    /// Scheduler configuration
    config: CoordinatorConfig,
}

/// Execution context for tracking in-progress plans
#[derive(Debug)]
struct ExecutionContext<A> {
    plan: ExecutionPlan<A>,
    started_at: Instant,
    completed_steps: HashSet<String>,
    failed_steps: HashSet<String>,
    current_step: Option<String>,
}

/// Resource manager for controlling execution resources
pub struct ResourceManager {
    /// Available gas pool
    gas_pool: Arc<Semaphore>,
    /// Memory semaphore
    memory_sem: Arc<Semaphore>,
    /// CPU semaphore
    cpu_sem: Arc<Semaphore>,
    /// Exclusive resource locks
    exclusive_locks: Arc<RwLock<HashSet<String>>>,
}

/// Dependency resolver for execution ordering
pub struct DependencyResolver {
    /// Dependency graph cache
    dependency_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

/// Coordinator configuration
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// Maximum concurrent executions
    pub max_concurrent_executions: usize,
    /// Resource pool sizes
    pub resource_limits: ResourceLimits,
    /// Execution timeout
    pub default_timeout: Duration,
    /// Queue size limits
    pub max_queue_size: usize,
    /// Enable execution optimization
    pub enable_optimization: bool,
}

/// Resource pool limits
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Total gas pool
    pub total_gas: u64,
    /// Total memory pool (MB)
    pub total_memory_mb: u64,
    /// Total CPU cores
    pub total_cpu_cores: f32,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_executions: 10,
            resource_limits: ResourceLimits {
                total_gas: 10_000_000,
                total_memory_mb: 1024,
                total_cpu_cores: 4.0,
            },
            default_timeout: Duration::from_secs(30),
            max_queue_size: 1000,
            enable_optimization: true,
        }
    }
}

impl<A> ExecutionCoordinator<A>
where
    A: Send + Sync + Clone + 'static,
{
    /// Create a new execution coordinator
    pub fn new() -> Self {
        Self::with_config(CoordinatorConfig::default())
    }

    /// Create coordinator with custom configuration
    pub fn with_config(config: CoordinatorConfig) -> Self {
        let resource_manager = Arc::new(ResourceManager::new(&config.resource_limits));
        let dependency_resolver = Arc::new(DependencyResolver::new());

        Self {
            pending_plans: Arc::new(RwLock::new(VecDeque::new())),
            executing_plans: Arc::new(RwLock::new(HashMap::new())),
            completed_executions: Arc::new(RwLock::new(VecDeque::new())),
            resource_manager,
            dependency_resolver,
            config,
        }
    }

    /// Submit an execution plan
    pub async fn submit_plan(&self, plan: ExecutionPlan<A>) -> Result<(), CoordinatorError> {
        // Validate plan
        self.validate_plan(&plan).await?;

        // Optimize execution order if enabled
        let optimized_plan = if self.config.enable_optimization {
            self.optimize_plan(plan).await?
        } else {
            plan
        };

        // Add to queue
        let mut pending = self.pending_plans.write().await;
        if pending.len() >= self.config.max_queue_size {
            return Err(CoordinatorError::QueueFull);
        }

        pending.push_back(optimized_plan);
        debug!("Execution plan queued: {}", pending.back().unwrap().plan_id);

        Ok(())
    }

    /// Execute the next plan in queue
    pub async fn execute_next(&self) -> Result<Option<ExecutionResult<A>>, CoordinatorError> {
        // Get next plan
        let plan = {
            let mut pending = self.pending_plans.write().await;
            pending.pop_front()
        };

        if let Some(plan) = plan {
            self.execute_plan(plan).await.map(Some)
        } else {
            Ok(None)
        }
    }

    /// Execute a specific plan
    async fn execute_plan(&self, plan: ExecutionPlan<A>) -> Result<ExecutionResult<A>, CoordinatorError> {
        let plan_id = plan.plan_id.clone();
        let start_time = Instant::now();

        debug!("Starting execution of plan: {}", plan_id);

        // Create execution context
        let context = ExecutionContext {
            plan: plan.clone(),
            started_at: start_time,
            completed_steps: HashSet::new(),
            failed_steps: HashSet::new(),
            current_step: None,
        };

        // Add to executing plans
        self.executing_plans.write().await.insert(plan_id.clone(), context);

        // Execute steps in dependency order
        let execution_order = self.dependency_resolver.resolve_dependencies(&plan).await?;
        let mut step_results = HashMap::new();
        let mut total_resources = ResourceRequirements::default();

        for step_id in execution_order {
            let step = plan.steps.iter()
                .find(|s| s.step_id == step_id)
                .ok_or_else(|| CoordinatorError::StepNotFound(step_id.clone()))?;

            // Acquire resources
            self.resource_manager.acquire_resources(&step.resources).await?;

            // Execute step with retry logic
            let step_result = self.execute_step_with_retry(step).await;

            // Release resources
            self.resource_manager.release_resources(&step.resources).await;

            // Update resources used
            total_resources.gas_limit += step.resources.gas_limit;
            total_resources.memory_mb = total_resources.memory_mb.max(step.resources.memory_mb);

            // Record result
            step_results.insert(step_id.clone(), step_result.clone());

            // Check if step failed
            if !step_result.success {
                // Handle failure based on step configuration
                break;
            }

            // Update execution context
            {
                let mut executing = self.executing_plans.write().await;
                if let Some(context) = executing.get_mut(&plan_id) {
                    context.completed_steps.insert(step_id);
                }
            }
        }

        // Remove from executing plans
        self.executing_plans.write().await.remove(&plan_id);

        // Create execution result
        let total_duration = start_time.elapsed();
        let success = step_results.values().all(|r| r.success);

        let result = ExecutionResult {
            plan_id: plan_id.clone(),
            success,
            step_results,
            total_duration,
            resources_used: total_resources,
            error: if success { None } else { Some(ExecutionError::StepFailed) },
        };

        // Store completed execution
        {
            let mut completed = self.completed_executions.write().await;
            completed.push_back(result.clone());

            // Keep only recent executions
            if completed.len() > 1000 {
                completed.pop_front();
            }
        }

        debug!("Completed execution of plan: {} in {:?}", plan_id, total_duration);

        Ok(result)
    }

    /// Execute a single step with retry logic
    async fn execute_step_with_retry(&self, step: &ExecutionStep<A>) -> StepResult<A> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < step.retry_config.max_attempts {
            attempts += 1;
            let start_time = Instant::now();

            // Simulate step execution (actual implementation would execute the action)
            let result = self.execute_single_step(step).await;

            let duration = start_time.elapsed();

            match result {
                Ok(output) => {
                    return StepResult {
                        step_id: step.step_id.clone(),
                        success: true,
                        duration,
                        retry_attempts: attempts,
                        output_action: output,
                        error: None,
                    };
                }
                Err(error) => {
                    last_error = Some(error.to_string());

                    if attempts < step.retry_config.max_attempts {
                        // Calculate delay with exponential backoff and jitter
                        let delay = self.calculate_retry_delay(&step.retry_config, attempts);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        StepResult {
            step_id: step.step_id.clone(),
            success: false,
            duration: Duration::default(),
            retry_attempts: attempts,
            output_action: None,
            error: last_error,
        }
    }

    /// Execute a single step (placeholder implementation)
    async fn execute_single_step(&self, _step: &ExecutionStep<A>) -> Result<Option<A>, CoordinatorError> {
        // Actual implementation would execute the action
        // For now, simulate success/failure
        if rand::random::<f64>() > 0.1 {
            Ok(None)
        } else {
            Err(CoordinatorError::StepExecutionFailed("Simulated failure".to_string()))
        }
    }

    /// Calculate retry delay with exponential backoff and jitter
    fn calculate_retry_delay(&self, config: &RetryConfig, attempt: u32) -> Duration {
        let base_delay = config.base_delay.as_millis() as f64;
        let exponential_delay = base_delay * config.backoff_multiplier.powi(attempt as i32 - 1);

        // Apply jitter
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * config.jitter_factor;
        let delay_with_jitter = exponential_delay * (1.0 + jitter);

        // Cap at max delay
        let final_delay = delay_with_jitter.min(config.max_delay.as_millis() as f64);

        Duration::from_millis(final_delay as u64)
    }

    /// Validate execution plan
    async fn validate_plan(&self, plan: &ExecutionPlan<A>) -> Result<(), CoordinatorError> {
        if plan.steps.is_empty() {
            return Err(CoordinatorError::InvalidPlan("No steps in plan".to_string()));
        }

        // Check for circular dependencies
        self.dependency_resolver.check_circular_dependencies(plan).await?;

        // Validate resource requirements
        if plan.resource_requirements.gas_limit > self.config.resource_limits.total_gas {
            return Err(CoordinatorError::InsufficientResources("Gas limit exceeded".to_string()));
        }

        Ok(())
    }

    /// Optimize execution plan for better performance
    async fn optimize_plan(&self, mut plan: ExecutionPlan<A>) -> Result<ExecutionPlan<A>, CoordinatorError> {
        // Optimization strategies:
        // 1. Reorder steps to minimize resource contention
        // 2. Batch similar operations
        // 3. Parallelize independent steps

        // For now, just return the original plan
        // Real implementation would apply sophisticated optimization algorithms
        Ok(plan)
    }

    /// Get execution statistics
    pub async fn get_statistics(&self) -> CoordinatorStatistics {
        let completed = self.completed_executions.read().await;
        let executing_count = self.executing_plans.read().await.len();
        let pending_count = self.pending_plans.read().await.len();

        let total_executions = completed.len();
        let successful_executions = completed.iter().filter(|r| r.success).count();

        let avg_duration = if total_executions > 0 {
            completed.iter()
                .map(|r| r.total_duration)
                .sum::<Duration>() / total_executions as u32
        } else {
            Duration::default()
        };

        CoordinatorStatistics {
            total_executions,
            successful_executions,
            currently_executing: executing_count,
            pending_executions: pending_count,
            average_execution_time: avg_duration,
            success_rate: if total_executions > 0 {
                successful_executions as f64 / total_executions as f64
            } else {
                0.0
            },
        }
    }
}

impl ResourceManager {
    fn new(limits: &ResourceLimits) -> Self {
        Self {
            gas_pool: Arc::new(Semaphore::new(limits.total_gas as usize)),
            memory_sem: Arc::new(Semaphore::new(limits.total_memory_mb as usize)),
            cpu_sem: Arc::new(Semaphore::new((limits.total_cpu_cores * 100.0) as usize)),
            exclusive_locks: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    async fn acquire_resources(&self, requirements: &ResourceRequirements) -> Result<(), CoordinatorError> {
        // Acquire gas
        self.gas_pool.acquire_many(requirements.gas_limit as u32).await
            .map_err(|_| CoordinatorError::ResourceAcquisitionFailed("Gas".to_string()))?;

        // Acquire memory
        self.memory_sem.acquire_many(requirements.memory_mb as u32).await
            .map_err(|_| CoordinatorError::ResourceAcquisitionFailed("Memory".to_string()))?;

        // Acquire CPU
        let cpu_permits = (requirements.cpu_cores * 100.0) as u32;
        self.cpu_sem.acquire_many(cpu_permits).await
            .map_err(|_| CoordinatorError::ResourceAcquisitionFailed("CPU".to_string()))?;

        // Acquire exclusive locks
        {
            let mut locks = self.exclusive_locks.write().await;
            for lock_name in &requirements.exclusive_locks {
                if locks.contains(lock_name) {
                    return Err(CoordinatorError::ResourceLocked(lock_name.clone()));
                }
                locks.insert(lock_name.clone());
            }
        }

        Ok(())
    }

    async fn release_resources(&self, requirements: &ResourceRequirements) {
        // Release exclusive locks first
        {
            let mut locks = self.exclusive_locks.write().await;
            for lock_name in &requirements.exclusive_locks {
                locks.remove(lock_name);
            }
        }

        // Note: Semaphore permits are automatically released when dropped
    }
}

impl DependencyResolver {
    fn new() -> Self {
        Self {
            dependency_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn resolve_dependencies<A>(&self, plan: &ExecutionPlan<A>) -> Result<Vec<String>, CoordinatorError> {
        let mut resolved_order = Vec::new();
        let mut remaining_steps: HashMap<String, Vec<String>> = plan.steps.iter()
            .map(|step| (step.step_id.clone(), step.dependencies.clone()))
            .collect();

        while !remaining_steps.is_empty() {
            let mut made_progress = false;

            // Find steps with no dependencies
            let ready_steps: Vec<String> = remaining_steps.iter()
                .filter(|(_, deps)| deps.is_empty())
                .map(|(id, _)| id.clone())
                .collect();

            if ready_steps.is_empty() {
                return Err(CoordinatorError::CircularDependency);
            }

            // Add ready steps to execution order
            for step_id in ready_steps {
                resolved_order.push(step_id.clone());
                remaining_steps.remove(&step_id);

                // Remove this step from other steps' dependencies
                for (_, deps) in remaining_steps.iter_mut() {
                    deps.retain(|dep| dep != &step_id);
                }

                made_progress = true;
            }

            if !made_progress {
                return Err(CoordinatorError::CircularDependency);
            }
        }

        Ok(resolved_order)
    }

    async fn check_circular_dependencies<A>(&self, plan: &ExecutionPlan<A>) -> Result<(), CoordinatorError> {
        // Use topological sort to detect cycles
        self.resolve_dependencies(plan).await.map(|_| ())
    }
}

/// Coordinator statistics
#[derive(Debug)]
pub struct CoordinatorStatistics {
    pub total_executions: usize,
    pub successful_executions: usize,
    pub currently_executing: usize,
    pub pending_executions: usize,
    pub average_execution_time: Duration,
    pub success_rate: f64,
}

/// Coordinator errors
#[derive(Debug, thiserror::Error)]
pub enum CoordinatorError {
    #[error("Invalid execution plan: {0}")]
    InvalidPlan(String),
    #[error("Circular dependency detected")]
    CircularDependency,
    #[error("Step not found: {0}")]
    StepNotFound(String),
    #[error("Queue is full")]
    QueueFull,
    #[error("Insufficient resources: {0}")]
    InsufficientResources(String),
    #[error("Resource acquisition failed: {0}")]
    ResourceAcquisitionFailed(String),
    #[error("Resource locked: {0}")]
    ResourceLocked(String),
    #[error("Step execution failed: {0}")]
    StepExecutionFailed(String),
}

/// Execution errors
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Step execution failed")]
    StepFailed,
    #[error("Execution timeout")]
    Timeout,
    #[error("Resource constraint violation")]
    ResourceConstraint,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let coordinator: ExecutionCoordinator<String> = ExecutionCoordinator::new();
        let stats = coordinator.get_statistics().await;
        assert_eq!(stats.total_executions, 0);
    }

    #[tokio::test]
    async fn test_dependency_resolution() {
        let resolver = DependencyResolver::new();

        let plan = ExecutionPlan {
            plan_id: "test".to_string(),
            steps: vec![
                ExecutionStep {
                    step_id: "step1".to_string(),
                    action: "action1".to_string(),
                    dependencies: vec![],
                    resources: ResourceRequirements::default(),
                    retry_config: RetryConfig::default(),
                    timeout: Duration::from_secs(10),
                },
                ExecutionStep {
                    step_id: "step2".to_string(),
                    action: "action2".to_string(),
                    dependencies: vec!["step1".to_string()],
                    resources: ResourceRequirements::default(),
                    retry_config: RetryConfig::default(),
                    timeout: Duration::from_secs(10),
                },
            ],
            resource_requirements: ResourceRequirements::default(),
            estimated_duration: Duration::from_secs(20),
            priority: ExecutionPriority::Normal,
            max_latency: Duration::from_secs(60),
        };

        let order = resolver.resolve_dependencies(&plan).await.unwrap();
        assert_eq!(order, vec!["step1", "step2"]);
    }
}