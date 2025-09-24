# 🚀 Artemis 启动指南

## 📋 **启动前准备清单**

### **1. 系统要求** ✅
- **Rust 环境**: 已安装 Rust 1.70+
- **reth 节点**: 本地 8545 端口运行
- **网络连接**: 稳定的互联网连接
- **系统资源**: 至少 4GB RAM, 2GB 磁盘空间

### **2. 必需配置** ⚙️

#### **A. 环境变量设置**
```bash
# 必需的环境变量
export SANDWICH_PRIVATE_KEY="0x你的私钥"
export ETH_RPC_URL="http://localhost:8545"  # 你的 reth 节点
export WSS_ENDPOINT="ws://localhost:8545"   # WebSocket 连接

# 可选配置
export RUST_LOG="info"                      # 日志级别
export MIN_PROFIT_ETH="0.001"               # 最小利润 (ETH)
export MAX_GAS_PRICE_GWEI="100"             # 最大 Gas 价格
```

#### **B. 配置文件设置**
创建 `config/sandwich.toml`:
```toml
[general]
searcher_address = "0x你的钱包地址"
min_profit_threshold = "1000000000000000000"  # 1 ETH (wei)
max_gas_price = "50000000000"                 # 50 gwei
weth_address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"

[rpc]
mainnet_url = "http://localhost:8545"
ws_url = "ws://localhost:8545"
timeout_seconds = 30
max_retries = 3

[execution]
flashbots_url = "https://relay.flashbots.net"
rbuilder_url = "https://api.rbuilder.xyz"
mev_share_url = "https://mev-share.flashbots.net"

[monitoring]
enable_metrics = true
metrics_port = 8080
log_level = "info"
```

### **3. 启动步骤** 🚀

#### **方法 1: 使用启动脚本 (推荐)**
```bash
# 1. 设置权限
chmod +x scripts/start_sandwich.sh

# 2. 启动程序
./scripts/start_sandwich.sh config/sandwich.toml
```

#### **方法 2: 直接运行**
```bash
# 1. 构建项目
cargo build --release

# 2. 运行程序
cargo run --release --package sandwich-bot --bin sandwich-bot -- \
  --wss ws://localhost:8545 \
  --private-key $SANDWICH_PRIVATE_KEY \
  --sandwich-contract 0x你的合约地址 \
  --min-profit-eth 0.001 \
  --max-gas-price-gwei 100
```

#### **方法 3: 使用配置文件**
```bash
# 使用配置文件启动
cargo run --release --package sandwich-bot --bin sandwich-bot -- \
  --config config/sandwich.toml
```

### **4. 验证启动** ✅

#### **检查日志输出**
启动成功后应该看到：
```
🥪 启动 Sandwich MEV 机器人
🔧 配置参数:
   - 最小利润: 0.0010 ETH
   - 最大 Gas: 100 gwei
   - 多肉模式: false
   - rbuilder: false
   - 调试模式: false
✅ Provider 连接成功
   - 搜索者地址: 0x...
🎯 策略配置完成
📡 收集器设置完成
🎯 Sandwich 策略加载完成
⚡ 执行器配置完成
🚀 启动 Sandwich 机器人...
✅ Sandwich 机器人运行中，监听 MEV 机会...
```

#### **检查监控端点**
```bash
# 健康检查
curl http://localhost:8080/health

# 指标查看
curl http://localhost:8080/metrics

# 统计信息
curl http://localhost:8080/stats
```

### **5. 常见问题解决** 🔧

#### **问题 1: 连接 reth 失败**
```bash
# 检查 reth 是否运行
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545

# 应该返回类似: {"jsonrpc":"2.0","id":1,"result":"0x..."}
```

#### **问题 2: 私钥格式错误**
```bash
# 确保私钥格式正确 (64 字符，无 0x 前缀)
export SANDWICH_PRIVATE_KEY="1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
```

#### **问题 3: 权限不足**
```bash
# 确保有足够的 ETH 余额
# 确保有足够的 Gas 费用
# 检查网络连接
```

### **6. 高级配置** ⚡

#### **性能优化**
```bash
# 启用 rbuilder 优化
cargo run --release --package sandwich-bot --bin sandwich-bot -- \
  --enable-rbuilder \
  --enable-multi-meat \
  --mempool-buffer-size 8192 \
  --block-buffer-size 2048
```

#### **调试模式**
```bash
# 启用调试日志
export RUST_LOG="debug"
cargo run --release --package sandwich-bot --bin sandwich-bot -- \
  --debug \
  --config config/sandwich.toml
```

#### **性能测试**
```bash
# 运行性能基准测试
cargo run --release --package sandwich-bot --bin sandwich-bot -- \
  --benchmark \
  --config config/sandwich.toml
```

### **7. 监控和维护** 📊

#### **实时监控**
```bash
# 查看实时日志
tail -f sandwich.log

# 查看指标
watch -n 1 'curl -s http://localhost:8080/metrics | grep artemis'
```

#### **性能调优**
- 调整 `mempool_buffer_size` 和 `block_buffer_size`
- 启用 `enable_rbuilder` 和 `enable_multi_meat`
- 调整 `min_profit_eth` 和 `max_gas_price_gwei`

### **8. 安全注意事项** 🛡️

#### **私钥安全**
- 使用环境变量存储私钥
- 不要将私钥写入配置文件
- 使用专门的 MEV 钱包

#### **网络安全**
- 确保防火墙配置正确
- 使用 HTTPS/WSS 连接
- 定期更新依赖

#### **资金管理**
- 设置合理的最大仓位
- 启用止损机制
- 监控资金使用情况

---

## 🎯 **快速启动命令**

```bash
# 一键启动 (需要先设置环境变量)
export SANDWICH_PRIVATE_KEY="你的私钥"
export ETH_RPC_URL="http://localhost:8545"
export WSS_ENDPOINT="ws://localhost:8545"

# 启动程序
./scripts/start_sandwich.sh
```

---

## 📞 **获取帮助**

如果遇到问题，请检查：
1. reth 节点是否正常运行
2. 网络连接是否稳定
3. 私钥和地址是否正确
4. 配置文件格式是否正确
5. 系统资源是否充足

**🎉 现在你可以开始使用 Artemis Sandwich 策略了！**
