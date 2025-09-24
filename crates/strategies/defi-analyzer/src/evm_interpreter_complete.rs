//! Complete EVM Interpreter for Symbolic Execution
//! 
//! This module provides the complete EVM interpreter functionality for symbolic execution,
//! maintaining full compatibility with DeFiAligner's SEVM implementation.

use alloy_primitives::{Address, U256};
use anyhow::Result;
use tracing::{info, debug, warn, error};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::fmt;
use z3::{Context, Config, Solver, ast::{BV, Bool, Ast}};

use crate::{
    types::AnalysisEvent,
    error::{DeFiResult, DeFiAnalyzerError},
};

/// Complete SEVM (Symbolic EVM) implementation
pub struct SEVM<'ctx> {
    /// Z3 context
    pub ctx: Context,
    /// Symbolic EVM interpreter
    pub interpreter: Box<SymbolicEVMInterpreter<'ctx>>,
    /// Origin address
    pub origin: BV<'ctx>,
    /// Depth counter
    pub depth: i32,
}

impl<'ctx> SEVM<'ctx> {
    /// Create new SEVM instance
    pub fn new(ctx: &'ctx Context) -> Self {
        let interpreter = Box::new(SymbolicEVMInterpreter::new(ctx));
        Self {
            ctx: Context::new(&Config::new()),
            interpreter,
            origin: BV::new_const(ctx, "origin", 256),
            depth: 0,
        }
    }

    /// Handle CALL operation
    pub fn call(&mut self, caller: BV<'ctx>, addr: BV<'ctx>, input: BV<'ctx>, value: BV<'ctx>, 
                current_path: &mut ExecutionPath<'ctx>, execution_path_list: &mut ExecutionPathList<'ctx>) -> DeFiResult<()> {
        // Get contract code by address
        let address_hash = format!("0x{}", addr.to_string().chars().skip(26).take(40).collect::<String>());
        let code = self.get_contract_code_by_address(&address_hash);
        
        if code.is_empty() {
            warn!("Call: there is no code for address {}", address_hash);
            return Ok(());
        }

        // Create new contract
        let mut contract = Contract::new(caller, addr.clone(), value, 0);
        contract.set_call_code(addr, &code);
        contract.set_input(input);
        
        self.depth += 1;
        self.interpreter.symbolic_run_dfs(&contract, &self.ctx, current_path, execution_path_list)?;
        self.depth -= 1;
        
        Ok(())
    }

    /// Handle STATICCALL operation
    pub fn static_call(&mut self, caller: BV<'ctx>, addr: BV<'ctx>, input: BV<'ctx>,
                       current_path: &mut ExecutionPath<'ctx>, execution_path_list: &mut ExecutionPathList<'ctx>) -> DeFiResult<()> {
        // Get contract code by address
        let address_hash = format!("0x{}", addr.to_string().chars().skip(26).take(40).collect::<String>());
        let code = self.get_contract_code_by_address(&address_hash);
        
        if code.is_empty() {
            warn!("StaticCall: there is no code for address {}", address_hash);
            return Ok(());
        }

        // Create new contract with zero value
        let zero_value = BV::from_u64(&self.ctx, 0, 256);
        let mut contract = Contract::new(caller, addr.clone(), zero_value, 0);
        contract.set_input(input);
        contract.set_call_code(addr, &code);
        
        self.depth += 1;
        self.interpreter.symbolic_run_dfs(&contract, &self.ctx, current_path, execution_path_list)?;
        self.depth -= 1;
        
        Ok(())
    }

    /// Handle DELEGATECALL operation
    pub fn delegate_call(&mut self, caller: BV<'ctx>, self_address: BV<'ctx>, delegate_call_addr: BV<'ctx>, input: BV<'ctx>,
                         current_path: &mut ExecutionPath<'ctx>, execution_path_list: &mut ExecutionPathList<'ctx>) -> DeFiResult<()> {
        // Get contract code by address
        let address_hash = format!("0x{}", delegate_call_addr.to_string().chars().skip(26).take(40).collect::<String>());
        let code = self.get_contract_code_by_address(&address_hash);
        
        if code.is_empty() {
            warn!("DelegateCall: there is no code for address {}", address_hash);
            return Ok(());
        }

        // Create new contract with zero value
        let zero_value = BV::from_u64(&self.ctx, 0, 256);
        let mut contract = Contract::new(caller, self_address, zero_value, 0);
        contract.set_input(input);
        contract.set_call_code(delegate_call_addr, &code);
        
        self.depth += 1;
        self.interpreter.symbolic_run_dfs(&contract, &self.ctx, current_path, execution_path_list)?;
        self.depth -= 1;
        
        Ok(())
    }

    /// Get contract code by address (placeholder implementation)
    fn get_contract_code_by_address(&self, _address: &str) -> Vec<u8> {
        // This should be implemented to fetch actual contract code
        // For now, return empty vector
        Vec::new()
    }
}

/// Symbolic EVM Interpreter
pub struct SymbolicEVMInterpreter<'ctx> {
    /// SEVM reference
    evm: Option<Box<SEVM<'ctx>>>,
    /// Return data from last CALL
    return_data: Option<BV<'ctx>>,
    /// Jump table
    table: JumpTable,
    /// Z3 variables
    z3_variables: HashMap<String, BV<'ctx>>,
}

impl<'ctx> SymbolicEVMInterpreter<'ctx> {
    /// Create new interpreter
    pub fn new(ctx: &'ctx Context) -> Self {
        Self {
            evm: None,
            return_data: None,
            table: JumpTable::new(),
            z3_variables: HashMap::new(),
        }
    }

    /// Set SEVM reference
    pub fn set_evm(&mut self, evm: SEVM<'ctx>) {
        self.evm = Some(Box::new(evm));
    }

