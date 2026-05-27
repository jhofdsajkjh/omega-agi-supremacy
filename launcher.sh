#!/bin/bash
#==============================================================================
# OMEGA AGI Supremacy - 智能服务启动器
# 检查配置完整性，自动启动所有服务，显示状态面板
#==============================================================================
set -e

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

ROCKET="🚀"
CHECK="✅"
CROSS="❌"
WARN="⚠️"
INFO="ℹ️"
ROBOT="🤖"
SPARKLES="✨"
GLOBE="🌐"
CIRCLE="🔵"
DIAMOND="🔷"

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# 加载 .env
load_env() {
    if [[ -f "$PROJECT_DIR/.env" ]]; then
        export $(grep -v '^#' "$PROJECT_DIR/.env" | grep -v '^$' | xargs)
    fi
}

# 默认值
OMEGA_WEB_PORT=${OMEGA_WEB_PORT:-5000}
OMEGA_WEB_HOST=${OMEGA_WEB_HOST:-0.0.0.0}
OMEGA_LOG_LEVEL=${OMEGA_LOG_LEVEL:-INFO}

#==============================================================================
# Banner
#==============================================================================
show_banner() {
    cat << 'EOF'

      ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
      ┃    ██╗  ██╗ █████╗  ██████╗██╗  ██╗     ┃
      ┃    ██║  ██║██╔══██╗██╔════╝██║ ██╔╝     ┃
      ┃    ███████║███████║██║     █████╔╝      ┃
      ┃    ██╔══██║██╔══██║██║     ██╔═██╗      ┃
      ┃    ██║  ██║██║  ██║╚██████╗██║  ██╗     ┃
      ┃    ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝     ┃
      ┃         A G I   S U P R E M A C Y         ┃
      ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛

                    🤖  Launcher v1.0  🤖

EOF
}

#==============================================================================
# 检查函数
#==============================================================================
check_config() {
    local missing=0

    echo -e "${BOLD}${CYAN}━━━ 配置检查 ━━━${NC}\n"

    # .env
    if [[ -f "$PROJECT_DIR/.env" ]]; then
        echo -e "  ${CHECK} 配置文件存在: .env"
    else
        echo -e "  ${CROSS} 配置文件缺失: .env"
        missing=$((missing + 1))
    fi

    # API Key
    if [[ -n "$OMEGA_API_KEY" ]]; then
        masked="${OMEGA_API_KEY:0:4}****${OMEGA_API_KEY: -4}"
        echo -e "  ${CHECK} API Key: $masked"
    else
        echo -e "  ${WARN} API Key 未配置 (需要 .env 或环境变量)"
    fi

    # Model
    if [[ -n "$OMEGA_MODEL_NAME" ]]; then
        echo -e "  ${CHECK} 模型: $OMEGA_MODEL_NAME"
    else
        echo -e "  ${WARN} 模型未配置"
    fi

    echo ""
    return $missing
}

check_dependencies() {
    echo -e "${BOLD}${CYAN}━━━ 依赖检查 ━━━${NC}\n"

    local all_ok=true

    # Python
    if command -v python3 &> /dev/null; then
        PYTHON_VERSION=$(python3 --version 2>&1 | awk '{print $2}')
        echo -e "  ${CHECK} Python: $PYTHON_VERSION"
    else
        echo -e "  ${CROSS} Python 未安装"
        all_ok=false
    fi

    # pip
    if python3 -m pip --version &> /dev/null; then
        echo -e "  ${CHECK} pip 可用"
    else
        echo -e "  ${WARN} pip 不可用"
    fi

    # 关键依赖
    for dep in openai requests flask; do
        if python3 -c "import $dep" 2>/dev/null; then
            echo -e "  ${CHECK} $dep 已安装"
        else
            echo -e "  ${CROSS} $dep 未安装"
            all_ok=false
        fi
    done

    echo ""
    $all_ok
}

check_port() {
    echo -e "${BOLD}${CYAN}━━━ 端口检查 ━━━${NC}\n"

    if command -v lsof &> /dev/null; then
        if lsof -i :$OMEGA_WEB_PORT &> /dev/null; then
            echo -e "  ${WARN} 端口 $OMEGA_WEB_PORT 已被占用"
            echo -e "      运行: ${CYAN}lsof -i :$OMEGA_WEB_PORT${NC} 查看占用进程"
            return 1
        else
            echo -e "  ${CHECK} 端口 $OMEGA_WEB_PORT 可用"
        fi
    else
        # 备用方案
        if python3 -c "
            import socket
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            try:
                s.bind(('0.0.0.0', $OMEGA_WEB_PORT))
                s.close()
                exit(0)
            except:
                exit(1)
        " 2>/dev/null; then
            echo -e "  ${CHECK} 端口 $OMEGA_WEB_PORT 可用"
        else
            echo -e "  ${WARN} 端口 $OMEGA_WEB_PORT 已被占用"
            return 1
        fi
    fi

    echo ""
    return 0
}

