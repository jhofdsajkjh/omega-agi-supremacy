#!/usr/bin/env python3
# ============================================================
#  OMEGA AGI 自诊断系统
#  读取所有模块状态，计算APEX ΔG指标，生成健康报告
# ============================================================

import os
import sys
import json
import time
import socket
import platform
import subprocess
import traceback
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Any, Tuple

# ---- 配置 ----
LOG_FILE = os.environ.get("OMEGA_LOG_FILE", "/tmp/omega_diagnosis.log")
REPORT_FILE = os.environ.get("OMEGA_REPORT_FILE", "/tmp/omega_health_report.json")
EVOMAP_API_URL = os.environ.get("EVOMAP_API_URL", "http://localhost:9000/api/health")
EVOMAP_API_KEY = os.environ.get("EVOMAP_API_KEY", "omega-health-key")

# ---- 日志模块 ----
class Logger:
    def __init__(self, log_file: str = LOG_FILE):
        self.log_file = log_file
        self._ensure_log_dir()

    def _ensure_log_dir(self):
        path = Path(self.log_file)
        path.parent.mkdir(parents=True, exist_ok=True)

    def _write(self, level: str, msg: str):
        ts = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
        entry = f"[{ts}] {level}: {msg}\n"
        print(entry, end="")
        try:
            with open(self.log_file, "a") as f:
                f.write(entry)
        except Exception:
            pass

    def info(self, msg: str):   self._write("INFO", msg)
    def warn(self, msg: str):   self._write("WARN", msg)
    def error(self, msg: str):  self._write("ERROR", msg)
    def debug(self, msg: str):  self._write("DEBUG", msg)

logger = Logger()


# ---- 系统信息收集 ----
def get_system_info() -> Dict[str, Any]:
    """获取基础系统信息"""
    info = {
        "hostname": socket.gethostname(),
        "platform": platform.system(),
        "platform_release": platform.release(),
        "platform_version": platform.version(),
        "architecture": platform.machine(),
        "processor": platform.processor(),
        "python_version": platform.python_version(),
        "timestamp_unix": int(time.time()),
        "timestamp_iso": datetime.now(timezone.utc).isoformat(),
    }

    # 加载平均值
    try:
        with open("/proc/loadavg", "r") as f:
            load = f.read().split()
            info["load_avg_1m"] = float(load[0])
            info["load_avg_5m"] = float(load[1])
            info["load_avg_15m"] = float(load[2])
    except Exception:
        info["load_avg_1m"] = info["load_avg_5m"] = info["load_avg_15m"] = 0.0

    # 内存信息
    try:
        lines = open("/proc/meminfo").readlines()
        mem = {}
        for line in lines:
            parts = line.split()
            if len(parts) >= 2:
                key = parts[0].rstrip(":")
                value = int(parts[1]) * 1024  # KB to bytes
                mem[key] = value
        info["memory_total"] = mem.get("MemTotal", 0)
        info["memory_available"] = mem.get("MemAvailable", mem.get("MemFree", 0))
        info["memory_used"] = info["memory_total"] - info["memory_available"]
        info["memory_percent"] = (info["memory_used"] / info["memory_total"] * 100) if info["memory_total"] else 0
    except Exception:
        info["memory_total"] = info["memory_available"] = info["memory_used"] = 0
        info["memory_percent"] = 0.0

    # 磁盘信息
    try:
        result = subprocess.run(["df", "-B1", "/"], capture_output=True, text=True, timeout=5)
        lines = result.stdout.strip().split("\n")
        if len(lines) >= 2:
            parts = lines[1].split()
            info["disk_total"] = int(parts[1])
            info["disk_used"] = int(parts[2])
            info["disk_available"] = int(parts[3])
            info["disk_percent"] = float(parts[4].rstrip("%"))
    except Exception:
        info["disk_total"] = info["disk_used"] = info["disk_available"] = 0
        info["disk_percent"] = 0.0

    # CPU核心数
    try:
        info["cpu_count"] = os.cpu_count()
        info["cpu_count_logical"] = os.cpu_count(logical=False) or os.cpu_count()
    except Exception:
        info["cpu_count"] = info["cpu_count_logical"] = 0

    return info


