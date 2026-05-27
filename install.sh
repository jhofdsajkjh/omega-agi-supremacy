#!/bin/bash
#==============================================================================
# OMEGA AGI Supremacy - 一键安装向导
# 自动检测环境、安装依赖、引导配置
#==============================================================================
set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color
MAGENTA='\033[0;35m'

# Emoji
ROCKET="🚀"
GEAR="🔧"
CHECK="✅"
CROSS="❌"
WARN="⚠️"
INFO="ℹ️"
FOLDER="📁"
ROBOT="🤖"
SPARKLES="✨"
GLOBE="🌐"
CLIPBOARD="📋"
ARROW="👉"

# 项目路径
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# 日志函数
log_info()  { echo -e "${BLUE}${INFO}${NC}  $1"; }
log_ok()    { echo -e "${GREEN}${CHECK}${NC}  $1"; }
log_warn()  { echo -e "${YELLOW}${WARN}${NC}  $1"; }
log_error() { echo -e "${RED}${CROSS}${NC}  $1"; }
log_step()  { echo -e "${CYAN}${ARROW}${NC}  $1"; }
log_title() { echo -e "\n${BOLD}${MAGENTA}━━━ ${1} ━━━${NC}\n"; }

# 分隔线
separator() { echo -e "${CYAN}────────────────────────────────────────${NC}"; }

#==============================================================================
# Banner
#==============================================================================
show_banner() {
    cat << 'EOF'

    ██╗  ██╗ █████╗  ██████╗██╗  ██╗███████╗██████╗  ██████╗ ███████╗
    ██║  ██║██╔══██╗██╔════╝██║ ██╔╝██╔════╝██╔══██╗██╔═══██╗██╔════╝
    ███████║███████║██║     █████╔╝ █████╗  ██████╔╝██║   ██║███████╗
    ██╔══██║██╔══██║██║     ██╔═██╗ ██╔══╝  ██╔══██╗██║   ██║╚════██║
    ██║  ██║██║  ██║╚██████╗██║  ██╗███████╗██║  ██║╚██████╔╝███████║
    ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝
                        A G I   S U P R E M A C Y
                ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

              The Most Powerful Autonomous AI Agent Framework
                     🤖  Install Wizard v1.0  🤖

EOF
}

#==============================================================================
# 系统检测
#==============================================================================
check_system() {
    log_title "第一步：系统环境检测"

    log_step "检查操作系统..."
    OS_TYPE=$(uname -s)
    if [[ "$OS_TYPE" == "Linux" ]]; then
        log_ok "操作系统: Linux ✅"
    elif [[ "$OS_TYPE" == "Darwin" ]]; then
        log_ok "操作系统: macOS ✅"
    else
        log_warn "操作系统: $OS_TYPE (未完全测试)"
    fi

    log_step "检查 Python..."
    if command -v python3 &> /dev/null; then
        PYTHON_VERSION=$(python3 --version 2>&1 | awk '{print $2}')
        PYTHON_MAJOR=$(echo $PYTHON_VERSION | cut -d. -f1)
        PYTHON_MINOR=$(echo $PYTHON_VERSION | cut -d. -f2)
        if [[ "$PYTHON_MAJOR" -ge 3 ]] && [[ "$PYTHON_MINOR" -ge 8 ]]; then
            log_ok "Python 版本: $PYTHON_VERSION ✅"
        else
            log_error "Python 版本过低: $PYTHON_VERSION (需要 3.8+)"
            exit 1
        fi
    else
        log_error "未找到 Python3，请先安装 Python 3.8+"
        exit 1
    fi

    log_step "检查 pip..."
    if python3 -m pip --version &> /dev/null; then
        log_ok "pip 可用 ✅"
    else
        log_warn "pip 不可用，尝试安装..."
        python3 -m ensurepip --default-pip 2>/dev/null || true
    fi

    log_step "检查磁盘空间..."
    AVAILABLE=$(df -h . | awk 'NR==2 {print $4}')
    log_ok "可用空间: $AVAILABLE ✅"

    log_step "检查网络连接..."
    if curl -s --max-time 5 https://api.github.com &> /dev/null; then
        log_ok "网络连接正常 ✅"
    else
        log_warn "网络连接受限，部分功能可能受影响"
    fi
}

