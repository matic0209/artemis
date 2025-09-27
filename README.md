# Artemis - 以太坊高级 MEV 框架

> 以 Rust 构建的高性能 MEV（Maximal Extractable Value）研究与实战平台，聚焦实时行情采集、策略编排、符号执行验证以及链上执行路径优化。

## 项目简介
- 支持图论、符号执行、REVM 复现等多种分析手段，用于发现潜在套利与防御策略
- 通过 Collector/Strategy/Executor 架构解耦数据来源、决策逻辑与执行流程
- 提供多种示例与集成，覆盖 OpenSea、Uniswap、Sandwich 等典型场景
- 内置 Prometheus 指标导出与基准测试模式，方便性能调优

## 核心特性
- **图论分析**：基于 Bellman-Ford 等算法检测负环，构建跨市场套利路径
- **符号执行**：集成 Z3，通过路径探索评估合约调用的可行性与风险
- **REVM 复现**：对候选交易进行具体执行验证，确保策略可落地
- **防御机制**：提供针对 Sandwich、抢跑/跟跑等场景的对策模块
- **模块化扩展**：Workspace 内含核心库、策略集合、演示样例与命令行工具

## 快速开始
1. **安装依赖**
   ```bash
   rustup update stable
   cargo --version
   ```
2. **复制环境变量模板并填写密钥**
   ```bash
   cp env.example .env
   # 设置 WSS、OpenSea API Key、私钥等
   ```
3. **构建与检查**
   ```bash
   cargo check --workspace
   cargo fmt --all --check
   ```
4. **运行示例**（以 `bin/artemis` 主程序为例）
   ```bash
   cargo run -p artemis -- \
     --wss wss://your-ethereum-node \
     --opensea_api_key <API_KEY> \
     --private_key <HEX_PRIVKEY> \
     --arb_contract_address <ADDRESS> \
     --bid_percentage 80
   ```
   若仅想体验框架，可从 `examples/` 目录选择更轻量的快速入门示例。

## 项目结构
- `crates/core/artemis-core`：Collector / Strategy / Executor 等核心运行时
- `crates/core/clients`：面向第三方服务的客户端实现（如 OpenSea）
- `crates/strategies`：MEV 策略集合，包含套利、防御、沙盒等子模块
- `examples/`：渐进式示例，覆盖 Alloy 接入、完整 MEV Bot 等场景
- `bin/`：框架提供的实际可执行程序（`artemis`、`cli` 等）
- `docs/`：本次重写的中文文档

## 文档导航
- `docs/项目概览.md`：整体背景、使用场景与组件职责
- `docs/快速开始.md`：环境准备、配置、运行与调试步骤
- `docs/架构设计.md`：框架内部模块关系、数据流示意与关键接口
- `docs/开发工作流.md`：代码风格、测试、性能分析与发布建议
- `docs/重构评估.md`：现状分析与潜在重构方向

## 开发约定
- 统一使用 `rustfmt`、`clippy` 保持代码风格
- 建议在提交前执行 `cargo check --workspace --all-targets`
- 对新增策略补充集成测试或最小化模拟验证
- 使用 `tracing` 与 Prometheus 指标定位性能瓶颈

## 许可
本项目遵循 MIT / Apache-2.0 双许可证发布。

## 重构提示
框架当前能够通过 `cargo check --workspace`（存在若干未使用字段的警告）。重构建议及优先级详见 `docs/重构评估.md`。
