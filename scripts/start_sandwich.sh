#!/bin/bash

# 🥪 Sandwich 策略启动脚本
# 使用方法: ./scripts/start_sandwich.sh [config_file]

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
    echo "║                       快速启动脚本                          ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# 检查依赖
check_dependencies() {
    log_info "检查系统依赖..."
    
    # 检查 Rust
    if ! command -v cargo &> /dev/null; then
        log_error "Rust 未安装，请先安装 Rust: https://rustup.rs/"
        exit 1
    fi
    
    # 检查 Git
    if ! command -v git &> /dev/null; then
        log_error "Git 未安装，请先安装 Git"
        exit 1
    fi
    
    log_success "系统依赖检查完成"
}

# 检查环境变量
check_environment() {
    log_info "检查环境变量..."
    
    # 检查私钥
    if [ -z "$SANDWICH_PRIVATE_KEY" ]; then
        log_warning "未设置 SANDWICH_PRIVATE_KEY 环境变量"
        log_warning "请在配置文件中设置私钥或设置环境变量"
    fi
    
    # 检查 RPC URL
    if [ -z "$ETH_RPC_URL" ]; then
        log_warning "未设置 ETH_RPC_URL 环境变量"
        log_warning "请确保配置文件中设置了有效的 RPC URL"
    fi
    
    log_success "环境变量检查完成"
}

# 构建项目
build_project() {
    log_info "构建 Artemis 项目..."
    
    # 检查是否在正确的目录
    if [ ! -f "Cargo.toml" ]; then
        log_error "请在 Artemis 项目根目录运行此脚本"
        exit 1
    fi
    
    # 构建项目
    if cargo build --release --bin sandwich-bot; then
        log_success "项目构建完成"
    else
        log_error "项目构建失败"
        exit 1
    fi
}

# 准备配置文件
prepare_config() {
    local config_file=${1:-"examples/sandwich_config.toml"}
    
    log_info "准备配置文件: $config_file"
    
    if [ ! -f "$config_file" ]; then
        log_error "配置文件不存在: $config_file"
        log_info "请复制 examples/sandwich_config.toml 并修改配置"
        exit 1
    fi
    
    log_success "配置文件准备完成"
}

# 启动策略
start_strategy() {
    local config_file=${1:-"examples/sandwich_config.toml"}
    
    log_info "启动 Sandwich 策略..."
    log_info "配置文件: $config_file"
    
    # 设置日志级别
    export RUST_LOG=info
    
    # 启动策略
    if cargo run --release --bin sandwich-bot -- --config "$config_file"; then
        log_success "Sandwich 策略启动成功"
    else
        log_error "Sandwich 策略启动失败"
        exit 1
    fi
}

# 显示使用说明
show_usage() {
    echo "使用方法:"
    echo "  $0 [config_file]"
    echo ""
    echo "参数:"
    echo "  config_file    配置文件路径 (默认: examples/sandwich_config.toml)"
    echo ""
    echo "环境变量:"
    echo "  SANDWICH_PRIVATE_KEY  搜索者私钥"
    echo "  ETH_RPC_URL          以太坊 RPC URL"
    echo "  RUST_LOG             日志级别 (默认: info)"
    echo ""
    echo "示例:"
    echo "  $0                                    # 使用默认配置"
    echo "  $0 config/my_config.toml             # 使用自定义配置"
    echo "  RUST_LOG=debug $0                    # 启用调试日志"
}

# 主函数
main() {
    local config_file=${1:-"examples/sandwich_config.toml"}
    
    # 显示横幅
    show_banner
    
    # 检查参数
    if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
        show_usage
        exit 0
    fi
    
    # 执行步骤
    check_dependencies
    check_environment
    build_project
    prepare_config "$config_file"
    start_strategy "$config_file"
}

# 错误处理
trap 'log_error "脚本执行失败，请检查错误信息"; exit 1' ERR

# 运行主函数
main "$@"
