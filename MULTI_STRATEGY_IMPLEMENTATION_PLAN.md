# Multi-Strategy Implementation Plan
## 复现 0x0e49 "Jack-of-All-Trades" 策略

---

## 📋 实现路线图

### Phase 1: 清算检测 (P0)

#### 1.1 Aave协议集成

**文件**: `crates/strategies/mev-arbitrage/src/detectors/liquidation.rs`

```rust
use alloy_primitives::{Address, U256};
use std::collections::HashMap;

/// Aave借贷仓位
pub struct AavePosition {
    pub user: Address,
    pub collateral_token: Address,
    pub collateral_amount: U256,
    pub debt_token: Address,
    pub debt_amount: U256,
    pub health_factor: f64,
}

/// 清算机会
pub struct LiquidationOpportunity {
    pub protocol: LiquidationProtocol,
    pub position: AavePosition,
    pub liquidation_reward: U256,
    pub collateral_to_seize: U256,
    pub debt_to_repay: U256,
    pub estimated_price_impact: PriceImpact,
}

pub enum LiquidationProtocol {
    AaveV2,
    AaveV3,
    Compound,
    MakerDAO,
}

/// 清算检测器
pub struct LiquidationDetector {
    aave_v2_pool: Address,
    aave_v3_pool: Address,
    health_threshold: f64,
}

impl LiquidationDetector {
    pub fn new() -> Self {
        Self {
            aave_v2_pool: "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse().unwrap(),
            aave_v3_pool: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2".parse().unwrap(),
            health_threshold: 1.0,
        }
    }
    
    /// 检测健康度低于阈值的仓位
    pub async fn detect_underwater_positions<P>(
        &self,
        provider: &P,
        context: &DetectionContext,
    ) -> Result<Vec<LiquidationOpportunity>>
    where
        P: alloy_provider::Provider,
    {
        let mut opportunities = vec![];
        
        // TODO: 查询Aave合约获取所有用户仓位
        // getUserAccountData(user)
        // - totalCollateralETH
        // - totalDebtETH
        // - healthFactor
        
        // 过滤 health_factor < 1.0
        // 计算清算奖励 (通常5-10%)
        
        Ok(opportunities)
    }
}
```

**依赖**:
- Aave V2/V3 ABI
- Alloy contract bindings

---

### Phase 2: 价格影响模拟 (P0)

#### 2.1 REVM清算模拟器

**文件**: `crates/strategies/mev-arbitrage/src/simulators/price_impact.rs`

```rust
use revm::{Database, EVM};

/// 价格影响数据
pub struct PriceImpact {
    /// 受影响的池
    pub affected_pools: HashMap<Address, PoolStateChange>,
    /// 代币价格变化
    pub price_changes: HashMap<Address, PriceChange>,
    /// Gas消耗
    pub gas_used: u64,
}

pub struct PoolStateChange {
    pub pool_address: Address,
    pub token0: Address,
    pub token1: Address,
    pub old_reserve0: U256,
    pub old_reserve1: U256,
    pub new_reserve0: U256,
    pub new_reserve1: U256,
}

pub struct PriceChange {
    pub token: Address,
    pub old_price_in_weth: U256,
    pub new_price_in_weth: U256,
    pub change_percentage: f64,
}

/// 价格影响模拟器
pub struct PriceImpactSimulator<DB> {
    evm: EVM<DB>,
}

impl<DB: Database> PriceImpactSimulator<DB> {
    pub fn new(db: DB) -> Self {
        Self {
            evm: EVM::new(db),
        }
    }
    
    /// 模拟清算交易，返回价格影响
    pub fn simulate_liquidation(
        &mut self,
        liquidation: &LiquidationOpportunity,
    ) -> Result<PriceImpact> {
        // 1. 记录清算前的池状态
        let pools_before = self.snapshot_pool_states();
        
        // 2. 执行清算交易
        // - 调用Aave.liquidationCall()
        // - 抵押品卖出到DEX
        self.execute_liquidation(liquidation)?;
        
        // 3. 记录清算后的池状态
        let pools_after = self.snapshot_pool_states();
        
        // 4. 计算差异
        let affected_pools = self.diff_pool_states(&pools_before, &pools_after);
        
        // 5. 计算价格变化
        let price_changes = self.calculate_price_changes(&affected_pools);
        
        Ok(PriceImpact {
            affected_pools,
            price_changes,
            gas_used: 0, // TODO
        })
    }
    
    /// 在新价格下查找套利机会
    pub fn find_arbitrage_after_impact(
        &self,
        impact: &PriceImpact,
        graph: &TokenGraph,
    ) -> Result<Vec<ArbitrageOpportunity>> {
        let mut opportunities = vec![];
        
        // 对于每个价格变化显著的代币
        for (token, price_change) in &impact.price_changes {
            if price_change.change_percentage.abs() < 0.5 {
                continue; // 忽略小于0.5%的变化
            }
            
            // 使用新价格查找三角套利路径
            let paths = graph.find_triangular_paths(*token);
            
            for path in paths {
                // 计算基于新价格的利润
                let profit = self.calculate_profit_with_new_prices(
                    &path,
                    &impact.price_changes
                );
                
                if profit > U256::ZERO {
                    opportunities.push(ArbitrageOpportunity {
                        path,
                        expected_profit: profit,
                        // ...
                    });
                }
            }
        }
        
        Ok(opportunities)
    }
}
```

