//! Complete Path Explorer for DFS Symbolic Execution
//! 
//! This module provides complete path exploration functionality for symbolic execution,
//! maintaining full compatibility with DeFiAligner's path explorer implementation.

use alloy_primitives::{Address, U256};
use anyhow::Result;
use tracing::{info, debug, warn, error};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};
use z3::{Context, Config, ast::{BV, Bool, Ast}};

use crate::{
    types::AnalysisEvent,
    error::{DeFiResult, DeFiAnalyzerError},
    evm_interpreter_complete::{SEVM, SymbolicEVMInterpreter, EVMExecutionState, ExecutionPath, ExecutionPathList, SymbolicStack, SymbolicMemory, Contract, OpCode},
};

/// Complete Path Explorer for DFS symbolic execution
pub struct PathExplorer<'ctx> {
    /// Z3 context
    ctx: z3::Context,
    /// SEVM instance
    sevm: SEVM<'ctx>,
    /// Execution paths
    execution_paths: Vec<ExecutionPath<'ctx>>,
    /// Visited states for loop detection
    visited_states: HashSet<String>,
    /// Configuration
    config: PathExplorerConfig,
    /// Statistics
    stats: PathExplorerStats,
    /// Start time
    start_time: Instant,
}

/// Path explorer configuration
#[derive(Debug, Clone)]
pub struct PathExplorerConfig {
    /// Maximum depth
    pub max_depth: u32,
    /// Maximum paths
    pub max_paths: u32,
    /// Timeout
    pub timeout: Duration,
    /// Enable pruning
    pub enable_pruning: bool,
    /// Enable loop detection
    pub enable_loop_detection: bool,
    /// Enable path optimization
    pub enable_path_optimization: bool,
    /// Enable symbolic execution
    pub enable_symbolic_execution: bool,
}

impl Default for PathExplorerConfig {
    fn default() -> Self {
        Self {
            max_depth: 1000,
            max_paths: 10000,
            timeout: Duration::from_secs(300),
            enable_pruning: true,
            enable_loop_detection: true,
            enable_path_optimization: true,
            enable_symbolic_execution: true,
        }
    }
}

/// Path explorer statistics
#[derive(Debug, Clone, Default)]
pub struct PathExplorerStats {
    /// Total paths explored
    pub total_paths: u32,
    /// Pruned paths
    pub pruned_paths: u32,
    /// Loops detected
    pub loops_detected: u32,
    /// Execution time
    pub execution_time: Duration,
    /// Max depth reached
    pub max_depth_reached: u32,
    /// Symbolic execution time
    pub symbolic_execution_time: Duration,
    /// Path optimization time
    pub path_optimization_time: Duration,
}

impl<'ctx> PathExplorer<'ctx> {
    /// Create new path explorer
    pub fn new(_ctx: z3::Context, config: PathExplorerConfig) -> Self {
        let ctx = z3::Context::new(&z3::Config::new());
        let sevm = SEVM::new(&ctx);
        Self {
            sevm,
            ctx,
            execution_paths: Vec::new(),
            visited_states: HashSet::new(),
            config,
            stats: PathExplorerStats::default(),
            start_time: Instant::now(),
        }
    }

    /// Explore paths using DFS
    pub fn explore_paths(&mut self, interpreter: &mut SymbolicEVMInterpreter, event: &AnalysisEvent) -> DeFiResult<Vec<ExecutionPath>> {
        info!("Starting path exploration for contract: {:?}", event.contract_address);
        
        let start_time = Instant::now();
        self.start_time = start_time;
        self.stats = PathExplorerStats::default();
        
        // Initialize execution state
        let initial_state = self.initialize_execution_state(interpreter, event)?;
        
        // Start DFS exploration
        let mut current_path = ExecutionPath::new();
        let mut execution_path_list = ExecutionPathList::new();
        
        self.dfs_explore(interpreter, &initial_state, &mut current_path, 0, &mut execution_path_list)?;
        
        // Update stats after all borrowing is done
        let execution_time = start_time.elapsed();
        self.stats.execution_time = execution_time;
        info!("Path exploration completed: {} paths found in {:?}", 
              self.stats.total_paths, self.stats.execution_time);
        
        Ok(execution_path_list.paths().clone())
    }

