#!/usr/bin/env python3
# ============================================================
#  OMEGA AGI 自诊断系统 V2.0
#  完整故障分类、根因分析、修复方案生成与执行
# ============================================================

import os, sys, json, time, socket, platform, subprocess, traceback
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Any, Tuple

LOG_FILE = os.environ.get("OMEGA_LOG_FILE", "/tmp/omega_diagnosis.log")
REPORT_FILE = os.environ.get("OMEGA_REPORT_FILE", "/tmp/omega_health_report.json")
EVOMAP_API_URL = os.environ.get("EVOMAP_API_URL", "http://localhost:9000/api/health")
EVOMAP_API_KEY = os.environ.get("EVOMAP_API_KEY", "omega-health-key")

class Logger:
    def __init__(self, log_file: str = LOG_FILE):
        self.log_file = log_file
        Path(log_file).parent.mkdir(parents=True, exist_ok=True)
    def _write(self, level: str, msg: str):
        ts = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
        entry = f"[{ts}] {level}: {msg}\n"
        print(entry, end="")
        try:
            with open(self.log_file, "a") as f: f.write(entry)
        except Exception: pass
    def info(self, m): self._write("INFO", m)
    def warn(self, m): self._write("WARN", m)
    def error(self, m): self._write("ERROR", m)
    def debug(self, m): self._write("DEBUG", m)
logger = Logger()

# ============================================================
# 故障分类体系
# ============================================================
class FaultClassifier:
    CATEGORIES = {
        "OOM": {"category": "RESOURCE", "severity": "CRITICAL",
            "keywords": ["oom", "out of memory", "killed", "cannot allocate", "memtest"],
            "indicators": {"memory_percent": 95}},
        "CPU_OVERLOAD": {"category": "RESOURCE", "severity": "HIGH",
            "keywords": ["cpu", "load", "overload", "100%"],
            "indicators": {"load_avg_1m": 8}},
        "DISK_FULL": {"category": "RESOURCE", "severity": "CRITICAL",
            "keywords": ["no space", "disk full", "enospc", "cannot write"],
            "indicators": {"disk_percent": 95}},
        "CONTAINER_CRASH": {"category": "SERVICE", "severity": "CRITICAL",
            "keywords": ["exit 1", "exited", "crash", "segfault", "signal 11"],
            "indicators": {"container_omega_agi_core_status": "exited"}},
        "NETWORK_TIMEOUT": {"category": "NETWORK", "severity": "HIGH",
            "keywords": ["timeout", "timed out", "connection refused"],
            "indicators": {"network_http": False}},
        "HEALTH_CHECK_FAIL": {"category": "SERVICE", "severity": "HIGH",
            "keywords": ["unhealthy", "health check fail"],
            "indicators": {"container_omega_agi_core_health": "unhealthy"}},
        "API_RATE_LIMIT": {"category": "NETWORK", "severity": "MEDIUM",
            "keywords": ["429", "rate limit", "too many requests"], "indicators": {}},
        "DNS_FAILURE": {"category": "NETWORK", "severity": "HIGH",
            "keywords": ["cannot resolve", "dns", "nxdomain"],
            "indicators": {"network_dns": False}},
        "FEISHU_FAIL": {"category": "SERVICE", "severity": "MEDIUM",
            "keywords": ["feishu", "lark", "飞书", "send message failed"], "indicators": {}},
        "AUTH_FAILURE": {"category": "SECURITY", "severity": "HIGH",
            "keywords": ["unauthorized", "401", "auth failed", "token invalid"], "indicators": {}},
        "LOG_STORM": {"category": "SYSTEM", "severity": "MEDIUM",
            "keywords": ["log storm", "verbose debug"], "indicators": {}},
    }
    @classmethod
    def classify(cls, symptoms: Dict[str, Any]) -> List[Dict[str, Any]]:
        results = []
        symptom_text = json.dumps(symptoms, default=str).lower()
        for fault_type, config in cls.CATEGORIES.items():
            score = 0.0
            matched = []
            for kw in config["keywords"]:
                if kw.lower() in symptom_text:
                    score += 1.0
                    matched.append(f"keyword:{kw}")
            for ind, threshold in config.get("indicators", {}).items():
                if ind in symptoms:
                    val = symptoms[ind]
                    if isinstance(val, (int, float)) and val >= threshold:
                        score += 2.0
                        matched.append(f"{ind}={val}")
                    elif isinstance(val, str) and val == threshold:
                        score += 2.0
                        matched.append(f"{ind}={val}")
            if score > 0:
                results.append({"fault_type": fault_type, "category": config["category"],
                    "severity": config["severity"], "score": score, "matched_indicators": matched})
        results.sort(key=lambda r: r["score"], reverse=True)
        return results

