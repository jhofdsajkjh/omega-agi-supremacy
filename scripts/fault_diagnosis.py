#!/usr/bin/env python3
# ============================================================
#  OMEGA AGI 自动故障诊断专家系统
#  基于症状匹配、根因分析、修复方案生成
# ============================================================

import os
import re
import sys
import json
import time
import socket
import subprocess
import traceback
import yaml
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Any, Tuple
from dataclasses import dataclass, field, asdict
from enum import Enum
import threading

# ---- 日志 ----
LOG_FILE = os.environ.get("OMEGA_LOG_FILE", "/tmp/omega_diagnosis.log")

def llog(level: str, msg: str):
    ts = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
    entry = f"[{ts}] {level}: {msg}\n"
    print(entry, end="")
    try:
        with open(LOG_FILE, "a") as f:
            f.write(entry)
    except Exception:
        pass

def log_info(msg):  llog("INFO", msg)
def log_warn(msg):  llog("WARN", msg)
def log_error(msg): llog("ERROR", msg)
def log_debug(msg): llog("DEBUG", msg)

# ---- 配置 ----
FAULT_PATTERNS_FILE = os.path.join(os.path.dirname(__file__), "fault_patterns.yaml")
HEALTH_MONITOR_FILE = os.path.join(os.path.dirname(__file__), "health_monitor.yaml")

# ---- 枚举定义 ----
class Severity(Enum):
    CRITICAL = "CRITICAL"
    HIGH = "HIGH"
    MEDIUM = "MEDIUM"
    LOW = "LOW"

class FaultCategory(Enum):
    SYSTEM = "SYSTEM"
    NETWORK = "NETWORK"
    SERVICE = "SERVICE"
    RESOURCE = "RESOURCE"
    DATABASE = "DATABASE"
    SECURITY = "SECURITY"
    APPLICATION = "APPLICATION"
    UNKNOWN = "UNKNOWN"

class ActionType(Enum):
    ALERT = "alert"
    RESTART = "restart"
    CLEANUP = "cleanup"
    ISOLATE = "isolate"
    SCALE = "scale"
    ESCALATE = "escalate"

# ---- 数据结构 ----
@dataclass
class Symptom:
    pattern: str          # 症状模式 (正则或关键词)
    weight: float        # 匹配权重
    source: str          # 来源 (log/dmesg/docker/ps/top/netstat/memory)
    example: str          # 示例日志

@dataclass
class FixStep:
    action: str
    command: str
    order: int
    optional: bool = False
    timeout_seconds: int = 30
    verify_after: bool = True

@dataclass
class FaultPattern:
    name: str
    category: str
    fault_type: str
    severity: str
    symptoms: List[Symptom]
    cause: str
    fix_steps: List[FixStep]
    verify: List[str]

@dataclass
class DiagnosisResult:
    fault_name: str
    fault_type: str
    category: FaultCategory
    severity: Severity
    confidence: float          # 0.0-1.0 置信度
    matched_symptoms: List[str]
    cause: str
    fix_plan: List[FixStep]
    verification: List[str]
    raw_symptoms: Dict[str, Any]

@dataclass
class SystemSnapshot:
    timestamp: str
    system_info: Dict[str, Any]
    containers: List[Dict[str, Any]]
    processes: List[Dict[str, Any]]
    network: Dict[str, Any]
    logs: Dict[str, str]
    dmesg: List[str]
    disk: Dict[str, Any]
    memory_trend: Optional[List[float]] = None