    /// Initialize execution state
    fn initialize_execution_state(&self, _interpreter: &mut SymbolicEVMInterpreter, event: &AnalysisEvent) -> DeFiResult<EVMExecutionState> {
        let state = EVMExecutionState {
            current_opcode: OpCode::STOP,
            current_pc: 0,
            current_memory: SymbolicMemory::new(&self.ctx),
            current_stack: SymbolicStack::new(1024),
            current_return_value: None,
            current_return_error: None,
            current_evm_depth: 0,
            current_called_contract: BV::new_const(&self.ctx, "Contract", 256),
        };
        
        Ok(state)
    }

    /// DFS exploration
    fn dfs_explore(&mut self, interpreter: &mut SymbolicEVMInterpreter, state: &EVMExecutionState, 
                  current_path: &mut ExecutionPath, depth: u32, execution_path_list: &mut ExecutionPathList) -> DeFiResult<()> {
        // Check timeout
        if self.start_time.elapsed() > self.config.timeout {
            warn!("Path exploration timeout reached");
            return Ok(());
        }

        // Check max depth
        if depth > self.config.max_depth {
            self.stats.max_depth_reached = depth;
            return Ok(());
        }

        // Check max paths
        if self.stats.total_paths >= self.config.max_paths {
            warn!("Maximum paths limit reached: {}", self.config.max_paths);
            return Ok(());
        }

        // Loop detection
        if self.config.enable_loop_detection {
            let state_hash = self.hash_state(state);
            if self.visited_states.contains(&state_hash) {
                self.stats.loops_detected += 1;
                debug!("Loop detected at depth {}", depth);
                return Ok(());
            }
            self.visited_states.insert(state_hash);
        }

        // Create new path
        let mut new_path = current_path.clone();
        new_path.push(EVMExecutionState {
            current_opcode: state.current_opcode,
            current_pc: state.current_pc,
            current_memory: state.current_memory.clone(),
            current_stack: state.current_stack.clone(),
            current_return_value: state.current_return_value.clone(),
            current_return_error: state.current_return_error.clone(),
            current_evm_depth: state.current_evm_depth,
            current_called_contract: state.current_called_contract.clone(),
        });

        // Check if path should be pruned
        if self.config.enable_pruning && self.should_prune_path(&new_path) {
            self.stats.pruned_paths += 1;
            debug!("Path pruned at depth {}", depth);
            return Ok(());
        }

        // Check if this is an end state
        if self.is_end_state(state) {
            execution_path_list.add_path(new_path);
            self.stats.total_paths += 1;
            debug!("Path completed at depth {}", depth);
            return Ok(());
        }

        // Continue exploration with symbolic execution
        if self.config.enable_symbolic_execution {
            self.symbolic_execution_explore(interpreter, state, &mut new_path, depth, execution_path_list)?;
        } else {
            // Simple path continuation
            self.stats.total_paths += 1;
        }
        
        Ok(())
    }

    /// Symbolic execution exploration
    fn symbolic_execution_explore(&mut self, interpreter: &mut SymbolicEVMInterpreter, 
                                 state: &EVMExecutionState, current_path: &mut ExecutionPath, 
                                 depth: u32, execution_path_list: &mut ExecutionPathList) -> DeFiResult<()> {
        let symbolic_start = Instant::now();
        
        // Create contract for symbolic execution
        let mut contract = Contract::new(
            BV::new_const(&self.ctx, "caller", 256),
            state.current_called_contract.clone(),
            BV::new_const(&self.ctx, "value", 256),
            100_000_000
        );
        
        // Set contract code (this should be loaded from the actual contract)
        let contract_code = self.get_contract_code(&state.current_called_contract);
        contract.set_call_code(state.current_called_contract.clone(), &contract_code);
        contract.set_input(BV::new_const(&self.ctx, "input", 256));
        
        // Perform symbolic execution
        let mut path_copy = current_path.clone();
        interpreter.symbolic_run_dfs(&contract, &self.ctx, &mut path_copy, execution_path_list)?;
        
        self.stats.symbolic_execution_time += symbolic_start.elapsed();
        Ok(())
    }

