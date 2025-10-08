# Artemis 核心框架集成架构

**创建时间**: 2025-10-02
**目标**: 正确集成原始 Artemis 框架 + REVM + Z3 + 图论

---

## 问题诊断

### 优化过程中丢失的功能

在之前的重构中，**完全脱离了 Artemis 的核心架构**，导致：

1. ❌ **Collector 层丢失** - 检测器没有数据源
2. ❌ **没有实现 Strategy trait** - 无法接入 Engine
3. ❌ **重复实现 Executor** - 应该用 artemis-core 的 Executor
4. ❌ **Engine 未使用** - 手动编排而不是用事件驱动

---

## 正确的架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Artemis 原始框架 (保留)                           │
└─────────────────────────────────────────────────────────────────────┘

[artemis-core]  核心框架 (不要修改!)
├── types.rs                          定义核心 trait
│   ├── Collector<E>                  数据收集 trait
│   ├── Strategy<E, A>                策略 trait
│   └── Executor<A>                   执行 trait
│
├── collectors/                       数据源 (已实现 ✅)
│   ├── BlockCollector               新区块事件
│   ├── MempoolCollector             Mempool 交易
│   ├── LogCollector                 Event logs
│   └── MevShareCollector            MEV-Share 事件
│
├── executors/                        执行器 (已实现 ✅)
│   ├── FlashbotsExecutor            Flashbots bundle
│   ├── MempoolExecutor              公共 mempool
│   └── RbuilderExecutor             Rbuilder 集成
│
└── engine/                           编排引擎 (已实现 ✅)
    ├── HighPerformanceEngine        高性能引擎
    ├── EventBus                     事件总线
    └── ExecutionCoordinator         执行协调器

┌─────────────────────────────────────────────────────────────────────┐
│              mev-arbitrage Strategy (需要重构!)                      │
└─────────────────────────────────────────────────────────────────────┘

[mev-arbitrage]  实现 Strategy<Event, Action>
├── ArbitrageStrategy                主策略 (实现 Strategy trait)
│   │
│   ├── sync_state()                 ✅ 初始化状态
│   │   ├── 加载 DEX pools
│   │   ├── 构建 Token Graph (petgraph)
│   │   └── 预热 Z3 cache
│   │
│   └── process_event(event)         ✅ 处理事件
│       │
│       ├─[Event::NewBlock]          新区块
│       │   └→ 更新 pool 状态
│       │
│       ├─[Event::PendingTx]         Pending 交易
│       │   └→ 检测 sandwich 机会
│       │
│       └─[Event::Log]               Swap 事件
│           └→ 更新储备量 + 检测套利
│
├── detectors/                       检测器 (内部模块)
│   ├── FastDetector                 2-3 hop 快速检测
│   ├── SymbolicDetector             Z3 符号执行 (10策略)
│   └── EnhancedDetector             Bellman-Ford 多跳
│
├── validators/                      验证器 (内部模块)
│   ├── REVMValidator                REVM 模拟
│   └── EconomicValidator            经济性验证
│
├── optimizers/                      优化器 (内部模块)
│   ├── Z3StrategyOptimizer          Z3 策略优化
│   └── GraphOptimizer               图论路径优化
│
└── utils/                           工具模块
    ├── token_graph.rs               Token 图 (petgraph)
    ├── pool_manager.rs              Pool 状态管理
    └── z3_cache.rs                  Z3 缓存

┌─────────────────────────────────────────────────────────────────────┐
│                         数据流 (正确)                                │
└─────────────────────────────────────────────────────────────────────┘

1. Collectors 生成 Events
   ├── BlockCollector     → NewBlock { hash, number }
   ├── MempoolCollector   → PendingTx { tx, ... }
   └── LogCollector       → SwapEvent { pool, reserves }

2. Engine 路由 Events 到 Strategies
   Engine.add_strategy(arbitrage_strategy)
         ↓
   arbitrage_strategy.process_event(event)

