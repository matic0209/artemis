# 🏗️ Sandwich 策略架构概览

## 🎯 **系统架构图**

```
┌─────────────────────────────────────────────────────────────────┐
│                        Artemis Sandwich Strategy                │
├─────────────────────────────────────────────────────────────────┤
│  📊 监控层 (Monitoring Layer)                                   │
│  ├─ Metrics Collection    ├─ Health Checks    ├─ Alerting      │
├─────────────────────────────────────────────────────────────────┤
│  🎛️ 控制层 (Control Layer)                                      │
│  ├─ Configuration        ├─ State Manager     ├─ Event Loop    │
├─────────────────────────────────────────────────────────────────┤
│  🧠 策略层 (Strategy Layer)                                     │
│  ├─ SandwichStrategy     ├─ Opportunity Detection              │
│  ├─ Profitability Check  ├─ Bundle Building                    │
├─────────────────────────────────────────────────────────────────┤
│  🧪 模拟层 (Simulation Layer)                                   │
│  ├─ REVM Engine          ├─ Transaction Executor               │
│  ├─ State Management     ├─ Gas Calculation                    │
├─────────────────────────────────────────────────────────────────┤
│  🔗 连接层 (Connection Layer)                                   │
│  ├─ RPC Provider         ├─ Pool Discovery     ├─ Executors    │
│  ├─ Flashbots Client     ├─ RBuilder Client                    │
├─────────────────────────────────────────────────────────────────┤
│  🌐 区块链层 (Blockchain Layer)                                 │
│  ├─ Ethereum Mainnet     ├─ Uniswap V2 Pools  ├─ MEV-Share    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🔄 **核心组件详解**

### **1. 策略核心 (Strategy Core)**

#### **SandwichStrategy**
```rust
pub struct SandwichStrategy {
    provider: Arc<Provider>,              // 区块链连接
    config: SandwichConfig,               // 配置参数
    pool_manager: PoolManager,            // 池子管理器
    inventory: TokenInventory,            // 代币库存
    state_manager: Arc<StateManager>,     // 状态管理器
    simulator: SandwichSimulator,         // REVM 模拟器
    bundle_builder: BundleBuilder,        // Bundle 构建器
    stats: SandwichStats,                 // 统计信息
    current_block: BlockInfo,             // 当前区块信息
}
```

**职责：**
- 🎯 协调整个策略执行流程
- 📊 管理状态和配置
- 🔄 处理事件循环和区块更新

### **2. REVM 模拟引擎 (Simulation Engine)**

#### **SandwichSimulator**
```rust
pub struct SandwichSimulator {
    provider: Arc<Provider>,
    revm_engine: Option<Arc<Mutex<RevmEngine>>>,
    transaction_executor: Option<TransactionExecutor>,
    config: SandwichConfig,
}
```

**核心功能：**
- 🧪 **高精度模拟** - 99%+ 准确度的 EVM 模拟
- ⚡ **快速检查** - 毫秒级盈利性预筛选
- 🔄 **状态管理** - 完整的链上状态同步
- 📊 **结果分析** - 详细的利润和 Gas 计算

#### **RevmEngine**
```rust
pub struct RevmEngine {
    evm: Evm<'static, (), InMemoryDB>,    // REVM 实例
    state_manager: StateManager,          // 状态管理器
    config: RevmConfig,                   // 配置参数
    initial_state_snapshot: Option<InMemoryDB>, // 初始状态快照
}
```

#### **TransactionExecutor**
```rust
pub struct TransactionExecutor {
    revm_engine: Arc<Mutex<RevmEngine>>,
    config: SandwichConfig,
}
```

### **3. 池子管理系统 (Pool Management)**

#### **PoolManager**
```rust
struct PoolManager {
    discovery: PoolDiscovery,             // 池子发现器
    known_pools: Arc<RwLock<HashMap<Address, Pool>>>, // 池子缓存
    provider: Arc<Provider>,              // 区块链连接
}
```

**功能特性：**
- 🔍 **自动发现** - 自动发现新的 Uniswap V2 池子
- 💾 **状态缓存** - 高效的池子状态缓存机制
- 🔄 **批量更新** - 并行更新多个池子状态
- 📊 **实时监控** - 实时监控池子流动性变化

### **4. Bundle 构建系统 (Bundle Building)**

#### **BundleBuilder**
```rust
struct BundleBuilder {
    config: SandwichConfig,
    provider: Option<Arc<Provider>>,
}
```

**构建流程：**
1. **前置交易** - 构建 `swapExactETHForTokens` 交易
2. **受害者交易** - RLP 编码原始受害者交易
3. **后置交易** - 构建 `swapExactTokensForETH` 交易
4. **Gas 计算** - 精确计算总 Gas 使用量

### **5. 状态管理系统 (State Management)**

#### **StateManager**
```rust
pub struct StateManager {
    pool_states: Arc<RwLock<HashMap<Address, PoolState>>>, // 池子状态
    token_inventories: Arc<RwLock<HashMap<Address, TokenInventory>>>, // 代币库存
    cache: Arc<Mutex<LruCache<CacheKey, CacheValue>>>, // LRU 缓存
}
```

**管理内容：**
- 🏊 **池子状态** - 储备量、流动性、最后更新时间
- 💰 **代币库存** - WETH 和代币余额
- 📊 **缓存管理** - 高效的 LRU 缓存机制

---

## 🔄 **数据流架构**

### **1. 事件驱动流程**

```
新区块事件 → 状态同步 → 交易监听 → 机会识别 → 模拟验证 → Bundle 构建 → 执行提交
```

### **2. 详细数据流**

```
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│   新区块     │───▶│   状态同步    │───▶│  池子更新    │
└─────────────┘    └──────────────┘    └─────────────┘
                                              │
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│  交易监听    │◀───│   事件分发    │◀───│   库存更新    │
└─────────────┘    └──────────────┘    └─────────────┘
       │
       ▼
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│  快速检查    │───▶│  机会生成     │───▶│  盈利性检查   │
└─────────────┘    └──────────────┘    └─────────────┘
                                              │
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│  Bundle构建  │◀───│   详细模拟    │◀───│   REVM引擎   │
└─────────────┘    └──────────────┘    └─────────────┘
       │
       ▼
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│   执行提交   │───▶│   监控记录    │───▶│   统计更新    │
└─────────────┘    └──────────────┘    └─────────────┘
```

---

## 🧩 **模块依赖关系**

### **核心依赖图**

```
SandwichStrategy
├── SandwichSimulator
│   ├── RevmEngine
│   │   ├── StateManager
│   │   └── Provider
│   └── TransactionExecutor
│       └── RevmEngine
├── PoolManager
│   ├── PoolDiscovery
│   └── Provider
├── BundleBuilder
│   └── SandwichConfig
├── StateManager
│   └── Provider
└── SandwichConfig
```

### **外部依赖**

```
Artemis Core
├── artemis_core::eth (Provider, Address, U256, etc.)
├── artemis_core::collectors (BlockCollector)
├── artemis_core::executors (Executor trait)
└── artemis_core::types (Strategy trait)