---

### Phase 3: 多策略组合引擎 (P0)

#### 3.1 策略组合器

**文件**: `crates/strategies/mev-arbitrage/src/composers/multi_strategy.rs`

```rust
/// 组合机会 (清算 + 套利)
pub struct ComposedOpportunity {
    /// 清算部分
    pub liquidation: LiquidationOpportunity,
    /// 清算引发的套利
    pub induced_arbitrages: Vec<ArbitrageOpportunity>,
    /// 总预期利润
    pub total_profit: U256,
    /// 总Gas成本
    pub total_gas: u64,
    /// 执行顺序
    pub execution_order: Vec<ExecutionStep>,
}

pub enum ExecutionStep {
    Liquidation {
        index: usize,
        opportunity: LiquidationOpportunity,
    },
    Arbitrage {
        index: usize,
        opportunity: ArbitrageOpportunity,
        depends_on: Vec<usize>, // 依赖的前序步骤
    },
}

/// 多策略组合器
pub struct MultiStrategyComposer {
    liquidation_detector: LiquidationDetector,
    arbitrage_detector: FastArbitrageDetector,
    price_simulator: PriceImpactSimulator,
}

impl MultiStrategyComposer {
    pub async fn compose_opportunities(
        &mut self,
        context: &DetectionContext,
    ) -> Result<Vec<ComposedOpportunity>> {
        // 1. 检测清算机会
        let liquidations = self.liquidation_detector
            .detect_underwater_positions(provider, context)
            .await?;
        
        let mut composed = vec![];
        
        for liq in liquidations {
            // 2. 模拟清算的价格影响
            let impact = self.price_simulator
                .simulate_liquidation(&liq)?;
            
            // 3. 检测清算后的套利机会
            let arbs = self.price_simulator
                .find_arbitrage_after_impact(&impact, &context.graph)?;
            
            if arbs.is_empty() {
                // 单纯清算，无套利
                continue;
            }
            
            // 4. 计算总利润
            let liquidation_profit = liq.liquidation_reward;
            let arbitrage_profit: U256 = arbs.iter()
                .map(|a| a.expected_profit)
                .sum();
            let total_profit = liquidation_profit + arbitrage_profit;
            
            // 5. 构建执行顺序
            let execution_order = self.build_execution_order(&liq, &arbs);
            
            composed.push(ComposedOpportunity {
                liquidation: liq,
                induced_arbitrages: arbs,
                total_profit,
                total_gas: impact.gas_used,
                execution_order,
            });
        }
        
        // 6. 按利润排序
        composed.sort_by(|a, b| b.total_profit.cmp(&a.total_profit));
        
        Ok(composed)
    }
    
    /// 构建执行顺序
    fn build_execution_order(
        &self,
        liq: &LiquidationOpportunity,
        arbs: &[ArbitrageOpportunity],
    ) -> Vec<ExecutionStep> {
        let mut steps = vec![];
        
        // 步骤1: 清算
        steps.push(ExecutionStep::Liquidation {
            index: 0,
            opportunity: liq.clone(),
        });
        
        // 步骤2+: 套利 (依赖清算)
        for (i, arb) in arbs.iter().enumerate() {
            steps.push(ExecutionStep::Arbitrage {
                index: i + 1,
                opportunity: arb.clone(),
                depends_on: vec![0], // 依赖步骤0 (清算)
            });
        }
        
        steps
    }
}
```

---

### Phase 4: 部分回滚机制 (P1)

#### 4.1 智能合约 - 部分回滚执行器

**文件**: `contracts/PartialRevertExecutor.sol`

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract PartialRevertExecutor {
    struct ExecutionGroup {
        address[] targets;
        bytes[] calldatas;
        bool canRevertIndependently;
    }
    
    /// 执行多组交易，每组可以独立失败
    function executeWithPartialRevert(
        ExecutionGroup[] calldata groups
    ) external returns (
        bool[] memory groupSuccess,
        bytes[] memory groupResults
    ) {
        groupSuccess = new bool[](groups.length);
        groupResults = new bytes[](groups.length);
        
        for (uint i = 0; i < groups.length; i++) {
            ExecutionGroup calldata group = groups[i];
            
            if (group.canRevertIndependently) {
                // 使用 try-catch 允许独立失败
                try this.executeGroup(group.targets, group.calldatas) 
                    returns (bytes memory result) 
                {
                    groupSuccess[i] = true;
                    groupResults[i] = result;
                } catch {
                    groupSuccess[i] = false;
                    // 继续执行下一组
                }
            } else {
                // 失败则整体回滚
                bytes memory result = this.executeGroup(
                    group.targets, 
                    group.calldatas
                );
                groupSuccess[i] = true;
                groupResults[i] = result;
            }
        }
    }
    
    function executeGroup(
        address[] calldata targets,
        bytes[] calldata calldatas
    ) external returns (bytes memory) {
        require(targets.length == calldatas.length);
        
        for (uint i = 0; i < targets.length; i++) {
            (bool success, bytes memory result) = targets[i].call(calldatas[i]);
            require(success, "Call failed");
        }
        
        return "";
    }
}
```

#### 4.2 Rust - 部分回滚构建器

**文件**: `crates/strategies/mev-arbitrage/src/builders/partial_revert.rs`

```rust
pub struct PartialRevertBuilder {
    executor_address: Address,
}