# ---- 症状收集器 ----
class SymptomCollector:
    """从系统各层面收集症状"""

    def __init__(self):
        self.snapshot: Optional[SystemSnapshot] = None

    def collect_all(self) -> SystemSnapshot:
        """收集完整系统快照"""
        log_info("开始收集系统症状...")

        snapshot = SystemSnapshot(
            timestamp=datetime.now(timezone.utc).isoformat(),
            system_info=self._collect_system_info(),
            containers=self._collect_docker_stats(),
            processes=self._collect_processes(),
            network=self._collect_network(),
            logs=self._collect_logs(),
            dmesg=self._collect_dmesg(),
            disk=self._collect_disk(),
        )

        self.snapshot = snapshot
        return snapshot

    def _collect_system_info(self) -> Dict[str, Any]:
        """收集系统基础信息"""
        info = {}
        try:
            info["hostname"] = socket.gethostname()

            # CPU/Load
            with open("/proc/loadavg", "r") as f:
                load = f.read().split()
                info["load_1m"] = float(load[0])
                info["load_5m"] = float(load[1])
                info["load_15m"] = float(load[2])

            # Memory
            lines = open("/proc/meminfo").readlines()
            for line in lines:
                parts = line.split()
                if len(parts) >= 2:
                    key = parts[0].rstrip(":")
                    val = int(parts[1]) * 1024
                    info[f"mem_{key}"] = val
            info["mem_available"] = info.get("mem_MemAvailable", info.get("mem_MemFree", 0))
            info["mem_total"] = info.get("mem_MemTotal", 1)
            info["mem_used"] = info["mem_total"] - info["mem_available"]
            info["mem_percent"] = (info["mem_used"] / info["mem_total"] * 100) if info["mem_total"] else 0

            # CPU count
            info["cpu_count"] = os.cpu_count() or 1

        except Exception as e:
            log_error(f"系统信息收集失败: {e}")

        return info

    def _collect_docker_stats(self) -> List[Dict[str, Any]]:
        """收集Docker容器状态"""
        containers = []
        known = ["omega_agi_core", "omega_self_healing", "omega_pipeline"]

        for name in known:
            result = {
                "name": name,
                "status": "not_found",
                "health": "none",
                "restart_count": 0,
                "cpu_percent": 0.0,
                "memory_mb": 0.0,
            }
            try:
                # Status
                r = subprocess.run(
                    ["docker", "inspect", "--format={{.State.Status}}", name],
                    capture_output=True, text=True, timeout=5
                )
                result["status"] = r.stdout.strip() if r.returncode == 0 else "not_found"

                # Health
                r = subprocess.run(
                    ["docker", "inspect", "--format={{.State.Health.Status}}", name],
                    capture_output=True, text=True, timeout=5
                )
                result["health"] = r.stdout.strip() if r.returncode == 0 else "none"

                # Restart count
                r = subprocess.run(
                    ["docker", "inspect", "--format={{.RestartCount}}", name],
                    capture_output=True, text=True, timeout=5
                )
                if r.returncode == 0:
                    result["restart_count"] = int(r.stdout.strip())

                # Stats
                r = subprocess.run(
                    ["docker", "stats", "--no-stream", "--format={{.CPUPerc}}|{{.MemUsage}}", name],
                    capture_output=True, text=True, timeout=10
                )
                if r.returncode == 0:
                    parts = r.stdout.strip().split("|")
                    if len(parts) >= 2:
                        cpu_str = parts[0].strip().rstrip("%")
                        result["cpu_percent"] = float(cpu_str) if cpu_str else 0.0
                        mem_str = parts[1].strip().split("/")[0].strip()
                        for unit in ["MiB", "GiB", "KiB"]:
                            if unit in mem_str:
                                val = float(mem_str.replace(unit, "").strip())
                                result["memory_mb"] = val * (1024 if unit == "GiB" else 1/1024 if unit == "KiB" else 1)
                                break

            except Exception as e:
                log_debug(f"Docker stats收集失败 {name}: {e}")

            containers.append(result)

        return containers

    def _collect_processes(self) -> List[Dict[str, Any]]:
        """收集进程信息"""
        processes = []
        try:
            r = subprocess.run(["ps", "aux"], capture_output=True, text=True, timeout=5)
            for line in r.stdout.split("\n"):
                if "grep" in line or not line.strip():
                    continue
                parts = line.split()
                if len(parts) >= 11:
                    try:
                        processes.append({
                            "pid": int(parts[1]),
                            "cpu": float(parts[2]),
                            "mem": float(parts[3]),
                            "command": " ".join(parts[10:]),
                        })
                    except (ValueError, IndexError):
                        pass
        except Exception as e:
            log_error(f"进程收集失败: {e}")
        return processes[:50]

    def _collect_network(self) -> Dict[str, Any]:
        """收集网络状态"""
        net = {"dns": False, "http": False, "evomap": False, "feishu": False}

        try:
            socket.gethostbyname("google.com")
            net["dns"] = True
        except Exception:
            pass

        try:
            r = subprocess.run(["curl", "-sf", "--max-time", "5", "https://www.google.com"],
                             capture_output=True, timeout=10)
            net["http"] = r.returncode == 0
        except Exception:
            pass

        try:
            r = subprocess.run(["curl", "-sf", "--max-time", "3", "http://localhost:9000/api/health"],
                             capture_output=True, timeout=5)
            net["evomap"] = r.returncode == 0
        except Exception:
            pass

        return net

    def _collect_logs(self) -> Dict[str, str]:
        """收集最近日志片段"""
        logs = {}
        log_files = [
            "/tmp/omega_diagnosis.log",
            "/tmp/omega_health_monitor.log",
            "/tmp/omega_recovery.log",
            "/tmp/omega_alerts.log",
        ]
        for f in log_files:
            try:
                if os.path.exists(f):
                    with open(f, "r") as fp:
                        lines = fp.readlines()
                        logs[os.path.basename(f)] = "".join(lines[-100:])  # 最近100行
            except Exception:
                pass
        return logs

    def _collect_dmesg(self) -> List[str]:
        """收集dmesg相关错误"""
        lines = []
        try:
            r = subprocess.run(["dmesg", "-T"], capture_output=True, text=True, timeout=5)
            if r.returncode == 0:
                for line in r.stdout.strip().split("\n"):
                    lower = line.lower()
                    if any(k in lower for k in ["error", "fault", "fail", "oom", "kill", "segfault", "warn"]):
                        lines.append(line)
        except Exception:
            pass
        return lines[-50:]

    def _collect_disk(self) -> Dict[str, Any]:
        """收集磁盘信息"""
        disk = {}
        try:
            r = subprocess.run(["df", "-B1", "/"], capture_output=True, text=True, timeout=5)
            parts = r.stdout.strip().split("\n")[-1].split()
            disk["total"] = int(parts[1])
            disk["used"] = int(parts[2])
            disk["available"] = int(parts[3])
            disk["percent"] = float(parts[4].rstrip("%"))
        except Exception:
            pass
        return disk