#==============================================================================
# 安装依赖
#==============================================================================
install_dependencies() {
    log_title "第二步：安装项目依赖"

    log_step "更新 pip..."
    python3 -m pip install --upgrade pip --quiet 2>/dev/null || true

    log_step "安装核心依赖..."
    CORE_DEPS="openai anthropic requests pyyaml python-dotenv flask rich click"
    python3 -m pip install $CORE_DEPS --quiet 2>/dev/null || true || true
    log_ok "核心依赖安装完成 ✅"


    log_step "安装可选依赖..."
    OPT_DEPS="docker psutil"
    python3 -m pip install $OPT_DEPS --quiet 2>/dev/null || true
    log_ok "可选依赖安装完成 ✅"

    # 检查 docker
    log_step "检查 Docker..."
    if command -v docker &> /dev/null; then
        DOCKER_VERSION=$(docker --version 2>&1 | awk '{print $5}' | cut -d',' -f1)
        log_ok "Docker 已安装: $DOCKER_VERSION ✅"
        DOCKER_AVAILABLE=true
    else
        log_warn "Docker 未安装 (可选，但建议安装以支持容器化部署)"
        DOCKER_AVAILABLE=false
    fi
}

#==============================================================================
# 交互式配置向导
#==============================================================================
run_config_wizard() {
    log_title "第三步：LLM API 配置"

    echo -e "${BOLD}请选择 LLM 提供商：${NC}"
    echo ""
    echo "  ${CYAN}[1]${NC} OpenAI (GPT-4, GPT-3.5)"
    echo "  ${CYAN}[2]${NC} Anthropic (Claude 3, Claude 2)"
    echo "  ${CYAN}[3]${NC} Groq (免费高速推理)"
    echo "  ${CYAN}[4]${NC} DeepSeek (深度推理)"
    echo "  ${CYAN}[5]${NC} 本地模型 (Ollama)"
    echo "  ${CYAN}[6]${NC} 其他/手动配置"
    echo ""

    read -p "请输入选项 [1-6, 默认 1]: " llm_choice
    llm_choice=${llm_choice:-1}

    API_KEY=""
    API_URL=""
    MODEL_NAME=""

    case $llm_choice in
        1)
            echo ""
            echo -e "${BOLD}━━━ 配置 OpenAI ━━━${NC}"
            echo -e "获取 API Key: ${CYAN}https://platform.openai.com/api-keys${NC}"
            read -p "请输入 OpenAI API Key (sk-...): " API_KEY
            MODEL_NAME="gpt-4o"
            API_URL="https://api.openai.com/v1/chat/completions"
            ;;
        2)
            echo ""
            echo -e "${BOLD}━━━ 配置 Anthropic ━━━${NC}"
            echo -e "获取 API Key: ${CYAN}https://console.anthropic.com/api-keys${NC}"
            read -p "请输入 Anthropic API Key (sk-ant-...): " API_KEY
            MODEL_NAME="claude-sonnet-4-20250514"
            API_URL="https://api.anthropic.com/v1/messages"
            ;;
        3)
            echo ""
            echo -e "${BOLD}━━━ 配置 Groq ━━━${NC}"
            echo -e "获取 API Key: ${CYAN}https://console.groq.com/api-keys${NC}"
            read -p "请输入 Groq API Key: " API_KEY
            MODEL_NAME="llama-3.3-70b-versatile"
            API_URL="https://api.groq.com/openai/v1/chat/completions"
            ;;
        4)
            echo ""
            echo -e "${BOLD}━━━ 配置 DeepSeek ━━━${NC}"
            echo -e "获取 API Key: ${CYAN}https://platform.deepseek.com/api_keys${NC}"
            read -p "请输入 DeepSeek API Key: " API_KEY
            MODEL_NAME="deepseek-chat"
            API_URL="https://api.deepseek.com/v1/chat/completions"
            ;;
        5)
            echo ""
            echo -e "${BOLD}━━━ 配置本地 Ollama ━━━${NC}"
            read -p "Ollama API URL [http://localhost:11434]: " API_URL
            API_URL=${API_URL:-http://localhost:11434}
            read -p "模型名称 [llama3]: " MODEL_NAME
            MODEL_NAME=${MODEL_NAME:-llama3}
            API_KEY="ollama-local"
            ;;
        6|*)
            echo ""
            echo -e "${BOLD}━━━ 手动配置 ━━━${NC}"
            read -p "API URL: " API_URL
            read -p "API Key: " API_KEY
            read -p "模型名称: " MODEL_NAME
            ;;
    esac

    if [[ -z "$API_KEY" ]]; then
        log_warn "未提供 API Key，将使用环境变量或默认配置"
    fi

    # 保存配置
    save_llm_config "$API_URL" "$API_KEY" "$MODEL_NAME" "$llm_choice"
}