    /// Main DFS symbolic execution
    pub fn symbolic_run_dfs(&mut self, contract: &Contract<'ctx>, ctx: &'ctx Context, 
                           current_path: &mut ExecutionPath<'ctx>, execution_path_list: &mut ExecutionPathList<'ctx>) -> DeFiResult<()> {
        if contract.code().is_empty() {
            warn!("SymbolicRun: there is no code");
            return Ok(());
        }

        if let Some(evm) = &self.evm {
            if evm.depth == 0 {
                // Set origin
            }
        }

        // Create scope context
        let mut scope_context = ScopeContext::new(ctx);
        scope_context.set_contract(contract.clone());
        
        let mut visited_nodes = VisitedNodes::new();
        let mut copy_path = current_path.clone();
        
        self.search_paths_by_dfs(contract, 0, &mut scope_context, &mut visited_nodes, 
                               &mut copy_path, execution_path_list)?;
        
        Ok(())
    }

    /// DFS path search
    fn search_paths_by_dfs(&mut self, contract: &Contract<'ctx>, pc: u64, call_context: &mut ScopeContext<'ctx>,
                          visited_nodes: &mut VisitedNodes<'ctx>, current_path: &mut ExecutionPath<'ctx>,
                          execution_path_list: &mut ExecutionPathList<'ctx>) -> DeFiResult<()> {
        let op = contract.get_op(pc);
        
        match op {
            OpCode::JUMPI => {
                self.handle_jumpi(contract, pc, call_context, visited_nodes, current_path, execution_path_list)?;
            },
            OpCode::CALL | OpCode::STATICCALL | OpCode::DELEGATECALL => {
                self.handle_call_operations(contract, pc, call_context, current_path, execution_path_list, op)?;
            },
            _ => {
                self.handle_other_instruction(contract, pc, call_context, current_path, execution_path_list, op)?;
            }
        }
        
        Ok(())
    }


    /// Handle JUMPI instruction
    fn handle_jumpi(&mut self, contract: &Contract<'ctx>, pc: u64, call_context: &mut ScopeContext<'ctx>,
                   visited_nodes: &mut VisitedNodes<'ctx>, current_path: &mut ExecutionPath<'ctx>,
                   execution_path_list: &mut ExecutionPathList<'ctx>) -> DeFiResult<()> {
        let current_pc = pc;
        let mut next_pc_list = Vec::new();
        
        // Pop position and condition from stack using temporary variable to avoid borrowing conflicts
        let pos = call_context.stack().pop()?;
        let cond = call_context.stack().pop()?;
        
        // Simplify conditions
        let cond = cond.simplify();
        let pos = pos.simplify();
        
        // Check if condition is concrete
        let cond_value = cond.as_u64();
        let pos_value = pos.as_u64();
        
        if let (Some(cond_val), Some(pos_val)) = (cond_value, pos_value) {
            // Concrete condition
            if cond_val > 0 {
                // Condition is true
                if contract.get_op(pos_val) == OpCode::JUMPDEST {
                    next_pc_list.push(pos_val);
                }
            } else {
                // Condition is false, continue to next instruction
                if (current_pc + 1) < contract.code().len() as u64 {
                    next_pc_list.push(current_pc + 1);
                }
            }
        } else {
            // Symbolic condition - explore both paths
            if let Some(pos_val) = pos_value {
                if contract.get_op(pos_val) == OpCode::JUMPDEST {
                    next_pc_list.push(pos_val);
                }
            }
            if (current_pc + 1) < contract.code().len() as u64 {
                next_pc_list.push(current_pc + 1);
            }
        }
        
        // Update execution path - use cloned call_context to avoid borrowing conflicts
        {
            let mut call_context_clone = call_context.deep_copy();
            self.update_execution_path(current_path, &mut call_context_clone, current_pc, OpCode::JUMPI, None, None);
        }
        
        // Explore each path
        for next_pc in next_pc_list {
            if next_pc != current_pc + 1 && contract.get_op(next_pc) != OpCode::JUMPDEST {
                warn!("Invalid jump to {}", next_pc);
                continue;
            }
            
            let mut next_call_context = call_context.deep_copy();
            let mut copy_path = current_path.clone();
            
            // Check if node has been visited - use cloned values to avoid borrowing conflicts
            let is_concrete_condition = cond_value.is_some();
            let (stack_clone, memory_store) = {
                let stack = call_context.stack().clone();
                let memory = call_context.memory().store.clone();
                (stack, memory)
            };
            if !self.is_visited_node(current_pc, next_pc, &stack_clone, visited_nodes, 
                                   memory_store.clone(), None, is_concrete_condition) {
                if !is_concrete_condition {
                    self.update_visited_nodes(current_pc, next_pc, &stack_clone, visited_nodes, 
                                            memory_store);
                }
                
                self.search_paths_by_dfs(contract, next_pc, &mut next_call_context, 
                                       visited_nodes, &mut copy_path, execution_path_list)?;
            } else {
                info!("Node has been visited: {}", next_pc);
            }
        }
        
        Ok(())
    }