# ---- Docker容器检查 ----
def check_docker_container(name: str) -> Dict[str, Any]:
    """检查单个Docker容器状态"""
    result = {
        "name": name,
        "exists": False,
        "status": "not_found",
        "health": "none",
        "restart_count": 0,
        "uptime_seconds": 0,
        "cpu_percent": 0.0,
        "memory_mb": 0.0,
        "ports": [],
    }

    try:
        # 检查容器是否存在
        inspect = subprocess.run(
            ["docker", "inspect", "--format={{json .State}}", name],
            capture_output=True, text=True, timeout=5
        )
        if inspect.returncode != 0:
            return result

        state = json.loads(inspect.stdout.strip())
        result["exists"] = True
        result["status"] = state.get("Status", "unknown")
        result["restart_count"] = state.get("RestartCount", 0)
        result["uptime_seconds"] = state.get("StartedAt", "")

        # 计算uptime
        try:
            from datetime import datetime
            started = state.get("StartedAt", "")
            if started:
                # docker format: 2024-01-01T00:00:00Z
                dt = datetime.fromisoformat(started.replace("Z", "+00:00"))
                result["uptime_seconds"] = int((datetime.now(timezone.utc) - dt).total_seconds())
        except Exception:
            result["uptime_seconds"] = 0

        # 健康状态
        health = state.get("Health", {})
        result["health"] = health.get("Status", "none") if health else "none"

        # 资源使用
        try:
            stats = subprocess.run(
                ["docker", "stats", "--no-stream", "--format={{.CPUPerc}}|{{.MemUsage}}", name],
                capture_output=True, text=True, timeout=10
            )
            if stats.returncode == 0:
                line = stats.stdout.strip()
                parts = line.split("|")
                if len(parts) >= 2:
                    cpu_str = parts[0].strip().rstrip("%")
                    result["cpu_percent"] = float(cpu_str) if cpu_str else 0.0
                    mem_str = parts[1].strip().split("/")[0].strip()
                    for unit in ["MiB", "GiB", "KiB"]:
                        if unit in mem_str:
                            val = float(mem_str.replace(unit, "").strip())
                            if unit == "GiB":
                                result["memory_mb"] = val * 1024
                            elif unit == "KiB":
                                result["memory_mb"] = val / 1024
                            else:
                                result["memory_mb"] = val
                            break
        except Exception:
            pass

    except subprocess.TimeoutExpired:
        logger.warn(f"Docker inspect timeout for {name}")
    except Exception as e:
        logger.error(f"Docker check error for {name}: {e}")

    return result


def check_all_docker_containers() -> List[Dict[str, Any]]:
    """检查所有相关Docker容器"""
    known_containers = [
        "omega_agi_core",
        "omega_self_healing",
        "omega_pipeline",
    ]

    results = []
    for name in known_containers:
        results.append(check_docker_container(name))

    return results


# ---- 进程检查 ----
def check_processes() -> List[Dict[str, Any]]:
    """检查关键进程状态"""
    important_processes = [
        "python", "java", "node", "docker", "rustc"
    ]

    results = []
    try:
        lines = open("/proc/[0-9]*/status").readlines() if os.path.exists("/proc") else []
        # 简化为 subprocess check
        ps_result = subprocess.run(
            ["ps", "aux"], capture_output=True, text=True, timeout=5
        )
        for line in ps_result.stdout.split("\n"):
            for proc in important_processes:
                if proc in line and "grep" not in line:
                    parts = line.split()
                    if len(parts) >= 11:
                        results.append({
                            "command": " ".join(parts[10:]),
                            "pid": int(parts[1]),
                            "cpu": float(parts[2]),
                            "mem": float(parts[3]),
                        })
                    break
    except Exception as e:
        logger.error(f"Process check error: {e}")

    return results[:20]  # 最多返回20个