    /// Hash state for loop detection
    fn hash_state(&self, state: &EVMExecutionState) -> String {
        format!("{}:{}:{}:{}", 
                state.current_pc, 
                state.current_stack.data.len(), 
                state.current_memory.data.len(), 
                0)
    }

    /// Check if path should be pruned
    fn should_prune_path(&self, path: &ExecutionPath) -> bool {
        if !self.config.enable_pruning {
            return false;
        }
        
        // Prune paths that are too long
        if path.len() > 1000 {
            return true;
        }
        
        // Prune paths with too many CALL operations
        let call_count = path.iter().filter(|step| {
            matches!(step.current_opcode, OpCode::CALL | OpCode::DELEGATECALL | OpCode::STATICCALL)
        }).count();
        
        if call_count > 10 {
            return true;
        }
        
        // Prune paths with repeated patterns
        if self.has_repeated_patterns(path) {
            return true;
        }
        
        false
    }

    /// Check for repeated patterns
    fn has_repeated_patterns(&self, path: &ExecutionPath) -> bool {
        if path.len() < 10 {
            return false;
        }
        
        // Check for repeated opcode sequences
        let opcodes: Vec<OpCode> = path.iter().map(|step| step.current_opcode).collect();
        
        for window_size in 3..=10 {
            for i in 0..=opcodes.len().saturating_sub(window_size * 2) {
                let pattern = &opcodes[i..i + window_size];
                let next_pattern = &opcodes[i + window_size..i + window_size * 2];
                
                if pattern == next_pattern {
                    return true;
                }
            }
        }
        
        false
    }

    /// Check if this is an end state
    fn is_end_state(&self, state: &EVMExecutionState) -> bool {
        // Check for end conditions
        state.current_pc > 1000 || state.current_evm_depth > 100
    }

    /// Get contract code
    fn get_contract_code(&self, _contract_address: &z3::ast::BV) -> Vec<u8> {
        // This should be implemented to fetch actual contract code
        // For now, return empty vector
        Vec::new()
    }

    /// Optimize paths
    pub fn optimize_paths(&mut self, paths: &mut Vec<ExecutionPath>) -> DeFiResult<()> {
        if !self.config.enable_path_optimization {
            return Ok(());
        }
        
        let optimization_start = Instant::now();
        
        // Remove duplicate paths
        let mut unique_paths = Vec::new();
        let mut seen_hashes = HashSet::new();
        
        for path in paths.iter() {
            let path_hash = self.hash_path(path);
            if !seen_hashes.contains(&path_hash) {
                seen_hashes.insert(path_hash);
                unique_paths.push(path.clone());
            }
        }
        
        *paths = unique_paths;
        
        // Sort paths by priority
        paths.sort_by(|a, b| {
            let a_priority = self.calculate_path_priority(a);
            let b_priority = self.calculate_path_priority(b);
            b_priority.cmp(&a_priority)
        });
        
        self.stats.path_optimization_time = optimization_start.elapsed();
        info!("Path optimization completed in {:?}", self.stats.path_optimization_time);
        
        Ok(())
    }

    /// Hash path for deduplication
    fn hash_path(&self, path: &ExecutionPath) -> String {
        let opcodes: Vec<String> = path.iter().map(|step| step.current_opcode.to_string()).collect();
        opcodes.join(":")
    }

    /// Calculate path priority
    fn calculate_path_priority(&self, path: &ExecutionPath) -> u32 {
        let mut priority = 0;
        
        // Higher priority for paths with CALL operations
        let call_count = path.iter().filter(|step| {
            matches!(step.current_opcode, OpCode::CALL | OpCode::DELEGATECALL | OpCode::STATICCALL)
        }).count();
        priority += call_count as u32 * 10;
        
        // Higher priority for paths with SSTORE operations
        let sstore_count = path.iter().filter(|step| step.current_opcode == OpCode::SSTORE).count();
        priority += sstore_count as u32 * 5;
        
        // Higher priority for shorter paths
        priority += (1000 - path.len() as u32).max(0);
        
        priority
    }

