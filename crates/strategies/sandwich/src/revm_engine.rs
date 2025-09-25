//! REVM 引擎 - 高精度链上模拟核心

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use tracing::{debug, info, warn, error};

use revm::{
    database::InMemoryDB,
    primitives::{
        AccountInfo, Address as RevmAddress, Bytecode, BlockEnv, CfgEnv, Env, ExecutionResult, Output, SpecId,
        TransactTo, TxEnv, U256 as RevmU256, B256, KECCAK_EMPTY,
    },
    handler::MainBuilder,
};

pub use revm::primitives::{TransactTo as RevmTransactTo, TxEnv as RevmTxEnv};

use artemis_core::eth::{Address, U256};
use alloy_provider::Provider;
use crate::types::{BlockInfo, TokenInventory, SandwichOpportunity};

/// REVM 配置
#[derive(Debug, Clone)]
pub struct RevmConfig {
    /// 内存限制 (字节)
    pub memory_limit: u64,
    /// Gas 限制
    pub gas_limit: u64,
    /// 规范 ID
    pub spec_id: SpecId,
    /// 是否启用跟踪
    pub enable_trace: bool,
    /// 状态缓存大小
    pub state_cache_size: usize,
}

impl Default for RevmConfig {
    fn default() -> Self {
        Self {
            memory_limit: 134_217_728, // 128 MB
            gas_limit: 30_000_000,     // 30M gas
            spec_id: SpecId::SHANGHAI,
            enable_trace: false,       // 生产环境关闭跟踪
            state_cache_size: 10_000,  // 缓存 10k 状态条目
        }
    }
}

/// REVM 引擎核心
pub struct RevmEngine {
    /// EVM 实例
    evm: MainBuilder<InMemoryDB>,
    /// 配置
    config: RevmConfig,
    /// 状态管理器
    state_manager: StateManager,
    /// 原始状态快照（用于回滚）
    initial_snapshot: Option<InMemoryDB>,
}

impl RevmEngine {
    /// 创建新的 REVM 引擎
    pub fn new(block: &BlockInfo, config: Option<RevmConfig>) -> Result<Self> {
        let config = config.unwrap_or_default();
        
        // 配置环境
        let mut cfg = CfgEnv::default();
        cfg.spec_id = config.spec_id;
        cfg.memory_limit = config.memory_limit;
        
        // 配置区块环境
        let mut block_env = BlockEnv::default();
        block_env.number = RevmU256::from(block.number.as_u64());
        block_env.basefee = RevmU256::from(block.base_fee_per_gas.to::<u128>());
        block_env.timestamp = RevmU256::from(block.timestamp.to::<u128>());
        block_env.coinbase = RevmAddress::from_slice(block.coinbase.as_bytes());
        block_env.gas_limit = RevmU256::from(config.gas_limit);
        block_env.difficulty = RevmU256::from(2500000000000000u64); // 固定难度
        
        // 创建数据库
        let db = InMemoryDB::default();
        
        // 构建 EVM using REVM 29.0.0 API
        let evm = MainBuilder::new()
            .with_cfg_env(cfg)
            .with_block_env(block_env)
            .with_db(db);
        
        info!("🧪 REVM 引擎已创建 - 区块: {}, Gas限制: {}", 
              block.number, config.gas_limit);
        
        Ok(Self {
            evm,
            config,
            state_manager: StateManager::new(),
            initial_snapshot: None,
        })
    }
    
    /// 从链上同步关键状态
    pub async fn sync_from_chain(&mut self, provider: Arc<Provider>) -> Result<()> {
        info!("🔄 开始从链上同步状态...");
        
        // 同步关键合约状态
        self.state_manager.sync_contracts(provider.clone()).await?;
        
        // 同步账户余额
        self.state_manager.sync_balances(provider.clone()).await?;
        
        // 应用状态到 EVM
        self.apply_state_to_evm().await?;
        
        // 创建初始快照
        self.create_snapshot()?;
        
        info!("✅ 状态同步完成");
        Ok(())
    }
    
    /// 应用状态管理器的状态到 EVM
    async fn apply_state_to_evm(&mut self) -> Result<()> {
        debug!("📋 应用状态到 EVM...");
        
        // 应用账户状态
        for (address, account_info) in &self.state_manager.accounts {
            let revm_address = RevmAddress::from_slice(address.as_bytes());
            self.evm.db_mut().insert_account_info(revm_address, account_info.clone());
        }
        
        // 应用存储状态
        for (contract_address, storage_map) in &self.state_manager.storage {
            let revm_address = RevmAddress::from_slice(contract_address.as_bytes());
            for (slot, value) in storage_map {
                self.evm.db_mut().insert_account_storage(
                    revm_address, 
                    *slot, 
                    *value
                )?;
            }
        }
        
        debug!("✅ 状态应用完成");
        Ok(())
    }
    