# ---- 专家系统核心 ----
class FaultDiagnosis:
    """
    故障诊断专家系统

    工作流程:
    1. 收集系统症状 (SymptomCollector)
    2. 加载故障模式库 (fault_patterns.yaml)
    3. 症状匹配 + 规则推理
    4. 生成诊断结果 + 修复方案
    """

    def __init__(self, patterns_file: str = FAULT_PATTERNS_FILE):
        self.patterns_file = patterns_file
        self.patterns: List[FaultPattern] = []
        self.collector = SymptomCollector()
        self._load_patterns()

    def _load_patterns(self):
        """从YAML加载故障模式"""
        try:
            with open(self.patterns_file, "r") as f:
                data = yaml.safe_load(f)

            for item in data.get("fault_patterns", []):
                symptoms = []
                for s in item.get("symptoms", []):
                    if isinstance(s, str):
                        symptoms.append(Symptom(pattern=s, weight=0.5, source="log", example=s))
                    else:
                        symptoms.append(Symptom(
                            pattern=s.get("pattern", s) if isinstance(s, dict) else str(s),
                            weight=s.get("weight", 0.5) if isinstance(s, dict) else 0.5,
                            source=s.get("source", "log") if isinstance(s, dict) else "log",
                            example=s.get("example", s) if isinstance(s, dict) else str(s),
                        ))

                fix_steps = []
                for i, fs in enumerate(item.get("fix_steps", [])):
                    if isinstance(fs, dict):
                        fix_steps.append(FixStep(
                            action=fs.get("action", ""),
                            command=fs.get("command", ""),
                            order=i,
                            optional=fs.get("optional", False),
                            timeout_seconds=fs.get("timeout_seconds", 30),
                            verify_after=fs.get("verify_after", True),
                        ))
                    else:
                        fix_steps.append(FixStep(action=str(fs), command="", order=i))

                self.patterns.append(FaultPattern(
                    name=item["name"],
                    category=item.get("category", "SYSTEM"),
                    fault_type=item.get("fault_type", "UNKNOWN"),
                    severity=item.get("severity", "MEDIUM"),
                    symptoms=symptoms,
                    cause=item.get("cause", ""),
                    fix_steps=fix_steps,
                    verify=item.get("verify", []),
                ))

            log_info(f"加载了 {len(self.patterns)} 个故障模式")
        except Exception as e:
            log_error(f"加载故障模式失败: {e}")
            self.patterns = []

    def diagnose(self, snapshot: SystemSnapshot) -> List[DiagnosisResult]:
        """
        诊断入口: 分析症状，返回诊断结果列表
        按置信度排序
        """
        log_info("开始故障诊断...")

        results = []
        for pattern in self.patterns:
            result = self._match_pattern(pattern, snapshot)
            if result and result.confidence > 0.3:
                results.append(result)

        # 按置信度排序
        results.sort(key=lambda r: r.confidence, reverse=True)

        log_info(f"诊断完成，找到 {len(results)} 个可能的故障")
        return results

    def _match_pattern(self, pattern: FaultPattern, snapshot: SystemSnapshot) -> Optional[DiagnosisResult]:
        """匹配单个故障模式"""
        matched = []
        total_weight = 0.0

        for symptom in pattern.symptoms:
            if self._check_symptom(symptom, snapshot):
                matched.append(symptom.pattern)
                total_weight += symptom.weight

        if not matched:
            return None

        # 置信度 = 匹配权重之和 / 总权重
        max_weight = sum(s.weight for s in pattern.symptoms)
        confidence = total_weight / max_weight if max_weight > 0 else 0.0

        # 如果只有症状匹配但无明确证据，降低置信度
        if confidence < 0.5:
            confidence *= 0.8

        return DiagnosisResult(
            fault_name=pattern.name,
            fault_type=pattern.fault_type,
            category=FaultCategory(pattern.category),
            severity=Severity(pattern.severity),
            confidence=min(confidence, 1.0),
            matched_symptoms=matched,
            cause=pattern.cause,
            fix_plan=pattern.fix_steps,
            verification=pattern.verify,
            raw_symptoms={
                "system": snapshot.system_info,
                "containers": snapshot.containers,
                "network": snapshot.network,
                "dmesg": snapshot.dmesg,
            },
        )

    def _check_symptom(self, symptom: Symptom, snapshot: SystemSnapshot) -> bool:
        """检查单个症状是否匹配"""
        pat = symptom.pattern.lower()
        source = symptom.source

        # 日志/控制台匹配
        if source in ("log", "console"):
            for log_name, content in snapshot.logs.items():
                if pat in content.lower():
                    return True
            return False

        # dmesg匹配
        if source == "dmesg":
            for line in snapshot.dmesg:
                if pat in line.lower():
                    return True
            return False

        # Docker容器状态匹配
        if source in ("docker", "container"):
            for c in snapshot.containers:
                for val in c.values():
                    if isinstance(val, str) and pat in val.lower():
                        return True
                    elif isinstance(val, (int, float)) and pat.replace(".", "").isdigit():
                        try:
                            if abs(float(val) - float(pat)) < 0.01:
                                return True
                        except ValueError:
                            pass
            return False

        # 系统指标匹配
        if source == "system":
            sys_info = snapshot.system_info
            # 常见指标名匹配
            for key, val in sys_info.items():
                if isinstance(val, (int, float)) and pat.replace(".", "").replace("-", "").isdigit():
                    threshold = float(pat)
                    return val >= threshold
            return False

        # 进程匹配
        if source == "process":
            for p in snapshot.processes:
                if pat in p.get("command", "").lower():
                    return True
            return False

        # 默认: 全局搜索
        content = json.dumps(asdict(snapshot), default=str).lower()
        return pat in content

    def diagnose_with_collection(self) -> List[DiagnosisResult]:
        """收集症状后诊断"""
        snapshot = self.collector.collect_all()
        return self.diagnose(snapshot)

    def explain_result(self, result: DiagnosisResult) -> str:
        """生成诊断报告文本"""
        lines = [
            f"\n{'='*60}",
            f"  🔍 诊断结果: {result.fault_name}",
            f"  故障类型: {result.fault_type} | 分类: {result.category.value}",
            f"  严重程度: {result.severity.value} | 置信度: {result.confidence:.0%}",
            f"{'='*60}",
            f"\n📋 匹配的症状 ({len(result.matched_symptoms)} 个):",
        ]
        for i, s in enumerate(result.matched_symptoms, 1):
            lines.append(f"  {i}. {s}")

        lines.extend([
            f"\n🔬 根因分析:",
            f"  {result.cause.strip()}",
            f"\n🔧 修复方案 ({len(result.fix_plan)} 步):",
        ])
        for i, step in enumerate(result.fix_plan, 1):
            lines.append(f"  {i}. [{step.action}] {step.command or '(无命令)'}")
            if step.optional:
                lines[-1] += " (可选)"

        lines.extend([
            f"\n✅ 验证方法:",
        ])
        for v in result.verification:
            lines.append(f"  - {v}")

        lines.append(f"\n{'='*60}")
        return "\n".join(lines)