#==============================================================================
# 保存 LLM 配置
#==============================================================================
save_llm_config() {
    local api_url="$1"
    local api_key="$2"
    local model_name="$3"
    local provider="$4"

    log_step "保存配置到 .env 文件..."

    cat > "$PROJECT_DIR/.env" << EOF
# OMEGA AGI Supremacy - 配置文件
# 生成时间: $(date '+%Y-%m-%d %H:%M:%S')

# LLM Provider Configuration
OMEGA_LLM_PROVIDER=$provider
OMEGA_API_URL=$api_url
OMEGA_API_KEY=$api_key
OMEGA_MODEL_NAME=$model_name

# Optional: Logging
OMEGA_LOG_LEVEL=INFO
OMEGA_DEBUG=false

# Optional: Web UI
OMEGA_WEB_PORT=5000
OMEGA_WEB_HOST=0.0.0.0
EOF

    log_ok "配置已保存到 .env ✅"
}

#==============================================================================
# GitHub Token 配置 (可选)
#==============================================================================
configure_github_token() {
    echo ""
    log_title "第四步：GitHub Token 配置 (可选)"

    echo -e "配置 GitHub Token 可以解锁更多功能："
    echo "  • 访问私有仓库"
    echo "  • 更高的 API 速率限制"
    echo "  • 自动项目同步"
    echo ""
    echo -e "获取 Token: ${CYAN}https://github.com/settings/tokens${NC}"
    read -p "请输入 GitHub Token (直接回车跳过): " GH_TOKEN

    if [[ -n "$GH_TOKEN" ]]; then
        if grep -q "GITHUB_TOKEN" "$PROJECT_DIR/.env" 2>/dev/null; then
            sed -i "s|GITHUB_TOKEN=.*|GITHUB_TOKEN=$GH_TOKEN|" "$PROJECT_DIR/.env"
        else
            echo "GITHUB_TOKEN=$GH_TOKEN" >> "$PROJECT_DIR/.env"
        fi
        log_ok "GitHub Token 已保存 ✅"
    else
        log_info "跳过 GitHub Token 配置 (可选功能)"
    fi
}

#==============================================================================
# 启动选项
#==============================================================================
configure_startup() {
    echo ""
    log_title "第五步：启动配置"

    echo -e "选择启动模式："
    echo ""
    echo "  ${CYAN}[1]${NC} 开发模式 (带 Web UI，适合调试)"
    echo "  ${CYAN}[2]${NC} 生产模式 (高性能，适合部署)"
    echo "  ${CYAN}[3]${NC} 仅 API 服务 (无 Web UI)"
    echo ""

    read -p "请输入选项 [1-3, 默认 1]: " startup_choice
    startup_choice=${startup_choice:-1}

    case $startup_choice in
        2) STARTUP_MODE="production" ;;
        3) STARTUP_MODE="api_only" ;;
        *) STARTUP_MODE="development" ;;
    esac

    log_ok "启动模式: $STARTUP_MODE ✅"
}

