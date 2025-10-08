#!/bin/bash

# MEV Arbitrage Bot - 本地节点启动脚本
#
# 使用方法:
#   ./scripts/start_local_bot.sh [options]
#
# 选项:
#   --dry-run       干运行模式（不发送真实交易）
#   --no-metrics    禁用指标收集
#   --log-level     日志级别 (trace|debug|info|warn|error)
#   --config        配置文件路径

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# 默认参数
DRY_RUN=""
METRICS="--metrics"
LOG_LEVEL="info"
CONFIG="config/local_node.toml"

# 解析命令行参数
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN="--dry-run"
            shift
            ;;
        --no-metrics)
            METRICS=""
            shift
            ;;
        --log-level)
            LOG_LEVEL="$2"
            shift 2
            ;;
        --config)
            CONFIG="$2"
            shift 2
            ;;
        *)
            print_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

print_info "🚀 MEV Arbitrage Bot - Local Node Startup Script"
echo ""

# 1. 检查环境
print_info "Step 1/6: Checking environment..."

# 检查 Rust
if ! command -v cargo &> /dev/null; then
    print_error "Rust/Cargo not found. Please install Rust: https://rustup.rs/"
    exit 1
fi
print_success "Rust installed: $(rustc --version)"

# 检查配置文件
if [ ! -f "$CONFIG" ]; then
    print_error "Config file not found: $CONFIG"
    print_info "Creating default config from template..."
    mkdir -p config
    cp config/local_node.toml.example "$CONFIG" 2>/dev/null || {
        print_error "Please create $CONFIG first"
        exit 1
    }
fi
print_success "Config file found: $CONFIG"

# 检查私钥环境变量
if [ -z "$PRIVATE_KEY" ]; then
    print_warning "PRIVATE_KEY environment variable not set"
    print_info "Please set it before running in production mode:"
    echo "  export PRIVATE_KEY=\"0x...\""

    if [ -z "$DRY_RUN" ]; then
        print_error "Cannot run in production mode without PRIVATE_KEY"
        print_info "Use --dry-run flag for testing without a wallet"
        exit 1
    fi
else
    # 隐藏私钥，只显示前后几位
    MASKED_KEY="${PRIVATE_KEY:0:6}...${PRIVATE_KEY: -4}"
    print_success "Private key found: $MASKED_KEY"
fi

# 2. 检查本地节点连接
print_info "Step 2/6: Checking local node connection..."

# 从配置文件中提取 HTTP URL
HTTP_URL=$(grep 'http_url' "$CONFIG" | cut -d'"' -f2)
WS_URL=$(grep 'ws_url' "$CONFIG" | cut -d'"' -f2)

if [ -z "$HTTP_URL" ]; then
    HTTP_URL="http://localhost:8545"
fi

if [ -z "$WS_URL" ]; then
    WS_URL="ws://localhost:8546"
fi

print_info "Testing connection to: $HTTP_URL"

# 测试 HTTP 连接
RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
    "$HTTP_URL" || echo "")

if echo "$RESPONSE" | grep -q "result"; then
    BLOCK_HEX=$(echo "$RESPONSE" | grep -o '"result":"[^"]*"' | cut -d'"' -f4)
    BLOCK_NUM=$((16#${BLOCK_HEX:2}))
    print_success "HTTP RPC connected - Current block: $BLOCK_NUM"
else
    print_error "Cannot connect to Reth HTTP RPC at $HTTP_URL"
    print_info "Please ensure Reth is running:"
    echo "  reth node --http.addr 0.0.0.0 --http.port 8545 --ws.addr 0.0.0.0 --ws.port 8546"
    exit 1
fi

# 3. 构建项目
print_info "Step 3/6: Building MEV Arbitrage Bot..."

if cargo build --release --bin artemis-local-node-bot --features full 2>&1 | tee /tmp/build.log | grep -i error; then
    print_error "Build failed. Check /tmp/build.log for details"
    exit 1
fi

print_success "Build completed successfully"

# 4. 创建日志目录
print_info "Step 4/6: Preparing log directory..."

mkdir -p logs
LOG_FILE="logs/mev-arbitrage-$(date +%Y%m%d-%H%M%S).log"
print_success "Logs will be written to: $LOG_FILE"

# 5. 显示配置摘要
print_info "Step 5/6: Configuration summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Config file:    $CONFIG"
echo "  HTTP URL:       $HTTP_URL"
echo "  WebSocket URL:  $WS_URL"
echo "  Log level:      $LOG_LEVEL"
echo "  Dry run:        ${DRY_RUN:-false}"
echo "  Metrics:        ${METRICS:-disabled}"
echo "  Log file:       $LOG_FILE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# 6. 启动 Bot
print_info "Step 6/6: Starting MEV Arbitrage Bot..."
echo ""

if [ -n "$DRY_RUN" ]; then
    print_warning "Running in DRY RUN mode - No real transactions will be sent"
fi

echo ""
print_success "🤖 MEV Arbitrage Bot starting..."
echo "   Press Ctrl+C to stop"
echo ""

# 设置日志环境变量
export RUST_LOG="${LOG_LEVEL},mev_arbitrage=debug,local_node_bot=debug"
export RUST_BACKTRACE=1

# 运行 Bot (同时输出到终端和日志文件)
cargo run --release --bin artemis-local-node-bot -- \
    --config "$CONFIG" \
    --log-level "$LOG_LEVEL" \
    $DRY_RUN \
    $METRICS \
    2>&1 | tee "$LOG_FILE"

print_info "Bot stopped. Logs saved to: $LOG_FILE"