# ============================================================
# 根因分析引擎
# ============================================================
class RootCauseAnalyzer:
    CAUSE_CHAIN = {
        "OOM": [("IMMEDIATE", "进程内存耗尽被OOM Killer终止"),
                 ("ROOT", "内存泄漏或突发流量"),
                 ("DEEP", "代码未正确释放内存 或 JVM堆配置不当"),
                 ("CONTEXT", "容器内存限制过低")],
        "CPU_OVERLOAD": [("IMMEDIATE", "CPU使用率持续100%"),
                 ("ROOT", "死循环/计算密集 或 并发过多"),
                 ("DEEP", "代码热路径效率低"),
                 ("CONTEXT", "未限流，未自动扩容")],
        "DISK_FULL": [("IMMEDIATE", "磁盘空间耗尽"),
                 ("ROOT", "日志无限增长/Docker资源积累"),
                 ("DEEP", "日志轮转未配置"),
                 ("CONTEXT", "监控告警阈值不当")],
        "CONTAINER_CRASH": [("IMMEDIATE", "容器异常退出"),
                 ("ROOT", "应用代码异常/依赖服务不可用"),
                 ("DEEP", "未捕获异常/配置错误"),
                 ("CONTEXT", "启动顺序不当/端口冲突")],
        "NETWORK_TIMEOUT": [("IMMEDIATE", "网络请求超时"),
                 ("ROOT", "目标服务不可达/网络隔离"),
                 ("DEEP", "防火墙阻断/DNS解析失败"),
                 ("CONTEXT", "网络配置错误")],
    }
    @classmethod
    def analyze(cls, fault_type: str, symptoms: Dict[str, Any]) -> Dict[str, Any]:
        chain = cls.CAUSE_CHAIN.get(fault_type, [("IMMEDIATE","未知直接原因"),("ROOT","未知根本原因"),("DEEP","未知深层原因"),("CONTEXT","未知上下文")])
        key_vals = {k: symptoms[k] for k in ["memory_percent","disk_percent","load_avg_1m","container_omega_agi_core_status","network_http"] if k in symptoms}
        return {"fault_type": fault_type,
                "cause_chain": [{"level": l, "description": d} for l, d in chain],
                "symptoms_summary": str(key_vals) if key_vals else "(无关键指标)"}

