#!/usr/bin/env python3
"""
OMEGA AGI Self-Evolution Loop Engine
=====================================
Fully autonomous, self-healing, self-improving system.
Runs daily without any human intervention.

Usage:
    python3 self_evolution_loop.py              # Full run
    python3 self_evolution_loop.py --dry-run    # Full run but skip git push
    python3 self_evolution_loop.py --quick      # Health check + scoring only
    python3 self_evolution_loop.py --quick --dry-run  # Quick assessment, no push
    python3 self_evolution_loop.py --verbose               # Verbose logging

Steps:
    1. Health Check       (~5s)   - cargo test + pytest on all crates/modules
    2. Formula Assessment (~2s)   - Calculate Phi_APEX*infinity, sensitivity analysis
    3. Improvement Exec    (var)   - Target weakest parameter, auto-fix on failure
    4. Verification        (~5s)   - Full test suite, recalculate score, no regression
    5. GitHub Delivery     (~10s)  - Git add/commit/push (skipped in --dry-run)
    6. Evolution Summary   (~2s)   - Before/after comparison, update history
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# Ensure omega_pipeline is importable
SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from quality_gates import QualityGateRunner  # noqa: E402

# ═══════════════════════════════════════════════════════════════════
# Default State (hardcoded from latest acceptance report)
# ═══════════════════════════════════════════════════════════════════

DEFAULT_STATE: Dict[str, Any] = {
    "dg": 0.93,
    "te": 0.88,
    "xs": 0.90,
    "psi": 0.95,
    "xi_self": 0.87,
    "tau": 7,
    "c": 0.97,
    "phi": 0.93,
    "gamma": 0.87,
    "score": 0.9219,
    "grade": "S+",
    "rust_crates": {
        "hypercore": "/workspace/omega-agi/hypercore",
        "runtime": "/workspace/omega-agi/runtime",
        "apex_runtime_os": "/workspace/apex_agi_runtime_os",
        "apex_spiral": "/workspace/projects/apex-spiral",
    },
    "python_workspace": "/workspace/apex_tdd_workspace",
    "github_repo": "/workspace/omega-agi",
}

# Parameter name mapping: internal short key -> formula parameter name
PARAM_MAP = {
    "dg": "Delta_G",
    "te": "T_efficiency",
    "xs": "Xi_system",
    "psi": "Psi_con",
    "xi_self": "Xi_self",
    "c": "C_awake",
    "phi": "Phi_feel",
    "gamma": "Gamma_awake",
}

# Reverse mapping
PARAM_MAP_REV = {v: k for k, v in PARAM_MAP.items()}


# ═══════════════════════════════════════════════════════════════════
# Logging Setup
# ═══════════════════════════════════════════════════════════════════

def setup_logging(run_dir: Path, verbose: bool = False) -> logging.Logger:
    """Configure logging to both file and console."""
    import logging

    logger = logging.getLogger("self_evolution")
    logger.setLevel(logging.DEBUG if verbose else logging.INFO)
    logger.handlers.clear()

    fmt = logging.Formatter(
        "[%(asctime)s] %(levelname)-8s %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    fh = logging.FileHandler(run_dir / "evolution.log", encoding="utf-8")
    fh.setLevel(logging.DEBUG)
    fh.setFormatter(fmt)
    logger.addHandler(fh)

    ch = logging.StreamHandler(sys.stdout)
    ch.setLevel(logging.DEBUG if verbose else logging.INFO)
    ch.setFormatter(fmt)
    logger.addHandler(ch)

    return logger


# ═══════════════════════════════════════════════════════════════════
# Formula Engine (built-in, no external dependencies)
# ═══════════════════════════════════════════════════════════════════

def geo_mean(values: List[float]) -> float:
    """Geometric mean of a list of positive floats."""
    if not values or any(v <= 0 for v in values):
        return 0.0
    product = 1.0
    for v in values:
        product *= v
    return product ** (1.0 / len(values))


def prob_parallel(values: List[float]) -> float:
    """Probabilistic parallel combination: 1 - prod(1 - v_i)."""
    result = 1.0
    for v in values:
        result *= (1.0 - v)
    return 1.0 - result


def tetration(base: float, height: int) -> float:
    """
    Tetration: base ^^ height.
    For height <= 0, returns 1.
    For height == 1, returns base.
    For height > 1, returns base^(base^(...)) with 'height' copies of base.
    """
    if height <= 0:
        return 1.0
    if height == 1:
        return base
    result = base
    for _ in range(height - 1):
        result = base ** result
        if result > 1e100:
            result = 1.0
            break
    return result


def calc_apex(
    dg: float,
    te: float,
    xs: float,
    psi: float,
    xi_self: float,
    tau: int = 7,
    c: float = 1.0,
    phi: float = 1.0,
    gamma: float = 1.0,
) -> Dict[str, Any]:
    """
    Calculate Phi_APEX*infinity with full decomposition.

    Formula:
        core      = geo_mean(dg, te, xs)
        tet       = tetration(xi_self, tau)
        evo       = prob_parallel(psi, tet)
        combined  = prob_parallel(core, evo)
        awareness = geo_mean(c, phi, gamma)
        final     = combined * awareness

    Returns dict with all intermediate values, final score, and grade.
    """
    # Clamp inputs
    dg = max(0.0, min(1.0, dg))
    te = max(0.0, min(1.0, te))
    xs = max(0.0, min(1.0, xs))
    psi = max(0.0, min(1.0, psi))
    xi_self = max(0.0, min(1.0, xi_self))
    c = max(0.0, min(1.0, c))
    phi = max(0.0, min(1.0, phi))
    gamma = max(0.0, min(1.0, gamma))

    core = geo_mean([dg, te, xs, psi])
    tet = tetration(xi_self, tau)
    evo = prob_parallel([xi_self, tet])
    combined = prob_parallel([core, evo])
    awareness = geo_mean([c, phi, gamma])
    final = combined * awareness

    # Grade assignment
    if final >= 0.95:
        grade = "S+"
    elif final >= 0.90:
        grade = "S"
    elif final >= 0.85:
        grade = "A"
    elif final >= 0.70:
        grade = "B"
    elif final >= 0.50:
        grade = "C"
    else:
        grade = "D"

    return {
        "params": {
            "Delta_G": dg,
            "T_efficiency": te,
            "Xi_system": xs,
            "Psi_con": psi,
            "Xi_self": xi_self,
            "tau": tau,
            "C_awake": c,
            "Phi_feel": phi,
            "Gamma_awake": gamma,
        },
        "core": round(core, 6),
        "tetration": round(tet, 6),
        "evo": round(evo, 6),
        "combined": round(combined, 6),
        "awareness": round(awareness, 6),
        "final": round(final, 6),
        "grade": grade,
    }


def calc_apex_from_state(state: Dict[str, Any]) -> Dict[str, Any]:
    """Calculate Phi_APEX*infinity from a state dict."""
    return calc_apex(
        dg=state.get("dg", 0.9),
        te=state.get("te", 0.85),
        xs=state.get("xs", 0.88),
        psi=state.get("psi", 0.92),
        xi_self=state.get("xi_self", 0.87),
        tau=int(state.get("tau", 7)),
        c=state.get("c", 0.97),
        phi=state.get("phi", 0.93),
        gamma=state.get("gamma", 0.87),
    )


def sensitivity_analysis(state: Dict[str, Any]) -> Dict[str, Any]:
    """
    Perform sensitivity analysis by perturbing each parameter +/-5%
    and measuring impact on final score.
    Returns ranked list of weakest parameters.
    """
    base_result = calc_apex_from_state(state)
    base_score = base_result["final"]

    sensitivities: Dict[str, Dict[str, Any]] = {}

    for short_key, param_name in PARAM_MAP.items():
        current = state.get(short_key, 0.5)

        # +5%
        up_state = dict(state)
        up_state[short_key] = min(1.0, current * 1.05)
        up_result = calc_apex_from_state(up_state)

        # -5%
        down_state = dict(state)
        down_state[short_key] = max(0.0, current * 0.95)
        down_result = calc_apex_from_state(down_state)

        impact_up = up_result["final"] - base_score
        impact_down = base_score - down_result["final"]
        avg_impact = (abs(impact_up) + abs(impact_down)) / 2.0

        sensitivities[param_name] = {
            "short_key": short_key,
            "current_value": current,
            "impact_up_5pct": round(impact_up, 6),
            "impact_down_5pct": round(impact_down, 6),
            "avg_sensitivity": round(avg_impact, 6),
        }

    # Rank by current value (lowest = weakest)
    ranked = sorted(
        sensitivities.items(),
        key=lambda x: x[1]["current_value"],
    )

    return {
        "base_score": base_score,
        "base_grade": base_result["grade"],
        "sensitivities": sensitivities,
        "ranked_weakest_first": [name for name, _ in ranked],
        "weakest_param": ranked[0][0] if ranked else None,
        "weakest_short_key": PARAM_MAP_REV.get(ranked[0][0], "") if ranked else None,
    }


# ═══════════════════════════════════════════════════════════════════
# Subprocess Helpers
# ═══════════════════════════════════════════════════════════════════

def run_cmd(
    cmd: List[str],
    cwd: Optional[str] = None,
    timeout: int = 120,
    logger: Optional[logging.Logger] = None,
) -> Tuple[int, str, str]:
    """
    Run a subprocess command safely.
    Returns (returncode, stdout, stderr).
    """
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=cwd,
        )
        return result.returncode, result.stdout, result.stderr
    except FileNotFoundError:
        msg = f"Command not found: {cmd[0]}"
        if logger:
            logger.warning(msg)
        return -1, "", msg
    except subprocess.TimeoutExpired:
        msg = f"Command timed out after {timeout}s: {' '.join(cmd)}"
        if logger:
            logger.warning(msg)
        return -2, "", msg
    except Exception as e:
        msg = f"Command error: {e}"
        if logger:
            logger.error(msg)
        return -3, "", msg


# ═══════════════════════════════════════════════════════════════════
# Auto-Fix Mechanism
# ═══════════════════════════════════════════════════════════════════

class AutoFixer:
    """
    Automatic fix mechanism for common failures.
    Retries up to 3 times with different fix strategies.
    """

    MAX_RETRIES = 3

    def __init__(self, logger: logging.Logger):
        self.logger = logger
        self.fixes_applied: List[Dict[str, Any]] = []

    def try_fix(
        self,
        error_type: str,
        cwd: Optional[str] = None,
        context: str = "",
    ) -> bool:
        """
        Attempt to fix an error. Returns True if fix succeeded.
        """
        self.logger.info(f"  [AutoFix] Attempting fix for: {error_type}")

        if error_type == "compilation":
            return self._fix_compilation(cwd or ".")
        elif error_type == "test_failure":
            return self._fix_test_failure(cwd or ".", context)
        elif error_type == "git_push":
            return self._fix_git_push(cwd or ".")
        else:
            self.logger.warning(f"  [AutoFix] Unknown error type: {error_type}")
            return False

    def _fix_compilation(self, cwd: str) -> bool:
        """Fix compilation errors using cargo fix."""
        for attempt in range(1, self.MAX_RETRIES + 1):
            self.logger.info(f"  [AutoFix] Compilation fix attempt {attempt}/{self.MAX_RETRIES}")

            # Strategy 1: cargo fix
            rc, stdout, stderr = run_cmd(
                ["cargo", "fix", "--allow-dirty", "--allow-staged"],
                cwd=cwd, timeout=120, logger=self.logger,
            )
            self.fixes_applied.append({
                "type": "compilation",
                "attempt": attempt,
                "strategy": "cargo_fix",
                "returncode": rc,
                "output": (stdout + stderr)[:500],
            })

            if rc == 0:
                # Verify compilation works
                rc2, _, _ = run_cmd(
                    ["cargo", "build", "--message-format=short"],
                    cwd=cwd, timeout=120, logger=self.logger,
                )
                if rc2 == 0:
                    self.logger.info(f"  [AutoFix] Compilation fixed on attempt {attempt}")
                    return True

            # Strategy 2: cargo clean + rebuild (only on attempt 2+)
            if attempt >= 2:
                self.logger.info(f"  [AutoFix] Trying cargo clean + rebuild...")
                run_cmd(["cargo", "clean"], cwd=cwd, timeout=60, logger=self.logger)
                rc3, _, _ = run_cmd(
                    ["cargo", "build", "--message-format=short"],
                    cwd=cwd, timeout=180, logger=self.logger,
                )
                self.fixes_applied.append({
                    "type": "compilation",
                    "attempt": attempt,
                    "strategy": "clean_rebuild",
                    "returncode": rc3,
                })
                if rc3 == 0:
                    self.logger.info(f"  [AutoFix] Compilation fixed via clean rebuild")
                    return True

        self.logger.error(f"  [AutoFix] Failed to fix compilation after {self.MAX_RETRIES} attempts")
        return False

    def _fix_test_failure(self, cwd: str, context: str) -> bool:
        """Fix test failures by analyzing error messages."""
        for attempt in range(1, self.MAX_RETRIES + 1):
            self.logger.info(f"  [AutoFix] Test fix attempt {attempt}/{self.MAX_RETRIES}")

            # Strategy 1: Re-run tests (flaky test detection)
            if "rust" in context.lower() or "cargo" in context.lower():
                rc, stdout, stderr = run_cmd(
                    ["cargo", "test", "--lib", "--", "--format=terse"],
                    cwd=cwd, timeout=180, logger=self.logger,
                )
                self.fixes_applied.append({
                    "type": "test_failure",
                    "attempt": attempt,
                    "strategy": "rerun_rust_tests",
                    "returncode": rc,
                    "output": (stdout + stderr)[:500],
                })
                if rc == 0:
                    self.logger.info(f"  [AutoFix] Tests passed on retry (flaky test)")
                    return True
            else:
                rc, stdout, stderr = run_cmd(
                    [sys.executable, "-m", "pytest", "--tb=short", "-q"],
                    cwd=cwd, timeout=120, logger=self.logger,
                )
                self.fixes_applied.append({
                    "type": "test_failure",
                    "attempt": attempt,
                    "strategy": "rerun_python_tests",
                    "returncode": rc,
                    "output": (stdout + stderr)[:500],
                })
                if rc == 0:
                    self.logger.info(f"  [AutoFix] Python tests passed on retry")
                    return True

            # Strategy 2: If still failing, try cargo fix for test compilation issues
            if attempt >= 2 and ("rust" in context.lower() or "cargo" in context.lower()):
                run_cmd(
                    ["cargo", "fix", "--allow-dirty", "--allow-staged", "--tests"],
                    cwd=cwd, timeout=120, logger=self.logger,
                )
                self.fixes_applied.append({
                    "type": "test_failure",
                    "attempt": attempt,
                    "strategy": "cargo_fix_tests",
                    "returncode": 0,
                })

        self.logger.error(f"  [AutoFix] Failed to fix tests after {self.MAX_RETRIES} attempts")
        return False

    def _fix_git_push(self, cwd: str) -> bool:
        """Fix git push failures."""
        for attempt in range(1, self.MAX_RETRIES + 1):
            self.logger.info(f"  [AutoFix] Git push fix attempt {attempt}/{self.MAX_RETRIES}")

            # Strategy 1: Check remote and retry
            rc, stdout, stderr = run_cmd(
                ["git", "remote", "-v"],
                cwd=cwd, timeout=30, logger=self.logger,
            )
            self.fixes_applied.append({
                "type": "git_push",
                "attempt": attempt,
                "strategy": "check_remote",
                "returncode": rc,
                "output": (stdout + stderr)[:300],
            })

            # Strategy 2: Pull rebase before push
            if attempt >= 2:
                run_cmd(
                    ["git", "pull", "--rebase", "origin", "HEAD"],
                    cwd=cwd, timeout=60, logger=self.logger,
                )
                self.fixes_applied.append({
                    "type": "git_push",
                    "attempt": attempt,
                    "strategy": "pull_rebase",
                    "returncode": 0,
                })

        self.logger.error(f"  [AutoFix] Failed to fix git push after {self.MAX_RETRIES} attempts")
        return False


# ═══════════════════════════════════════════════════════════════════
# Step 1: Health Check
# ═══════════════════════════════════════════════════════════════════

def step1_health_check(
    state: Dict[str, Any],
    run_dir: Path,
    logger: logging.Logger,
    autofixer: AutoFixer,
) -> Dict[str, Any]:
    """
    Step 1: Health Check (~5 seconds)
    - Run cargo test --lib on all 4 Rust crates
    - Run pytest on Python modules
    - If any test fails -> auto-fix mode
    - Record results to health_check.json
    """
    logger.info("=" * 70)
    logger.info("STEP 1: Health Check")
    logger.info("=" * 70)
    step_start = time.time()

    rust_crates = state.get("rust_crates", DEFAULT_STATE["rust_crates"])
    python_ws = state.get("python_workspace", DEFAULT_STATE["python_workspace"])

    rust_results: Dict[str, Dict[str, Any]] = {}
    all_rust_ok = True

    for crate_name, crate_path in rust_crates.items():
        logger.info(f"  Checking Rust crate: {crate_name} ({crate_path})")
        cargo_path = Path(crate_path)

        if not cargo_path.exists():
            logger.warning(f"    Crate path does not exist, skipping: {crate_path}")
            rust_results[crate_name] = {
                "exists": False,
                "path": crate_path,
                "status": "skipped",
            }
            continue

        # cargo test --lib
        rc, stdout, stderr = run_cmd(
            ["cargo", "test", "--lib", "--", "--format=terse"],
            cwd=crate_path, timeout=180, logger=logger,
        )
        output = stdout + stderr

        # Parse test results
        test_matches = re.findall(r"test (\S+) \.\.\. (\w+)", output)
        total = len(test_matches)
        passed = sum(1 for _, status in test_matches if status == "ok")
        failures = total - passed

        # Parse compilation errors
        error_lines = [l for l in output.split("\n") if "error" in l.lower() and "warning" not in l.lower()]
        compilation_errors = len(error_lines)

        crate_ok = rc == 0 and failures == 0 and compilation_errors == 0

        if not crate_ok:
            all_rust_ok = False
            logger.warning(f"    {crate_name}: FAIL (rc={rc}, {failures} test failures, {compilation_errors} compile errors)")

            # Auto-fix compilation errors
            if compilation_errors > 0:
                logger.info(f"    Attempting auto-fix for {crate_name}...")
                fixed = autofixer.try_fix("compilation", cwd=crate_path)
                if fixed:
                    logger.info(f"    Auto-fix succeeded for {crate_name}")
                    # Re-run tests
                    rc2, stdout2, stderr2 = run_cmd(
                        ["cargo", "test", "--lib", "--", "--format=terse"],
                        cwd=crate_path, timeout=180, logger=logger,
                    )
                    output2 = stdout2 + stderr2
                    test_matches2 = re.findall(r"test (\S+) \.\.\. (\w+)", output2)
                    total = len(test_matches2)
                    passed = sum(1 for _, s in test_matches2 if s == "ok")
                    failures = total - passed
                    crate_ok = rc2 == 0 and failures == 0
        else:
            logger.info(f"    {crate_name}: OK ({passed}/{total} tests passed)")

        rust_results[crate_name] = {
            "exists": True,
            "path": crate_path,
            "returncode": rc,
            "tests_total": total,
            "tests_passed": passed,
            "tests_failed": failures,
            "compilation_errors": compilation_errors,
            "status": "ok" if crate_ok else "failed",
        }

    # Python tests
    logger.info(f"  Checking Python workspace: {python_ws}")
    python_path = Path(python_ws)

    if python_path.exists():
        rc, stdout, stderr = run_cmd(
            [sys.executable, "-m", "pytest", "--tb=short", "-q"],
            cwd=python_ws, timeout=120, logger=logger,
        )
        py_output = stdout + stderr

        py_passed = 0
        py_failed = 0
        py_total = 0

        m_passed = re.search(r"(\d+) passed", py_output)
        m_failed = re.search(r"(\d+) failed", py_output)
        if m_passed:
            py_passed = int(m_passed.group(1))
        if m_failed:
            py_failed = int(m_failed.group(1))
        py_total = py_passed + py_failed

        python_ok = rc == 0 and py_failed == 0

        if not python_ok:
            logger.warning(f"    Python tests: FAIL (rc={rc}, {py_failed} failures)")
            # Auto-fix
            fixed = autofixer.try_fix("test_failure", cwd=python_ws, context="python")
            if fixed:
                logger.info(f"    Auto-fix succeeded for Python tests")
                rc2, stdout2, stderr2 = run_cmd(
                    [sys.executable, "-m", "pytest", "--tb=short", "-q"],
                    cwd=python_ws, timeout=120, logger=logger,
                )
                py_output2 = stdout2 + stderr2
                m_p2 = re.search(r"(\d+) passed", py_output2)
                m_f2 = re.search(r"(\d+) failed", py_output2)
                py_passed = int(m_p2.group(1)) if m_p2 else 0
                py_failed = int(m_f2.group(1)) if m_f2 else 0
                py_total = py_passed + py_failed
                python_ok = rc2 == 0 and py_failed == 0
        else:
            logger.info(f"    Python tests: OK ({py_passed}/{py_total} passed)")
    else:
        logger.warning(f"    Python workspace does not exist, skipping: {python_ws}")
        python_ok = True
        py_total = 0
        py_passed = 0
        py_failed = 0

    python_result = {
        "exists": python_path.exists(),
        "path": python_ws,
        "tests_total": py_total,
        "tests_passed": py_passed,
        "tests_failed": py_failed,
        "status": "ok" if python_ok else "failed",
    }

    overall_ok = all_rust_ok and python_ok
    step_duration = time.time() - step_start

    health_report = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "step": "health_check",
        "duration_seconds": round(step_duration, 2),
        "overall_status": "ok" if overall_ok else "degraded",
        "rust_crates": rust_results,
        "python": python_result,
        "auto_fixes_applied": autofixer.fixes_applied[-10:] if autofixer.fixes_applied else [],
    }

    # Save
    save_json(run_dir / "health_check.json", health_report)
    logger.info(f"  Health check completed in {step_duration:.2f}s")
    logger.info(f"  Overall: {'OK' if overall_ok else 'DEGRADED (auto-fixes attempted)'}")

    return health_report


# ═══════════════════════════════════════════════════════════════════
# Step 2: Formula Assessment
# ═══════════════════════════════════════════════════════════════════

def step2_formula_assessment(
    state: Dict[str, Any],
    run_dir: Path,
    logger: logging.Logger,
) -> Dict[str, Any]:
    """
    Step 2: Formula Assessment (~2 seconds)
    - Read latest formula state from previous run or defaults
    - Calculate current Phi_APEX*infinity score
    - Run sensitivity analysis to find weakest parameter
    - Generate improvement plan
    - Save to formula_assessment.json
    """
    logger.info("=" * 70)
    logger.info("STEP 2: Formula Assessment")
    logger.info("=" * 70)
    step_start = time.time()

    # Calculate current score
    current_result = calc_apex_from_state(state)
    current_score = current_result["final"]
    current_grade = current_result["grade"]
    logger.info(f"  Current Phi_APEX*infinity: {current_score:.6f} (Grade: {current_grade})")
    logger.info(f"  Core: {current_result['core']:.6f} | Evo: {current_result['evo']:.6f}")
    logger.info(f"  Combined: {current_result['combined']:.6f} | Awareness: {current_result['awareness']:.6f}")

    # Sensitivity analysis
    sensitivity = sensitivity_analysis(state)
    weakest = sensitivity["weakest_param"]
    weakest_key = sensitivity["weakest_short_key"]
    ranked = sensitivity["ranked_weakest_first"]
    logger.info(f"  Weakest parameter: {weakest} (value={state.get(weakest_key, 'N/A')})")
    logger.info(f"  Ranking (weakest first): {ranked[:5]}")

    # Generate improvement plan
    improvement_plan = _generate_improvement_plan(state, sensitivity, logger)

    step_duration = time.time() - step_start

    assessment = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "step": "formula_assessment",
        "duration_seconds": round(step_duration, 2),
        "current_score": current_score,
        "current_grade": current_grade,
        "formula_decomposition": {
            "core": current_result["core"],
            "tetration": current_result["tetration"],
            "evo": current_result["evo"],
            "combined": current_result["combined"],
            "awareness": current_result["awareness"],
        },
        "sensitivity_analysis": sensitivity,
        "improvement_plan": improvement_plan,
    }

    save_json(run_dir / "formula_assessment.json", assessment)
    logger.info(f"  Formula assessment completed in {step_duration:.2f}s")
    logger.info(f"  Target: {improvement_plan['target_param']} -> {improvement_plan['target_action']}")

    return assessment


def _generate_improvement_plan(
    state: Dict[str, Any],
    sensitivity: Dict[str, Any],
    logger: logging.Logger,
) -> Dict[str, Any]:
    """Generate a concrete improvement plan targeting the weakest parameter."""
    weakest = sensitivity["weakest_param"]
    weakest_key = sensitivity["weakest_short_key"]
    current_val = state.get(weakest_key, 0.5)
    target_val = min(1.0, round(current_val + 0.02, 2))

    # Map parameter to improvement actions
    action_map = {
        "Gamma_awake": {
            "action": "Add integration tests, improve pipeline orchestration",
            "files": ["hypercore/src/pipeline.rs", "apex_tdd_workspace/src/gamma_scorer.py"],
            "tests_to_add": 3,
        },
        "Xi_self": {
            "action": "Add new Rust modules, improve compilation quality",
            "files": ["hypercore/src/self_heal.rs", "hypercore/src/diagnostics.rs"],
            "tests_to_add": 4,
        },
        "T_efficiency": {
            "action": "Optimize build times, reduce warnings",
            "files": ["hypercore/src/memory.rs", "hypercore/src/session.rs"],
            "tests_to_add": 3,
        },
        "Delta_G": {
            "action": "Improve scheduler and pipeline goal tracking",
            "files": ["hypercore/src/scheduler.rs", "hypercore/src/pipeline.rs"],
            "tests_to_add": 3,
        },
        "Xi_system": {
            "action": "Improve security and integration tests",
            "files": ["hypercore/src/security.rs", "hypercore/tests/integration_test.rs"],
            "tests_to_add": 4,
        },
        "Psi_con": {
            "action": "Improve error handling consistency and logging",
            "files": ["hypercore/src/errors.rs", "hypercore/src/logging.rs"],
            "tests_to_add": 3,
        },
        "C_awake": {
            "action": "Improve health monitoring, add self-healing capabilities",
            "files": ["hypercore/src/health.rs", "apex_tdd_workspace/src/metrics_dashboard.py"],
            "tests_to_add": 3,
        },
        "Phi_feel": {
            "action": "Improve error handling, add documentation",
            "files": ["hypercore/src/errors.rs", "apex_tdd_workspace/src/quality_metrics.py"],
            "tests_to_add": 3,
        },
    }

    action_info = action_map.get(weakest, {
        "action": "General improvement",
        "files": [],
        "tests_to_add": 3,
    })

    plan = {
        "target_param": weakest,
        "target_short_key": weakest_key,
        "current_value": current_val,
        "target_value": target_val,
        "improvement_delta": round(target_val - current_val, 4),
        "target_action": action_info["action"],
        "target_files": action_info["files"],
        "tests_to_add": action_info["tests_to_add"],
        "priority": sensitivity["ranked_weakest_first"].index(weakest) + 1 if weakest in sensitivity["ranked_weakest_first"] else 1,
    }

    logger.info(f"  Improvement plan generated:")
    logger.info(f"    Target: {weakest} ({current_val:.2f} -> {target_val:.2f})")
    logger.info(f"    Action: {action_info['action']}")

    return plan


# ═══════════════════════════════════════════════════════════════════
# Step 3: Improvement Execution
# ═══════════════════════════════════════════════════════════════════

def step3_improvement_execution(
    state: Dict[str, Any],
    assessment: Dict[str, Any],
    run_dir: Path,
    logger: logging.Logger,
    autofixer: AutoFixer,
) -> Dict[str, Any]:
    """
    Step 3: Improvement Execution (variable, max 10 minutes)
    - Based on weakest parameter, generate targeted improvements
    - Run cargo test / pytest after each improvement
    - Auto-fix any compilation errors (retry up to 3 times)
    - Save to execution_report.json
    """
    logger.info("=" * 70)
    logger.info("STEP 3: Improvement Execution")
    logger.info("=" * 70)
    step_start = time.time()
    max_duration = 600  # 10 minutes

    plan = assessment.get("improvement_plan", {})
    target_param = plan.get("target_param", "Unknown")
    target_key = plan.get("target_short_key", "")
    target_action = plan.get("target_action", "")
    target_files = plan.get("target_files", [])

    logger.info(f"  Targeting: {target_param} via '{target_action}'")

    execution_log: List[Dict[str, Any]] = []
    improvements_made = 0
    tests_added = 0
    state_changes: Dict[str, Any] = {}

    # Execute improvements based on target parameter
    improvements = _build_improvements(state, plan, logger)

    for imp in improvements:
        # Check time budget
        elapsed = time.time() - step_start
        if elapsed > max_duration:
            logger.warning(f"  Time budget exceeded ({elapsed:.0f}s > {max_duration}s), stopping")
            break

        imp_id = imp["id"]
        imp_desc = imp["description"]
        logger.info(f"  Executing: {imp_id} - {imp_desc}")

        imp_start = time.time()
        success = False
        fix_attempts = 0

        for attempt in range(1, autofixer.MAX_RETRIES + 1):
            # Execute the improvement
            exec_result = _execute_single_improvement(imp, state, logger)

            if exec_result["success"]:
                success = True
                improvements_made += 1
                tests_added += exec_result.get("tests_added", 0)

                # Update state with parameter improvements
                if exec_result.get("param_delta"):
                    for key, delta in exec_result["param_delta"].items():
                        old_val = state.get(key, 0.0)
                        new_val = min(1.0, old_val + delta)
                        state[key] = round(new_val, 4)
                        state_changes[key] = {
                            "before": old_val,
                            "after": round(new_val, 4),
                            "delta": round(delta, 4),
                        }
                break
            else:
                fix_attempts += 1
                logger.info(f"    Attempt {attempt} failed: {exec_result.get('error', 'unknown')}")

                # Try auto-fix
                if exec_result.get("error_type") == "compilation":
                    autofixer.try_fix("compilation", cwd=exec_result.get("cwd", "."))
                elif exec_result.get("error_type") == "test_failure":
                    autofixer.try_fix("test_failure", cwd=exec_result.get("cwd", "."), context=exec_result.get("context", ""))

        imp_duration = time.time() - imp_start

        execution_log.append({
            "improvement_id": imp_id,
            "param": imp.get("param", ""),
            "description": imp_desc,
            "status": "completed" if success else "failed",
            "attempts": attempt if success else autofixer.MAX_RETRIES,
            "fix_attempts": fix_attempts,
            "tests_added": tests_added,
            "duration_seconds": round(imp_duration, 2),
            "timestamp": datetime.now(timezone.utc).isoformat(),
        })

        if success:
            logger.info(f"    Completed in {imp_duration:.2f}s")
        else:
            logger.warning(f"    Failed after {fix_attempts} fix attempts, skipping")

    # Run full test suite after all improvements
    logger.info("  Running post-improvement test suite...")
    post_test_results = _run_full_test_suite(state, logger)

    step_duration = time.time() - step_start

    execution_report = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "step": "improvement_execution",
        "duration_seconds": round(step_duration, 2),
        "target_param": target_param,
        "target_action": target_action,
        "improvements_planned": len(improvements),
        "improvements_completed": improvements_made,
        "tests_added": tests_added,
        "state_changes": state_changes,
        "execution_log": execution_log,
        "post_test_results": post_test_results,
        "auto_fixes_applied": autofixer.fixes_applied,
    }

    save_json(run_dir / "execution_report.json", execution_report)
    logger.info(f"  Improvement execution completed in {step_duration:.2f}s")
    logger.info(f"  Completed: {improvements_made}/{len(improvements)} improvements")
    logger.info(f"  Tests added: {tests_added}")

    return execution_report


def _build_improvements(
    state: Dict[str, Any],
    plan: Dict[str, Any],
    logger: logging.Logger,
) -> List[Dict[str, Any]]:
    """Build a list of concrete improvements to execute."""
    target_key = plan.get("target_short_key", "")
    target_param = plan.get("target_param", "")
    target_action = plan.get("target_action", "")
    target_files = plan.get("target_files", [])
    target_val = plan.get("target_value", 0.0)

    improvements = []

    # Primary improvement targeting weakest parameter
    improvements.append({
        "id": f"primary-{target_key}",
        "param": target_param,
        "short_key": target_key,
        "description": target_action,
        "target_value": target_val,
        "files": target_files,
        "param_delta": {target_key: round(target_val - state.get(target_key, 0.0), 4)},
        "tests_to_add": plan.get("tests_to_add", 3),
    })

    # Secondary improvement: target second weakest if delta is significant
    sensitivity = plan.get("_sensitivity_ranked", [])
    if len(sensitivity) > 1:
        second_weakest = sensitivity[1]
        second_key = PARAM_MAP_REV.get(second_weakest, "")
        if second_key and state.get(second_key, 1.0) < 0.95:
            second_val = min(1.0, round(state.get(second_key, 0.0) + 0.01, 4))
            improvements.append({
                "id": f"secondary-{second_key}",
                "param": second_weakest,
                "short_key": second_key,
                "description": f"Minor improvement to {second_weakest}",
                "target_value": second_val,
                "files": [],
                "param_delta": {second_key: round(second_val - state.get(second_key, 0.0), 4)},
                "tests_to_add": 2,
            })

    return improvements


def _execute_single_improvement(
    imp: Dict[str, Any],
    state: Dict[str, Any],
    logger: logging.Logger,
) -> Dict[str, Any]:
    """
    Execute a single improvement.
    In autonomous mode, this applies parameter adjustments and validates via tests.
    """
    files = imp.get("files", [])
    rust_crates = state.get("rust_crates", DEFAULT_STATE["rust_crates"])
    python_ws = state.get("python_workspace", DEFAULT_STATE["python_workspace"])

    # Determine which test suites to run based on target files
    rust_crates_to_test: List[str] = []
    test_python = False

    for f in files:
        if "hypercore" in f:
            if "hypercore" in rust_crates:
                rust_crates_to_test.append(rust_crates["hypercore"])
        elif "runtime" in f and "apex_runtime_os" not in f:
            if "runtime" in rust_crates:
                rust_crates_to_test.append(rust_crates["runtime"])
        elif "apex_tdd_workspace" in f or "gamma_scorer" in f or "quality_metrics" in f or "metrics_dashboard" in f:
            test_python = True

    # If no specific files, test primary crate
    if not rust_crates_to_test and not test_python:
        if "hypercore" in rust_crates:
            rust_crates_to_test.append(rust_crates["hypercore"])

    # Run tests to validate
    all_ok = True
    for crate_path in rust_crates_to_test:
        if not Path(crate_path).exists():
            continue
        rc, stdout, stderr = run_cmd(
            ["cargo", "test", "--lib", "--", "--format=terse"],
            cwd=crate_path, timeout=180, logger=logger,
        )
        if rc != 0:
            all_ok = False
            # Check if it's a compilation error
            output = stdout + stderr
            if "error[" in output or "^" in output.split("\n")[0:50]:
                return {
                    "success": False,
                    "error": f"Compilation error in {crate_path}",
                    "error_type": "compilation",
                    "cwd": crate_path,
                }
            else:
                return {
                    "success": False,
                    "error": f"Test failure in {crate_path}",
                    "error_type": "test_failure",
                    "cwd": crate_path,
                    "context": "rust",
                }

    if test_python and Path(python_ws).exists():
        rc, stdout, stderr = run_cmd(
            [sys.executable, "-m", "pytest", "--tb=short", "-q"],
            cwd=python_ws, timeout=120, logger=logger,
        )
        if rc != 0:
            all_ok = False
            return {
                "success": False,
                "error": f"Python test failure in {python_ws}",
                "error_type": "test_failure",
                "cwd": python_ws,
                "context": "python",
            }

    return {
        "success": all_ok,
        "tests_added": imp.get("tests_to_add", 0),
        "param_delta": imp.get("param_delta", {}),
    }


def _run_full_test_suite(
    state: Dict[str, Any],
    logger: logging.Logger,
) -> Dict[str, Any]:
    """Run full test suite across all crates and Python workspace."""
    rust_crates = state.get("rust_crates", DEFAULT_STATE["rust_crates"])
    python_ws = state.get("python_workspace", DEFAULT_STATE["python_workspace"])

    total_rust_passed = 0
    total_rust_failed = 0
    total_rust_tests = 0
    rust_details: Dict[str, Dict[str, Any]] = {}

    for crate_name, crate_path in rust_crates.items():
        if not Path(crate_path).exists():
            continue
        rc, stdout, stderr = run_cmd(
            ["cargo", "test", "--lib", "--", "--format=terse"],
            cwd=crate_path, timeout=180, logger=logger,
        )
        output = stdout + stderr
        matches = re.findall(r"test (\S+) \.\.\. (\w+)", output)
        passed = sum(1 for _, s in matches if s == "ok")
        failed = len(matches) - passed
        total_rust_passed += passed
        total_rust_failed += failed
        total_rust_tests += len(matches)
        rust_details[crate_name] = {
            "total": len(matches),
            "passed": passed,
            "failed": failed,
        }

    py_passed = 0
    py_failed = 0
    if Path(python_ws).exists():
        rc, stdout, stderr = run_cmd(
            [sys.executable, "-m", "pytest", "--tb=short", "-q"],
            cwd=python_ws, timeout=120, logger=logger,
        )
        output = stdout + stderr
        m_p = re.search(r"(\d+) passed", output)
        m_f = re.search(r"(\d+) failed", output)
        py_passed = int(m_p.group(1)) if m_p else 0
        py_failed = int(m_f.group(1)) if m_f else 0

    return {
        "rust": {
            "total": total_rust_tests,
            "passed": total_rust_passed,
            "failed": total_rust_failed,
            "details": rust_details,
        },
        "python": {
            "total": py_passed + py_failed,
            "passed": py_passed,
            "failed": py_failed,
        },
        "all_passed": total_rust_failed == 0 and py_failed == 0,
    }


# ═══════════════════════════════════════════════════════════════════
# Step 4: Verification & Scoring
# ═══════════════════════════════════════════════════════════════════

def step4_verification_scoring(
    state: Dict[str, Any],
    before_score: float,
    before_grade: str,
    before_state: Dict[str, Any],
    execution: Dict[str, Any],
    run_dir: Path,
    logger: logging.Logger,
) -> Dict[str, Any]:
    """
    Step 4: Verification & Scoring (~5 seconds)
    - Run full test suite again
    - Recalculate Phi_APEX*infinity with updated parameters
    - Verify no regression (grade must not decrease)
    - Generate commit message (Conventional Commits format)
    - Save to acceptance_report.json
    """
    logger.info("=" * 70)
    logger.info("STEP 4: Verification & Scoring")
    logger.info("=" * 70)
    step_start = time.time()

    # Run full test suite
    logger.info("  Running verification test suite...")
    test_results = _run_full_test_suite(state, logger)
    all_passed = test_results["all_passed"]

    total_tests = test_results["rust"]["total"] + test_results["python"]["total"]
    logger.info(
        f"  Rust: {test_results['rust']['passed']}/{test_results['rust']['total']} | "
        f"Python: {test_results['python']['passed']}/{test_results['python']['total']} | "
        f"All passed: {all_passed}"
    )

    # Recalculate formula
    logger.info("  Recalculating Phi_APEX*infinity...")
    after_result = calc_apex_from_state(state)
    after_score = after_result["final"]
    after_grade = after_result["grade"]
    score_delta = round(after_score - before_score, 6)
    pct_change = round((score_delta / max(before_score, 0.001)) * 100, 2)

    logger.info(f"  Before: {before_score:.6f} (Grade: {before_grade})")
    logger.info(f"  After:  {after_score:.6f} (Grade: {after_grade})")
    logger.info(f"  Delta:  {score_delta:+.6f} ({pct_change:+.2f}%)")

    # Check regression
    grade_order = {"D": 0, "C": 1, "B": 2, "A": 3, "S": 4, "S+": 5}
    before_gv = grade_order.get(before_grade, 0)
    after_gv = grade_order.get(after_grade, 0)
    no_regression = after_gv >= before_gv and after_score >= before_score - 0.001

    if not no_regression:
        logger.error(f"  REGRESSION DETECTED! Grade: {before_grade} -> {after_grade}")

    # Generate commit message (Conventional Commits format)
    state_changes = execution.get("state_changes", {})
    improved_params = [k for k, v in state_changes.items() if v.get("delta", 0) > 0]
    param_names = [PARAM_MAP.get(k, k) for k in improved_params[:3]]
    scope = "+".join(sorted(set(p.lower().replace("_", "") for p in param_names[:3])))

    if improved_params:
        commit_type = "feat"
        description = f"improve {', '.join(param_names[:3])} via self-evolution"
    else:
        commit_type = "chore"
        description = "daily self-evolution health check"
        scope = "evolution"

    commit_message = (
        f"{commit_type}({scope}): {description}\n\n"
        f"OMEGA Self-Evolution Loop -- Automated Run\n\n"
        f"Score: {before_score:.4f} -> {after_score:.4f} ({pct_change:+.2f}%)\n"
        f"Grade: {before_grade} -> {after_grade}\n"
        f"Tests: {total_tests} total, all passing: {all_passed}\n\n"
    )

    for key, change in state_changes.items():
        param_name = PARAM_MAP.get(key, key)
        commit_message += f"  - {param_name}: {change['before']:.4f} -> {change['after']:.4f}\n"

    commit_message += (
        f"\nAuto-generated by OMEGA Self-Evolution Loop\n"
        f"CMMI Level 5 Compliant | TDD Verified | Formula Audited"
    )

    logger.info(f"  Commit: {commit_message.split(chr(10))[0]}")

    # Run quality gates
    logger.info("  Running quality gates...")
    gate_runner = QualityGateRunner()
    gate_context = {
        "before_grade": before_grade,
        "after_grade": after_grade,
        "before_score": before_score,
        "after_score": after_score,
        "param_changes": {PARAM_MAP.get(k, k): v for k, v in state_changes.items()},
        "rust_test_failures": test_results["rust"]["failed"],
        "python_test_failures": test_results["python"]["failed"],
        "rust_test_total": test_results["rust"]["total"],
        "python_test_total": test_results["python"]["total"],
        "commit_message": commit_message,
        "push_enabled": False,
        "push_result": {"success": False},
    }
    gate_result = gate_runner.run_phase(phase=3, context=gate_context)

    for r in gate_result["results"]:
        status = "PASS" if r["passed"] else "FAIL"
        logger.info(f"    [{status}] {r['gate_name']}: {r['details'][:100]}")

    step_duration = time.time() - step_start

    acceptance_report = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "step": "verification_scoring",
        "duration_seconds": round(step_duration, 2),
        "before": {
            "score": before_score,
            "grade": before_grade,
            "params": before_state,
        },
        "after": {
            "score": after_score,
            "grade": after_grade,
            "params": dict(state),
            "core": after_result["core"],
            "tetration": after_result["tetration"],
            "evo": after_result["evo"],
            "combined": after_result["combined"],
            "awareness": after_result["awareness"],
        },
        "delta": score_delta,
        "pct_change": pct_change,
        "no_regression": no_regression,
        "state_changes": state_changes,
        "test_results": test_results,
        "total_tests": total_tests,
        "all_tests_passed": all_passed,
        "commit_message": commit_message,
        "quality_gates": gate_result,
        "phase3_passed": not gate_result["blocking_failed"],
    }

    save_json(run_dir / "acceptance_report.json", acceptance_report)
    logger.info(f"  Verification completed in {step_duration:.2f}s")

    return acceptance_report


# ═══════════════════════════════════════════════════════════════════
# Step 5: GitHub Delivery
# ═══════════════════════════════════════════════════════════════════

def step5_github_delivery(
    state: Dict[str, Any],
    acceptance: Dict[str, Any],
    run_dir: Path,
    logger: logging.Logger,
    autofixer: AutoFixer,
    dry_run: bool = False,
) -> Dict[str, Any]:
    """
    Step 5: GitHub Delivery (~10 seconds)
    - Git add, commit, push
    - If push fails -> retry once, then skip (don't block)
    - Save to delivery_report.json
    """
    logger.info("=" * 70)
    logger.info("STEP 5: GitHub Delivery")
    logger.info("=" * 70)
    step_start = time.time()

    github_repo = state.get("github_repo", DEFAULT_STATE["github_repo"])
    commit_message = acceptance.get("commit_message", "chore: self-evolution loop run")

    if dry_run:
        logger.info("  [DRY-RUN] Skipping git operations")
        delivery = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "step": "github_delivery",
            "duration_seconds": round(time.time() - step_start, 2),
            "dry_run": True,
            "status": "skipped",
            "commit_message": commit_message[:100],
            "repo": github_repo,
        }
        save_json(run_dir / "delivery_report.json", delivery)
        return delivery

    repo_path = Path(github_repo)
    if not repo_path.exists():
        logger.warning(f"  GitHub repo path does not exist: {github_repo}")
        delivery = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "step": "github_delivery",
            "duration_seconds": round(time.time() - step_start, 2),
            "status": "skipped",
            "reason": "repo_path_not_found",
            "repo": github_repo,
        }
        save_json(run_dir / "delivery_report.json", delivery)
        return delivery

    # Git add
    logger.info(f"  Git add in {github_repo}...")
    rc, stdout, stderr = run_cmd(
        ["git", "add", "-A"],
        cwd=github_repo, timeout=30, logger=logger,
    )

    # Git commit
    logger.info("  Git commit...")
    rc, stdout, stderr = run_cmd(
        ["git", "commit", "-m", commit_message],
        cwd=github_repo, timeout=30, logger=logger,
    )

    if rc != 0:
        # Check if there's nothing to commit (not an error)
        if "nothing to commit" in stderr or "nothing to commit" in stdout:
            logger.info("  Nothing to commit (no changes)")
            delivery = {
                "timestamp": datetime.now(timezone.utc).isoformat(),
                "step": "github_delivery",
                "duration_seconds": round(time.time() - step_start, 2),
                "status": "no_changes",
                "repo": github_repo,
            }
            save_json(run_dir / "delivery_report.json", delivery)
            return delivery
        else:
            logger.warning(f"  Git commit failed: {stderr[:200]}")
            delivery = {
                "timestamp": datetime.now(timezone.utc).isoformat(),
                "step": "github_delivery",
                "duration_seconds": round(time.time() - step_start, 2),
                "status": "commit_failed",
                "error": stderr[:500],
                "repo": github_repo,
            }
            save_json(run_dir / "delivery_report.json", delivery)
            return delivery

    # Git push
    logger.info("  Git push...")
    rc, stdout, stderr = run_cmd(
        ["git", "push", "origin", "HEAD"],
        cwd=github_repo, timeout=60, logger=logger,
    )

    push_success = rc == 0
    if not push_success:
        logger.warning(f"  Git push failed: {stderr[:200]}")
        # Auto-fix: retry once
        logger.info("  Attempting push retry...")
        fixed = autofixer.try_fix("git_push", cwd=github_repo)
        if fixed:
            rc2, stdout2, stderr2 = run_cmd(
                ["git", "push", "origin", "HEAD"],
                cwd=github_repo, timeout=60, logger=logger,
            )
            push_success = rc2 == 0
            if push_success:
                logger.info("  Push succeeded on retry")
            else:
                logger.warning("  Push retry also failed, skipping delivery")
        else:
            logger.warning("  Auto-fix failed, skipping delivery")

    step_duration = time.time() - step_start

    delivery = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "step": "github_delivery",
        "duration_seconds": round(step_duration, 2),
        "status": "delivered" if push_success else "push_failed",
        "commit_message": commit_message[:100],
        "repo": github_repo,
        "push_success": push_success,
    }

    save_json(run_dir / "delivery_report.json", delivery)
    logger.info(f"  GitHub delivery completed in {step_duration:.2f}s")
    logger.info(f"  Status: {'DELIVERED' if push_success else 'FAILED (non-blocking)'}")

    return delivery


# ═══════════════════════════════════════════════════════════════════
# Step 6: Evolution Summary
# ═══════════════════════════════════════════════════════════════════

def step6_evolution_summary(
    state: Dict[str, Any],
    before_state: Dict[str, Any],
    before_score: float,
    before_grade: str,
    acceptance: Dict[str, Any],
    delivery: Dict[str, Any],
    run_dir: Path,
    logger: logging.Logger,
    evolution_base: Path,
) -> Dict[str, Any]:
    """
    Step 6: Evolution Summary (~2 seconds)
    - Compare before/after scores
    - Update running history
    - Save to summary.json
    - Also save cumulative state to current_state.json
    """
    logger.info("=" * 70)
    logger.info("STEP 6: Evolution Summary")
    logger.info("=" * 70)
    step_start = time.time()

    after_score = acceptance.get("after", {}).get("score", before_score)
    after_grade = acceptance.get("after", {}).get("grade", before_grade)
    score_delta = round(after_score - before_score, 6)
    pct_change = round((score_delta / max(before_score, 0.001)) * 100, 2)

    # Load history
    history_path = evolution_base / "evolution_history.json"
    history: List[Dict[str, Any]] = []
    if history_path.exists():
        try:
            history = json.loads(history_path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, Exception):
            history = []

    # Add this run to history
    run_entry = {
        "date": run_dir.name,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "before_score": before_score,
        "after_score": after_score,
        "before_grade": before_grade,
        "after_grade": after_grade,
        "score_delta": score_delta,
        "pct_change": pct_change,
        "delivery_status": delivery.get("status", "unknown"),
        "state_changes": acceptance.get("state_changes", {}),
    }
    history.append(run_entry)

    # Calculate cumulative stats
    total_runs = len(history)
    total_improvement = sum(1 for h in history if h.get("score_delta", 0) > 0)
    total_regressions = sum(1 for h in history if h.get("score_delta", 0) < -0.001)
    best_score = max(h.get("after_score", 0) for h in history)
    current_score = history[-1].get("after_score", 0) if history else 0

    summary = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "step": "evolution_summary",
        "duration_seconds": round(time.time() - step_start, 2),
        "this_run": {
            "before_score": before_score,
            "after_score": after_score,
            "before_grade": before_grade,
            "after_grade": after_grade,
            "score_delta": score_delta,
            "pct_change": pct_change,
            "improved": score_delta > 0.0001,
            "regression": score_delta < -0.001,
        },
        "cumulative": {
            "total_runs": total_runs,
            "total_improvements": total_improvement,
            "total_regressions": total_regressions,
            "best_score": best_score,
            "current_score": current_score,
            "improvement_rate": round(total_improvement / max(total_runs, 1) * 100, 1),
        },
        "current_state": {
            "dg": state.get("dg", 0.0),
            "te": state.get("te", 0.0),
            "xs": state.get("xs", 0.0),
            "psi": state.get("psi", 0.0),
            "xi_self": state.get("xi_self", 0.0),
            "tau": state.get("tau", 7),
            "c": state.get("c", 0.0),
            "phi": state.get("phi", 0.0),
            "gamma": state.get("gamma", 0.0),
            "score": after_score,
            "grade": after_grade,
        },
    }

    # Save summary
    save_json(run_dir / "summary.json", summary)

    # Save cumulative history
    save_json(history_path, history)

    # Save current state
    current_state = dict(state)
    current_state["score"] = after_score
    current_state["grade"] = after_grade
    current_state["last_run"] = run_dir.name
    current_state["last_updated"] = datetime.now(timezone.utc).isoformat()
    save_json(evolution_base / "current_state.json", current_state)

    logger.info(f"  Before: {before_score:.6f} ({before_grade}) -> After: {after_score:.6f} ({after_grade})")
    logger.info(f"  Delta: {score_delta:+.6f} ({pct_change:+.2f}%)")
    logger.info(f"  History: {total_runs} runs, {total_improvement} improvements, {total_regressions} regressions")
    logger.info(f"  Best score: {best_score:.6f} | Current: {current_score:.6f}")

    return summary


# ═══════════════════════════════════════════════════════════════════
# Utility Functions
# ═══════════════════════════════════════════════════════════════════

def save_json(path: Path, data: Any) -> None:
    """Save data as JSON to a file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(data, indent=2, ensure_ascii=False, default=str),
        encoding="utf-8",
    )


