#!/bin/bash
# ============================================================
#  自动恢复脚本 - OM-E-GA 自愈系统
#  检测服务崩溃，自动重启，记录日志
# ============================================================

set -e

LOG_FILE="${LOG_FILE:-/tmp/omega_recovery.log}"
RECOVERY_LOCK="/tmp/omega_recovery.lock"
MAX_RESTART_ATTEMPTS="${MAX_RESTART_ATTEMPTS:-5}"
RESTART_COOLDOWN="${RESTART_COOLDOWN:-60}"
SERVICES=("omega_agi_core" "omega_self_healing")

# ---- 日志函数 ----
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG_FILE"
}

log_error() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] ERROR: $*" | tee -a "$LOG_FILE" >&2
}

# ---- 获取Docker服务状态 ----
get_container_status() {
    local name="$1"
    local status
    status=$(docker inspect --format='{{.State.Status}}' "$name" 2>/dev/null || echo "not_found")
    echo "$status"
}

# ---- 获取容器重启次数 ----
get_restart_count() {
    local name="$1"
    local count
    count=$(docker inspect --format='{{.RestartCount}}' "$name" 2>/dev/null || echo "999")
    echo "$count"
}

# ---- 获取容器健康状态 ----
get_health_status() {
    local name="$1"
    local health
    health=$(docker inspect --format='{{.State.Health.Status}}' "$name" 2>/dev/null || echo "none")
    echo "$health"
}

# ---- 检查端口可用性 ----
check_port() {
    local port="$1"
    if command -v ss &> /dev/null; then
        ss -tlnp 2>/dev/null | grep -q ":$port " && return 0
    elif command -v netstat &> /dev/null; then
        netstat -tlnp 2>/dev/null | grep -q ":$port " && return 0
    fi
    # fallback: use curl
    curl -sf "http://localhost:$port/health" &>/dev/null && return 0
    return 1
}

# ---- 获取容器运行时间 ----
get_uptime() {
    local name="$1"
    local started
    started=$(docker inspect --format='{{.State.StartedAt}}' "$name" 2>/dev/null || echo "")
    if [ -n "$started" ]; then
        local started_epoch
        started_epoch=$(date -d "$started" +%s 2>/dev/null || echo "0")
        local now_epoch
        now_epoch=$(date +%s)
        local uptime_seconds=$((now_epoch - started_epoch))
        echo "$uptime_seconds"
    else
        echo "0"
    fi
}

# ---- 检查进程内存使用 ----
check_process_memory() {
    local name="$1"
    local mem_bytes
    mem_bytes=$(docker stats "$name" --no-stream --format '{{.MemUsage}}' 2>/dev/null | awk '{print $1}' | sed 's/MiB\|GiB//' || echo "0")
    echo "${mem_bytes:-0}"
}

# ---- 获取容器CPU使用率 ----
get_cpu_usage() {
    local name="$1"
    local cpu
    cpu=$(docker stats "$name" --no-stream --format '{{.CPUPerc}}' 2>/dev/null | tr -d '%' || echo "0")
    echo "${cpu:-0}"
}

# ---- 恢复单个服务 ----
recover_service() {
    local name="$1"
    local method="${2:-restart}"
    
    log "==== 开始恢复服务: $name (方法: $method) ===="
    
    local current_status
    current_status=$(get_container_status "$name")
    log "当前状态: $current_status"
    
    local restart_count
    restart_count=$(get_restart_count "$name")
    log "重启次数: $restart_count"
    
    # 检查是否超过最大重启次数
    if [ "$restart_count" -ge "$MAX_RESTART_ATTEMPTS" ]; then
        log_error "服务 $name 重启次数已达上限 ($MAX_RESTART_ATTEMPTS)，跳过自动恢复"
        return 1
    fi
    
    case "$method" in
        restart)
            log "执行 docker restart $name ..."
            docker restart "$name" &>> "$LOG_FILE"
            ;;
        stop-start)
            log "执行 docker stop + start $name ..."
            docker stop "$name" &>> "$LOG_FILE"
            sleep 5
            docker start "$name" &>> "$LOG_FILE"
            ;;
        recreate)
            log "执行 docker-compose recreate $name ..."
            cd "$(dirname "$LOG_FILE")" 2>/dev/null || true
            docker-compose restart "$name" &>> "$LOG_FILE" || \
            (docker stop "$name" && docker rm "$name" && docker-compose up -d "$name") &>> "$LOG_FILE"
            ;;
        *)
            log_error "未知的恢复方法: $method"
            return 1
            ;;
    esac
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        log "✅ 服务 $name 恢复成功"
        
        # 等待服务启动
        log "等待服务启动 (15秒)..."
        sleep 15
        
        # 验证服务状态
        local new_status
        new_status=$(get_container_status "$name")
        if [ "$new_status" == "running" ]; then
            log "✅ 验证通过: $name 正在运行"
            
            # 发送恢复通知
            send_recovery_notification "$name" "SUCCESS" "服务已恢复"
            return 0
        else
            log_error "验证失败: $name 状态为 $new_status"
            send_recovery_notification "$name" "FAILED" "服务恢复后状态异常"
            return 1
        fi
    else
        log_error "❌ 服务 $name 恢复失败 (exit code: $exit_code)"
        send_recovery_notification "$name" "ERROR" "恢复操作失败"
        return 1
    fi
}