    /// Get statistics
    pub fn get_stats(&self) -> &PathExplorerStats {
        &self.stats
    }

    /// Get execution paths
    pub fn get_execution_paths(&self) -> &Vec<ExecutionPath<'ctx>> {
        &self.execution_paths
    }

    /// Reset explorer
    pub fn reset(&mut self) {
        self.execution_paths.clear();
        self.visited_states.clear();
        self.stats = PathExplorerStats::default();
        self.start_time = Instant::now();
    }

    /// Set configuration
    pub fn set_config(&mut self, config: PathExplorerConfig) {
        self.config = config;
    }

    /// Get configuration
    pub fn get_config(&self) -> &PathExplorerConfig {
        &self.config
    }
}

/// Path analysis result
#[derive(Debug, Clone)]
pub struct PathAnalysisResult {
    /// Analysis ID
    pub analysis_id: String,
    /// Contract address
    pub contract_address: Address,
    /// Total paths found
    pub total_paths: u32,
    /// Unique paths
    pub unique_paths: u32,
    /// Execution time
    pub execution_time: Duration,
    /// Path statistics
    pub path_statistics: PathStatistics,
    /// Risk assessment
    pub risk_assessment: PathRiskAssessment,
}

/// Path statistics
#[derive(Debug, Clone)]
pub struct PathStatistics {
    /// Average path length
    pub average_path_length: f64,
    /// Maximum path length
    pub max_path_length: usize,
    /// Minimum path length
    pub min_path_length: usize,
    /// Paths with calls
    pub paths_with_calls: u32,
    /// Paths with storage operations
    pub paths_with_storage: u32,
    /// Paths with external calls
    pub paths_with_external_calls: u32,
}

/// Path risk assessment
#[derive(Debug, Clone)]
pub struct PathRiskAssessment {
    /// Overall risk level
    pub overall_risk: RiskLevel,
    /// Risk factors
    pub risk_factors: Vec<PathRiskFactor>,
    /// Risk score
    pub risk_score: f64,
}

/// Path risk factor
#[derive(Debug, Clone)]
pub struct PathRiskFactor {
    /// Factor type
    pub factor_type: PathRiskFactorType,
    /// Description
    pub description: String,
    /// Severity
    pub severity: RiskLevel,
    /// Count
    pub count: u32,
}

/// Path risk factor type
#[derive(Debug, Clone)]
pub enum PathRiskFactorType {
    /// Deep call stack
    DeepCallStack,
    /// Many external calls
    ManyExternalCalls,
    /// Complex control flow
    ComplexControlFlow,
    /// Storage manipulation
    StorageManipulation,
    /// Gas limit issues
    GasLimitIssues,
}

/// Risk level
#[derive(Debug, Clone)]
pub enum RiskLevel {
    /// Low risk
    Low,
    /// Medium risk
    Medium,
    /// High risk
    High,
    /// Critical risk
    Critical,
}

/// Path analyzer
pub struct PathAnalyzer<'ctx> {
    /// Path explorer
    explorer: PathExplorer<'ctx>,
    /// Analysis results
    analysis_results: Vec<PathAnalysisResult>,
}

impl<'ctx> PathAnalyzer<'ctx> {
    /// Create new path analyzer
    pub fn new(explorer: PathExplorer<'ctx>) -> Self {
        Self {
            explorer,
            analysis_results: Vec::new(),
        }
    }

    /// Analyze execution paths
    pub fn analyze_paths(&mut self, paths: &[ExecutionPath], contract_address: Address) -> DeFiResult<PathAnalysisResult> {
        let analysis_id = format!("analysis_{}", chrono::Utc::now().timestamp());
        
        // Calculate path statistics
        let path_statistics = self.calculate_path_statistics(paths);
        
        // Assess risks
        let risk_assessment = self.assess_path_risks(paths);
        
        let result = PathAnalysisResult {
            analysis_id,
            contract_address,
            total_paths: paths.len() as u32,
            unique_paths: paths.len() as u32, // This should be calculated properly
            execution_time: self.explorer.get_stats().execution_time,
            path_statistics,
            risk_assessment,
        };
        
        self.analysis_results.push(result.clone());
        Ok(result)
    }