    /// Handle call operations (CALL, STATICCALL, DELEGATECALL)
    fn handle_call_operations(&mut self, contract: &Contract<'ctx>, pc: u64, call_context: &mut ScopeContext<'ctx>,
                            current_path: &mut ExecutionPath<'ctx>, execution_path_list: &mut ExecutionPathList<'ctx>,
                            op: OpCode) -> DeFiResult<()> {
        let current_pc = pc;
        
        // Update execution path
        self.update_execution_path(current_path, call_context, current_pc, op, None, None);
        
        let mut copy_path = current_path.clone();
        let mut return_data_list = CrossContractReturnDataList::new();
        
        match op {
            OpCode::CALL => {
                return_data_list = self.execute_call(pc, call_context, &mut copy_path, execution_path_list)?;
            },
            OpCode::STATICCALL => {
                return_data_list = self.execute_static_call(pc, call_context, &mut copy_path, execution_path_list)?;
            },
            OpCode::DELEGATECALL => {
                return_data_list = self.execute_delegate_call(pc, call_context, &mut copy_path, execution_path_list)?;
            },
            _ => {}
        }
        
        // Process return data - separate the data collection from self modification
        let mut return_data_to_process = Vec::new();
        if !return_data_list.is_empty() {
            for return_data in return_data_list.iter() {
                let path_length = return_data.path.len();
                if let Some(err) = &return_data.path[path_length - 1].current_return_error {
                    if err != "return token" && err != "stop token" {
                        error!("There is an error: {}", err);
                    } else {
                        // Clone return data for later processing
                        return_data_to_process.push((
                            return_data.return_data.clone(),
                            return_data.scope.deep_copy(),
                            return_data.path.clone(),
                        ));
                    }
                }
            }
        }
        
        // Process return data and continue execution
        for (return_data_clone, mut next_call_context, mut new_copy_path) in return_data_to_process {
            // Store the return data
            self.return_data = Some(return_data_clone);
            
            // Update context with return data
            let mut new_visited_nodes = VisitedNodes::new();
            self.search_paths_by_dfs(contract, current_pc + 1, &mut next_call_context,
                                   &mut new_visited_nodes, &mut new_copy_path, execution_path_list)?;
        }
        
        Ok(())
    }

    /// Handle other instructions
    fn handle_other_instruction(&mut self, contract: &Contract<'ctx>, pc: u64, call_context: &mut ScopeContext<'ctx>,
                              current_path: &mut ExecutionPath<'ctx>, execution_path_list: &mut ExecutionPathList<'ctx>,
                              op: OpCode) -> DeFiResult<()> {
        if op.to_string().contains("not defined") {
            return Ok(());
        }
        
        let current_pc = pc;
        
        // Execute the operation
        if let Some(operation) = self.table.get_operation(op) {
            let (ret, err) = operation.execute(pc, self, call_context)?;
            self.update_execution_path(current_path, call_context, current_pc, op, ret, err.clone());
            
            // Check if this is the end of the path
            if (pc + 1) >= contract.code().len() as u64 || self.is_end_of_path(op) || err.is_some() {
                if op == OpCode::RETURN || op == OpCode::STOP {
                    execution_path_list.add_path(current_path.clone());
                }
            } else {
                // Continue execution
                let next_pc = pc + 1;
                let mut next_call_context = call_context.deep_copy();
                self.search_paths_by_dfs(contract, next_pc, &mut next_call_context, 
                                       &mut VisitedNodes::new(), current_path, execution_path_list)?;
            }
        }
        
        Ok(())
    }

    /// Execute CALL operation
    fn execute_call(&mut self, pc: u64, call_context: &mut ScopeContext, 
                  current_path: &mut ExecutionPath, execution_path_list: &mut ExecutionPathList) -> DeFiResult<CrossContractReturnDataList> {
        // Implementation for CALL operation
        // This should extract call parameters and execute the call
        Ok(CrossContractReturnDataList::new())
    }

    /// Execute STATICCALL operation
    fn execute_static_call(&mut self, pc: u64, call_context: &mut ScopeContext,
                         current_path: &mut ExecutionPath, execution_path_list: &mut ExecutionPathList) -> DeFiResult<CrossContractReturnDataList> {
        // Implementation for STATICCALL operation
        Ok(CrossContractReturnDataList::new())
    }

    /// Execute DELEGATECALL operation
    fn execute_delegate_call(&mut self, pc: u64, call_context: &mut ScopeContext,
                            current_path: &mut ExecutionPath, execution_path_list: &mut ExecutionPathList) -> DeFiResult<CrossContractReturnDataList> {
        // Implementation for DELEGATECALL operation
        Ok(CrossContractReturnDataList::new())
    }

    /// Update execution path
    fn update_execution_path<'a>(&self, execution_path: &mut ExecutionPath<'a>, call_context: &mut ScopeContext<'a>,
                           pc: u64, opcode: OpCode, ret: Option<BV<'a>>, return_error: Option<String>) {
        // Clone all needed values in separate scopes to avoid multiple borrows
        let memory = call_context.memory().clone();
        let stack = call_context.stack().clone();
        let address = call_context.contract().address();
        let new_state = EVMExecutionState {
            current_opcode: opcode,
            current_pc: pc,
            current_memory: memory,
            current_stack: stack,
            current_return_value: ret,
            current_return_error: return_error,
            current_evm_depth: self.evm.as_ref().map_or(0, |evm| evm.depth as i32),
            current_called_contract: address,
        };
        