3. Strategy 分析并生成 Actions
   ArbitrageStrategy::process_event()
         ↓ 检测
   FastDetector.detect()
         ↓ Z3 优化
   Z3StrategyOptimizer.optimize()
         ↓ REVM 验证
   REVMValidator.validate()
         ↓ 生成
   Action::FlashbotsBundle { txs, target_block }

4. Engine 分发 Actions 到 Executors
   FlashbotsExecutor.execute(bundle)
         ↓
   eth_sendBundle RPC
```

---

## 核心集成点

### 1. 实现 Strategy Trait

```rust
// 文件: crates/strategies/mev-arbitrage/src/strategy.rs

use artemis_core::types::{Strategy, Events, Actions};
use async_trait::async_trait;
use anyhow::Result;

pub struct ArbitrageStrategy {
    // 数据层
    pool_manager: PoolManager,           // DashMap<Address, PoolState>
    token_graph: Arc<TokenGraph>,        // petgraph

    // 检测层
    fast_detector: FastDetector,
    symbolic_detector: SymbolicDetector,

    // 优化层
    z3_optimizer: Z3StrategyOptimizer,

    // 验证层
    revm_validator: REVMValidator,

    // 配置
    config: ArbitrageConfig,
}

#[async_trait]
impl Strategy<Events, Actions> for ArbitrageStrategy {
    /// 初始化: 加载链上状态
    async fn sync_state(&mut self) -> Result<()> {
        tracing::info!("Syncing arbitrage strategy state...");

        // 1. 从链上加载所有 DEX pools
        let pools = self.load_dex_pools().await?;
        tracing::info!("Loaded {} DEX pools", pools.len());

        // 2. 构建 Token Graph (使用 petgraph)
        self.token_graph = Arc::new(
            TokenGraph::from_pools(&pools)?
        );
        tracing::info!("Built token graph with {} tokens",
                       self.token_graph.token_count());

        // 3. 预热 Z3 cache (加载历史成功策略)
        self.symbolic_detector.warmup_cache().await?;

        Ok(())
    }

    /// 事件处理: Collector 事件 → 检测 → Actions
    async fn process_event(&mut self, event: Events) -> Vec<Actions> {
        match event {
            // 1. 新区块: 更新状态 + 检测套利
            Events::NewBlock(block) => {
                self.handle_new_block(block).await
            }

            // 2. Pending 交易: 检测 sandwich/frontrun
            Events::Transaction(tx) => {
                self.handle_pending_tx(tx).await
            }

            // 3. 其他事件
            _ => vec![],
        }
    }
}

impl ArbitrageStrategy {
    async fn handle_new_block(&mut self, block: NewBlock) -> Vec<Actions> {
        let mut actions = Vec::new();

        // 1. 更新所有 pool 状态 (批量 RPC)
        if let Err(e) = self.pool_manager.update_all_pools(block.number).await {
            tracing::warn!("Failed to update pools: {}", e);
            return actions;
        }

        // 2. 构建检测上下文
        let context = DetectionContext {
            block_number: block.number.as_u64(),
            timestamp: current_timestamp(),
            pool_states: self.pool_manager.get_all_states(),
            token_graph: self.token_graph.clone(),
            // ...
        };

        // 3. 并行运行多个检测器
        let (fast_opps, symbolic_opps) = tokio::join!(
            self.fast_detector.detect(&context),
            self.symbolic_detector.detect(&context),
        );

        // 4. 合并机会
        let mut opportunities = Vec::new();
        if let Ok(opps) = fast_opps {
            opportunities.extend(opps);
        }
        if let Ok(opps) = symbolic_opps {
            opportunities.extend(opps);
        }

        // 5. 对每个机会: 优化 → 验证 → 转 Action
        for opp in opportunities {
            // Z3 优化
            let optimized = match self.z3_optimizer.optimize(&opp).await {
                Ok(opt) => opt,
                Err(_) => continue,
            };

            // REVM 验证
            let validation = match self.revm_validator.validate(&optimized).await {
                Ok(v) => v,
                Err(_) => continue,
            };

            if validation.is_valid && validation.estimated_profit > MIN_PROFIT {
                // 转换为 Flashbots Bundle Action
                let bundle = self.build_flashbots_bundle(&optimized);
                actions.push(Actions::FlashbotsBundle(bundle));
            }
        }

        actions
    }