# ============================================================
# 修复方案生成器
# ============================================================
class FixPlanGenerator:
    PLANS = {
        "OOM": {"summary": "内存溢出需立即释放内存或重启服务", "steps": [
            {"order":1,"action":"查看OOM Killer日志","command":"dmesg -T 2>/dev/null | grep -i oom | tail -20","verify":True},
            {"order":2,"action":"找出高内存容器","command":"docker stats --no-stream --format '{{.Name}}\t{{.MemUsage}}' 2>/dev/null | sort -k2 -hr | head -5 || echo 'docker not available'","verify":False},
            {"order":3,"action":"重启问题容器","command":"docker restart omega_agi_core 2>/dev/null || echo 'container not found'","verify":True},
            {"order":4,"action":"验证内存状态","command":"free -m","verify":True}], "preventive": "设置容器内存限制，配置OOM告警"},
        "CPU_OVERLOAD": {"summary": "CPU过载需限流或扩容", "steps": [
            {"order":1,"action":"查看CPU占用","command":"top -bn1 | head -15","verify":False},
            {"order":2,"action":"重启容器","command":"docker restart omega_agi_core 2>/dev/null || echo 'no docker'","verify":True},
            {"order":3,"action":"验证CPU","command":"uptime","verify":True}], "preventive": "配置CPU告警，设置限流"},
        "DISK_FULL": {"summary": "磁盘不足需清理", "steps": [
            {"order":1,"action":"查看磁盘","command":"df -h","verify":False},
            {"order":2,"action":"清理Docker资源","command":"docker system prune -af --volumes 2>/dev/null || rm -f /tmp/omega_*.log; true","verify":False},
            {"order":3,"action":"清理临时文件","command":"rm -f /tmp/omega_*.log /tmp/omega_*.json /tmp/omega_diagnosis_*.json; true","verify":False},
            {"order":4,"action":"验证磁盘","command":"df -h /","verify":True}], "preventive": "配置日志轮转，设置磁盘告警"},
        "CONTAINER_CRASH": {"summary": "容器崩溃需查看日志并重启", "steps": [
            {"order":1,"action":"查看崩溃日志","command":"docker logs --tail 100 omega_agi_core 2>&1 | tail -30 || echo 'no logs'","verify":False},
            {"order":2,"action":"重启容器","command":"docker restart omega_agi_core 2>/dev/null || echo 'restart failed'","verify":True},
            {"order":3,"action":"验证运行","command":"docker ps 2>/dev/null | grep omega_agi_core || echo 'not running'","verify":True}], "preventive": "配置健康检查，设置重启策略"},
        "NETWORK_TIMEOUT": {"summary": "网络超时需检查连通性", "steps": [
            {"order":1,"action":"测试外网","command":"curl -sf --max-time 5 https://www.google.com && echo OK || echo 'network issue'","verify":False},
            {"order":2,"action":"重启Docker","command":"systemctl restart docker 2>/dev/null || echo 'systemd not available'","verify":True},
            {"order":3,"action":"验证网络","command":"curl -sf --max-time 10 https://www.google.com && echo OK","verify":True}], "preventive": "配置网络监控，设置超时告警"},
        "HEALTH_CHECK_FAIL": {"summary": "健康检查失败需排查并重启", "steps": [
            {"order":1,"action":"查看健康详情","command":"docker inspect --format='{{.State.Health.Status}}' omega_agi_core 2>/dev/null || echo 'none'","verify":False},
            {"order":2,"action":"重启容器","command":"docker restart omega_agi_core 2>/dev/null || echo 'restart failed'","verify":True},
            {"order":3,"action":"验证健康","command":"sleep 20 && docker inspect --format='{{.State.Health.Status}}' omega_agi_core 2>/dev/null","verify":True}], "preventive": "配置健康检查重试机制"},
        "API_RATE_LIMIT": {"summary": "API限流需实现退避重试", "steps": [
            {"order":1,"action":"识别限流","command":"grep -r '429\\|rate.limit' /tmp/omega*.log 2>/dev/null | tail -10 || echo 'no logs'","verify":False},
            {"order":2,"action":"等待恢复","command":"echo 'waiting'; sleep 60","verify":False}], "preventive": "实现指数退避，配置限流"},
        "DNS_FAILURE": {"summary": "DNS失败需修复DNS配置", "steps": [
            {"order":1,"action":"检查DNS","command":"cat /etc/resolv.conf","verify":False},
            {"order":2,"action":"使用Google DNS","command":"echo 'nameserver 8.8.8.8' > /etc/resolv.conf 2>/dev/null || echo 'no write permission'","verify":True}], "preventive": "配置多DNS服务器"},
    }
    @classmethod
    def generate(cls, fault_type: str) -> Optional[Dict[str, Any]]:
        if fault_type not in cls.PLANS:
            return {"summary": "通用恢复", "steps": [
                {"order":1,"action":"重启服务","command":"docker restart omega_agi_core 2>/dev/null || echo 'failed'","verify":True},
                {"order":2,"action":"验证状态","command":"docker ps 2>/dev/null | grep omega || echo 'not found'","verify":True}], "preventive": "加强监控"}
        return cls.PLANS[fault_type]
    @classmethod
    def generate_all(cls, fault_types: List[str]) -> List[Dict[str, Any]]:
        plans = []
        for ft in fault_types:
            p = cls.generate(ft)
            if p:
                p["fault_type"] = ft
                plans.append(p)
        return plans

