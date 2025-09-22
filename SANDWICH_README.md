# 🥪 Artemis Sandwich Strategy

> **高性能 MEV Sandwich 套利策略 - 基于 REVM 的高精度模拟引擎**

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![REVM](https://img.shields.io/badge/REVM-latest-green.svg)](https://github.com/bluealloy/revm)

## 🎯 **项目概述**

Artemis Sandwich Strategy 是一个完整的 MEV (Maximal Extractable Value) 套利解决方案，集成了高精度的 REVM 模拟引擎，能够在毫秒级时间内识别、模拟和执行 Sandwich 攻击机会。

### ✨ **核心特性**

- 🧪 **REVM 集成** - 99%+ 精度的 EVM 模拟引擎
- ⚡ **高性能** - 毫秒级响应，支持高并发处理
- 🛡️ **风险控制** - 多层验证和失败预测机制
- 📊 **实时监控** - 完整的指标收集和健康检查
- 🔧 **模块化设计** - 易于扩展和维护的架构

---

## 🚀 **快速开始**

### **1. 环境准备**

```bash
# 克隆仓库
git clone https://github.com/your-fork/artemis.git
cd artemis

# 切换到生产优化分支
git checkout production-optimized

# 安装 Rust (如果未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### **2. 配置设置**

```bash
# 复制配置模板
cp examples/sandwich_config.toml config/my_sandwich.toml

# 编辑配置文件
nano config/my_sandwich.toml
```

**关键配置项：**
```toml
[general]
searcher_address = "0xYourSearcherAddress"
min_profit_threshold = "1000000000000000000"  # 1 ETH
max_gas_price = "50000000000"                 # 50 gwei

[rpc]
mainnet_url = "https://eth-mainnet.alchemyapi.io/v2/your-key"
ws_url = "wss://eth-mainnet.alchemyapi.io/v2/your-key"
```

### **3. 环境变量设置**

```bash
# 设置私钥 (重要!)
export SANDWICH_PRIVATE_KEY="your_private_key_here"

# 设置 RPC URL (可选，配置文件中已设置)
export ETH_RPC_URL="https://eth-mainnet.alchemyapi.io/v2/your-key"

# 设置日志级别
export RUST_LOG="info"  # debug, info, warn, error
```

### **4. 运行策略**

#### **方法 1: 使用启动脚本 (推荐)**
```bash
# 使用默认配置
./scripts/start_sandwich.sh

# 使用自定义配置
./scripts/start_sandwich.sh config/my_sandwich.toml

# 启用调试模式
RUST_LOG=debug ./scripts/start_sandwich.sh
```

#### **方法 2: 直接运行**
```bash
# 构建项目
cargo build --release

# 运行策略
cargo run --release --bin sandwich-bot -- --config config/my_sandwich.toml
```

---

## 📚 **详细文档**

### **核心文档**
- 📖 [快速上手指南](SANDWICH_QUICK_START_GUIDE.md) - 完整的快速入门教程
- 🏗️ [架构概览](SANDWICH_ARCHITECTURE_OVERVIEW.md) - 系统架构和组件详解
- 🔧 [配置示例](examples/sandwich_config.toml) - 详细的配置选项说明

### **技术文档**
- 🧪 [REVM 集成状态](SANDWICH_REVM_INTEGRATION_STATUS.md) - REVM 集成详情
- ✅ [主流程完成报告](SANDWICH_MAIN_PROCESS_COMPLETION.md) - 功能完成情况
- 🚀 [近期优化总结](FINAL_IMPROVEMENTS_SUMMARY.md) - 性能优化详情

---

## 🔄 **核心流程**

### **1. 系统架构**
```
新区块 → 状态同步 → 交易监听 → 机会识别 → REVM模拟 → Bundle构建 → 执行提交
```

### **2. 关键组件**
- **SandwichStrategy** - 策略核心协调器
- **SandwichSimulator** - REVM 高精度模拟引擎
- **PoolManager** - 池子状态管理系统
- **BundleBuilder** - 交易 Bundle 构建器
- **StateManager** - 状态缓存和同步管理

### **3. 执行流程**
1. **初始化** - 启动 REVM 引擎，同步链上状态
2. **监听** - 实时监听新区块和待处理交易
3. **识别** - 快速识别潜在的 Sandwich 机会
4. **模拟** - 使用 REVM 进行高精度模拟验证
5. **构建** - 构建完整的 Sandwich Bundle
6. **执行** - 提交到 Flashbots/rbuilder 执行

---

## ⚙️ **配置说明**

### **基础配置**
```toml
[general]
searcher_address = "0x..."           # 搜索者地址
min_profit_threshold = "1000000000000000000"  # 最小利润 (wei)
max_gas_price = "50000000000"        # 最大 gas 价格 (wei)
weth_address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"  # WETH 地址
```

### **RPC 配置**
```toml
[rpc]
mainnet_url = "https://eth-mainnet.alchemyapi.io/v2/your-key"
ws_url = "wss://eth-mainnet.alchemyapi.io/v2/your-key"
timeout_seconds = 30
max_retries = 3
```

### **执行配置**
```toml
[execution]
primary_executor = "flashbots"       # 主要执行器
fallback_executors = ["rbuilder", "mev_share"]
max_bundle_size = 10
bundle_timeout_seconds = 300
```

### **风险控制**
```toml
[risk]
max_position_size = "10000000000000000000"  # 最大仓位 (10 ETH)
max_daily_loss = "5000000000000000000"      # 最大日损失 (5 ETH)
enable_stop_loss = true
stop_loss_percentage = 5.0
```

---

## 📊 **监控和调试**

### **健康检查**
```bash
# 检查策略状态
curl http://localhost:8080/health

# 查看指标
curl http://localhost:8080/metrics

# 查看统计信息
curl http://localhost:8080/stats
```

### **日志级别**
```bash
# 调试模式
RUST_LOG=debug ./scripts/start_sandwich.sh

# 信息模式 (默认)
RUST_LOG=info ./scripts/start_sandwich.sh

# 警告模式
RUST_LOG=warn ./scripts/start_sandwich.sh
```

### **关键指标**
- **机会发现率** - 识别到的潜在机会数量
- **模拟成功率** - REVM 模拟的成功率
- **执行成功率** - Bundle 执行的成功率
- **平均利润** - 每次成功套利的平均利润
- **处理时间** - 从识别到执行的总时间

---

## 🛡️ **安全注意事项**

### **私钥安全**
- ✅ 使用环境变量存储私钥
- ✅ 不要在代码中硬编码私钥
- ✅ 使用专门的搜索者钱包
- ❌ 不要使用主钱包进行 MEV 操作

### **风险控制**
- ✅ 设置合理的最大仓位大小
- ✅ 启用止损机制
- ✅ 监控失败率和回撤
- ✅ 定期检查系统健康状态

### **合规性**
- ⚠️ 了解当地 MEV 相关法规
- ⚠️ 遵循平台使用条款
- ⚠️ 实施适当的风险披露
- ⚠️ 遵循最佳道德实践

---

## 🔧 **故障排除**

### **常见问题**

#### **1. REVM 初始化失败**
```bash
# 错误: 初始化 REVM 模拟器失败
# 解决: 检查 RPC 连接
```
**解决方案：**
- 检查 RPC URL 是否正确
- 确认网络连接稳定
- 检查 API 密钥是否有效

#### **2. 池子状态查询失败**
```bash
# 错误: 查询池子储备量失败
# 解决: 检查池子地址
```
**解决方案：**
- 确认池子地址正确
- 检查合约是否支持 `getReserves()` 方法
- 验证池子是否为 Uniswap V2 格式

#### **3. Bundle 构建失败**
```bash
# 错误: 构建前置交易失败
# 解决: 检查交易参数
```
**解决方案：**
- 确认搜索者地址有效
- 检查 WETH 余额充足
- 验证 Gas 价格设置合理

### **调试技巧**
```bash
# 启用详细日志
RUST_LOG=debug ./scripts/start_sandwich.sh

# 检查配置文件
./scripts/start_sandwich.sh config/my_sandwich.toml --dry-run

# 验证 RPC 连接
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  https://eth-mainnet.alchemyapi.io/v2/your-key
```

---

## 📈 **性能优化**

### **系统要求**
- **CPU**: 4+ 核心，支持 AVX2 指令集
- **内存**: 8GB+ RAM
- **网络**: 低延迟网络连接 (< 50ms)
- **存储**: SSD 存储，10GB+ 可用空间

### **优化建议**
- 使用高质量的 RPC 提供商 (Alchemy, Infura)
- 部署在靠近矿池的地理位置
- 启用所有性能优化选项
- 定期监控和调优参数

---

## 🤝 **贡献指南**

### **开发环境设置**
```bash
# 克隆仓库
git clone https://github.com/your-fork/artemis.git
cd artemis

# 安装开发依赖
cargo install cargo-watch cargo-test

# 运行测试
cargo test

# 运行示例
cargo run --example sandwich-bot
```

### **代码规范**
- 遵循 Rust 官方代码规范
- 使用 `rustfmt` 格式化代码
- 使用 `clippy` 进行代码检查
- 编写完整的单元测试

---

## 📄 **许可证**

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

---

## ⚠️ **免责声明**

本软件仅供教育和研究目的使用。使用本软件进行 MEV 套利存在以下风险：

- **财务风险** - 可能导致资金损失
- **技术风险** - 软件可能存在 bug 或漏洞
- **合规风险** - 可能违反某些司法管辖区的法规
- **竞争风险** - MEV 市场竞争激烈

**使用者应自行承担所有风险，开发者不对任何损失负责。**

---

## 📞 **支持**

- 📖 **文档**: 查看项目文档目录
- 🐛 **问题**: 提交 GitHub Issue
- 💬 **讨论**: 加入社区讨论
- 📧 **联系**: 通过 GitHub 联系维护者

---

## 🎉 **开始使用**

```bash
# 1. 克隆项目
git clone https://github.com/your-fork/artemis.git
cd artemis && git checkout production-optimized

# 2. 配置环境
cp examples/sandwich_config.toml config/my_config.toml
# 编辑配置文件...

# 3. 设置环境变量
export SANDWICH_PRIVATE_KEY="your_private_key"

# 4. 启动策略
./scripts/start_sandwich.sh config/my_config.toml
```

**🚀 准备好征服 MEV 市场了！**