#==============================================================================
# 启动 Web UI
#==============================================================================
start_web_ui() {
    echo -e "${BOLD}${CYAN}━━━ 启动 Web UI ━━━${NC}\n"

    local web_ui_main="$PROJECT_DIR/omega-agi/web_ui/app.py"
    local web_ui_alt="$PROJECT_DIR/web_ui/app.py"

    if [[ -f "$web_ui_main" ]]; then
        echo -e "  ${INFO} 启动 Web UI: $web_ui_main"
        cd "$PROJECT_DIR"
        nohup python3 "$web_ui_main" > logs/web_ui.log 2>&1 &
        WEB_UI_PID=$!
        echo -e "  ${CHECK} Web UI 已启动 (PID: $WEB_UI_PID)"
        echo $WEB_UI_PID > .web_ui.pid
        return 0
    elif [[ -f "$web_ui_alt" ]]; then
        echo -e "  ${INFO} 启动 Web UI: $web_ui_alt"
        cd "$PROJECT_DIR"
        nohup python3 "$web_ui_alt" > logs/web_ui.log 2>&1 &
        WEB_UI_PID=$!
        echo -e "  ${CHECK} Web UI 已启动 (PID: $WEB_UI_PID)"
        echo $WEB_UI_PID > .web_ui.pid
        return 0
    else
        echo -e "  ${CROSS} 未找到 Web UI 入口"
        echo -e "  ${INFO} 尝试查找..."
        find "$PROJECT_DIR" -name "app.py" -o -name "web.py" 2>/dev/null | head -5
        return 1
    fi
}

#==============================================================================
# 启动 Core Agent
#==============================================================================
start_core_agent() {
    echo -e "${BOLD}${CYAN}━━━ 启动 Core Agent ━━━${NC}\n"

    local agent_main="$PROJECT_DIR/omega-agi/agent.py"
    local agent_alt="$PROJECT_DIR/omega-agi/__main__.py"

    if [[ -f "$agent_main" ]]; then
        echo -e "  ${INFO} 启动 Core Agent: $agent_main"
        cd "$PROJECT_DIR"
        nohup python3 "$agent_main" > logs/agent.log 2>&1 &
        AGENT_PID=$!
        echo -e "  ${CHECK} Core Agent 已启动 (PID: $AGENT_PID)"
        echo $AGENT_PID > .agent.pid
    elif [[ -f "$agent_alt" ]]; then
        echo -e "  ${INFO} 启动 Core Agent: $agent_alt"
        cd "$PROJECT_DIR"
        nohup python3 -m omega-agi > logs/agent.log 2>&1 &
        AGENT_PID=$!
        echo -e "  ${CHECK} Core Agent 已启动 (PID: $AGENT_PID)"
        echo $AGENT_PID > .agent.pid
    else
        echo -e "  ${WARN} 未找到 Core Agent 入口，跳过"
    fi

    echo ""
}

#==============================================================================
# 启动监控
#==============================================================================
start_monitor() {
    echo -e "${BOLD}${CYAN}━━━ 启动健康监控 ━━━${NC}\n"

    local monitor_script="$PROJECT_DIR/scripts/health_check.sh"

    if [[ -f "$monitor_script" ]]; then
        chmod +x "$monitor_script"
        nohup bash "$monitor_script" > logs/monitor.log 2>&1 &
        echo -e "  ${CHECK} 健康监控已启动"
    else
        echo -e "  ${INFO} 健康监控脚本未找到，跳过"
    fi

    echo ""
}