    /// Calculate path statistics
    fn calculate_path_statistics(&self, paths: &[ExecutionPath]) -> PathStatistics {
        if paths.is_empty() {
            return PathStatistics {
                average_path_length: 0.0,
                max_path_length: 0,
                min_path_length: 0,
                paths_with_calls: 0,
                paths_with_storage: 0,
                paths_with_external_calls: 0,
            };
        }
        
        let total_length: usize = paths.iter().map(|path| path.len()).sum();
        let average_path_length = total_length as f64 / paths.len() as f64;
        let max_path_length = paths.iter().map(|path| path.len()).max().unwrap_or(0);
        let min_path_length = paths.iter().map(|path| path.len()).min().unwrap_or(0);
        
        let paths_with_calls = paths.iter().filter(|path| {
            path.iter().any(|step| matches!(step.current_opcode, OpCode::CALL | OpCode::DELEGATECALL | OpCode::STATICCALL))
        }).count() as u32;
        
        let paths_with_storage = paths.iter().filter(|path| {
            path.iter().any(|step| step.current_opcode == OpCode::SSTORE || step.current_opcode == OpCode::SLOAD)
        }).count() as u32;
        
        let paths_with_external_calls = paths_with_calls; // Simplified
        
        PathStatistics {
            average_path_length,
            max_path_length,
            min_path_length,
            paths_with_calls,
            paths_with_storage,
            paths_with_external_calls,
        }
    }

    /// Assess path risks
    fn assess_path_risks(&self, paths: &[ExecutionPath]) -> PathRiskAssessment {
        let mut risk_factors = Vec::new();
        let mut risk_score: f32 = 0.0;
        
        // Check for deep call stacks
        let max_depth = paths.iter()
            .map(|path| path.iter().map(|step| step.current_evm_depth).max().unwrap_or(0))
            .max().unwrap_or(0);
        
        if max_depth > 10 {
            risk_factors.push(PathRiskFactor {
                factor_type: PathRiskFactorType::DeepCallStack,
                description: format!("Deep call stack detected: {}", max_depth),
                severity: if max_depth > 20 { RiskLevel::Critical } else { RiskLevel::High },
                count: 1,
            });
            risk_score += 0.3;
        }
        
        // Check for many external calls
        let total_calls: u32 = paths.iter()
            .map(|path| path.iter().filter(|step| matches!(step.current_opcode, OpCode::CALL | OpCode::DELEGATECALL | OpCode::STATICCALL)).count() as u32)
            .sum();
        
        if total_calls > 50 {
            risk_factors.push(PathRiskFactor {
                factor_type: PathRiskFactorType::ManyExternalCalls,
                description: format!("Many external calls detected: {}", total_calls),
                severity: if total_calls > 100 { RiskLevel::Critical } else { RiskLevel::High },
                count: total_calls,
            });
            risk_score += 0.4;
        }
        
        // Check for complex control flow
        let complex_paths = paths.iter().filter(|path| {
            let jump_count = path.iter().filter(|step| step.current_opcode == OpCode::JUMP || step.current_opcode == OpCode::JUMPI).count();
            jump_count > 5
        }).count();
        
        if complex_paths > 0 {
            risk_factors.push(PathRiskFactor {
                factor_type: PathRiskFactorType::ComplexControlFlow,
                description: format!("Complex control flow in {} paths", complex_paths),
                severity: if complex_paths > 5 { RiskLevel::High } else { RiskLevel::Medium },
                count: complex_paths as u32,
            });
            risk_score += 0.2;
        }
        
        let overall_risk = if risk_score > 0.7 {
            RiskLevel::Critical
        } else if risk_score > 0.5 {
            RiskLevel::High
        } else if risk_score > 0.3 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };
        
        PathRiskAssessment {
            overall_risk,
            risk_factors,
            risk_score: risk_score.min(1.0) as f64,
        }
    }

    /// Get analysis results
    pub fn get_analysis_results(&self) -> &Vec<PathAnalysisResult> {
        &self.analysis_results
    }

    /// Clear analysis results
    pub fn clear_analysis_results(&mut self) {
        self.analysis_results.clear();
    }
}
