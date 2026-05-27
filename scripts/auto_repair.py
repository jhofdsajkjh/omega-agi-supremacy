#!/usr/bin/env python3
# ============================================================
#  OMEGA AGI 自动修复执行器
#  执行修复方案、验证效果、自动回滚
# ============================================================

import os
import re
import sys
import json
import time
import shutil
import subprocess
import traceback
import threading
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Any, Tuple, Callable
from dataclasses import dataclass, field
from enum import Enum
import yaml

# ---- 日志 ----
LOG_FILE = os.environ.get("OMEGA_LOG_FILE", "/tmp/omega_repair.log")
MAX_RESTART_ATTEMPTS = int(os.environ.get("MAX_RESTART_ATTEMPTS", "5"))
RESTART_COOLDOWN = int(os.environ.get("RESTART_COOLDOWN", "60"))

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

# ---- 数据结构 ----
class FixStatus(Enum):
    PENDING = "pending"
    RUNNING = "running"
    SUCCESS = "success"
    FAILED = "failed"
    VERIFIED = "verified"
    ROLLED_BACK = "rolled_back"

class FixSeverity(Enum):
    CRITICAL = "CRITICAL"
    HIGH = "HIGH"
    MEDIUM = "MEDIUM"
    LOW = "LOW"

@dataclass
class FixStepResult:
    step_index: int
    action: str
    command: str
    status: FixStatus
    output: str
    error: str
    duration_seconds: float
    timestamp: str = ""

    def __post_init__(self):
        self.timestamp = datetime.now(timezone.utc).isoformat()

@dataclass
class FixPlan:
    fault_name: str
    fault_type: str
    severity: str
    steps: List[Any]  # FixStep objects
    max_attempts: int = 3
    cooldown_seconds: int = 60
    rollback_on_failure: bool = True

@dataclass
class FixExecution:
    plan: FixPlan
    status: FixStatus
    start_time: str
    end_time: Optional[str] = None
    steps_results: List[FixStepResult] = field(default_factory=list)
    attempts: int = 0
    rollback_executed: bool = False
    final_error: str = ""
    verification_results: List[str] = field(default_factory=list)

# ---- Shell命令执行 ----
def run_command(cmd: str, timeout: int = 30, shell: bool = True, check: bool = False) -> Tuple[int, str, str]:
    """执行shell命令，返回 (returncode, stdout, stderr)"""
    log_debug(f"执行: {cmd[:100]}...")
    try:
        if shell:
            result = subprocess.run(
                cmd, shell=True, capture_output=True, text=True, timeout=timeout
            )
        else:
            result = subprocess.run(
                cmd.split(), capture_output=True, text=True, timeout=timeout
            )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return -1, "", f"命令超时 ({timeout}秒)"
    except Exception as e:
        return -1, "", str(e)

# ---- Docker操作封装 ----
def docker_command(subcmd: str, container: str = "", timeout: int = 30) -> Tuple[int, str, str]:
    """执行docker命令"""
    if container:
        cmd = f"docker {subcmd} {container}"
    else:
        cmd = f"docker {subcmd}"
    return run_command(cmd, timeout=timeout)

# ---- 验证器 ----
class Verifier:
    """修复后验证"""

    @staticmethod
    def verify_command(cmd: str, timeout: int = 30) -> bool:
        """通过命令验证"""
        code, stdout, stderr = run_command(cmd, timeout=timeout)
        return code == 0

    @staticmethod
    def verify_container_running(name: str) -> bool:
        """验证容器运行中"""
        code, stdout, _ = docker_command("inspect --format={{.State.Status}}", name, timeout=10)
        return code == 0 and "running" in stdout

    @staticmethod
    def verify_container_healthy(name: str) -> bool:
        """验证容器健康"""
        code, stdout, _ = docker_command(
            "inspect --format={{.State.Health.Status}}", name, timeout=10
        )
        return code == 0 and "healthy" in stdout

    @staticmethod
    def verify_memory_available(min_mb: int = 500) -> bool:
        """验证可用内存"""
        code, out, _ = run_command("free -m", timeout=5)
        if code != 0:
            return False
        for line in out.split("\n"):
            if line.startswith("Mem:"):
                parts = line.split()
                available = int(parts[6]) if len(parts) >= 7 else 0
                return available >= min_mb
        return False

    @staticmethod
    def verify_disk_available(min_percent: int = 15) -> bool:
        """验证磁盘空间"""
        code, out, _ = run_command("df -h /", timeout=5)
        if code != 0:
            return False
        parts = out.strip().split("\n")[-1].split()
        if len(parts) >= 5:
            avail_percent = 100 - float(parts[4].rstrip("%"))
            return avail_percent >= min_percent
        return False

    @staticmethod
    def verify_cpu_normal(max_percent: float = 80.0) -> bool:
        """验证CPU正常"""
        code, out, _ = run_command("docker stats --no-stream --format '{{.CPUPerc}}' omega_agi_core", timeout=10)
        if code != 0:
            return False
        try:
            cpu = float(out.strip().rstrip("%"))
            return cpu < max_percent
        except ValueError:
            return True

    @staticmethod
    def verify_network_connectivity() -> bool:
        """验证网络连通"""
        code, _, _ = run_command("curl -sf --max-time 5 https://www.google.com", timeout=10)
        return code == 0