#==============================================================================
# 显示状态面板
#==============================================================================
show_status_panel() {
    echo ""
    echo -e "${BOLD}${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}${GREEN}  ✅ OMEGA AGI 已成功启动！${NC}"
    echo -e "${BOLD}${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""

    echo -e "  ${GLOBE} 访问地址:"
    echo -e "     ${CYAN}http://localhost:${OMEGA_WEB_PORT}${NC}"
    echo -e "     ${CYAN}http://0.0.0.0:${OMEGA_WEB_PORT}${NC}"
    echo ""

    echo -e "  ${SPARKLES} 服务状态:"
    if [[ -f .web_ui.pid ]]; then
        WEB_PID=$(cat .web_ui.pid)
        if kill -0 $WEB_PID 2>/dev/null; then
            echo -e "     ${CHECK} Web UI:    运行中 (PID $WEB_PID)"
        else
            echo -e "     ${CROSS} Web UI:    已停止"
        fi
    fi

    if [[ -f .agent.pid ]]; then
        AGENT_PID=$(cat .agent.pid)
        if kill -0 $AGENT_PID 2>/dev/null; then
            echo -e "     ${CHECK} Core Agent: 运行中 (PID $AGENT_PID)"
        else
            echo -e "     ${CROSS} Core Agent: 已停止"
        fi
    fi

    echo ""
    echo -e "  ${INFO} 日志位置: $PROJECT_DIR/logs/"
    echo -e "  ${INFO} 停止服务: bash launcher.sh --stop"
    echo -e "  ${INFO} 查看日志: tail -f logs/*.log"
    echo ""

    echo -e "${BOLD}${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

#==============================================================================
# 停止服务
#==============================================================================
stop_services() {
    echo -e "\n${BOLD}${YELLOW}━━━ 停止服务 ━━━${NC}\n"

    for pidfile in .web_ui.pid .agent.pid; do
        if [[ -f "$pidfile" ]]; then
            PID=$(cat "$pidfile")
            if kill -0 $PID 2>/dev/null; then
                echo -e "  ${INFO} 停止进程 $PID..."
                kill $PID 2>/dev/null || true
                sleep 1
                kill -9 $PID 2>/dev/null || true
                echo -e "  ${CHECK} 进程 $PID 已停止"
            fi
            rm -f "$pidfile"
        fi
    done

    echo -e "\n  ${CHECK} 所有服务已停止"
}

#==============================================================================
# 查看状态
#==============================================================================
show_status() {
    echo -e "${BOLD}${CYAN}━━━ 服务状态 ━━━${NC}\n"

    for pidfile in .web_ui.pid .agent.pid; do
        name=$(echo $pidfile | sed 's/.pid//' | sed 's/./ &/' | tr '[:lower:]' '[:upper:]')
        if [[ -f "$pidfile" ]]; then
            PID=$(cat "$pidfile")
            if kill -0 $PID 2>/dev/null; then
                echo -e "  ${CHECK} ${name^^}: 运行中 (PID $PID)"
            else
                echo -e "  ${CROSS} ${name^^}: 已停止"
            fi
        else
            echo -e "  ${WARN} ${name^^}: 未启动"
        fi
    done

    echo ""
}

#==============================================================================
# 完整健康检查
#==============================================================================
full_health_check() {
    echo -e "${BOLD}${CYAN}━━━ 健康检查 ━━━${NC}\n"

    local ok=true

    # 进程检查
    if [[ -f .web_ui.pid ]]; then
        PID=$(cat .web_ui.pid)
        if kill -0 $PID 2>/dev/null; then
            echo -e "  ${CHECK} Web UI 进程:  正常"
        else
            echo -e "  ${CROSS} Web UI 进程:  异常"
            ok=false
        fi
    fi

    # 端口检查
    if python3 -c "
        import socket
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            s.bind(('0.0.0.0', $OMEGA_WEB_PORT))
            s.close()
            exit(1)
        except:
            exit(0)
    " 2>/dev/null; then
        echo -e "  ${CHECK} Web UI 端口:  监听中 (:$OMEGA_WEB_PORT)"
    else
        echo -e "  ${CHECK} Web UI 端口:  正常 (:$OMEGA_WEB_PORT)"
    fi

    # 日志检查
    if [[ -f logs/web_ui.log ]] && [[ -s logs/web_ui.log ]]; then
        last_line=$(tail -1 logs/web_ui.log)
        if echo "$last_line" | grep -qi "error\|exception\|traceback"; then
            echo -e "  ${WARN} Web UI 日志:  有警告"
        else
            echo -e "  ${CHECK} Web UI 日志:  正常"
        fi
    fi

    echo ""
    $ok
}

#==============================================================================
# 主流程
#==============================================================================
main() {
    load_env

    clear
    show_banner

    # 创建日志目录
    mkdir -p "$PROJECT_DIR/logs"

    echo -e "${BOLD}正在启动 OMEGA AGI Supremacy...${NC}\n"

    # 预检查
    check_config
    check_dependencies || true
    check_port || true

    echo -e "${BOLD}${YELLOW}━━━ 启动服务 ━━━${NC}\n"

    start_web_ui
    start_core_agent
    start_monitor

    # 等待服务启动
    sleep 2

    show_status_panel
    show_status
}

#==============================================================================
# 入口
#==============================================================================
case "${1:-}" in
    --stop|-s)
        load_env
        stop_services
        ;;
    --status|--check|-c)
        load_env
        show_status
        full_health_check
        ;;
    --restart)
        load_env
        stop_services
        sleep 2
        main
        ;;
    --help|-h)
        echo -e "${BOLD}OMEGA AGI 启动器${NC}"
        echo ""
        echo "用法: launcher.sh [选项]"
        echo ""
        echo "  无参数     启动所有服务"
        echo "  --stop     停止所有服务"
        echo "  --status   查看服务状态"
        echo "  --restart  重启所有服务"
        echo "  --help     显示帮助"
        ;;
    *)
        main
        ;;
esac