    async fn handle_pending_tx(&mut self, tx: Transaction) -> Vec<Actions> {
        // TODO: Sandwich 检测
        vec![]
    }
}
```

### 2. 集成到 Engine

```rust
// 文件: bin/artemis/src/main.rs

use artemis_core::{
    engine::HighPerformanceEngine,
    collectors::{BlockCollector, MempoolCollector},
    executors::FlashbotsExecutor,
    types::Events,
};
use mev_arbitrage::ArbitrageStrategy;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 创建 Provider
    let provider = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .on_ws(WsConnect::new(WS_RPC_URL))
            .await?
    );

    // 2. 创建 Collectors
    let block_collector = Box::new(BlockCollector::new(provider.clone()));
    let mempool_collector = Box::new(MempoolCollector::new(provider.clone()));

    // 3. 创建 Strategy
    let mut arbitrage_strategy = ArbitrageStrategy::new(config);
    arbitrage_strategy.sync_state().await?;

    // 4. 创建 Executor
    let flashbots_executor = Box::new(FlashbotsExecutor::new(
        flashbots_signer,
        provider.clone(),
    ));

    // 5. 组装到 Engine
    let mut engine = HighPerformanceEngine::new()
        .with_max_concurrent_strategies(10)
        .with_circuit_breaker_enabled(true);

    // 添加 Collector
    engine.add_collector(Box::new(block_collector.map(Events::NewBlock)));
    engine.add_collector(Box::new(mempool_collector.map(Events::Transaction)));

    // 添加 Strategy
    engine.add_strategy(Box::new(arbitrage_strategy));

    // 添加 Executor
    engine.add_executor(Box::new(flashbots_executor));

    // 6. 启动 Engine
    tracing::info!("Starting Artemis MEV Bot...");
    engine.run().await?;

    Ok(())
}
```

---

## 模块职责划分

### artemis-core (不要修改)
- ✅ 提供 Collector/Strategy/Executor trait
- ✅ 提供 Engine 编排
- ✅ 提供常用 Collectors (Block, Mempool, Log)
- ✅ 提供常用 Executors (Flashbots, Mempool)

### mev-arbitrage (你的策略)
- ✅ 实现 `Strategy<Events, Actions>` trait
- ✅ 使用 artemis-core 的 Collectors (不要重复造轮子!)
- ✅ 使用 artemis-core 的 Executors (不要重复造轮子!)
- ✅ 专注于套利逻辑:
  - 检测器 (Fast/Symbolic/Enhanced)
  - 优化器 (Z3/Graph)
  - 验证器 (REVM/Economic)
  - 工具 (TokenGraph/PoolManager)

### 子模块分工

| 模块 | 职责 | 依赖 |
|------|------|------|
| **graph-theory** | Token Graph, 路径搜索 | petgraph |
| **symbolic-execution** | Z3 策略优化, 缓存 | z3 |
| **revm-validation** | EVM 模拟验证 | revm |
| **defense** | 抗 MEV 攻击检测 | - |

---

## 集成 Reth (可选)

如果需要更快的链上数据访问:

```rust
// 使用 Reth 作为本地节点
use reth::providers::StateProvider;

pub struct RethPoolFetcher {
    state_provider: Arc<dyn StateProvider>,
}