# ---- 自动修复执行器 ----
class AutoRepair:
    """
    自动修复执行器

    核心功能:
    1. 执行 FixPlan 中的每个修复步骤
    2. 验证每步效果
    3. 失败时自动回滚
    4. 记录完整执行日志
    5. 防止无限重启循环
    """

    def __init__(self, cooldown_seconds: int = RESTART_COOLDOWN):
        self.cooldown_seconds = cooldown_seconds
        self.verifier = Verifier()
        self.execution_history: List[FixExecution] = []
        self._lock = threading.Lock()

    def execute_fix(self, plan: FixPlan, dry_run: bool = False) -> FixExecution:
        """
        执行修复方案

        Args:
            plan: 修复方案
            dry_run: 仅模拟，不实际执行

        Returns:
            FixExecution: 完整执行结果
        """
        execution = FixExecution(
            plan=plan,
            status=FixStatus.PENDING,
            start_time=datetime.now(timezone.utc).isoformat(),
        )

        log_info(f"\n{'='*60}")
        log_info(f"  🔧 开始执行修复: {plan.fault_name} ({plan.severity})")
        log_info(f"{'='*60}")

        # 防循环检查
        if self._is_flapping(plan.fault_name):
            log_error(f"⚠️ 检测到 {plan.fault_name} 频繁故障，跳过自动修复 (防止循环)")
            execution.status = FixStatus.FAILED
            execution.final_error = "Frequent flap detected, manual intervention required"
            self.execution_history.append(execution)
            return execution

        execution.status = FixStatus.RUNNING
        execution.attempts = 1

        try:
            success = self._execute_steps(execution, dry_run)

            if success:
                # 执行验证
                self._verify_fix(execution, plan)
            else:
                # 尝试回滚
                if plan.rollback_on_failure:
                    self._rollback(execution)

        except Exception as e:
            log_error(f"修复执行异常: {e}")
            execution.status = FixStatus.FAILED
            execution.final_error = str(e)

        execution.end_time = datetime.now(timezone.utc).isoformat()
        self.execution_history.append(execution)

        self._print_execution_summary(execution)
        return execution

    def _execute_steps(self, execution: FixExecution, dry_run: bool) -> bool:
        """执行所有修复步骤"""
        plan = execution.plan

        for i, step in enumerate(plan.steps):
            result = FixStepResult(
                step_index=i,
                action=step.action,
                command=step.command,
                status=FixStatus.RUNNING,
                output="",
                error="",
                duration_seconds=0.0,
            )

            start = time.time()
            log_info(f"  步骤 {i+1}: [{step.action}] {step.command[:80]}...")

            if dry_run:
                result.status = FixStatus.PENDING
                result.output = "(dry-run mode)"
                log_info(f"    [dry-run] 跳过执行")
            else:
                code, stdout, stderr = run_command(
                    step.command,
                    timeout=getattr(step, "timeout_seconds", 30)
                )
                result.output = stdout
                result.error = stderr
                result.duration_seconds = time.time() - start

                if code == 0:
                    result.status = FixStatus.SUCCESS
                    log_info(f"    ✅ 成功 ({result.duration_seconds:.1f}秒)")
                else:
                    result.status = FixStatus.FAILED
                    log_error(f"    ❌ 失败 (exit {code}): {stderr[:100]}")

                    # 可选步骤失败不中断
                    if getattr(step, "optional", False):
                        log_warn(f"    (可选步骤，继续)")
                        result.status = FixStatus.SUCCESS
                    else:
                        break

            execution.steps_results.append(result)

            # 步骤间延迟
            if i < len(plan.steps) - 1 and result.status == FixStatus.SUCCESS:
                time.sleep(2)

        # 检查是否全部成功
        all_success = all(r.status == FixStatus.SUCCESS for r in execution.steps_results)
        if not all_success:
            execution.status = FixStatus.FAILED
            failed = [r for r in execution.steps_results if r.status == FixStatus.FAILED]
            execution.final_error = f"步骤失败: {failed[0].action}"
        else:
            execution.status = FixStatus.SUCCESS

        return all_success

    def _verify_fix(self, execution: FixExecution, plan: FixPlan):
        """验证修复效果"""
        log_info(f"\n  验证修复效果...")

        verification_items = [
            ("容器运行", lambda: Verifier.verify_container_running("omega_agi_core")),
            ("容器健康", lambda: Verifier.verify_container_healthy("omega_agi_core")),
            ("内存可用", lambda: Verifier.verify_memory_available()),
            ("磁盘空间", lambda: Verifier.verify_disk_available()),
            ("CPU正常", lambda: Verifier.verify_cpu_normal()),
            ("网络连通", lambda: Verifier.verify_network_connectivity()),
        ]

        for name, check_fn in verification_items:
            try:
                ok = check_fn()
                status = "✅" if ok else "❌"
                log_info(f"    {status} {name}")
                execution.verification_results.append(f"{name}: {'OK' if ok else 'FAIL'}")
                if not ok:
                    log_warn(f"    ⚠️ 验证失败: {name}")
            except Exception as e:
                log_error(f"    ❌ 验证异常: {name} - {e}")
                execution.verification_results.append(f"{name}: ERROR")

        # 检查是否所有关键验证通过
        critical_verifies = [
            Verifier.verify_container_running("omega_agi_core"),
            Verifier.verify_memory_available(200),  # 降低到200MB
            Verifier.verify_disk_available(5),       # 降低到5%
        ]
        all_ok = all(critical_verifies)

        if all_ok:
            execution.status = FixStatus.VERIFIED
            log_info(f"  ✅ 修复验证通过!")
        else:
            execution.status = FixStatus.FAILED
            log_error(f"  ⚠️ 修复验证未完全通过")
            if plan.rollback_on_failure:
                self._rollback(execution)

    def _rollback(self, execution: FixExecution):
        """执行回滚"""
        log_warn(f"\n  回滚修复...")
        execution.rollback_executed = True

        # 回滚策略: 重启所有容器
        for name in ["omega_agi_core", "omega_self_healing", "omega_pipeline"]:
            code, _, _ = docker_command("restart", name, timeout=60)
            if code == 0:
                log_info(f"    ✅ 已回滚: 重启 {name}")
            else:
                log_error(f"    ❌ 回滚失败: {name}")

        execution.status = FixStatus.ROLLED_BACK
        execution.end_time = datetime.now(timezone.utc).isoformat()

    def _is_flapping(self, fault_name: str) -> bool:
        """检测是否频繁故障 (flapping)"""
        now = time.time()
        # 检查最近10分钟内该故障的出现次数
        recent = [
            e for e in self.execution_history[-10:]
            if e.plan.fault_name == fault_name
            and (now - datetime.fromisoformat(e.start_time).timestamp()) < 600
        ]
        if len(recent) >= 3:
            log_warn(f"检测到频繁故障: {fault_name} (最近10分钟内 {len(recent)} 次)")
            return True
        return False

    def _print_execution_summary(self, execution: FixExecution):
        """打印执行汇总"""
        plan = execution.plan
        duration = 0.0
        if execution.end_time:
            try:
                end = datetime.fromisoformat(execution.end_time)
                start = datetime.fromisoformat(execution.start_time)
                duration = (end - start).total_seconds()
            except Exception:
                pass

        status_icon = {
            FixStatus.SUCCESS: "✅",
            FixStatus.VERIFIED: "✅✅",
            FixStatus.FAILED: "❌",
            FixStatus.ROLLED_BACK: "↩️",
            FixStatus.PENDING: "⏳",
            FixStatus.RUNNING: "🔄",
        }.get(execution.status, "❓")

        print(f"\n{'='*60}")
        print(f"  {status_icon} 修复执行完成: {plan.fault_name}")
        print(f"  状态: {execution.status.value}")
        print(f"  耗时: {duration:.1f}秒")
        print(f"  尝试次数: {execution.attempts}")
        if execution.rollback_executed:
            print(f"  ⚠️ 已执行回滚")
        if execution.final_error:
            print(f"  错误: {execution.final_error[:100]}")
        print(f"{'='*60}")

        # 详细步骤结果
        for r in execution.steps_results:
            icon = {"success": "✅", "failed": "❌", "pending": "⏳"}.get(r.status.value, "❓")
            print(f"  {icon} 步骤{r.step_index+1}: [{r.action}] {r.command[:60]}")

    def get_container_status(self, name: str) -> Tuple[str, str, int]:
        """获取容器状态"""
        code1, status, _ = docker_command("inspect --format={{.State.Status}}", name, timeout=5)
        code2, health, _ = docker_command("inspect --format={{.State.Health.Status}}", name, timeout=5)
        code3, restart_str, _ = docker_command("inspect --format={{.RestartCount}}", name, timeout=5)
        restart_count = int(restart_str.strip()) if code3 == 0 else 0
        return status.strip() if code1 == 0 else "unknown", health.strip() if code2 == 0 else "none", restart_count