# ---- 网络检查 ----
def check_network_connectivity() -> Dict[str, Any]:
    """检查网络连接状态"""
    checks = {
        "dns": False,
        "http": False,
        "evomap": False,
    }

    # DNS检查
    try:
        socket.gethostbyname("google.com")
        checks["dns"] = True
    except Exception:
        pass

    # HTTP检查
    try:
        result = subprocess.run(
            ["curl", "-sf", "--max-time", "5", "https://www.google.com"],
            capture_output=True, timeout=10
        )
        checks["http"] = result.returncode == 0
    except Exception:
        pass

    # EvoMap连接
    try:
        result = subprocess.run(
            ["curl", "-sf", "--max-time", "3", EVOMAP_API_URL],
            capture_output=True, timeout=5
        )
        checks["evomap"] = result.returncode == 0
    except Exception:
        pass

    return checks


# ---- APEX ΔG指标计算 ----
def calculate_apex_dg(system_info: Dict, containers: List[Dict]) -> Dict[str, float]:
    """
    计算APEX ΔG (吉布斯自由能变化) 指标
    ΔG < 0 表示系统向有序化方向发展（健康）
    ΔG > 0 表示系统向混沌化方向发展（风险）
    
    参考值:
    - ΔG < -50: 极低熵，系统运行完美
    - ΔG = -20 ~ -50: 低熵，健康
    - ΔG = -10 ~ -20: 轻微混乱，需要关注
    - ΔG = 0 ~ -10: 警戒状态
    - ΔG > 0: 系统失序，需要立即干预
    """

    G_components = {}

    # 1. 资源压力 ΔG (基于CPU、内存、磁盘)
    cpu_load = system_info.get("load_avg_1m", 0)
    cpu_cores = max(system_info.get("cpu_count_logical", 1), 1)
    normalized_cpu = cpu_load / cpu_cores if cpu_cores else 0

    mem_percent = system_info.get("memory_percent", 0)
    disk_percent = system_info.get("disk_percent", 0)

    G_resource = (
        normalized_cpu * 0.4 +
        (mem_percent / 100) * 0.35 +
        (disk_percent / 100) * 0.25
    ) * 100

    G_components["resource_pressure"] = G_resource

    # 2. 服务稳定性 ΔG (基于容器健康状态)
    stability_scores = []
    for c in containers:
        if not c["exists"]:
            stability_scores.append(0.0)
        elif c["status"] == "running":
            if c["health"] == "healthy":
                stability_scores.append(1.0)
            elif c["health"] == "starting":
                stability_scores.append(0.6)
            elif c["health"] == "unhealthy":
                stability_scores.append(0.1)
            else:
                stability_scores.append(0.8)
        else:
            stability_scores.append(0.0)

    avg_stability = sum(stability_scores) / len(stability_scores) if stability_scores else 0
    G_stability = (1.0 - avg_stability) * 100
    G_components["service_stability"] = G_stability

    # 3. 故障传播 ΔG (基于重启次数和级联失败)
    restart_penalty = 0
    for c in containers:
        rc = c.get("restart_count", 0)
        if rc >= 5:
            restart_penalty += 30
        elif rc >= 3:
            restart_penalty += 15
        elif rc >= 1:
            restart_penalty += 5

    G_components["restart_penalty"] = restart_penalty

    # 4. 网络连通性 ΔG
    net = system_info.get("network_checks", {})
    if not net.get("dns", True):
        G_components["network_dns_failure"] = 20
    if not net.get("http", True):
        G_components["network_http_failure"] = 10

    # 总ΔG
    total_dG = sum(G_components.values())

    # 分类
    if total_dG < -50:
        category = "EXCELLENT"
    elif total_dG < -20:
        category = "HEALTHY"
    elif total_dG < -10:
        category = "CAUTION"
    elif total_dG < 0:
        category = "WARNING"
    elif total_dG < 20:
        category = "CRITICAL"
    else:
        category = "COLLAPSE"

    return {
        "total_dG": total_dG,
        "category": category,
        "components": G_components,
        "resource_pressure": G_components.get("resource_pressure", 0),
        "service_stability": G_components.get("service_stability", 0),
    }