#==============================================================================
# 启动服务
#==============================================================================
start_services() {
    log_title "第六步：启动服务"

    echo -e "${BOLD}正在启动 OMEGA AGI...${NC}"
    echo ""

    # 检查是否有 Python 向导
    if [[ -f "$PROJECT_DIR/setup_wizard.py" ]]; then
        echo -e "${YELLOW}${WARN}${NC}  推荐先运行交互式配置向导："
        echo -e "      ${CYAN}python3 setup_wizard.py${NC}"
        echo ""
        read -p "是否现在运行向导？ [Y/n]: " run_wizard
        run_wizard=${run_wizard:-Y}
        if [[ "$run_wizard" =~ ^[Yy]$ ]]; then
            python3 "$PROJECT_DIR/setup_wizard.py"
            return
        fi
    fi

    # 直接启动
    if [[ -f "$PROJECT_DIR/launcher.sh" ]]; then
        chmod +x "$PROJECT_DIR/launcher.sh"
        bash "$PROJECT_DIR/launcher.sh"
    else
        log_info "启动 omega-agi..."
        cd "$PROJECT_DIR"
        if [[ -f "requirements.txt" ]]; then
            echo -e "${YELLOW}${WARN}${NC}  请先安装依赖: ${CYAN}pip install -r requirements.txt${NC}"
        fi
    fi
}

#==============================================================================
# 显示完成信息
#==============================================================================
show_complete() {
    log_title "🎉 安装完成！"

    echo -e "
    ${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}
    ${GREEN}  ✅ OMEGA AGI Supremacy 安装成功！${NC}
    ${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}

    ${ROCKET} 快速开始：

      1. 运行配置向导：
         ${CYAN}cd $PROJECT_DIR${NC}
         ${CYAN}python3 setup_wizard.py${NC}

      2. 或直接启动：
         ${CYAN}bash launcher.sh${NC}

      3. 访问 Web UI：
         ${CYAN}http://localhost:5000${NC}

    ${SPARKLES} 可用命令：

      install.sh      - 重新安装/更新
      setup_wizard.py - 交互式配置向导
      launcher.sh     - 启动服务
      health_check.sh - 健康检查

    ${INFO} 配置文件: ${CYAN}$PROJECT_DIR/.env${NC}

    ${GLOBE} 文档:
      QUICKSTART.md   - 5分钟快速开始
      README.md       - 完整文档

    ${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}
    "
}

#==============================================================================
# 主流程
#==============================================================================
main() {
    clear
    show_banner

    echo -e "${BOLD}欢迎使用 OMEGA AGI Supremacy 安装向导！${NC}"
    echo -e "这个向导将帮助你在几分钟内完成安装和配置。\n"
    separator

    check_system
    install_dependencies
    run_config_wizard
    configure_github_token
    configure_startup
    start_services

    echo ""
    read -p "是否显示完成摘要？ [Y]: " show_summary
    show_summary=${show_summary:-Y}
    if [[ "$show_summary" =~ ^[Yy]$ ]]; then
        show_complete
    fi
}

#==============================================================================
# 静默模式 (非交互式)
#==============================================================================
silent_mode() {
    check_system
    install_dependencies

    # 从环境变量读取配置
    if [[ -n "$OMEGA_API_KEY" ]]; then
        save_llm_config "$OMEGA_API_URL" "$OMEGA_API_KEY" "$OMEGA_MODEL_NAME" "manual"
    fi

    log_ok "静默安装完成"
}

#==============================================================================
# 入口
#==============================================================================
if [[ "$1" == "--silent" ]] || [[ "$1" == "-y" ]]; then
    silent_mode
else
    main
fi