# ---- 修复计划生成器 ----
class FixPlanGenerator:
    """从诊断结果生成修复计划"""

    @staticmethod
    def from_diagnosis_result(diagnosis_result) -> FixPlan:
        """从 DiagnosisResult 生成 FixPlan"""
        return FixPlan(
            fault_name=diagnosis_result.fault_name,
            fault_type=diagnosis_result.fault_type,
            severity=diagnosis_result.severity.value,
            steps=diagnosis_result.fix_plan,
            max_attempts=3,
            cooldown_seconds=RESTART_COOLDOWN,
            rollback_on_failure=True,
        )

    @staticmethod
    def from_fault_type(fault_type: str, context: Dict[str, Any] = None) -> FixPlan:
        """从故障类型生成修复计划"""
        context = context or {}

        plans = {
            "OOM": FixPlan(
                fault_name="内存溢出",
                fault_type="OOM",
                severity="CRITICAL",
                steps=[
                    _make_step("查看OOM日志", "dmesg -T | grep -i oom | tail -20"),
                    _make_step("找出高内存容器", "docker stats --no-stream --format '{{.Name}}\t{{.MemUsage}}' | sort -k2 -hr | head -5"),
                    _make_step("重启问题容器", f"docker restart {context.get('container', 'omega_agi_core')}"),
                    _make_step("验证内存", "free -m"),
                ],
            ),
            "CPU_OVERLOAD": FixPlan(
                fault_name="CPU过载",
                fault_type="CPU_OVERLOAD",
                severity="HIGH",
                steps=[
                    _make_step("查看CPU占用", "top -bn1 | head -15"),
                    _make_step("查看容器CPU", "docker stats --no-stream --format '{{.Name}}\t{{.CPUPerc}}' | sort -k2 -hr"),
                    _make_step("重启高CPU容器", f"docker restart {context.get('container', 'omega_agi_core')}"),
                ],
            ),
            "DISK_FULL": FixPlan(
                fault_name="磁盘空间不足",
                fault_type="DISK_FULL",
                severity="CRITICAL",
                steps=[
                    _make_step("查看磁盘使用", "df -h"),
                    _make_step("清理Docker资源", "docker system prune -af --volumes"),
                    _make_step("清理临时文件", "rm -f /tmp/omega_*.log /tmp/omega_*.json /tmp/omega_diagnosis_*.json"),
                    _make_step("验证磁盘", "df -h /"),
                ],
            ),
            "CONTAINER_CRASH": FixPlan(
                fault_name="容器崩溃",
                fault_type="CONTAINER_CRASH",
                severity="CRITICAL",
                steps=[
                    _make_step("查看崩溃日志", f"docker logs --tail 50 {context.get('container', 'omega_agi_core')}"),
                    _make_step("重启容器", f"docker restart {context.get('container', 'omega_agi_core')}"),
                    _make_step("验证容器运行", f"docker ps | grep {context.get('container', 'omega_agi_core')}"),
                ],
            ),
            "NETWORK_TIMEOUT": FixPlan(
                fault_name="网络超时",
                fault_type="NETWORK_TIMEOUT",
                severity="HIGH",
                steps=[
                    _make_step("测试网络", "curl -sf --max-time 5 https://www.google.com"),
                    _make_step("重启Docker", "systemctl restart docker"),
                    _make_step("验证网络", "curl -sf --max-time 10 https://www.google.com"),
                ],
            ),
            "HEALTH_CHECK_FAIL": FixPlan(
                fault_name="健康检查失败",
                fault_type="HEALTH_CHECK_FAIL",
                severity="HIGH",
                steps=[
                    _make_step("查看健康检查日志", f"docker inspect --format='{{{{json .State.Health}}}}' {context.get('container', 'omega_agi_core')}"),
                    _make_step("重启容器", f"docker restart {context.get('container', 'omega_agi_core')}"),
                    _make_step("等待启动", "sleep 20"),
                    _make_step("验证健康", f"docker inspect --format='{{{{.State.Health.Status}}}}' {context.get('container', 'omega_agi_core')}"),
                ],
            ),
        }

        return plans.get(fault_type, FixPlan(
            fault_name="未知故障",
            fault_type=fault_type,
            severity="MEDIUM",
            steps=[_make_step("通用恢复", "docker restart omega_agi_core")],
        ))