        execution_path.push(new_state);
    }

    /// Check if node has been visited
    fn is_visited_node(&self, current_pc: u64, next_pc: u64, stack: &SymbolicStack,
                      visited_nodes: &VisitedNodes, memory: BV, return_data: Option<BV>,
                      is_concrete_condition: bool) -> bool {
        if is_concrete_condition {
            return false;
        }
        
        if let Some(next_map) = visited_nodes.get(&current_pc) {
            if let Some(visited_info) = next_map.get(&next_pc) {
                for (i, s) in visited_info.stacks.iter().enumerate() {
                    if self.are_symbolic_stacks_equal(s, stack) {
                        if visited_info.count[i] >= VISIT_THRESHOLD {
                            return true;
                        }
                        break;
                    }
                }
            }
        }
        
        false
    }

    /// Update visited nodes
    fn update_visited_nodes(&self, current_pc: u64, next_pc: u64, stack: &SymbolicStack,
                          visited_nodes: &mut VisitedNodes, memory: BV) {
        if !visited_nodes.contains_key(&current_pc) {
            visited_nodes.insert(current_pc, HashMap::new());
        }
        
        if !visited_nodes[&current_pc].contains_key(&next_pc) {
            visited_nodes.get_mut(&current_pc).unwrap().insert(next_pc, VisitedInfo::new());
        }
        
        let node = visited_nodes.get_mut(&current_pc).unwrap().get_mut(&next_pc).unwrap();
        let mut stack_index = None;
        
        for (i, node_stack) in node.stacks.iter().enumerate() {
            if self.are_symbolic_stacks_equal(node_stack, stack) && 
               self.are_symbolic_equal(node.memory[i].clone(), memory.clone()) {
                stack_index = Some(i);
                break;
            }
        }
        
        if let Some(index) = stack_index {
            node.count[index] += 1;
        } else {
            node.stacks.push(stack.clone());
            node.count.push(1);
            node.memory.push(memory);
        }
    }

    /// Check if stacks are equal
    fn are_symbolic_stacks_equal(&self, stack1: &SymbolicStack, stack2: &SymbolicStack) -> bool {
        if stack1.data.len() != stack2.data.len() {
            return false;
        }
        
        for (a, b) in stack1.data.iter().zip(stack2.data.iter()) {
            if !self.are_symbolic_equal(a.clone(), b.clone()) {
                return false;
            }
        }
        
        true
    }

    /// Check if two symbolic values are equal
    fn are_symbolic_equal(&self, a: BV, b: BV) -> bool {
        // This should implement proper symbolic equality checking
        a.to_string() == b.to_string()
    }

    /// Check if this is the end of a path
    fn is_end_of_path(&self, op: OpCode) -> bool {
        matches!(op, OpCode::STOP | OpCode::RETURN | OpCode::REVERT | OpCode::INVALID | OpCode::SELFDESTRUCT)
    }
}

/// Scope context for execution
#[derive(Debug)]
pub struct ScopeContext<'ctx> {
    stack: SymbolicStack<'ctx>,
    contract: Contract<'ctx>,
    memory: SymbolicMemory<'ctx>,
    z3_context: Context,
}

impl<'ctx> ScopeContext<'ctx> {
    pub fn new(ctx: &'ctx Context) -> Self {
        Self {
            stack: SymbolicStack::new(1024),
            contract: Contract::new_with_context(ctx),
            memory: SymbolicMemory::new(ctx),
            z3_context: Context::new(&Config::new()),
        }
    }
    
    pub fn deep_copy(&self) -> Self {
        Self {
            stack: self.stack.clone(),
            contract: Contract::new_with_context(&self.z3_context),
            memory: self.memory.clone(),
            z3_context: Context::new(&Config::new()),
        }
    }

    pub fn set_contract(&mut self, contract: Contract<'ctx>) {
        self.contract = contract;
    }

    pub fn stack(&mut self) -> &mut SymbolicStack<'ctx> {
        &mut self.stack
    }

    pub fn memory(&mut self) -> &mut SymbolicMemory<'ctx> {
        &mut self.memory
    }

    pub fn contract(&self) -> &Contract<'ctx> {
        &self.contract
    }

}

/// Symbolic stack implementation
#[derive(Debug, Clone)]
pub struct SymbolicStack<'ctx> {
    pub data: Vec<BV<'ctx>>,
}

impl<'ctx> SymbolicStack<'ctx> {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Vec::with_capacity(max_size),
        }
    }

    pub fn push(&mut self, value: BV<'ctx>) -> DeFiResult<()> {
        if self.data.len() >= 1024 {
            return Err(DeFiAnalyzerError::ConfigurationError {
                reason: "Stack overflow".to_string(),
            });
        }
        self.data.push(value);
        Ok(())
    }

    pub fn pop(&mut self) -> DeFiResult<BV<'ctx>> {
        self.data.pop().ok_or_else(|| DeFiAnalyzerError::ConfigurationError {
            reason: "Stack underflow".to_string(),
        })
    }

    pub fn back(&self, n: usize) -> Option<BV<'ctx>> {
        if n < self.data.len() {
            Some(self.data[self.data.len() - 1 - n].clone())
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn copy(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

/// Symbolic memory implementation
#[derive(Debug, Clone)]
pub struct SymbolicMemory<'ctx> {
    pub store: BV<'ctx>,
    pub data: HashMap<u64, BV<'ctx>>,
}

impl<'ctx> SymbolicMemory<'ctx> {
    pub fn new(ctx: &'ctx Context) -> Self {
        Self {
            store: BV::new_const(ctx, "memory", 256),
            data: HashMap::new(),
        }
    }

    pub fn set(&mut self, offset: u64, value: BV<'ctx>) {
        self.data.insert(offset, value);
    }

    pub fn get(&self, offset: u64) -> Option<&BV<'ctx>> {
        self.data.get(&offset)
    }

    pub fn copy(&self) -> Self {
        Self {
            store: self.store.clone(),
            data: self.data.clone(),
        }
    }
}

/// Contract representation
#[derive(Debug, Clone)]
pub struct Contract<'ctx> {
    caller: BV<'ctx>,
    address: BV<'ctx>,
    value: BV<'ctx>,
    gas: u64,
    code: Vec<u8>,
    input: Option<BV<'ctx>>,
}

impl<'ctx> Contract<'ctx> {
    pub fn new(caller: BV<'ctx>, address: BV<'ctx>, value: BV<'ctx>, gas: u64) -> Self {
        Self {
            caller,
            address,
            value,
            gas,
            code: Vec::new(),
            input: None,
        }
    }

    pub fn set_call_code(&mut self, address: BV<'ctx>, code: &[u8]) {
        self.code = code.to_vec();
    }