Alloy Framework
├── alloy_provider (RPC 连接)
├── alloy_consensus (交易类型)
├── alloy_primitives (基础类型)
└── alloy_rpc_types (RPC 类型)

REVM
├── revm (EVM 模拟)
├── revm_primitives (基础类型)
└── revm_db (状态数据库)
```

---

## ⚡ **性能特性**

### **1. 并发处理**
- **并行池子查询** - 同时查询多个池子状态
- **异步事件处理** - 非阻塞的事件处理循环
- **并发模拟** - 支持多个机会的并行模拟

### **2. 内存优化**
- **对象池** - 减少内存分配开销
- **LRU 缓存** - 智能的缓存管理
- **零拷贝序列化** - 高效的序列化处理

### **3. SIMD 优化**
- **向量化计算** - 使用 AVX2 加速数学运算
- **批量处理** - 批量处理多个计算任务
- **CPU 优化** - 针对现代 CPU 的优化

### **4. 网络优化**
- **连接池** - 复用 RPC 连接
- **批量请求** - 减少网络往返次数
- **压缩传输** - 减少网络带宽使用

---

## 🛡️ **安全特性**

### **1. 错误处理**
- **统一错误类型** - `ArtemisError` 统一错误处理
- **优雅降级** - 部分功能失败不影响整体运行
- **重试机制** - 自动重试失败的操作

### **2. 状态一致性**
- **原子操作** - 确保状态更新的原子性
- **快照机制** - REVM 状态快照和回滚
- **并发安全** - 线程安全的状态管理

### **3. 资源管理**
- **内存限制** - 防止内存泄漏
- **连接限制** - 限制并发连接数
- **超时控制** - 防止长时间阻塞

---

## 📊 **监控和可观测性**

### **1. 指标收集**
```rust
// 性能指标
metrics::counter!("artemis.sandwich.opportunities_found").increment(1);
metrics::histogram!("artemis.sandwich.processing_time").record(time.as_millis() as f64);
metrics::histogram!("artemis.sandwich.estimated_profit").record(profit.as_u128() as f64 / 1e18);

// REVM 指标
metrics::gauge!("artemis.sandwich.revm_initialized").set(1.0);
metrics::counter!("artemis.sandwich.revm_simulations_success").increment(1);
metrics::counter!("artemis.sandwich.revm_simulations_failed").increment(1);
```

### **2. 日志系统**
```rust
// 结构化日志
info!("🥪 发现 Sandwich 机会! 利润: {:.4} ETH, 处理时间: {:.2}ms",
      profit.to::<u128>() as f64 / 1e18,
      processing_time.as_millis());

debug!("🧪 REVM 模拟成功 - 净利润: {:.6} ETH, Gas: {}, ROI: {:.2}%",
       net_profit.as_u128() as f64 / 1e18,
       total_gas,
       roi);
```

### **3. 健康检查**
- **系统状态** - CPU、内存、网络使用情况
- **策略状态** - 机会发现率、成功率、利润统计
- **连接状态** - RPC 连接、外部服务连接状态

---

## 🚀 **扩展性设计**

### **1. 模块化架构**
- **插件系统** - 支持自定义策略插件
- **配置驱动** - 通过配置文件控制行为
- **接口抽象** - 清晰的接口定义便于扩展

### **2. 水平扩展**
- **多实例部署** - 支持多个策略实例并行运行
- **负载均衡** - 智能的负载分配机制
- **状态共享** - 分布式状态管理

### **3. 垂直扩展**
- **性能调优** - 针对不同硬件优化
- **资源池化** - 动态资源分配
- **缓存分层** - 多级缓存架构

---

## 🎯 **总结**

Artemis Sandwich 策略采用现代化的微服务架构设计，具有以下特点：

- **🏗️ 模块化设计** - 清晰的组件分离和接口定义
- **⚡ 高性能** - 并发处理、内存优化、SIMD 加速
- **🛡️ 高可靠** - 完善的错误处理和状态管理
- **📊 可观测** - 全面的监控和日志系统
- **🚀 可扩展** - 支持水平和垂直扩展

这种架构设计确保了策略的**高性能**、**高可靠性**和**易维护性**，为 MEV 套利提供了坚实的技术基础。