# ---- 健康评分 ----
def calculate_health_score(dG: Dict, containers: List[Dict], network: Dict) -> Tuple[float, str]:
    """
    计算综合健康评分 (0-100)
    """
    score = 100.0

    # ΔG惩罚
    dG_total = dG["total_dG"]
    if dG_total > 0:
        score -= min(dG_total * 0.5, 40)
    elif dG_total < -20:
        score += min(abs(dG_total) * 0.1, 5)  # 奖励超健康状态

    # 容器可用性
    available = sum(1 for c in containers if c["exists"] and c["status"] == "running")
    total = len(containers)
    if total > 0:
        container_score = (available / total) * 30
        score = score * 0.7 + container_score

    # 网络连通性
    if not network.get("http", True):
        score -= 10
    if not network.get("evomap", True):
        score -= 5

    score = max(0.0, min(100.0, score))

    level = "EXCELLENT" if score >= 95 else \
            "HEALTHY" if score >= 80 else \
            "CAUTION" if score >= 60 else \
            "WARNING" if score >= 40 else \
            "CRITICAL" if score >= 20 else "DOWN"

    return score, level


# ---- 模块依赖检查 ----
def check_module_dependencies() -> Dict[str, Any]:
    """检查关键模块依赖"""
    deps = {}

    # 检查关键目录
    paths = [
        "/root/omega-agi-supremacy/omega-agi",
        "/root/omega-agi-supremacy/omega_pipeline",
        "/root/omega-agi-supremacy/scripts",
    ]
    for p in paths:
        deps[p] = os.path.exists(p)

    # 检查关键文件
    files = [
        "/root/omega-agi-supremacy/deploy.sh",
        "/root/omega-agi-supremacy/omega_pipeline/supremacy_autopilot.py",
    ]
    for f in files:
        deps[f"file:{f}"] = os.path.isfile(f)

    # 检查Python模块
    try:
        import requests
        deps["module:requests"] = True
    except ImportError:
        deps["module:requests"] = False

    try:
        import aiohttp
        deps["module:aiohttp"] = True
    except ImportError:
        deps["module:aiohttp"] = False

    return deps


# ---- 健康报告生成 ----
def generate_health_report() -> Dict[str, Any]:
    """生成完整健康报告"""
    logger.info("==== 开始自诊断检查 ====")

    report = {
        "version": "1.0",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "timestamp_unix": int(time.time()),
    }

    # 1. 系统信息
    logger.info("收集系统信息...")
    report["system"] = get_system_info()

    # 2. 网络连通性
    logger.info("检查网络连通性...")
    network = check_network_connectivity()
    report["system"]["network_checks"] = network
    logger.debug(f"网络检查: {network}")

    # 3. Docker容器
    logger.info("检查Docker容器...")
    containers = check_all_docker_containers()
    report["containers"] = containers
    for c in containers:
        logger.debug(f"  {c['name']}: status={c['status']} health={c['health']}")

    # 4. APEX ΔG
    logger.info("计算APEX ΔG指标...")
    dG = calculate_apex_dg(report["system"], containers)
    report["apex_dG"] = dG
    logger.info(f"  ΔG = {dG['total_dG']:.2f} [{dG['category']}]")
    for k, v in dG.get("components", {}).items():
        logger.debug(f"    {k}: {v:.2f}")

    # 5. 健康评分
    score, level = calculate_health_score(dG, containers, network)
    report["health_score"] = {
        "score": score,
        "level": level,
    }
    logger.info(f"  健康评分: {score:.1}/100 [{level}]")

    # 6. 模块依赖
    logger.info("检查模块依赖...")
    report["dependencies"] = check_module_dependencies()

    # 7. 综合诊断结论
    issues = []
    if score < 60:
        issues.append("系统健康评分过低")
    if dG["total_dG"] > 0:
        issues.append("APEX ΔG为正，系统存在失序风险")
    if any(c["status"] != "running" for c in containers if c["exists"]):
        issues.append("有容器未正常运行")
    if not network.get("http", True):
        issues.append("网络HTTP连接失败")
    if report["system"].get("memory_percent", 0) > 90:
        issues.append("内存使用率过高")
    if report["system"].get("disk_percent", 0) > 90:
        issues.append("磁盘空间不足")

    report["diagnosis"] = {
        "status": "HEALTHY" if score >= 80 else "UNHEALTHY",
        "issues": issues,
        "recommendations": generate_recommendations(dG, containers, report["system"]),
    }

    # 保存报告
    try:
        with open(REPORT_FILE, "w") as f:
            json.dump(report, f, indent=2, default=str)
        logger.info(f"报告已保存: {REPORT_FILE}")
    except Exception as e:
        logger.error(f"报告保存失败: {e}")

    return report