# ---- 发送恢复通知到EvoMap ----
send_recovery_notification() {
    local service="$1"
    local status="$2"
    local message="$3"
    
    # 尝试发送到EvoMap (如果API可用)
    if [ -n "${EVOMAP_API_URL:-}" ] && [ -n "${EVOMAP_API_KEY:-}" ]; then
        curl -sf -X POST "$EVOMAP_API_URL/notify" \
            -H "Authorization: Bearer $EVOMAP_API_KEY" \
            -H "Content-Type: application/json" \
            -d "{
                \"service\": \"$service\",
                \"status\": \"$status\",
                \"message\": \"$message\",
                \"timestamp\": \"$(date -Iseconds)\",
                \"host\": \"$(hostname)\"
            }" &>> "$LOG_FILE" || true
    fi
}

# ---- 完整系统诊断 ----
run_full_diagnosis() {
    log "==== 开始完整系统诊断 ===="
    
    local issues=()
    local all_ok=true
    
    # 检查所有服务
    for svc in "${SERVICES[@]}"; do
        local status health restart_count uptime cpu mem
        
        status=$(get_container_status "$svc")
        health=$(get_health_status "$svc")
        restart_count=$(get_restart_count "$svc")
        uptime=$(get_uptime "$svc")
        cpu=$(get_cpu_usage "$svc")
        mem=$(check_process_memory "$svc")
        
        log "[诊断] $svc | status=$status | health=$health | restarts=$restart_count | uptime=${uptime}s | cpu=${cpu}% | mem=${mem}MiB"
        
        if [ "$status" != "running" ]; then
            all_ok=false
            issues+=("服务 $svc 未运行 (状态: $status)")
        fi
        
        if [ "$health" == "unhealthy" ]; then
            all_ok=false
            issues+=("服务 $svc 健康检查失败")
        fi
        
        if [ "$restart_count" -ge 3 ]; then
            issues+=("服务 $svc 重启频繁 (${restart_count}次)")
        fi
        
        # 如果服务运行超过1小时且CPU持续>90%，可能是资源问题
        if [ "$uptime" -gt 3600 ] && [ "$(echo "$cpu > 90" | bc 2>/dev/null || echo 0)" -eq 1 ]; then
            issues+=("服务 $svc CPU持续过高 (${cpu}%)")
        fi
    done
    
    # 检查磁盘空间
    local disk_usage
    disk_usage=$(df / | tail -1 | awk '{print $5}' | tr -d '%')
    if [ "$disk_usage" -gt 90 ]; then
        all_ok=false
        issues+=("磁盘空间不足 (${disk_usage}%)")
    fi
    
    # 检查内存
    local mem_available
    mem_available=$(free -m | awk '/Mem:/ {print $7}')
    if [ "$mem_available" -lt 500 ]; then
        issues+=("系统可用内存不足 (${mem_available}MB)")
    fi
    
    # 输出诊断结果
    if [ "$all_ok" = true ]; then
        log "✅ 系统诊断通过: 所有服务正常"
        return 0
    else
        log_error "⚠️ 系统诊断发现问题:"
        for issue in "${issues[@]}"; do
            log_error "  - $issue"
        done
        return 1
    fi
}

# ---- 锁定防止重复运行 ----
acquire_lock() {
    local lock_fd=200
    eval "exec $lock_fd>$RECOVERY_LOCK"
    
    if ! flock -n $lock_fd; then
        log_error "另一个恢复进程正在运行，退出"
        exit 0
    fi
    
    echo $$ > "$RECOVERY_LOCK"
}

release_lock() {
    rm -f "$RECOVERY_LOCK"
}