# ============================================================
# 修复执行器
# ============================================================
class FixExecutor:
    def __init__(self): self.history: List[Dict[str, Any]] = []
    def execute_plan(self, plan: Dict[str, Any], dry_run: bool = False) -> Dict[str, Any]:
        result = {"fault_type": plan.get("fault_type","UNKNOWN"), "status": "pending",
                  "steps_executed": [], "start_time": datetime.now(timezone.utc).isoformat(),
                  "end_time": "", "success": False}
        logger.info(f"\n{'='*60}\n  🔧 执行修复: {plan.get('fault_type')} - {plan.get('summary','')[:50]}\n{'='*60}")
        if dry_run:
            logger.info("[dry-run] 跳过执行")
            result["status"] = "dry_run"
            result["end_time"] = datetime.now(timezone.utc).isoformat()
            self.history.append(result)
            return result
        all_ok = True
        for step in plan.get("steps", []):
            ord_, action, cmd, verify = step.get("order",0), step.get("action",""), step.get("command",""), step.get("verify",False)
            logger.info(f"  步骤{ord_}: [{action}] {cmd[:80]}...")
            sr = {"order": ord_, "action": action, "command": cmd, "success": False}
            start = time.time()
            try:
                r = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=60)
                sr["returncode"] = r.returncode
                sr["output"] = r.stdout[:200] if r.stdout else ""
                sr["error"] = r.stderr[:200] if r.stderr else ""
                sr["duration"] = time.time() - start
                if r.returncode == 0:
                    sr["success"] = True
                    logger.info(f"    ✅ 成功 ({sr['duration']:.1f}s)")
                else:
                    logger.error(f"    ❌ 失败 (exit {r.returncode})")
                    if verify: all_ok = False
            except subprocess.TimeoutExpired:
                sr["error"] = "超时 (60秒)"
                logger.error(f"    ❌ 超时")
                all_ok = False
            except Exception as e:
                sr["error"] = str(e)
                logger.error(f"    ❌ 异常: {e}")
                all_ok = False
            result["steps_executed"].append(sr)
            if ord_ < len(plan.get("steps",[])): time.sleep(2)
        result["status"] = "verified" if all_ok else "partial"
        result["success"] = all_ok
        result["end_time"] = datetime.now(timezone.utc).isoformat()
        ok_cnt = sum(1 for s in result["steps_executed"] if s["success"])
        logger.info(f"\n  步骤完成: {ok_cnt}/{len(result['steps_executed'])}")
        self.history.append(result)
        return result
    def execute_all_plans(self, plans: List[Dict[str, Any]], dry_run: bool = False) -> List[Dict[str, Any]]:
        return [self.execute_plan(p, dry_run) for p in plans]

# ============================================================
# 原有函数 (保持兼容)
# ============================================================
def get_system_info() -> Dict[str, Any]:
    info = {"hostname": socket.gethostname(), "platform": platform.system(),
            "platform_release": platform.release(), "platform_version": platform.version(),
            "architecture": platform.machine(), "processor": platform.processor(),
            "python_version": platform.python_version(),
            "timestamp_unix": int(time.time()), "timestamp_iso": datetime.now(timezone.utc).isoformat()}
    try:
        with open("/proc/loadavg") as f:
            load = f.read().split()
            info["load_avg_1m"] = float(load[0]); info["load_avg_5m"] = float(load[1]); info["load_avg_15m"] = float(load[2])
    except: info["load_avg_1m"] = info["load_avg_5m"] = info["load_avg_15m"] = 0.0
    try:
        lines = open("/proc/meminfo").readlines()
        mem = {}
        for line in lines:
            parts = line.split()
            if len(parts) >= 2: mem[parts[0].rstrip(":")] = int(parts[1]) * 1024
        info["memory_total"] = mem.get("MemTotal", 0)
        info["memory_available"] = mem.get("MemAvailable", mem.get("MemFree", 0))
        info["memory_used"] = info["memory_total"] - info["memory_available"]
        info["memory_percent"] = (info["memory_used"] / info["memory_total"] * 100) if info["memory_total"] else 0
    except: pass
    try:
        r = subprocess.run(["df","-B1","/"], capture_output=True, text=True, timeout=5)
        parts = r.stdout.strip().split("\n")[-1].split()
        info["disk_total"] = int(parts[1]); info["disk_used"] = int(parts[2])
        info["disk_available"] = int(parts[3]); info["disk_percent"] = float(parts[4].rstrip("%"))
    except: pass
    try:
        info["cpu_count"] = os.cpu_count()
        info["cpu_count_logical"] = os.cpu_count(logical=False) or os.cpu_count()
    except: pass
    return info

