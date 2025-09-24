#!/bin/bash

# 🥪 Artemis Sandwich 策略快速启动脚本
# 适用于本地 reth 节点部署

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 显示横幅
show_banner() {
    echo -e "${BLUE}"
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║                    🥪 Artemis Sandwich Strategy             ║"
    echo "║                       本地 reth 节点版本                    ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# 检查 reth 节点
check_reth_node() {
    log_info "检查 reth 节点连接..."
    
    if curl -s -X POST -H "Content-Type: application/json" \
        --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        http://localhost:8545 > /dev/null 2>&1; then
        log_success "reth 节点连接正常"
    else
        log_error "无法连接到 reth 节点 (localhost:8545)"
        log_error "请确保 reth 节点正在运行"
        exit 1
    fi
}

# 检查环境变量
check_environment() {
    log_info "检查环境变量..."
    
    if [ -z "$SANDWICH_PRIVATE_KEY" ]; then
        log_error "未设置 SANDWICH_PRIVATE_KEY 环境变量"
        log_info "请设置: export SANDWICH_PRIVATE_KEY=\"你的私钥\""
        exit 1
    fi
    
    if [ ${#SANDWICH_PRIVATE_KEY} -ne 64 ]; then
        log_error "私钥长度不正确 (应该是 64 字符)"
        exit 1
    fi
    
    log_success "环境变量检查完成"
}

# 构建项目
build_project() {
    log_info "构建 Artemis 项目..."
    
    if cargo build --release --package sandwich-bot; then
        log_success "项目构建完成"
    else
        log_error "项目构建失败"
        exit 1
    fi
}

# 启动策略
start_strategy() {
    log_info "启动 Sandwich 策略..."
    
    # 设置默认环境变量
    export ETH_RPC_URL=${ETH_RPC_URL:-"http://localhost:8545"}
    export WSS_ENDPOINT=${WSS_ENDPOINT:-"ws://localhost:8545"}
    export RUST_LOG=${RUST_LOG:-"info"}
    
    log_info "配置参数:"
    log_info "  - RPC URL: $ETH_RPC_URL"
    log_info "  - WebSocket: $WSS_ENDPOINT"
    log_info "  - 日志级别: $RUST_LOG"
    log_info "  - 搜索者地址: 0x$(echo $SANDWICH_PRIVATE_KEY | cut -c1-8)..."
    
    # 启动程序
    cargo run --release --package sandwich-bot --bin sandwich-bot -- \
        --wss "$WSS_ENDPOINT" \
        --private-key "$SANDWICH_PRIVATE_KEY" \
        --sandwich-contract "0x0000000000000000000000000000000000000000" \
        --min-profit-eth 0.001 \
        --max-gas-price-gwei 100 \
        --mempool-buffer-size 4096 \
        --block-buffer-size 1024
}

# 显示使用说明
show_usage() {
    echo "使用方法:"
    echo "  $0"
    echo ""
    echo "环境变量:"
    echo "  SANDWICH_PRIVATE_KEY  搜索者私钥 (必需)"
    echo "  ETH_RPC_URL          RPC URL (默认: http://localhost:8545)"
    echo "  WSS_ENDPOINT         WebSocket URL (默认: ws://localhost:8545)"
    echo "  RUST_LOG             日志级别 (默认: info)"
    echo ""
    echo "示例:"
    echo "  export SANDWICH_PRIVATE_KEY=\"你的私钥\""
    echo "  $0"
    echo ""
    echo "  RUST_LOG=debug $0  # 启用调试日志"
}

# 主函数
main() {
    # 显示横幅
    show_banner
    
    # 检查参数
    if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
        show_usage
        exit 0
    fi
    
    # 执行步骤
    check_reth_node
    check_environment
    build_project
    start_strategy
}

# 错误处理
trap 'log_error "脚本执行失败，请检查错误信息"; exit 1' ERR

# 运行主函数
main "$@"