# ---- 快速诊断 ----
class QuickDiagnosis:
    """快速诊断: 不收集完整快照，直接基于日志/指标诊断"""

    def __init__(self):
        self.diagnoser = FaultDiagnosis()

    def run(self) -> List[DiagnosisResult]:
        """运行快速诊断"""
        log_info("快速诊断模式...")

        # 只收集关键指标
        snapshot = SystemSnapshot(
            timestamp=datetime.now(timezone.utc).isoformat(),
            system_info=self._quick_system(),
            containers=self._quick_docker(),
            processes=[],
            network=self._quick_network(),
            logs=self._quick_logs(),
            dmesg=self._quick_dmesg(),
            disk=self._quick_disk(),
        )

        results = self.diagnoser.diagnose(snapshot)

        # 按置信度输出
        if results:
            for r in results:
                log_warn(f"⚠️  [{r.severity.value}] {r.fault_name} (置信度 {r.confidence:.0%})")
                for s in r.matched_symptoms[:3]:
                    log_debug(f"    症状: {s}")
        else:
            log_info("✅ 未发现明显故障")

        return results

    def _quick_system(self) -> Dict[str, Any]:
        info = {}
        try:
            info["hostname"] = socket.gethostname()
            with open("/proc/loadavg") as f:
                load = f.read().split()
                info["load_1m"] = float(load[0])
            with open("/proc/meminfo") as f:
                for line in f:
                    parts = line.split()
                    if len(parts) >= 2 and parts[0].rstrip(":") == "MemAvailable":
                        info["mem_available"] = int(parts[1]) * 1024
                    elif parts[0].rstrip(":") == "MemTotal":
                        info["mem_total"] = int(parts[1]) * 1024
            info["mem_percent"] = ((info.get("mem_total", 1) - info.get("mem_available", 0)) / info.get("mem_total", 1) * 100) if info.get("mem_total") else 0
            r = subprocess.run(["df", "-B1", "/"], capture_output=True, text=True, timeout=5)
            parts = r.stdout.strip().split("\n")[-1].split()
            info["disk_percent"] = float(parts[4].rstrip("%"))
        except Exception:
            pass
        return info

    def _quick_docker(self) -> List[Dict[str, Any]]:
        containers = []
        known = ["omega_agi_core", "omega_self_healing"]
        for name in known:
            try:
                r = subprocess.run(
                    ["docker", "inspect", "--format={{json .State}}", name],
                    capture_output=True, text=True, timeout=5
                )
                if r.returncode == 0:
                    state = json.loads(r.stdout.strip())
                    containers.append({
                        "name": name,
                        "status": state.get("Status", "unknown"),
                        "health": state.get("Health", {}).get("Status", "none") if state.get("Health") else "none",
                        "restart_count": state.get("RestartCount", 0),
                    })
            except Exception:
                pass
        return containers

    def _quick_network(self) -> Dict[str, Any]:
        return {"http": False, "dns": False}

    def _quick_logs(self) -> Dict[str, str]:
        logs = {}
        for f in ["/tmp/omega_diagnosis.log", "/tmp/omega_recovery.log"]:
            try:
                if os.path.exists(f):
                    with open(f) as fp:
                        lines = fp.readlines()
                        logs[os.path.basename(f)] = "".join(lines[-200:])
            except Exception:
                pass
        return logs

    def _quick_dmesg(self) -> List[str]:
        try:
            r = subprocess.run(["dmesg", "-T"], capture_output=True, text=True, timeout=5)
            if r.returncode == 0:
                return [l for l in r.stdout.strip().split("\n") if any(k in l.lower() for k in ["error", "fail", "oom", "warn", "fault"])][-30:]
        except Exception:
            pass
        return []

    def _quick_disk(self) -> Dict[str, Any]:
        try:
            r = subprocess.run(["df", "-B1", "/"], capture_output=True, text=True, timeout=5)
            parts = r.stdout.strip().split("\n")[-1].split()
            return {"percent": float(parts[4].rstrip("%"))}
        except Exception:
            return {}