def check_docker_container(name: str) -> Dict[str, Any]:
    result = {"name": name, "exists": False, "status": "not_found", "health": "none",
             "restart_count": 0, "uptime_seconds": 0, "cpu_percent": 0.0, "memory_mb": 0.0}
    try:
        r = subprocess.run(["docker","inspect","--format={{json .State}}",name], capture_output=True, text=True, timeout=5)
        if r.returncode != 0: return result
        state = json.loads(r.stdout.strip())
        result["exists"] = True
        result["status"] = state.get("Status","unknown")
        result["restart_count"] = state.get("RestartCount", 0)
        try:
            started = state.get("StartedAt","")
            if started:
                dt = datetime.fromisoformat(started.replace("Z","+00:00"))
                result["uptime_seconds"] = int((datetime.now(timezone.utc) - dt).total_seconds())
        except: pass
        health = state.get("Health", {})
        result["health"] = health.get("Status","none") if health else "none"
        try:
            s = subprocess.run(["docker","stats","--no-stream","--format={{.CPUPerc}}|{{.MemUsage}}",name],
                               capture_output=True, text=True, timeout=10)
            if s.returncode == 0:
                parts = s.stdout.strip().split("|")
                if len(parts) >= 2:
                    result["cpu_percent"] = float(parts[0].strip().rstrip("%")) if parts[0].strip().rstrip("%") else 0.0
                    mem_str = parts[1].strip().split("/")[0].strip()
                    for unit in ["MiB","GiB","KiB"]:
                        if unit in mem_str:
                            val = float(mem_str.replace(unit,"").strip())
                            result["memory_mb"] = val * (1024 if unit=="GiB" else 1/1024 if unit=="KiB" else 1)
                            break
        except: pass
    except Exception as e:
        logger.warn(f"Docker check error for {name}: {e}")
    return result

def check_all_docker_containers() -> List[Dict[str, Any]]:
    return [check_docker_container(n) for n in ["omega_agi_core","omega_self_healing","omega_pipeline"]]

def check_network_connectivity() -> Dict[str, Any]:
    checks = {"dns": False, "http": False, "evomap": False}
    try:
        socket.gethostbyname("google.com"); checks["dns"] = True
    except: pass
    try:
        r = subprocess.run(["curl","-sf","--max-time","5","https://www.google.com"], capture_output=True, timeout=10)
        checks["http"] = r.returncode == 0
    except: pass
    try:
        r = subprocess.run(["curl","-sf","--max-time","3",EVOMAP_API_URL], capture_output=True, timeout=5)
        checks["evomap"] = r.returncode == 0
    except: pass
    return checks

def calculate_apex_dg(system_info: Dict, containers: List[Dict]) -> Dict[str, float]:
    G_components = {}
    cpu_load = system_info.get("load_avg_1m", 0)
    cpu_cores = max(system_info.get("cpu_count_logical", 1), 1)
    normalized_cpu = cpu_load / cpu_cores
    mem_percent = system_info.get("memory_percent", 0)
    disk_percent = system_info.get("disk_percent", 0)
    G_components["resource_pressure"] = (normalized_cpu*0.4 + (mem_percent/100)*0.35 + (disk_percent/100)*0.25) * 100
    stability_scores = []
    for c in containers:
        if not c["exists"]: stability_scores.append(0.0)
        elif c["status"] == "running":
            stability_scores.append(1.0 if c["health"]=="healthy" else 0.3 if c["health"]=="unhealthy" else 0.8)
        else: stability_scores.append(0.0)
    avg_stab = sum(stability_scores)/len(stability_scores) if stability_scores else 0
    G_components["service_stability"] = (1.0 - avg_stab) * 100
    G_components["restart_penalty"] = sum(30 if c.get("restart_count",0)>=5 else 15 if c.get("restart_count",0)>=3 else 5 if c.get("restart_count",0)>=1 else 0 for c in containers)
    if not system_info.get("network_checks",{}).get("dns", True): G_components["network_dns_failure"] = 20
    if not system_info.get("network_checks",{}).get("http", True): G_components["network_http_failure"] = 10
    total_dG = sum(G_components.values())
    if total_dG < -50: cat="EXCELLENT"
    elif total_dG < -20: cat="HEALTHY"
    elif total_dG < -10: cat="CAUTION"
    elif total_dG < 0: cat="WARNING"
    elif total_dG < 20: cat="CRITICAL"
    else: cat="COLLAPSE"
    return {"total_dG": total_dG, "category": cat, "components": G_components}