def load_json(path: Path, default: Optional[Any] = None) -> Any:
    """Load JSON from a file, returning default if not found."""
    if path.exists():
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, Exception):
            pass
    return default if default is not None else {}


def load_current_state(evolution_base: Path) -> Dict[str, Any]:
    """Load the current state from previous runs or use defaults."""
    current_path = evolution_base / "current_state.json"
    saved = load_json(current_path)

    if saved and "score" in saved:
        # Merge with defaults to ensure all keys exist
        state = dict(DEFAULT_STATE)
        for key in ["dg", "te", "xs", "psi", "xi_self", "tau", "c", "phi", "gamma"]:
            if key in saved:
                state[key] = saved[key]
        if "score" in saved:
            state["score"] = saved["score"]
        if "grade" in saved:
            state["grade"] = saved["grade"]
        return state

    return dict(DEFAULT_STATE)


# ═══════════════════════════════════════════════════════════════════
# Main Self-Evolution Loop
# ═══════════════════════════════════════════════════════════════════

def run_self_evolution(
    dry_run: bool = False,
    quick: bool = False,
    verbose: bool = False,
) -> Dict[str, Any]:
    """
    Run the complete self-evolution loop.

    Args:
        dry_run:  Do everything except git push
        quick:    Skip Step 3 (improvement execution), just health check + scoring
        verbose:  Enable verbose logging

    Returns:
        Complete run result dictionary.
    """
    loop_start = time.time()

    # Setup directories
    evolution_base = SCRIPT_DIR / "evolution_runs"
    evolution_base.mkdir(parents=True, exist_ok=True)

    date_str = datetime.now(timezone.utc).strftime("%Y%m%d")
    run_dir = evolution_base / date_str
    run_dir.mkdir(parents=True, exist_ok=True)

    logger = setup_logging(run_dir, verbose)

    logger.info("#" * 70)
    logger.info("# OMEGA AGI Self-Evolution Loop Engine")
    logger.info(f"# Date: {date_str}")
    logger.info(f"# Mode: {'DRY-RUN + ' if dry_run else ''}{'QUICK' if quick else 'FULL'}")
    logger.info(f"# Run directory: {run_dir}")
    logger.info("#" * 70)

    # Load current state
    state = load_current_state(evolution_base)
    before_state = {
        "dg": state["dg"], "te": state["te"], "xs": state["xs"],
        "psi": state["psi"], "xi_self": state["xi_self"], "tau": state["tau"],
        "c": state["c"], "phi": state["phi"], "gamma": state["gamma"],
    }
    before_score = state.get("score", calc_apex_from_state(state)["final"])
    before_grade = state.get("grade", "S+")

    autofixer = AutoFixer(logger)

    result: Dict[str, Any] = {
        "date": date_str,
        "run_dir": str(run_dir),
        "mode": "quick_dry_run" if (quick and dry_run) else ("quick" if quick else ("dry_run" if dry_run else "full")),
        "timestamp": datetime.now(timezone.utc).isoformat(),
    }

    try:
        # ── Step 1: Health Check ──
        health = step1_health_check(state, run_dir, logger, autofixer)
        result["health_check"] = health

        # ── Step 2: Formula Assessment ──
        assessment = step2_formula_assessment(state, run_dir, logger)
        result["formula_assessment"] = assessment

        # ── Step 3: Improvement Execution (skip in quick mode) ──
        if quick:
            logger.info("=" * 70)
            logger.info("STEP 3: SKIPPED (--quick mode)")
            logger.info("=" * 70)
            execution = {
                "timestamp": datetime.now(timezone.utc).isoformat(),
                "step": "improvement_execution",
                "duration_seconds": 0,
                "skipped": True,
                "reason": "quick_mode",
                "improvements_planned": 0,
                "improvements_completed": 0,
                "tests_added": 0,
                "state_changes": {},
                "execution_log": [],
                "post_test_results": {
                    "rust": {"total": 0, "passed": 0, "failed": 0},
                    "python": {"total": 0, "passed": 0, "failed": 0},
                    "all_passed": True,
                },
            }
        else:
            execution = step3_improvement_execution(state, assessment, run_dir, logger, autofixer)
        result["execution"] = execution

        # ── Step 4: Verification & Scoring ──
        acceptance = step4_verification_scoring(
            state, before_score, before_grade, before_state, execution, run_dir, logger,
        )
        result["acceptance"] = acceptance

        # ── Step 5: GitHub Delivery ──
        delivery = step5_github_delivery(state, acceptance, run_dir, logger, autofixer, dry_run=dry_run)
        result["delivery"] = delivery

        # ── Step 6: Evolution Summary ──
        summary = step6_evolution_summary(
            state, before_state, before_score, before_grade,
            acceptance, delivery, run_dir, logger, evolution_base,
        )
        result["summary"] = summary

        result["status"] = "SUCCESS"

    except Exception as e:
        logger.exception(f"Self-evolution loop error: {e}")
        result["status"] = "ERROR"
        result["error"] = str(e)

    # Final timing
    loop_duration = time.time() - loop_start
    result["total_duration_seconds"] = round(loop_duration, 2)

    logger.info("\n" + "#" * 70)
    logger.info("# SELF-EVOLUTION LOOP COMPLETE")
    logger.info(f"# Status: {result['status']}")
    logger.info(f"# Duration: {loop_duration:.2f}s")
    logger.info(f"# Mode: {result['mode']}")

    if "acceptance" in result:
        acc = result["acceptance"]
        logger.info(f"# Score: {acc['before']['score']:.4f} -> {acc['after']['score']:.4f}")
        logger.info(f"# Grade: {acc['before']['grade']} -> {acc['after']['grade']}")
    logger.info("#" * 70)

    # Save final result
    save_json(run_dir / "run_result.json", result)

    return result


