# MEV Arbitrage Quick Start Guide

## 快速开始

这个指南将帮助你快速上手新的 MEV 套利系统架构。

## 安装和设置

### 1. 添加依赖

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
artemis-mev-arbitrage = { path = "crates/strategies/mev-arbitrage", features = ["full"] }
defi-analyzer = { path = "crates/strategies/defi-analyzer" }
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"
tracing = "0.1"
z3 = "0.12"
```

### 2. 基本设置

```rust
use artemis_mev_arbitrage::{
    UnifiedArbitrageManager, ManagerConfig, DetectorSpec, DetectorConfig,
    ExplorerSpec, ExplorerConfig, DetectionContext, DetectionParams
};
use std::time::Duration;
use alloy_primitives::{Address, U256};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::init();

    // 创建配置
    let config = create_basic_config();

    // 创建管理器
    let mut manager = UnifiedArbitrageManager::new(config).await?;

    // 运行检测循环
    run_detection_loop(&mut manager).await?;

    Ok(())
}
```

## 基础配置

### 简单配置

```rust
fn create_basic_config() -> ManagerConfig {
    ManagerConfig {
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
        validators: vec![],  // 暂时不使用验证器
        optimizer: OptimizerSpec {
            optimizer_type: "z3".to_string(),
            enabled: false,  // 暂时不启用优化器
        },
        executor: ExecutorSpec {
            executor_type: "simulation".to_string(),
            enabled: false,  // 暂时不执行
        },
        global: GlobalConfig {
            max_detection_time: Duration::from_secs(60),
            max_opportunities_per_cycle: 20,
            min_confidence_threshold: 0.6,
            enable_parallel_processing: true,
            enable_caching: true,
            cache_ttl: Duration::from_secs(30),
        },
    }
}
```

### 检测循环

```rust
async fn run_detection_loop(manager: &mut UnifiedArbitrageManager) -> anyhow::Result<()> {
    loop {
        // 获取当前状态
        let context = create_detection_context().await?;

        // 执行检测周期
        let results = manager.process_arbitrage_cycle(&context).await?;

        // 处理结果
        for result in results {
            println!("发现机会: {} (置信度: {:.2})",
                     result.opportunity.id,
                     result.overall_confidence);

            if result.success && result.overall_confidence > 0.8 {
                println!("  高质量机会!");
                println!("  预期利润: {} wei", result.opportunity.expected_profit);
                println!("  预期气体成本: {} wei", result.opportunity.gas_cost);

                if let Some(plan) = &result.execution_plan {
                    println!("  执行步骤: {}", plan.steps.len());
                }
            }
        }

        // 等待下一次检测
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn create_detection_context() -> anyhow::Result<DetectionContext> {
    // 这里应该从实际的区块链数据源获取信息
    Ok(DetectionContext {
        block_number: 18_000_000, // 示例区块号
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        gas_price: U256::from(20_000_000_000u64), // 20 Gwei
        state_snapshot: create_mock_state_snapshot(),
        market_data: create_mock_market_data(),
        detection_params: DetectionParams {
            min_profit_wei: U256::from(1_000_000_000_000_000u64), // 0.001 ETH
            max_gas_cost: U256::from(300_000),
            min_confidence: 0.6,
            enable_flash_loans: true,
            target_tokens: vec![
                // WETH, USDC, DAI 等主要代币地址
            ],
        },
    })
}
```

## 高级配置示例

### 多检测器配置

```rust
fn create_advanced_config() -> ManagerConfig {
    ManagerConfig {
        detectors: vec![
            // 增强图论检测器 - 主要检测器
            DetectorSpec {
                name: "enhanced_graph".to_string(),
                detector_type: "enhanced_graph".to_string(),
                config: DetectorConfig {
                    enabled: true,
                    confidence_threshold: 0.7,
                    max_opportunities: 15,
                    timeout: Duration::from_secs(30),
                    detector_specific: serde_json::json!({
                        "max_depth": 5,
                        "enable_liquidity_analysis": true,
                        "min_liquidity_threshold": "1000000000000000000" // 1 ETH
                    }),
                },
                weight: 0.6, // 60% 权重
            },
            // 快速检测器 - 辅助检测器
            DetectorSpec {
                name: "fast".to_string(),
                detector_type: "fast".to_string(),
                config: DetectorConfig {
                    enabled: true,
                    confidence_threshold: 0.8, // 更高的阈值
                    max_opportunities: 5,
                    timeout: Duration::from_secs(5),
                    detector_specific: serde_json::json!({
                        "focus_on_hot_pairs": true,
                        "min_volume_24h": "100000000000000000000" // 100 ETH
                    }),
                },
                weight: 0.4, // 40% 权重
            },
        ],
        explorer: ExplorerSpec {
            explorer_type: "defi_analyzer".to_string(),
            config: ExplorerConfig {
                max_depth: 7,
                max_paths: 200,
                enable_optimization: true,
                timeout: Duration::from_secs(15),
            },
        },
        validators: vec![
            // 经济性验证器
            ValidatorSpec {
                name: "economic".to_string(),
                validator_type: "economic".to_string(),
                config: ValidatorConfig {
                    enabled_types: vec![ValidationType::Economic],
                    strict_mode: false,
                    timeout: Duration::from_secs(5),
                },
                required: true, // 必须通过
            },
            // 风险验证器
            ValidatorSpec {
                name: "risk".to_string(),
                validator_type: "risk".to_string(),
                config: ValidatorConfig {
                    enabled_types: vec![ValidationType::Risk],
                    strict_mode: true,
                    timeout: Duration::from_secs(3),
                },
                required: false, // 可选
            },
        ],
        optimizer: OptimizerSpec {
            optimizer_type: "z3".to_string(),
            enabled: true, // 启用优化
        },
        executor: ExecutorSpec {
            executor_type: "simulation".to_string(),
            enabled: true, // 启用模拟执行
        },
        global: GlobalConfig {
            max_detection_time: Duration::from_secs(120),
            max_opportunities_per_cycle: 50,
            min_confidence_threshold: 0.6,
            enable_parallel_processing: true,
            enable_caching: true,
            cache_ttl: Duration::from_secs(60),
        },
    }
}
```

## 自定义组件示例

### 自定义检测器

```rust
use artemis_mev_arbitrage::{ArbitrageDetector, DetectionContext, DetectionResult, ArbitrageOpportunity, OpportunityType};
use async_trait::async_trait;

pub struct CustomPairDetector {
    config: DetectorConfig,
    target_pairs: Vec<(Address, Address)>,
}

impl CustomPairDetector {
    pub fn new(config: DetectorConfig, target_pairs: Vec<(Address, Address)>) -> Self {
        Self { config, target_pairs }
    }
}

#[async_trait]
impl ArbitrageDetector for CustomPairDetector {
    async fn detect(&mut self, context: &DetectionContext) -> anyhow::Result<DetectionResult> {
        let start_time = std::time::Instant::now();
        let mut opportunities = Vec::new();

        // 遍历目标交易对
        for (token_a, token_b) in &self.target_pairs {
            // 获取不同 DEX 上的价格
            let price_uniswap = get_price_from_uniswap(*token_a, *token_b, &context.market_data)?;
            let price_sushiswap = get_price_from_sushiswap(*token_a, *token_b, &context.market_data)?;

            // 计算价格差异
            let price_diff = if price_uniswap > price_sushiswap {
                price_uniswap - price_sushiswap
            } else {
                price_sushiswap - price_uniswap
            };

            let price_ratio = price_diff as f64 / price_uniswap.min(price_sushiswap) as f64;

            // 如果价格差异超过阈值，创建机会
            if price_ratio > 0.005 { // 0.5% 价格差异
                let estimated_profit = calculate_arbitrage_profit(*token_a, *token_b, price_ratio);

                if estimated_profit > context.detection_params.min_profit_wei {
                    opportunities.push(ArbitrageOpportunity {
                        id: format!("custom_{}_{}", token_a, token_b),
                        opportunity_type: OpportunityType::SimpleArbitrage {
                            token_a: *token_a,
                            token_b: *token_b,
                            path: vec![*token_a, *token_b],
                        },
                        expected_profit: estimated_profit,
                        gas_cost: U256::from(150_000), // 估算
                        confidence: (price_ratio * 100.0).min(0.95), // 基于价格差异的置信度
                        risk_level: RiskLevel::Low,
                        deadline: Some(std::time::Instant::now() + Duration::from_secs(30)),
                        required_capital: estimated_profit / 20, // 5% 资本要求
                        metadata: serde_json::json!({
                            "price_uniswap": price_uniswap,
                            "price_sushiswap": price_sushiswap,
                            "price_ratio": price_ratio,
                            "detection_method": "custom_pair"
                        }),
                    });
                }
            }
        }

        Ok(DetectionResult {
            opportunities,
            detection_time: start_time.elapsed(),
            detector_id: "custom_pair".to_string(),
            confidence_threshold: self.config.confidence_threshold,
            metadata: std::collections::HashMap::new(),
        })
    }

    fn config(&self) -> &DetectorConfig {
        &self.config
    }

    fn update_config(&mut self, config: DetectorConfig) -> anyhow::Result<()> {
        self.config = config;
        Ok(())
    }

    fn metadata(&self) -> DetectorMetadata {
        DetectorMetadata {
            name: "Custom Pair Detector".to_string(),
            version: "1.0.0".to_string(),
            description: "Detects arbitrage opportunities between specific token pairs".to_string(),
            supported_opportunity_types: vec!["SimpleArbitrage".to_string()],
            performance_metrics: PerformanceMetrics {
                avg_detection_time: Duration::from_millis(10),
                success_rate: 0.75,
                total_detections: 100,
                avg_profit_accuracy: 0.80,
            },
        }
    }

    async fn health_check(&self) -> anyhow::Result<HealthStatus> {
        Ok(HealthStatus {
            is_healthy: true,
            status_message: "Custom pair detector is healthy".to_string(),
            last_check: std::time::Instant::now(),
            metrics: std::collections::HashMap::new(),
        })
    }
}

// 辅助函数
fn get_price_from_uniswap(token_a: Address, token_b: Address, market_data: &MarketData) -> anyhow::Result<u128> {
    // 实现从 Uniswap 获取价格的逻辑
    Ok(1000000000000000000u128) // 示例价格
}

fn get_price_from_sushiswap(token_a: Address, token_b: Address, market_data: &MarketData) -> anyhow::Result<u128> {
    // 实现从 SushiSwap 获取价格的逻辑
    Ok(1005000000000000000u128) // 示例价格（略高）
}

fn calculate_arbitrage_profit(token_a: Address, token_b: Address, price_ratio: f64) -> U256 {
    // 简化的利润计算
    U256::from((price_ratio * 1000000000000000000.0) as u128) // 基于 1 ETH 的交易
}
```

### 注册和使用自定义组件

```rust
async fn setup_with_custom_detector() -> anyhow::Result<UnifiedArbitrageManager> {
    let mut factory = ArbitrageComponentFactory::new();

    // 注册自定义检测器
    factory.register_detector("custom_pair", Box::new(|config| {
        let target_pairs = vec![
            (WETH_ADDRESS, USDC_ADDRESS),
            (WETH_ADDRESS, DAI_ADDRESS),
            (USDC_ADDRESS, DAI_ADDRESS),
        ];
        Ok(Box::new(CustomPairDetector::new(config.clone(), target_pairs)))
    }));

    // 创建配置
    let config = ManagerConfig {
        detectors: vec![
            DetectorSpec {
                name: "custom_pair".to_string(),
                detector_type: "custom_pair".to_string(),
                config: DetectorConfig {
                    enabled: true,
                    confidence_threshold: 0.6,
                    max_opportunities: 5,
                    timeout: Duration::from_secs(10),
                    detector_specific: serde_json::json!({}),
                },
                weight: 1.0,
            },
        ],
        // ... 其他配置
        ..Default::default()
    };

    // 使用自定义工厂创建管理器
    let manager = UnifiedArbitrageManager::new_with_factory(config, factory).await?;
    Ok(manager)
}
```

## 集成现有 defi-analyzer 组件

### 使用 IntegratedArbitrageSystem

```rust
use artemis_mev_arbitrage::IntegratedArbitrageSystem;
use z3::Context;

async fn run_integrated_system() -> anyhow::Result<()> {
    // 创建 Z3 上下文
    let z3_ctx = Context::new(&z3::Config::new());

    // 创建集成系统
    let mut system = IntegratedArbitrageSystem::new(&z3_ctx)?;

    // 创建状态快照
    let state_snapshot = get_current_blockchain_state().await?;

    // 运行集成检测
    let opportunities = system.integrated_arbitrage_detection(&state_snapshot).await?;

    for opportunity in opportunities {
        println!("=== 集成检测结果 ===");
        println!("图论检测: 置信度 {:.2}", opportunity.graph_detection.confidence);
        println!("MEV 策略数量: {}", opportunity.mev_strategies.len());
        println!("Z3 约束满足: {}", opportunity.z3_optimization.is_satisfiable);
        println!("综合置信度: {:.2}", opportunity.overall_confidence);

        if opportunity.overall_confidence > 0.8 {
            println!("推荐执行此机会!");
        }
    }

    Ok(())
}
```

## 监控和指标

### 基本监控

```rust
async fn monitor_system(manager: &UnifiedArbitrageManager) -> anyhow::Result<()> {
    let metrics = manager.get_metrics().await;

    println!("=== 系统指标 ===");
    println!("总检测次数: {}", metrics.total_detections);
    println!("成功检测次数: {}", metrics.successful_detections);
    println!("检测成功率: {:.2}%",
             metrics.successful_detections as f64 / metrics.total_detections as f64 * 100.0);
    println!("平均检测时间: {:?}", metrics.avg_detection_time);
    println!("平均验证时间: {:?}", metrics.avg_validation_time);
    println!("缓存命中率: {:.2}%",
             metrics.cache_hits as f64 / (metrics.cache_hits + metrics.cache_misses) as f64 * 100.0);

    Ok(())
}
```

### 健康检查

```rust
async fn health_check(manager: &mut UnifiedArbitrageManager) -> anyhow::Result<()> {
    let health_status = manager.health_check().await?;

    for (component, status) in health_status {
        if status.is_healthy {
            println!("✓ {}: {}", component, status.status_message);
        } else {
            eprintln!("✗ {}: {}", component, status.status_message);
        }
    }

    Ok(())
}
```

## 最佳实践

### 1. 错误处理

```rust
async fn robust_detection_loop(manager: &mut UnifiedArbitrageManager) -> anyhow::Result<()> {
    loop {
        match run_detection_cycle(manager).await {
            Ok(results) => {
                process_results(results).await?;
            }
            Err(e) => {
                eprintln!("检测周期失败: {}", e);

                // 检查是否是严重错误
                if is_critical_error(&e) {
                    return Err(e);
                }

                // 非严重错误，等待后重试
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

fn is_critical_error(error: &anyhow::Error) -> bool {
    // 根据错误类型判断是否严重
    error.to_string().contains("Z3 context") ||
    error.to_string().contains("out of memory")
}
```

### 2. 配置管理

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub arbitrage: ManagerConfig,
    pub network: NetworkConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub block_confirmations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: Option<String>,
}

fn load_config() -> anyhow::Result<AppConfig> {
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config.toml".to_string());

    let config_str = std::fs::read_to_string(config_path)?;
    let config: AppConfig = toml::from_str(&config_str)?;

    Ok(config)
}
```

### 3. 生产环境部署

```toml
# config.toml
[arbitrage.global]
max_detection_time = "60s"
max_opportunities_per_cycle = 30
min_confidence_threshold = 0.75
enable_parallel_processing = true
enable_caching = true
cache_ttl = "30s"

[[arbitrage.detectors]]
name = "enhanced_graph"
detector_type = "enhanced_graph"
weight = 0.7

[arbitrage.detectors.config]
enabled = true
confidence_threshold = 0.8
max_opportunities = 15
timeout = "30s"

[network]
rpc_url = "https://mainnet.infura.io/v3/YOUR_PROJECT_ID"
chain_id = 1
block_confirmations = 3

[logging]
level = "info"
file_path = "/var/log/mev-arbitrage.log"
```

这个快速入门指南提供了从基础到高级的完整使用示例，帮助开发者快速理解和使用新的 MEV 套利系统架构。通过这些示例，你可以：

1. 快速搭建基本的套利检测系统
2. 配置多个检测器和验证器
3. 创建自定义组件
4. 集成现有的 defi-analyzer 组件
5. 实施生产级的监控和错误处理

这个新架构的主要优势是高度的可配置性和可扩展性，使得你可以根据具体需求灵活调整系统行为。