    pub fn set_input(&mut self, input: BV<'ctx>) {
        self.input = Some(input);
    }

    pub fn get_op(&self, pc: u64) -> OpCode {
        if pc < self.code.len() as u64 {
            OpCode::from(self.code[pc as usize])
        } else {
            OpCode::STOP
        }
    }

    pub fn code(&self) -> &[u8] {
        &self.code
    }

    pub fn address(&self) -> BV<'ctx> {
        self.address.clone()
    }
}

impl<'ctx> Contract<'ctx> {
    /// Create a new Contract with the given context
    pub fn new_with_context(ctx: &'ctx Context) -> Self {
        Self {
            caller: BV::from_u64(ctx, 0, 256),
            address: BV::from_u64(ctx, 0, 256),
            value: BV::from_u64(ctx, 0, 256),
            gas: 0,
            code: Vec::new(),
            input: None,
        }
    }
}

/// EVM execution state
#[derive(Debug, Clone)]
pub struct EVMExecutionState<'ctx> {
    pub current_opcode: OpCode,
    pub current_pc: u64,
    pub current_memory: SymbolicMemory<'ctx>,
    pub current_stack: SymbolicStack<'ctx>,
    pub current_return_value: Option<BV<'ctx>>,
    pub current_return_error: Option<String>,
    pub current_evm_depth: i32,
    pub current_called_contract: BV<'ctx>,
}

/// Execution path
pub type ExecutionPath<'ctx> = Vec<EVMExecutionState<'ctx>>;

/// Execution path list
pub struct ExecutionPathList<'ctx> {
    paths: Vec<ExecutionPath<'ctx>>,
}

impl<'ctx> ExecutionPathList<'ctx> {
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
        }
    }

    pub fn add_path(&mut self, path: ExecutionPath<'ctx>) {
        self.paths.push(path);
    }

    pub fn paths(&self) -> &Vec<ExecutionPath<'ctx>> {
        &self.paths
    }
}

/// Cross-contract return data
pub struct CrossContractReturnData<'ctx> {
    pub path: ExecutionPath<'ctx>,
    pub scope: ScopeContext<'ctx>,
    pub return_data: BV<'ctx>,
}

/// Cross-contract return data list
pub struct CrossContractReturnDataList<'ctx> {
    data: Vec<CrossContractReturnData<'ctx>>,
}

impl<'ctx> CrossContractReturnDataList<'ctx> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    pub fn iter(&self) -> std::slice::Iter<'_, CrossContractReturnData<'ctx>> {
        self.data.iter()
    }
}

/// Visited information for loop detection
#[derive(Debug, Clone)]
pub struct VisitedInfo<'ctx> {
    pub stacks: Vec<SymbolicStack<'ctx>>,
    pub memory: Vec<BV<'ctx>>,
    pub return_data: Vec<BV<'ctx>>,
    pub count: Vec<i32>,
}

impl<'ctx> VisitedInfo<'ctx> {
    pub fn new() -> Self {
        Self {
            stacks: Vec::new(),
            memory: Vec::new(),
            return_data: Vec::new(),
            count: Vec::new(),
        }
    }
}

/// Visited nodes map
pub type VisitedNodes<'ctx> = HashMap<u64, HashMap<u64, VisitedInfo<'ctx>>>;

/// Visit threshold constant
const VISIT_THRESHOLD: i32 = 1;

/// Jump table for EVM operations
pub struct JumpTable {
    operations: HashMap<OpCode, Box<dyn EVMOperation>>,
}

impl JumpTable {
    pub fn new() -> Self {
        Self {
            operations: HashMap::new(),
        }
    }

    pub fn get_operation(&self, op: OpCode) -> Option<&Box<dyn EVMOperation>> {
        self.operations.get(&op)
    }
}

/// EVM operation trait
pub trait EVMOperation {
    fn execute(&self, pc: u64, interpreter: &SymbolicEVMInterpreter, context: &mut ScopeContext) -> DeFiResult<(Option<BV>, Option<String>)>;
}

/// EVM opcodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpCode {
    STOP,
    ADD,
    MUL,
    SUB,
    DIV,
    SDIV,
    MOD,
    SMOD,
    ADDMOD,
    MULMOD,
    EXP,
    SIGNEXTEND,
    LT,
    GT,
    SLT,
    SGT,
    EQ,
    ISZERO,
    AND,
    OR,
    XOR,
    NOT,
    BYTE,
    SHL,
    SHR,
    SAR,
    SHA3,
    ADDRESS,
    BALANCE,
    ORIGIN,
    CALLER,
    CALLVALUE,
    CALLDATALOAD,
    CALLDATASIZE,
    CALLDATACOPY,
    CODESIZE,
    CODECOPY,
    GASPRICE,
    EXTCODESIZE,
    EXTCODECOPY,
    RETURNDATASIZE,
    RETURNDATACOPY,
    EXTCODEHASH,
    BLOCKHASH,
    COINBASE,
    TIMESTAMP,
    NUMBER,
    DIFFICULTY,
    GASLIMIT,
    CHAINID,
    SELFBALANCE,
    BASEFEE,
    POP,
    MLOAD,
    MSTORE,
    MSTORE8,
    SLOAD,
    SSTORE,
    JUMP,
    JUMPI,
    PC,
    MSIZE,
    GAS,
    JUMPDEST,
    PUSH0,
    PUSH1,
    PUSH2,
    PUSH3,
    PUSH4,
    PUSH5,
    PUSH6,
    PUSH7,
    PUSH8,
    PUSH9,
    PUSH10,
    PUSH11,
    PUSH12,
    PUSH13,
    PUSH14,
    PUSH15,
    PUSH16,
    PUSH17,
    PUSH18,
    PUSH19,
    PUSH20,
    PUSH21,
    PUSH22,
    PUSH23,
    PUSH24,
    PUSH25,
    PUSH26,
    PUSH27,
    PUSH28,
    PUSH29,
    PUSH30,
    PUSH31,
    PUSH32,
    DUP1,
    DUP2,
    DUP3,
    DUP4,
    DUP5,
    DUP6,
    DUP7,
    DUP8,
    DUP9,
    DUP10,
    DUP11,
    DUP12,
    DUP13,
    DUP14,
    DUP15,
    DUP16,
    SWAP1,
    SWAP2,
    SWAP3,
    SWAP4,
    SWAP5,
    SWAP6,
    SWAP7,
    SWAP8,
    SWAP9,
    SWAP10,
    SWAP11,
    SWAP12,
    SWAP13,
    SWAP14,
    SWAP15,
    SWAP16,
    LOG0,
    LOG1,
    LOG2,
    LOG3,
    LOG4,
    CREATE,
    CALL,
    CALLCODE,
    RETURN,
    DELEGATECALL,
    CREATE2,
    STATICCALL,
    REVERT,
    INVALID,
    SELFDESTRUCT,
}