def calculate_health_score(dG: Dict, containers: List[Dict], network: Dict) -> Tuple[float, str]:
    score = 100.0
    dG_total = dG["total_dG"]
    if dG_total > 0: score -= min(dG_total * 0.5, 40)
    avail = sum(1 for c in containers if c["exists"] and c["status"] == "running")
    if containers: score = score * 0.7 + (avail/len(containers)) * 30
    if not network.get("http", True): score -= 10
    if not network.get("evomap", True): score -= 5
    score = max(0.0, min(100.0, score))
    level = "EXCELLENT" if score>=95 else "HEALTHY" if score>=80 else "CAUTION" if score>=60 else "WARNING" if score>=40 else "CRITICAL" if score>=20 else "DOWN"
    return score, level

# ============================================================
# 增强版报告生成
# ============================================================
def generate_health_report() -> Dict[str, Any]:
    logger.info("==== 开始自诊断检查 (V2.0) ====")
    report = {"version": "2.0", "generated_at": datetime.now(timezone.utc).isoformat(), "timestamp_unix": int(time.time())}
    logger.info("收集系统信息...")
    system = get_system_info()
    report["system"] = system
    logger.info("检查网络连通性...")
    network = check_network_connectivity()
    report["system"]["network_checks"] = network
    logger.info("检查Docker容器...")
    containers = check_all_docker_containers()
    report["containers"] = containers
    logger.info("计算APEX ΔG指标...")
    dG = calculate_apex_dg(report["system"], containers)
    report["apex_dG"] = dG
    logger.info(f"  ΔG = {dG['total_dG']:.2f} [{dG['category']}]")
    score, level = calculate_health_score(dG, containers, network)
    report["health_score"] = {"score": score, "level": level}
    logger.info(f"  健康评分: {score:.1}/100 [{level}]")

    # --- 新增: 故障分类 ---
    logger.info("进行故障分类...")
    symptoms = {**system, "network_checks": network}
    for c in containers:
        symptoms[f"container_{c['name']}_status"] = c["status"]
        symptoms[f"container_{c['name']}_health"] = c["health"]
        symptoms[f"container_{c['name']}_restart"] = c["restart_count"]
    fault_classifications = FaultClassifier.classify(symptoms)
    report["fault_classifications"] = fault_classifications

    # --- 新增: 根因分析 (针对每个检测到的故障) ---
    root_causes = []
    for fc in fault_classifications:
        if fc["score"] >= 1.0:
            root_causes.append(RootCauseAnalyzer.analyze(fc["fault_type"], symptoms))
    report["root_causes"] = root_causes

    # --- 新增: 修复方案 (按优先级) ---
    if fault_classifications:
        high_severity = [fc["fault_type"] for fc in fault_classifications if fc["severity"] in ("CRITICAL","HIGH")]
        report["fix_plans"] = FixPlanGenerator.generate_all(high_severity)
        logger.info(f"  生成 {len(report['fix_plans'])} 个修复方案")

    # --- 综合结论 ---
    issues = []
    if score < 60: issues.append("系统健康评分过低")
    if dG["total_dG"] > 0: issues.append("APEX ΔG为正，系统存在失序风险")
    if any(c["status"] != "running" for c in containers if c["exists"]): issues.append("有容器未正常运行")
    if not network.get("http", True): issues.append("网络HTTP连接失败")
    if system.get("memory_percent", 0) > 90: issues.append("内存使用率过高")
    if system.get("disk_percent", 0) > 90: issues.append("磁盘空间不足")
    for fc in fault_classifications:
        if fc["severity"] == "CRITICAL": issues.append(f"检测到严重故障: {fc['fault_type']}")

    report["diagnosis"] = {
        "status": "HEALTHY" if score >= 80 and not fault_classifications else "UNHEALTHY",
        "issues": issues,
        "recommendations": _generate_recommendations(dG, containers, system, fault_classifications),
    }

    try:
        with open(REPORT_FILE, "w") as f:
            json.dump(report, f, indent=2, default=str)
        logger.info(f"报告已保存: {REPORT_FILE}")
    except Exception as e:
        logger.error(f"报告保存失败: {e}")
    return report