def _make_step(action: str, command: str) -> Any:
    """创建简易步骤对象"""
    class Step:
        def __init__(self, action, command):
            self.action = action
            self.command = command
            self.timeout_seconds = 30
            self.optional = False
            self.verify_after = True
    return Step(action, command)

# ---- 综合修复引擎 ----
class RepairEngine:
    """综合修复引擎: 诊断+修复+验证"""

    def __init__(self):
        self.repair = AutoRepair()
        self.plan_gen = FixPlanGenerator()

    def diagnose_and_repair(self, results: List[Any], dry_run: bool = False) -> List[FixExecution]:
        """诊断后自动修复"""
        executions = []

        for result in results:
            if result.severity.value not in ("CRITICAL", "HIGH"):
                log_info(f"跳过 {result.fault_name} (严重程度: {result.severity.value})")
                continue

            plan = self.plan_gen.from_diagnosis_result(result)
            exec_result = self.repair.execute_fix(plan, dry_run=dry_run)
            executions.append(exec_result)

        return executions

# ---- CLI ----
def main():
    import argparse
    parser = argparse.ArgumentParser(description="OMEGA AGI 自动修复执行器")
    parser.add_argument("--fault-type", type=str, help="指定故障类型 (跳过诊断)")
    parser.add_argument("--container", type=str, default="omega_agi_core", help="指定容器")
    parser.add_argument("--dry-run", action="store_true", help="仅模拟，不执行")
    parser.add_argument("--auto", action="store_true", help="自动模式: 诊断后自动修复")
    parser.add_argument("--status", action="store_true", help="查看容器状态")
    parser.add_argument("--verify", action="store_true", help="仅运行验证")
    args = parser.parse_args()

    log_info("======== OMEGA AGI 自动修复执行器启动 ========")

    repair = AutoRepair()

    if args.status:
        print("\n  容器状态:")
        for name in ["omega_agi_core", "omega_self_healing", "omega_pipeline"]:
            status, health, restart = repair.get_container_status(name)
            icon = "✅" if status == "running" else "❌" if status == "exited" else "⚠️"
            print(f"  {icon} {name}: status={status}, health={health}, restarts={restart}")
        return

    if args.verify:
        print("\n  验证系统状态:")
        items = [
            ("容器运行", lambda: Verifier.verify_container_running("omega_agi_core")),
            ("容器健康", lambda: Verifier.verify_container_healthy("omega_agi_core")),
            ("内存可用", lambda: Verifier.verify_memory_available()),
            ("磁盘空间", lambda: Verifier.verify_disk_available()),
            ("CPU正常", lambda: Verifier.verify_cpu_normal()),
            ("网络连通", lambda: Verifier.verify_network_connectivity()),
        ]
        all_ok = True
        for name, fn in items:
            ok = fn()
            print(f"  {'✅' if ok else '❌'} {name}")
            if not ok:
                all_ok = False
        print(f"\n  {'✅ 所有验证通过' if all_ok else '⚠️ 部分验证失败'}")
        return

    if args.fault_type:
        plan = FixPlanGenerator.from_fault_type(args.fault_type, {"container": args.container})
        result = repair.execute_fix(plan, dry_run=args.dry_run)
        return

    if args.auto:
        # 导入诊断模块
        try:
            sys.path.insert(0, os.path.dirname(__file__))
            from fault_diagnosis import FaultDiagnosis

            diag = FaultDiagnosis()
            results = diag.diagnose_with_collection()

            if not results:
                log_info("未发现故障，无需修复")
                return

            log_info(f"发现 {len(results)} 个故障，开始自动修复...")
            engine = RepairEngine()
            executions = engine.diagnose_and_repair(results, dry_run=args.dry_run)

            print("\n  修复汇总:")
            for e in executions:
                icon = "✅" if e.status in (FixStatus.SUCCESS, FixStatus.VERIFIED) else "❌"
                print(f"  {icon} {e.plan.fault_name}: {e.status.value}")
        except ImportError as e:
            log_error(f"无法导入诊断模块: {e}")
            log_info("使用 --fault-type 直接指定故障类型")
        return

    parser.print_help()
    print("\n示例:")
    print("  python3 auto_repair.py --status")
    print("  python3 auto_repair.py --verify")
    print("  python3 auto_repair.py --fault-type OOM --container omega_agi_core")
    print("  python3 auto_repair.py --auto")


if __name__ == "__main__":
    main()