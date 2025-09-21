//! # Sandwich Strategy
//! 
//! 基于 rusty-sando 的高性能 Sandwich 攻击策略，使用 Alloy + rbuilder 优化。
//! 
//! ## 功能特性
//! 
//! - **全面泛化**: 可以 sandwich 任何引入滑点的交易
//! - **V2/V3 支持**: 支持 Uniswap V2 和 V3 池子
//! - **多肉 Sandwich**: 支持多个受害者交易的 bundle
//! - **Gas 优化**: 使用 Huff 合约和非常规 gas 优化
//! - **本地模拟**: 快速并发 EVM 模拟寻找机会
//! - **代币粉尘**: 在每个 bundle 结束时存储粉尘以降低下次交易的 gas
//! - **Salmonella 检查**: 检测 ERC20 transfer 函数是否使用异常操作码
//! 
//! ## 架构设计
//! 
//! ```
//! SandwichMempoolCollector → SandwichStrategy → SandwichExecutor
//!           ↓                        ↓                    ↓
//!    过滤潜在目标交易         模拟和优化机会        执行优化的 bundle
//! ```

/// 核心类型定义
pub mod types;

/// 策略实现
pub mod strategy;

/// 专用收集器
pub mod collectors;

/// 专用执行器
pub mod executors;

/// 合约绑定
pub mod contracts;

/// 模拟器模块
pub mod simulator;

/// 工具函数
pub mod utils;

// 重新导出主要类型
pub use strategy::SandwichStrategy;
pub use types::{Event, Action, SandwichConfig, SandwichBundle, SandwichOpportunity};
pub use collectors::sandwich_mempool_collector::SandwichMempoolCollector;
pub use executors::sandwich_executor::SandwichExecutor;