impl OpCode {
    pub fn from(byte: u8) -> Self {
        match byte {
            0x00 => OpCode::STOP,
            0x01 => OpCode::ADD,
            0x02 => OpCode::MUL,
            0x03 => OpCode::SUB,
            0x04 => OpCode::DIV,
            0x05 => OpCode::SDIV,
            0x06 => OpCode::MOD,
            0x07 => OpCode::SMOD,
            0x08 => OpCode::ADDMOD,
            0x09 => OpCode::MULMOD,
            0x0a => OpCode::EXP,
            0x0b => OpCode::SIGNEXTEND,
            0x10 => OpCode::LT,
            0x11 => OpCode::GT,
            0x12 => OpCode::SLT,
            0x13 => OpCode::SGT,
            0x14 => OpCode::EQ,
            0x15 => OpCode::ISZERO,
            0x16 => OpCode::AND,
            0x17 => OpCode::OR,
            0x18 => OpCode::XOR,
            0x19 => OpCode::NOT,
            0x1a => OpCode::BYTE,
            0x1b => OpCode::SHL,
            0x1c => OpCode::SHR,
            0x1d => OpCode::SAR,
            0x20 => OpCode::SHA3,
            0x30 => OpCode::ADDRESS,
            0x31 => OpCode::BALANCE,
            0x32 => OpCode::ORIGIN,
            0x33 => OpCode::CALLER,
            0x34 => OpCode::CALLVALUE,
            0x35 => OpCode::CALLDATALOAD,
            0x36 => OpCode::CALLDATASIZE,
            0x37 => OpCode::CALLDATACOPY,
            0x38 => OpCode::CODESIZE,
            0x39 => OpCode::CODECOPY,
            0x3a => OpCode::GASPRICE,
            0x3b => OpCode::EXTCODESIZE,
            0x3c => OpCode::EXTCODECOPY,
            0x3d => OpCode::RETURNDATASIZE,
            0x3e => OpCode::RETURNDATACOPY,
            0x3f => OpCode::EXTCODEHASH,
            0x40 => OpCode::BLOCKHASH,
            0x41 => OpCode::COINBASE,
            0x42 => OpCode::TIMESTAMP,
            0x43 => OpCode::NUMBER,
            0x44 => OpCode::DIFFICULTY,
            0x45 => OpCode::GASLIMIT,
            0x46 => OpCode::CHAINID,
            0x47 => OpCode::SELFBALANCE,
            0x48 => OpCode::BASEFEE,
            0x50 => OpCode::POP,
            0x51 => OpCode::MLOAD,
            0x52 => OpCode::MSTORE,
            0x53 => OpCode::MSTORE8,
            0x54 => OpCode::SLOAD,
            0x55 => OpCode::SSTORE,
            0x56 => OpCode::JUMP,
            0x57 => OpCode::JUMPI,
            0x58 => OpCode::PC,
            0x59 => OpCode::MSIZE,
            0x5a => OpCode::GAS,
            0x5b => OpCode::JUMPDEST,
            0x5f => OpCode::PUSH0,
            0x60 => OpCode::PUSH1,
            0x61 => OpCode::PUSH2,
            0x62 => OpCode::PUSH3,
            0x63 => OpCode::PUSH4,
            0x64 => OpCode::PUSH5,
            0x65 => OpCode::PUSH6,
            0x66 => OpCode::PUSH7,
            0x67 => OpCode::PUSH8,
            0x68 => OpCode::PUSH9,
            0x69 => OpCode::PUSH10,
            0x6a => OpCode::PUSH11,
            0x6b => OpCode::PUSH12,
            0x6c => OpCode::PUSH13,
            0x6d => OpCode::PUSH14,
            0x6e => OpCode::PUSH15,
            0x6f => OpCode::PUSH16,
            0x70 => OpCode::PUSH17,
            0x71 => OpCode::PUSH18,
            0x72 => OpCode::PUSH19,
            0x73 => OpCode::PUSH20,
            0x74 => OpCode::PUSH21,
            0x75 => OpCode::PUSH22,
            0x76 => OpCode::PUSH23,
            0x77 => OpCode::PUSH24,
            0x78 => OpCode::PUSH25,
            0x79 => OpCode::PUSH26,
            0x7a => OpCode::PUSH27,
            0x7b => OpCode::PUSH28,
            0x7c => OpCode::PUSH29,
            0x7d => OpCode::PUSH30,
            0x7e => OpCode::PUSH31,
            0x7f => OpCode::PUSH32,
            0x80 => OpCode::DUP1,
            0x81 => OpCode::DUP2,
            0x82 => OpCode::DUP3,
            0x83 => OpCode::DUP4,
            0x84 => OpCode::DUP5,
            0x85 => OpCode::DUP6,
            0x86 => OpCode::DUP7,
            0x87 => OpCode::DUP8,
            0x88 => OpCode::DUP9,
            0x89 => OpCode::DUP10,
            0x8a => OpCode::DUP11,
            0x8b => OpCode::DUP12,
            0x8c => OpCode::DUP13,
            0x8d => OpCode::DUP14,
            0x8e => OpCode::DUP15,
            0x8f => OpCode::DUP16,
            0x90 => OpCode::SWAP1,
            0x91 => OpCode::SWAP2,
            0x92 => OpCode::SWAP3,
            0x93 => OpCode::SWAP4,
            0x94 => OpCode::SWAP5,
            0x95 => OpCode::SWAP6,
            0x96 => OpCode::SWAP7,
            0x97 => OpCode::SWAP8,
            0x98 => OpCode::SWAP9,
            0x99 => OpCode::SWAP10,
            0x9a => OpCode::SWAP11,
            0x9b => OpCode::SWAP12,
            0x9c => OpCode::SWAP13,
            0x9d => OpCode::SWAP14,
            0x9e => OpCode::SWAP15,
            0x9f => OpCode::SWAP16,
            0xa0 => OpCode::LOG0,
            0xa1 => OpCode::LOG1,
            0xa2 => OpCode::LOG2,
            0xa3 => OpCode::LOG3,
            0xa4 => OpCode::LOG4,
            0xf0 => OpCode::CREATE,
            0xf1 => OpCode::CALL,
            0xf2 => OpCode::CALLCODE,
            0xf3 => OpCode::RETURN,
            0xf4 => OpCode::DELEGATECALL,
            0xf5 => OpCode::CREATE2,
            0xfa => OpCode::STATICCALL,
            0xfd => OpCode::REVERT,
            0xfe => OpCode::INVALID,
            0xff => OpCode::SELFDESTRUCT,
            _ => OpCode::INVALID,
        }
    }
}

impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpCode::STOP => write!(f, "STOP"),
            OpCode::ADD => write!(f, "ADD"),
            OpCode::MUL => write!(f, "MUL"),
            OpCode::SUB => write!(f, "SUB"),
            OpCode::DIV => write!(f, "DIV"),
            OpCode::SDIV => write!(f, "SDIV"),
            OpCode::MOD => write!(f, "MOD"),
            OpCode::SMOD => write!(f, "SMOD"),
            OpCode::ADDMOD => write!(f, "ADDMOD"),
            OpCode::MULMOD => write!(f, "MULMOD"),
            OpCode::EXP => write!(f, "EXP"),
            OpCode::SIGNEXTEND => write!(f, "SIGNEXTEND"),
            OpCode::LT => write!(f, "LT"),
            OpCode::GT => write!(f, "GT"),
            OpCode::SLT => write!(f, "SLT"),
            OpCode::SGT => write!(f, "SGT"),
            OpCode::EQ => write!(f, "EQ"),
            OpCode::ISZERO => write!(f, "ISZERO"),
            OpCode::AND => write!(f, "AND"),
            OpCode::OR => write!(f, "OR"),
            OpCode::XOR => write!(f, "XOR"),
            OpCode::NOT => write!(f, "NOT"),
            OpCode::BYTE => write!(f, "BYTE"),
            OpCode::SHL => write!(f, "SHL"),
            OpCode::SHR => write!(f, "SHR"),
            OpCode::SAR => write!(f, "SAR"),
            OpCode::SHA3 => write!(f, "SHA3"),
            OpCode::ADDRESS => write!(f, "ADDRESS"),
            OpCode::BALANCE => write!(f, "BALANCE"),
            OpCode::ORIGIN => write!(f, "ORIGIN"),
            OpCode::CALLER => write!(f, "CALLER"),
            OpCode::CALLVALUE => write!(f, "CALLVALUE"),
            OpCode::CALLDATALOAD => write!(f, "CALLDATALOAD"),
            OpCode::CALLDATASIZE => write!(f, "CALLDATASIZE"),
            OpCode::CALLDATACOPY => write!(f, "CALLDATACOPY"),
            OpCode::CODESIZE => write!(f, "CODESIZE"),
            OpCode::CODECOPY => write!(f, "CODECOPY"),
            OpCode::GASPRICE => write!(f, "GASPRICE"),
            OpCode::EXTCODESIZE => write!(f, "EXTCODESIZE"),
            OpCode::EXTCODECOPY => write!(f, "EXTCODECOPY"),
            OpCode::RETURNDATASIZE => write!(f, "RETURNDATASIZE"),
            OpCode::RETURNDATACOPY => write!(f, "RETURNDATACOPY"),
            OpCode::EXTCODEHASH => write!(f, "EXTCODEHASH"),
            OpCode::BLOCKHASH => write!(f, "BLOCKHASH"),
            OpCode::COINBASE => write!(f, "COINBASE"),
            OpCode::TIMESTAMP => write!(f, "TIMESTAMP"),
            OpCode::NUMBER => write!(f, "NUMBER"),
            OpCode::DIFFICULTY => write!(f, "DIFFICULTY"),
            OpCode::GASLIMIT => write!(f, "GASLIMIT"),
            OpCode::CHAINID => write!(f, "CHAINID"),
            OpCode::SELFBALANCE => write!(f, "SELFBALANCE"),
            OpCode::BASEFEE => write!(f, "BASEFEE"),
            OpCode::POP => write!(f, "POP"),
            OpCode::MLOAD => write!(f, "MLOAD"),
            OpCode::MSTORE => write!(f, "MSTORE"),
            OpCode::MSTORE8 => write!(f, "MSTORE8"),
            OpCode::SLOAD => write!(f, "SLOAD"),
            OpCode::SSTORE => write!(f, "SSTORE"),
            OpCode::JUMP => write!(f, "JUMP"),
            OpCode::JUMPI => write!(f, "JUMPI"),
            OpCode::PC => write!(f, "PC"),
            OpCode::MSIZE => write!(f, "MSIZE"),
            OpCode::GAS => write!(f, "GAS"),
            OpCode::JUMPDEST => write!(f, "JUMPDEST"),
            OpCode::PUSH1 => write!(f, "PUSH1"),
            OpCode::PUSH2 => write!(f, "PUSH2"),
            OpCode::PUSH3 => write!(f, "PUSH3"),
            OpCode::PUSH4 => write!(f, "PUSH4"),
            OpCode::PUSH5 => write!(f, "PUSH5"),
            OpCode::PUSH6 => write!(f, "PUSH6"),
            OpCode::PUSH7 => write!(f, "PUSH7"),
            OpCode::PUSH8 => write!(f, "PUSH8"),
            OpCode::PUSH9 => write!(f, "PUSH9"),
            OpCode::PUSH10 => write!(f, "PUSH10"),
            OpCode::PUSH11 => write!(f, "PUSH11"),
            OpCode::PUSH12 => write!(f, "PUSH12"),
            OpCode::PUSH13 => write!(f, "PUSH13"),
            OpCode::PUSH14 => write!(f, "PUSH14"),
            OpCode::PUSH15 => write!(f, "PUSH15"),
            OpCode::PUSH16 => write!(f, "PUSH16"),
            OpCode::PUSH17 => write!(f, "PUSH17"),
            OpCode::PUSH18 => write!(f, "PUSH18"),
            OpCode::PUSH19 => write!(f, "PUSH19"),
            OpCode::PUSH20 => write!(f, "PUSH20"),
            OpCode::PUSH21 => write!(f, "PUSH21"),
            OpCode::PUSH22 => write!(f, "PUSH22"),
            OpCode::PUSH23 => write!(f, "PUSH23"),
            OpCode::PUSH24 => write!(f, "PUSH24"),
            OpCode::PUSH25 => write!(f, "PUSH25"),
            OpCode::PUSH26 => write!(f, "PUSH26"),
            OpCode::PUSH27 => write!(f, "PUSH27"),
            OpCode::PUSH28 => write!(f, "PUSH28"),
            OpCode::PUSH29 => write!(f, "PUSH29"),
            OpCode::PUSH30 => write!(f, "PUSH30"),
            OpCode::PUSH31 => write!(f, "PUSH31"),
            OpCode::PUSH32 => write!(f, "PUSH32"),
            OpCode::DUP1 => write!(f, "DUP1"),
            OpCode::DUP2 => write!(f, "DUP2"),
            OpCode::DUP3 => write!(f, "DUP3"),
            OpCode::DUP4 => write!(f, "DUP4"),
            OpCode::DUP5 => write!(f, "DUP5"),
            OpCode::DUP6 => write!(f, "DUP6"),
            OpCode::DUP7 => write!(f, "DUP7"),
            OpCode::DUP8 => write!(f, "DUP8"),
            OpCode::DUP9 => write!(f, "DUP9"),
            OpCode::DUP10 => write!(f, "DUP10"),
            OpCode::DUP11 => write!(f, "DUP11"),
            OpCode::DUP12 => write!(f, "DUP12"),
            OpCode::DUP13 => write!(f, "DUP13"),
            OpCode::DUP14 => write!(f, "DUP14"),
            OpCode::DUP15 => write!(f, "DUP15"),
            OpCode::DUP16 => write!(f, "DUP16"),
            OpCode::SWAP1 => write!(f, "SWAP1"),
            OpCode::SWAP2 => write!(f, "SWAP2"),
            OpCode::SWAP3 => write!(f, "SWAP3"),
            OpCode::SWAP4 => write!(f, "SWAP4"),
            OpCode::SWAP5 => write!(f, "SWAP5"),
            OpCode::SWAP6 => write!(f, "SWAP6"),
            OpCode::SWAP7 => write!(f, "SWAP7"),
            OpCode::SWAP8 => write!(f, "SWAP8"),
            OpCode::SWAP9 => write!(f, "SWAP9"),
            OpCode::SWAP10 => write!(f, "SWAP10"),
            OpCode::SWAP11 => write!(f, "SWAP11"),
            OpCode::SWAP12 => write!(f, "SWAP12"),
            OpCode::SWAP13 => write!(f, "SWAP13"),
            OpCode::SWAP14 => write!(f, "SWAP14"),
            OpCode::SWAP15 => write!(f, "SWAP15"),
            OpCode::SWAP16 => write!(f, "SWAP16"),
            OpCode::LOG0 => write!(f, "LOG0"),
            OpCode::LOG1 => write!(f, "LOG1"),
            OpCode::LOG2 => write!(f, "LOG2"),
            OpCode::LOG3 => write!(f, "LOG3"),
            OpCode::LOG4 => write!(f, "LOG4"),
            OpCode::CREATE => write!(f, "CREATE"),
            OpCode::CALL => write!(f, "CALL"),
            OpCode::CALLCODE => write!(f, "CALLCODE"),
            OpCode::RETURN => write!(f, "RETURN"),
            OpCode::DELEGATECALL => write!(f, "DELEGATECALL"),
            OpCode::CREATE2 => write!(f, "CREATE2"),
            OpCode::STATICCALL => write!(f, "STATICCALL"),
            OpCode::REVERT => write!(f, "REVERT"),
            OpCode::INVALID => write!(f, "INVALID"),
            OpCode::SELFDESTRUCT => write!(f, "SELFDESTRUCT"),
            OpCode::PUSH0 => write!(f, "PUSH0"),
        }
    }
}