# ---- CLI ----
def main():
    import argparse
    parser = argparse.ArgumentParser(description="OMEGA AGI 自动故障诊断专家系统")
    parser.add_argument("--full", action="store_true", help="完整诊断 (收集所有系统数据)")
    parser.add_argument("--quick", action="store_true", help="快速诊断 (基于现有日志)")
    parser.add_argument("--explain", action="store_true", help="详细解释每个诊断结果")
    parser.add_argument("--json", action="store_true", help="JSON格式输出")
    parser.add_argument("--watch", action="store_true", help="持续监控模式")
    parser.add_argument("--interval", type=int, default=60, help="监控间隔(秒)")
    args = parser.parse_args()

    log_info("======== OMEGA AGI 故障诊断专家系统启动 ========")

    if args.watch:
        log_info(f"持续监控模式，间隔 {args.interval} 秒")
        while True:
            if args.full:
                diag = FaultDiagnosis()
                results = diag.diagnose_with_collection()
            else:
                qdiag = QuickDiagnosis()
                results = qdiag.run()

            if args.explain and results:
                for r in results:
                    print(diag.explain_result(r) if 'diag' in dir() else "")
            time.sleep(args.interval)
        return

    # 默认完整诊断
    diag = FaultDiagnosis()
    results = diag.diagnose_with_collection()

    if args.json:
        output = [{
            "fault_name": r.fault_name,
            "fault_type": r.fault_type,
            "category": r.category.value,
            "severity": r.severity.value,
            "confidence": round(r.confidence, 3),
            "matched_symptoms": r.matched_symptoms,
            "cause": r.cause,
            "fix_plan": [{"action": s.action, "command": s.command} for s in r.fix_plan],
            "verification": r.verification,
        } for r in results]
        print(json.dumps(output, indent=2, ensure_ascii=False))
        return

    if not results:
        log_info("✅ 系统健康，未检测到故障")
        return

    log_warn(f"⚠️ 检测到 {len(results)} 个潜在故障:")
    for r in results:
        icon = "🔴" if r.severity == Severity.CRITICAL else "🟠" if r.severity == Severity.HIGH else "🟡"
        log_warn(f"  {icon} [{r.severity.value}] {r.fault_name} (置信度 {r.confidence:.0%})")

    if args.explain:
        for r in results:
            print(diag.explain_result(r))

    # 输出修复建议汇总
    print("\n" + "="*60)
    print("  📋 修复建议汇总")
    print("="*60)
    for r in results:
        if r.severity in (Severity.CRITICAL, Severity.HIGH):
            print(f"\n【{r.fault_name}】({r.severity.value})")
            for step in r.fix_plan[:3]:  # 最多显示前3步
                if step.command:
                    print(f"  → {step.action}: {step.command}")
    print()


if __name__ == "__main__":
    main()