    /// 创建状态快照
    pub fn create_snapshot(&mut self) -> Result<()> {
        self.initial_snapshot = Some(self.evm.db().clone());
        debug!("📸 状态快照已创建");
        Ok(())
    }
    
    /// 回滚到初始状态
    pub fn rollback_to_snapshot(&mut self) -> Result<()> {
        if let Some(snapshot) = &self.initial_snapshot {
            *self.evm.db_mut() = snapshot.clone();
            debug!("🔄 已回滚到初始状态");
            Ok(())
        } else {
            Err(anyhow!("没有可用的状态快照"))
        }
    }
    
    /// 执行交易
    pub fn execute_transaction(&mut self, tx_env: TxEnv) -> Result<TransactionResult> {
        // 设置交易环境
        self.evm.env.tx = tx_env;
        
        // 执行交易
        let result = self.evm.transact().map_err(|e| anyhow!("交易执行失败: {}", e))?;
        
        match result.result {
            ExecutionResult::Success { gas_used, output, .. } => {
                let output_data = match output {
                    Output::Call(data) => data.to_vec(),
                    Output::Create(data, _) => data.to_vec(),
                };
                
                Ok(TransactionResult {
                    success: true,
                    gas_used,
                    output: output_data,
                    state_changes: self.extract_state_changes(&result)?,
                    revert_reason: None,
                })
            }
            ExecutionResult::Revert { gas_used, output } => {
                Ok(TransactionResult {
                    success: false,
                    gas_used,
                    output: output.to_vec(),
                    state_changes: vec![],
                    revert_reason: Some(format!("Revert: {}", hex::encode(&output))),
                })
            }
            ExecutionResult::Halt { reason, gas_used } => {
                Ok(TransactionResult {
                    success: false,
                    gas_used,
                    output: vec![],
                    state_changes: vec![],
                    revert_reason: Some(format!("Halt: {:?}", reason)),
                })
            }
        }
    }
    
    /// 提取状态变化
    fn extract_state_changes(&self, _result: &impl std::fmt::Debug) -> Result<Vec<StateChange>> {
        // Simplified implementation - return empty changes for now
        Ok(vec![])
    }
    
    /// 获取账户余额
    pub fn get_balance(&self, address: Address) -> Result<U256> {
        let revm_address = RevmAddress::from_slice(address.as_bytes());
        let account = self.evm.db().load_account(revm_address)?;
        Ok(U256::from(account.info.balance.as_limbs()))
    }
    
    /// 获取存储值
    pub fn get_storage(&self, address: Address, slot: U256) -> Result<U256> {
        let revm_address = RevmAddress::from_slice(address.as_bytes());
        let revm_slot = RevmU256::from_limbs(slot.0);
        let value = self.evm.db().storage(revm_address, revm_slot)?;
        Ok(U256::from(value.as_limbs()))
    }
    
    /// 设置账户余额
    pub fn set_balance(&mut self, address: Address, balance: U256) -> Result<()> {
        let revm_address = RevmAddress::from_slice(address.as_bytes());
        let revm_balance = RevmU256::from_limbs(balance.0);
        
        // 获取或创建账户
        let mut account = self.evm.db().load_account(revm_address)?.clone();
        account.info.balance = revm_balance;
        
        // 更新账户
        self.evm.db_mut().insert_account_info(revm_address, account.info);
        Ok(())
    }
    
    /// 设置存储值
    pub fn set_storage(&mut self, address: Address, slot: U256, value: U256) -> Result<()> {
        let revm_address = RevmAddress::from_slice(address.as_bytes());
        let revm_slot = RevmU256::from_limbs(slot.0);
        let revm_value = RevmU256::from_limbs(value.0);
        
        self.evm.db_mut().insert_account_storage(revm_address, revm_slot, revm_value)?;
        Ok(())
    }
}

/// 状态管理器
pub struct StateManager {
    /// 账户状态
    pub accounts: HashMap<Address, AccountInfo>,
    /// 存储状态
    pub storage: HashMap<Address, HashMap<RevmU256, RevmU256>>,
    /// 合约字节码缓存
    pub bytecode_cache: HashMap<Address, Bytecode>,
}