impl PartialRevertBuilder {
    pub fn build_bundle(
        &self,
        opportunity: &ComposedOpportunity,
    ) -> Result<FlashbotsBundle> {
        let mut groups = vec![];
        
        // 组1: 清算 (必须成功)
        groups.push(ExecutionGroup {
            targets: vec![/* Aave地址 */],
            calldatas: vec![/* liquidationCall calldata */],
            can_revert_independently: false, // 清算失败则整体失败
        });
        
        // 组2+: 每个套利独立
        for arb in &opportunity.induced_arbitrages {
            groups.push(ExecutionGroup {
                targets: vec![/* DEX地址 */],
                calldatas: vec![/* swap calldatas */],
                can_revert_independently: true, // 套利失败可跳过
            });
        }
        
        // 构建调用executeWithPartialRevert的交易
        let tx = self.build_executor_call(groups)?;
        
        Ok(FlashbotsBundle {
            txs: vec![tx],
            target_block: Some(block + 1),
            // ...
        })
    }
}
```

---

### Phase 5: 集成到ArbitrageStrategy (P0)

#### 5.1 扩展策略检测

**文件**: `crates/strategies/mev-arbitrage/src/strategy.rs`

```rust
pub struct ArbitrageStrategy<P> {
    // 现有字段...
    provider: Arc<P>,
    pool_manager: Arc<PoolManager<P>>,
    token_graph: Arc<TokenGraph>,
    fast_detector: FastArbitrageDetector,
    symbolic_detector: SymbolicDetector,
    
    // 新增字段
    liquidation_detector: LiquidationDetector,
    multi_strategy_composer: MultiStrategyComposer,
    partial_revert_builder: PartialRevertBuilder,
}

impl<P> ArbitrageStrategy<P> {
    async fn handle_new_block(&mut self, block: NewBlock) -> Vec<ArbitrageAction> {
        // 1. 更新池状态 (现有)
        self.pool_manager.update_all_pools().await?;
        
        // 2. 构建检测上下文
        let context = DetectionContext { /* ... */ };
        
        let mut actions = Vec::new();
        
        // 3. 运行现有检测器 (快速 + 符号)
        let simple_arbs = self.run_simple_detectors(&context).await;
        
        // 4. 运行多策略组合 (新增)
        let composed_opps = self.multi_strategy_composer
            .compose_opportunities(&context)
            .await?;
        
        // 5. 优先选择组合机会 (利润更高)
        for opp in composed_opps {
            if opp.total_profit > self.config.min_profit_wei {
                // 使用部分回滚构建器
                let bundle = self.partial_revert_builder
                    .build_bundle(&opp)?;
                
                actions.push(ArbitrageAction::SubmitFlashbotsBundle {
                    txs: bundle.txs,
                    target_block: block.number + 1,
                    // ...
                });
            }
        }
        
        // 6. 如果没有组合机会，使用简单套利
        if actions.is_empty() {
            for arb in simple_arbs {
                // 现有逻辑...
            }
        }
        
        actions
    }
}
```

---

## 🗓️ 实施时间表

### Week 1: 基础设施
- [ ] Day 1-2: LiquidationDetector skeleton
- [ ] Day 3-4: PriceImpactSimulator with REVM
- [ ] Day 5: 集成测试

### Week 2: 组合引擎
- [ ] Day 1-2: MultiStrategyComposer
- [ ] Day 3-4: 执行顺序构建
- [ ] Day 5: 测试组合逻辑

### Week 3: 部分回滚
- [ ] Day 1-2: Solidity合约
- [ ] Day 3-4: Rust构建器
- [ ] Day 5: 端到端测试

### Week 4: 集成&优化
- [ ] Day 1-2: 集成到ArbitrageStrategy
- [ ] Day 3: Gas优化
- [ ] Day 4: 性能测试
- [ ] Day 5: 文档和示例

---

## 📊 成功指标

| 指标 | 目标 |
|------|------|
| 检测延迟 | < 500ms |
| 模拟准确度 | > 95% |
| 组合机会发现率 | > 10% liquidations有套利 |
| Gas效率 | +50% vs 独立交易 |
| 利润提升 | +20% vs 单一策略 |

---

## ⚠️ 风险缓解

1. **REVM模拟不准确**
   - 缓解: 与真实交易对比验证
   - 回退: 如果误差>2%，禁用组合

2. **Gas成本过高**
   - 缓解: 严格的利润阈值
   - 回退: 只在高波动期启用

3. **竞争加剧**
   - 缓解: 快速检测(<100ms)
   - 回退: 降低出价以控制成本

---

**下一步**: 开始实现 Phase 1 - LiquidationDetector