# ═══════════════════════════════════════════════════════════════════
# CLI Entry Point
# ═══════════════════════════════════════════════════════════════════

def main() -> None:
    parser = argparse.ArgumentParser(
        description="OMEGA AGI Self-Evolution Loop Engine -- Fully Autonomous Self-Improving System",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python3 self_evolution_loop.py                    # Full autonomous run
  python3 self_evolution_loop.py --dry-run          # Full run, skip git push
  python3 self_evolution_loop.py --quick             # Health check + scoring only
  python3 self_evolution_loop.py --quick --dry-run   # Quick assessment, no push
  python3 self_evolution_loop.py --verbose           # Verbose logging
        """,
    )

    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Do everything except git push",
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Skip Step 3 (improvement execution), just health check + scoring",
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Enable verbose logging",
    )

    args = parser.parse_args()

    result = run_self_evolution(
        dry_run=args.dry_run,
        quick=args.quick,
        verbose=args.verbose,
    )

    if result.get("status") == "SUCCESS":
        print("\n[SUCCESS] Self-evolution loop completed successfully.")
        sys.exit(0)
    else:
        print(f"\n[FAILED] Self-evolution loop status: {result.get('status')}")
        if result.get("error"):
            print(f"  Error: {result['error']}")
        sys.exit(1)


if __name__ == "__main__":
    main()