# ---- 主恢复流程 ----
main() {
    log ""
    log "================================================"
    log "  🔧 OMEGA AGI 自动恢复系统启动"
    log "  $(date)"
    log "================================================"
    
    acquire_lock
    
    trap release_lock EXIT
    
    local needs_recovery=false
    
    # 依次检查每个服务
    for svc in "${SERVICES[@]}"; do
        local status health
        
        status=$(get_container_status "$svc")
        health=$(get_health_status "$svc")
        
        log "检查服务: $svc (status=$status, health=$health)"
        
        # 判断是否需要恢复
        local should_recover=false
        
        if [ "$status" != "running" ]; then
            log "⚠️ $svc 未运行，需要恢复"
            should_recover=true
        elif [ "$health" == "unhealthy" ]; then
            log "⚠️ $svc 健康检查失败，需要恢复"
            should_recover=true
        elif [ "$health" == "starting" ] && [ "$(get_uptime "$svc")" -gt 300 ]; then
            # 健康检查启动超过5分钟仍未通过
            log "⚠️ $svc 启动超时，需要恢复"
            should_recover=true
        fi
        
        if [ "$should_recover" = true ]; then
            needs_recovery=true
            
            # 根据重启次数选择恢复策略
            local restart_count
            restart_count=$(get_restart_count "$svc")
            
            local method="restart"
            if [ "$restart_count" -ge 3 ]; then
                method="recreate"
                log "高频重启，采用强力恢复策略: recreate"
            elif [ "$restart_count" -ge 1 ]; then
                method="stop-start"
                log "检测到重启历史，采用中等恢复策略: stop-start"
            fi
            
            if ! recover_service "$svc" "$method"; then
                log_error "恢复服务 $svc 失败"
            fi
        fi
    done
    
    # 无论是否需要恢复，都运行完整诊断
    if ! run_full_diagnosis; then
        log "系统诊断发现问题，生成详细报告..."
        generate_diagnosis_report
    fi
    
    log ""
    log "================================================"
    log "  ✅ 自动恢复流程完成"
    log "================================================"
}

# ---- 生成诊断报告 ----
generate_diagnosis_report() {
    local report_file="/tmp/omega_diagnosis_$(date +%Y%m%d_%H%M%S).json"
    
    {
        echo "{"
        echo "  \"timestamp\": \"$(date -Iseconds)\","
        echo "  \"hostname\": \"$(hostname)\","
        echo "  \"services\": {"
        local first=true
        for svc in "${SERVICES[@]}"; do
            $first && echo "    \"$svc\": {" || echo "    ,\"$svc\": {"
            first=false
            echo "      \"status\": \"$(get_container_status "$svc")\","
            echo "      \"health\": \"$(get_health_status "$svc")\","
            echo "      \"restart_count\": $(get_restart_count "$svc"),"
            echo "      \"uptime_seconds\": $(get_uptime "$svc"),"
            echo "      \"cpu_percent\": \"$(get_cpu_usage "$svc")%\","
            echo "      \"memory_mib\": \"$(check_process_memory "$svc")\""
            echo -n "    }"
        done
        echo "  },"
        echo "  \"system\": {"
        echo "    \"disk_usage_percent\": $(df / | tail -1 | awk '{print $5}' | tr -d '%'),"
        echo "    \"memory_available_mb\": $(free -m | awk '/Mem:/ {print $7}'),"
        echo "    \"load_average\": $(uptime | awk -F'load average:' '{print $2}')"
        echo "  }"
        echo "}"
    } > "$report_file"
    
    log "诊断报告已保存: $report_file"
}

# ---- CLI接口 ----
case "${1:-run}" in
    run)
        main
        ;;
    diagnose)
        run_full_diagnosis
        ;;
    recover)
        [ -z "$2" ] && { log_error "用法: $0 recover <服务名>"; exit 1; }
        acquire_lock
        recover_service "$2" "${3:-restart}"
        release_lock
        ;;
    report)
        generate_diagnosis_report
        cat "$report_file"
        ;;
    status)
        for svc in "${SERVICES[@]}"; do
            echo "$svc: $(get_container_status "$svc") | $(get_health_status "$svc") | restarts=$(get_restart_count "$svc")"
        done
        ;;
    *)
        echo "用法: $0 {run|diagnose|recover|report|status}"
        echo "  run       - 运行自动恢复流程 (默认)"
        echo "  diagnose  - 仅运行诊断"
        echo "  recover   - 恢复指定服务 [方法]"
        echo "  report    - 生成诊断报告"
        echo "  status    - 查看服务状态"
        exit 1
        ;;
esac