def _generate_recommendations(dG, containers, system, fault_classifications):
    recs = []
    if dG["total_dG"] > 20: recs.append("⚠️ 系统ΔG严重超过阈值，建议立即介入")
    elif dG["total_dG"] > 0: recs.append("⚠️ 系统ΔG为正，存在失序风险")
    for c in containers:
        if c["restart_count"] >= 5: recs.append(f"容器 {c['name']} 重启次数过多({c['restart_count']}次)")
        if c["health"] == "unhealthy": recs.append(f"容器 {c['name']} 健康检查失败")
    if system.get("memory_percent",0) > 85: recs.append(f"内存使用率过高({system['memory_percent']:.0f}%)")
    if system.get("disk_percent",0) > 85: recs.append(f"磁盘使用率过高({system['disk_percent']:.0f}%)")
    for fc in fault_classifications[:3]:
        if fc["severity"] in ("CRITICAL","HIGH"):
            recs.append(f"🔧 [{fc['severity']}] {fc['fault_type']} - 建议执行修复方案")
    if not recs: recs.append("✅ 系统运行正常，无需特殊干预")
    return recs

def report_to_evomap(report: Dict) -> bool:
    try:
        payload = json.dumps({
            "timestamp": report["timestamp_unix"],
            "hostname": report["system"]["hostname"],
            "health_score": report["health_score"]["score"],
            "health_level": report["health_score"]["level"],
            "apex_dG": report["apex_dG"]["total_dG"],
            "dG_category": report["apex_dG"]["category"],
            "fault_count": len(report.get("fault_classifications", [])),
        })
        r = subprocess.run(["curl","-sf","-X","POST",EVOMAP_API_URL,
                           "-H",f"Authorization: Bearer {EVOMAP_API_KEY}",
                           "-H","Content-Type: application/json","-d",payload],
                          capture_output=True, timeout=10)
        if r.returncode == 0:
            logger.info("EvoMap 上报成功")
            return True
        logger.warn("EvoMap 上报失败")
        return False
    except Exception as e:
        logger.error(f"EvoMap 上报异常: {e}")
        return False

# ============================================================
# CLI
# ============================================================
def main():
    import argparse
    parser = argparse.ArgumentParser(description="OMEGA AGI 自诊断系统 V2.0")
    parser.add_argument("--once", action="store_true", help="单次运行后退出")
    parser.add_argument("--report", action="store_true", help="生成并打印报告")
    parser.add_argument("--evomap", action="store_true", help="上报到EvoMap")
    parser.add_argument("--continuous", action="store_true", help="持续监控模式")
    parser.add_argument("--interval", type=int, default=60, help="监控间隔(秒)")
    parser.add_argument("--auto-fix", action="store_true", help="自动修复高优先级故障")
    parser.add_argument("--dry-run", action="store_true", help="仅模拟修复，不实际执行")
    parser.add_argument("--json", action="store_true", help="JSON格式输出")
    args = parser.parse_args()

    logger.info("======== OMEGA AGI 自诊断系统 V2.0 启动 ========")

    if args.once or args.report:
        report = generate_health_report()
        if args.json:
            print(json.dumps(report, indent=2, default=str))
        else:
            print(f"\n健康评分: {report['health_score']['score']:.1}/100 [{report['health_score']['level']}]")
            print(f"APEX ΔG: {report['apex_dG']['total_dG']:.2f} [{report['apex_dG']['category']}]")
            if report.get("fault_classifications"):
                print(f"\n检测到 {len(report['fault_classifications'])} 个故障:")
                for fc in report["fault_classifications"]:
                    icon = "🔴" if fc["severity"]=="CRITICAL" else "🟠" if fc["severity"]=="HIGH" else "🟡"
                    print(f"  {icon} [{fc['severity']}] {fc['fault_type']} (score={fc['score']:.0f})")
            if report.get("fix_plans"):
                print(f"\n修复方案:")
                for p in report["fix_plans"]:
                    print(f"  📋 {p['fault_type']}: {p['summary']}")
        if args.evomap: report_to_evomap(report)
        return

    if args.continuous:
        logger.info(f"持续监控模式，间隔 {args.interval} 秒")
        while True:
            report = generate_health_report()
            if args.auto_fix and report.get("fix_plans"):
                executor = FixExecutor()
                for p in report["fix_plans"]:
                    executor.execute_plan(p, dry_run=args.dry_run)
            if args.evomap: report_to_evomap(report)
            time.sleep(args.interval)
        return

    report = generate_health_report()
    if args.evomap: report_to_evomap(report)

if __name__ == "__main__":
    main()