impl StateManager {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            storage: HashMap::new(),
            bytecode_cache: HashMap::new(),
        }
    }
    
    /// 同步关键合约状态
    pub async fn sync_contracts(&mut self, provider: Arc<Provider>) -> Result<()> {
        debug!("🔄 同步合约状态...");
        
        // 关键合约地址
        let contracts = self.get_key_contracts();
        
        for contract_addr in contracts {
            // 获取合约字节码
            if let Ok(code) = provider.get_code(contract_addr, None).await {
                if !code.is_empty() {
                    let bytecode = Bytecode::new_raw(code.0.into());
                    self.bytecode_cache.insert(contract_addr, bytecode);
                    
                    // 创建合约账户信息
                    let account_info = AccountInfo {
                        balance: RevmU256::ZERO,
                        nonce: 1,
                        code_hash: KECCAK_EMPTY, // 简化：使用空哈希
                        code: Some(self.bytecode_cache.get(&contract_addr).unwrap().clone()),
                    };
                    
                    self.accounts.insert(contract_addr, account_info);
                }
            }
        }
        
        debug!("✅ 合约状态同步完成，共 {} 个合约", self.accounts.len());
        Ok(())
    }
    
    /// 同步账户余额
    pub async fn sync_balances(&mut self, provider: Arc<Provider>) -> Result<()> {
        debug!("💰 同步账户余额...");
        
        // 这里可以添加需要同步余额的账户
        // 目前为空实现，后续根据需要添加
        
        Ok(())
    }
    
    /// 获取关键合约列表
    fn get_key_contracts(&self) -> Vec<Address> {
        vec![
            // WETH 合约
            "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(),
            // Uniswap V2 Router
            "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap(),
            // Uniswap V3 Router
            "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap(),
        ]
    }
    
    /// 设置搜索者账户初始状态
    pub fn setup_searcher_account(&mut self, address: Address, weth_balance: U256) -> Result<()> {
        debug!("👤 设置搜索者账户: {:?}", address);
        
        // 创建账户信息
        let account_info = AccountInfo {
            balance: RevmU256::ZERO,
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: None,
        };
        
        self.accounts.insert(address, account_info);
        
        // 设置 WETH 余额
        let weth_address: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap();
        let balance_slot = self.calculate_erc20_balance_slot(address, 3)?; // WETH balanceOf slot = 3
        
        self.storage.entry(weth_address)
            .or_insert_with(HashMap::new)
            .insert(balance_slot, RevmU256::from_limbs(weth_balance.0));
        
        debug!("✅ 搜索者账户设置完成，WETH 余额: {}", weth_balance);
        Ok(())
    }
    
    /// 计算 ERC20 余额存储槽
    fn calculate_erc20_balance_slot(&self, account: Address, balance_slot: u8) -> Result<RevmU256> {
        // ERC20 余额存储槽计算: keccak256(account + slot)
        use revm::primitives::keccak256;
        
        let mut input = [0u8; 64];
        input[12..32].copy_from_slice(account.as_bytes()); // account (右对齐到32字节)
        input[63] = balance_slot; // slot (右对齐到32字节)
        
        let hash = keccak256(&input);
        Ok(RevmU256::from_be_bytes(hash.0))
    }
}

/// 交易结果
#[derive(Debug, Clone)]
pub struct TransactionResult {
    /// 是否成功
    pub success: bool,
    /// Gas 消耗
    pub gas_used: u64,
    /// 输出数据
    pub output: Vec<u8>,
    /// 状态变化
    pub state_changes: Vec<StateChange>,
    /// 回滚原因
    pub revert_reason: Option<String>,
}

/// 状态变化
#[derive(Debug, Clone)]
pub struct StateChange {
    /// 地址
    pub address: Address,
    /// 变化类型
    pub change_type: StateChangeType,
    /// 旧值
    pub old_value: RevmU256,
    /// 新值
    pub new_value: RevmU256,
}

/// 状态变化类型
#[derive(Debug, Clone)]
pub enum StateChangeType {
    /// 余额变化
    Balance,
    /// 存储变化
    Storage(RevmU256),
    /// Nonce 变化
    Nonce,
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BlockInfo;
    
    #[test]
    fn test_revm_engine_creation() {
        let block = BlockInfo {
            number: U256::from(18000000),
            timestamp: U256::from(1690000000),
            base_fee_per_gas: U256::from(20000000000u64), // 20 gwei
            coinbase: Address::zero(),
        };
        
        let engine = RevmEngine::new(&block, None);
        assert!(engine.is_ok());
    }
    
    #[test]
    fn test_state_manager_creation() {
        let state_manager = StateManager::new();
        assert!(state_manager.accounts.is_empty());
        assert!(state_manager.storage.is_empty());
    }
    
    #[test]
    fn test_balance_slot_calculation() {
        let state_manager = StateManager::new();
        let account = Address::from([1u8; 20]);
        let slot = state_manager.calculate_erc20_balance_slot(account, 3);
        assert!(slot.is_ok());
    }
}