impl RethPoolFetcher {
    /// 直接从 Reth 数据库读取 pool 状态 (比 RPC 快 100x)
    pub fn get_pool_reserves(&self, pool: Address) -> Result<(U256, U256)> {
        let reserve0_slot = keccak256("reserve0");
        let reserve1_slot = keccak256("reserve1");

        let reserve0 = self.state_provider.storage(pool, reserve0_slot)?;
        let reserve1 = self.state_provider.storage(pool, reserve1_slot)?;

        Ok((reserve0, reserve1))
    }
}
```

---

## 数据流完整示例

```
时间线: Block N 到来

1. BlockCollector (artemis-core)
   ├─ 订阅 provider.subscribe_blocks()
   └─ 发出 Event::NewBlock { number: N, hash }
        ↓
2. Engine (artemis-core)
   ├─ EventBus 接收 Event
   └─ 路由到 ArbitrageStrategy
        ↓
3. ArbitrageStrategy.process_event()
   ├─ 更新 pool 状态 (Multicall3 批量)
   ├─ FastDetector.detect()
   │   └─ 使用 TokenGraph 找三角套利 (petgraph)
   ├─ SymbolicDetector.detect()
   │   └─ Z3 符号执行 10 种策略 (并行)
   ├─ Z3StrategyOptimizer.optimize()
   │   └─ 优化参数 (金额/路径)
   └─ REVMValidator.validate()
       └─ EVM 模拟验证利润
        ↓
4. 生成 Action::FlashbotsBundle
   └─ { txs: [approve, swap1, swap2, swap3], block: N+1 }
        ↓
5. Engine 分发到 FlashbotsExecutor (artemis-core)
   ├─ 签名 bundle
   ├─ eth_sendBundle RPC
   └─ 监控 bundle 状态
        ↓
6. Metrics (artemis-core)
   └─ Prometheus 记录: 延迟/成功率/利润
```

---

## 重构步骤

### Phase 1: 最小改动接入 (1天)

1. ✅ 在 `mev-arbitrage/src/strategy.rs` 实现 `Strategy<Events, Actions>`
2. ✅ 修改 `bin/artemis/src/main.rs` 使用 Engine
3. ✅ 删除重复的 Collector/Executor 代码

### Phase 2: 完善检测器 (3天)

1. ✅ FastDetector 使用 TokenGraph (petgraph)
2. ✅ SymbolicDetector 优化并行
3. ✅ REVMValidator 完整实现

### Phase 3: 性能优化 (2天)

1. ✅ 批量 RPC (Multicall3)
2. ✅ Z3 增量求解
3. ✅ 缓存优化

### Phase 4: 监控告警 (1天)

1. ✅ 接入 artemis-core 的 Prometheus metrics
2. ✅ 添加业务指标
3. ✅ Grafana dashboard

---

## 关键原则

### DO ✅
- ✅ **复用 artemis-core** 的 Collector/Executor/Engine
- ✅ **专注于策略逻辑** - 检测/优化/验证
- ✅ **保留原框架设计** - 事件驱动架构
- ✅ **利用现有集成** - Reth/Z3/petgraph

### DON'T ❌
- ❌ **不要修改 artemis-core** (除非 bug)
- ❌ **不要重复实现 Collector** (已经有了!)
- ❌ **不要重复实现 Executor** (已经有了!)
- ❌ **不要绕过 Engine** (不要手动编排)

---

## 成功标准

- [ ] ArbitrageStrategy 实现 `Strategy<Events, Actions>` trait
- [ ] 使用 artemis-core 的 BlockCollector 获取区块
- [ ] 使用 artemis-core 的 MempoolCollector 监控 mempool
- [ ] 使用 artemis-core 的 FlashbotsExecutor 提交 bundle
- [ ] 使用 Engine 编排整个流程
- [ ] 保留所有功能: Z3/REVM/petgraph/图论
- [ ] 端到端延迟 <1s
- [ ] 能发现并执行真实套利

---

**下一步**: 实现 `ArbitrageStrategy` 的 Strategy trait