def generate_recommendations(dG: Dict, containers: List[Dict], system: Dict) -> List[str]:
    """根据诊断结果生成建议"""
    recs = []

    if dG["total_dG"] > 20:
        recs.append("⚠️ 系统ΔG严重超过阈值，建议立即介入")
    elif dG["total_dG"] > 0:
        recs.append("⚠️ 系统ΔG为正，存在失序风险，建议检查各Layer状态")

    for c in containers:
        if c["restart_count"] >= 5:
            recs.append(f"容器 {c['name']} 重启次数过多({c['restart_count']}次)，建议人工检查")
        if c["health"] == "unhealthy":
            recs.append(f"容器 {c['name']} 健康检查失败，建议查看日志: docker logs {c['name']}")

    if system.get("memory_percent", 0) > 85:
        recs.append(f"内存使用率过高({system['memory_percent']:.0f}%)，考虑扩容或优化")

    if system.get("disk_percent", 0) > 85:
        recs.append(f"磁盘使用率过高({system['disk_percent']:.0f}%)，建议清理日志")

    if not recs:
        recs.append("✅ 系统运行正常，无需特殊干预")

    return recs


# ---- 上报EvoMap ----
def report_to_evomap(report: Dict[str, Any]) -> bool:
    """上报健康报告到EvoMap"""
    try:
        payload = json.dumps({
            "timestamp": report["timestamp_unix"],
            "hostname": report["system"]["hostname"],
            "health_score": report["health_score"]["score"],
            "health_level": report["health_score"]["level"],
            "apex_dG": report["apex_dG"]["total_dG"],
            "dG_category": report["apex_dG"]["category"],
            "container_count": len(report["containers"]),
            "running_containers": sum(1 for c in report["containers"] if c["status"] == "running"),
            "memory_percent": report["system"].get("memory_percent", 0),
            "disk_percent": report["system"].get("disk_percent", 0),
            "load_avg": report["system"].get("load_avg_1m", 0),
        })

        result = subprocess.run(
            ["curl", "-sf", "-X", "POST", EVOMAP_API_URL,
             "-H", f"Authorization: Bearer {EVOMAP_API_KEY}",
             "-H", "Content-Type: application/json",
             "-d", payload],
            capture_output=True, timeout=10
        )
        success = result.returncode == 0
        if success:
            logger.info("EvoMap 上报成功")
        else:
            logger.warn("EvoMap 上报失败")
        return success
    except Exception as e:
        logger.error(f"EvoMap 上报异常: {e}")
        return False


# ---- CLI ----
def main():
    import argparse
    parser = argparse.ArgumentParser(description="OMEGA AGI 自诊断系统")
    parser.add_argument("--once", action="store_true", help="单次运行后退出")
    parser.add_argument("--report", action="store_true", help="生成并打印报告")
    parser.add_argument("--evomap", action="store_true", help="上报到EvoMap")
    parser.add_argument("--continuous", action="store_true", help="持续监控模式")
    parser.add_argument("--interval", type=int, default=60, help="监控间隔(秒)")
    args = parser.parse_args()

    logger.info("======== OMEGA AGI 自诊断系统启动 ========")

    if args.once or args.report:
        report = generate_health_report()
        print(json.dumps(report, indent=2, default=str))
        if args.evomap:
            report_to_evomap(report)
        return

    if args.continuous:
        logger.info(f"持续监控模式，间隔 {args.interval} 秒")
        while True:
            report = generate_health_report()
            if args.evomap:
                report_to_evomap(report)
            time.sleep(args.interval)
        return

    # 默认单次运行
    report = generate_health_report()
    if args.evomap:
        report_to_evomap(report)


if __name__ == "__main__":
    main()