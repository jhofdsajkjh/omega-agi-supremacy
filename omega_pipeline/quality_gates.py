"""
CMMI Level 5 Quality Gates for OMEGA AGI Pipeline
==================================================
Each gate has:
- name: Human-readable name
- phase: Which phase it belongs to (1=Planning, 2=Execution, 3=Audit)
- check(context: dict) -> GateResult
- severity: "blocking" | "warning" | "info"

GateResult contains:
- passed: bool
- severity: str
- details: str
- gate_name: str
"""

from __future__ import annotations

import math
import re
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional


# ═══════════════════════════════════════════════════════════════════
# Gate Result
# ═══════════════════════════════════════════════════════════════════

@dataclass
class GateResult:
    """Single quality gate evaluation result."""
    passed: bool
    severity: str  # "blocking" | "warning" | "info"
    details: str
    gate_name: str
    timestamp: str = ""
    rationale: str = ""

    def to_dict(self) -> Dict[str, Any]:
        return {
            "gate_name": self.gate_name,
            "passed": self.passed,
            "severity": self.severity,
            "details": self.details,
            "rationale": self.rationale,
            "timestamp": self.timestamp,
        }


# ═══════════════════════════════════════════════════════════════════
# Base Quality Gate
# ═══════════════════════════════════════════════════════════════════

class QualityGate:
    """Abstract base class for all quality gates."""

    name: str = "base"
    phase: int = 0
    severity: str = "blocking"
    description: str = ""

    def check(self, context: Dict[str, Any]) -> GateResult:
        raise NotImplementedError("Subclasses must implement check()")

    def _result(self, passed: bool, details: str, rationale: str = "") -> GateResult:
        from datetime import datetime, timezone
        return GateResult(
            passed=passed,
            severity=self.severity,
            details=details,
            gate_name=self.name,
            timestamp=datetime.now(timezone.utc).isoformat(),
            rationale=rationale or self.description,
        )


# ═══════════════════════════════════════════════════════════════════
# Phase 1 Gates (Planning)
# ═══════════════════════════════════════════════════════════════════

class PlanCompletenessGate(QualityGate):
    """
    Plan has all required fields for each improvement:
    id, param, file, tests_added, phase
    """
    name = "PlanCompletenessGate"
    phase = 1
    severity = "blocking"
    description = "Every improvement in the plan must have: id, param, file, tests_added, phase"

    REQUIRED_FIELDS = {"id", "param", "file", "tests_added", "phase"}

    def check(self, context: Dict[str, Any]) -> GateResult:
        improvements = context.get("improvements", [])
        if not improvements:
            return self._result(False, "Plan has no improvements")

        missing = []
        for i, imp in enumerate(improvements):
            imp_keys = set(imp.keys()) if isinstance(imp, dict) else set()
            missing_fields = self.REQUIRED_FIELDS - imp_keys
            if missing_fields:
                missing.append(f"Improvement[{i}] missing: {missing_fields}")

        if missing:
            return self._result(
                False,
                f"Completeness check failed: {'; '.join(missing)}",
                rationale="CMMI ML5: All artifacts must be fully specified before execution"
            )

        return self._result(
            True,
            f"All {len(improvements)} improvements have required fields"
        )


class PlanMinImprovementsGate(QualityGate):
    """
    At least 3 improvements in the plan.
    """
    name = "PlanMinImprovementsGate"
    phase = 1
    severity = "blocking"
    description = "Plan must contain at least 3 improvements for meaningful impact"

    MIN_IMPROVEMENTS = 3

    def check(self, context: Dict[str, Any]) -> GateResult:
        improvements = context.get("improvements", [])
        count = len(improvements)
        passed = count >= self.MIN_IMPROVEMENTS

        return self._result(
            passed,
            f"Improvement count: {count} (minimum: {self.MIN_IMPROVEMENTS})",
            rationale=f"CMMI ML5: Minimum {self.MIN_IMPROVEMENTS} improvements ensure statistical significance"
        )


class PlanMinTestsGate(QualityGate):
    """
    At least 10 total new tests planned.
    """
    name = "PlanMinTestsGate"
    phase = 1
    severity = "blocking"
    description = "Plan must specify at least 10 total new tests"

    MIN_TESTS = 10

    def check(self, context: Dict[str, Any]) -> GateResult:
        improvements = context.get("improvements", [])
        total_tests = sum(
            imp.get("tests_added", imp.get("tests", 0))
            for imp in improvements
            if isinstance(imp, dict)
        )
        passed = total_tests >= self.MIN_TESTS

        return self._result(
            passed,
            f"Total planned tests: {total_tests} (minimum: {self.MIN_TESTS})",
            rationale=f"CMMI ML5: Minimum {self.MIN_TESTS} tests ensure adequate verification coverage"
        )


class FormulaSensitivityGate(QualityGate):
    """
    Plan targets the weakest parameter identified by sensitivity analysis.
    """
    name = "FormulaSensitivityGate"
    phase = 1
    severity = "warning"
    description = "Plan should target the weakest parameter(s) identified by sensitivity analysis"

    def check(self, context: Dict[str, Any]) -> GateResult:
        sensitivity = context.get("sensitivity_analysis", {})
        improvements = context.get("improvements", [])

        if not sensitivity or not improvements:
            return self._result(
                True,
                "Sensitivity analysis not available; gate skipped (non-blocking)"
            )

        # Extract the sensitivities sub-dict (handles both flat and nested formats)
        if "sensitivities" in sensitivity and isinstance(sensitivity["sensitivities"], dict):
            sens_map = sensitivity["sensitivities"]
        elif "weakest_param" in sensitivity:
            sens_map = sensitivity
        else:
            sens_map = {}

        if not sens_map:
            return self._result(
                True,
                "Sensitivity data not in expected format; gate skipped (non-blocking)"
            )

        # Find weakest parameter from sensitivity analysis (lowest current_value)
        weakest_param = min(sens_map, key=lambda k: sens_map.get(k, {}).get("current_value", 1.0))

        # Check if any improvement targets the weakest parameter
        targeted_params = set()
        for imp in improvements:
            if isinstance(imp, dict):
                targeted_params.add(imp.get("param", ""))

        targets_weakest = weakest_param in targeted_params

        return self._result(
            targets_weakest,
            f"Weakest parameter: {weakest_param} | "
            f"Targeted params: {targeted_params} | "
            f"{'Targets weakest' if targets_weakest else 'Does NOT target weakest param'}",
            rationale="CMMI ML5: Resource allocation should prioritize highest-impact improvements"
        )


# ═══════════════════════════════════════════════════════════════════
# Phase 2 Gates (Execution)
# ═══════════════════════════════════════════════════════════════════

class CompilationGate(QualityGate):
    """
    cargo build returns 0 errors.
    """
    name = "CompilationGate"
    phase = 2
    severity = "blocking"
    description = "Rust compilation must succeed with 0 errors"

    def check(self, context: Dict[str, Any]) -> GateResult:
        compilation = context.get("compilation", {})
        errors = compilation.get("errors", -1)

        if errors < 0:
            # No compilation data available (e.g., demo mode)
            return self._result(
                True,
                "Compilation check skipped (no Rust project detected)"
            )

        passed = errors == 0
        return self._result(
            passed,
            f"Compilation errors: {errors} {'(PASS)' if passed else '(FAIL)'}",
            rationale="CMMI ML5: Zero-defect compilation is mandatory for production code"
        )


class RustTestGate(QualityGate):
    """
    cargo test passes with 0 failures.
    """
    name = "RustTestGate"
    phase = 2
    severity = "blocking"
    description = "All Rust tests must pass with 0 failures"

    def check(self, context: Dict[str, Any]) -> GateResult:
        rust_tests = context.get("rust_tests", {})
        failures = rust_tests.get("failures", 0)
        total = rust_tests.get("total", 0)

        if total == 0:
            return self._result(
                True,
                "Rust test gate skipped (no Rust tests detected)"
            )

        passed = failures == 0
        return self._result(
            passed,
            f"Rust tests: {total - failures}/{total} passed, {failures} failures",
            rationale="CMMI ML5: All unit tests must pass before proceeding"
        )


class PythonTestGate(QualityGate):
    """
    pytest passes with 0 failures.
    """
    name = "PythonTestGate"
    phase = 2
    severity = "blocking"
    description = "All Python tests must pass with 0 failures"

    def check(self, context: Dict[str, Any]) -> GateResult:
        python_tests = context.get("python_tests", {})
        failures = python_tests.get("failures", 0)
        total = python_tests.get("total", 0)

        if total == 0:
            return self._result(
                True,
                "Python test gate skipped (no Python tests detected)"
            )

        passed = failures == 0
        return self._result(
            passed,
            f"Python tests: {total - failures}/{total} passed, {failures} failures",
            rationale="CMMI ML5: All unit tests must pass before proceeding"
        )


class NewTestCountGate(QualityGate):
    """
    At least 80% of planned tests were actually created.
    """
    name = "NewTestCountGate"
    phase = 2
    severity = "warning"
    description = "At least 80% of planned tests must be actually created and passing"

    MIN_RATIO = 0.80

    def check(self, context: Dict[str, Any]) -> GateResult:
        planned = context.get("planned_test_count", 0)
        actual = context.get("actual_test_count", 0)

        if planned == 0:
            return self._result(
                True,
                "Test count gate skipped (no tests planned)"
            )

        ratio = actual / planned if planned > 0 else 0.0
        passed = ratio >= self.MIN_RATIO

        return self._result(
            passed,
            f"Test creation ratio: {actual}/{planned} = {ratio:.1%} "
            f"(minimum: {self.MIN_RATIO:.0%})",
            rationale=f"CMMI ML5: {self.MIN_RATIO:.0%}% test realization ensures plan integrity"
        )


# ═══════════════════════════════════════════════════════════════════
# Phase 3 Gates (Audit)
# ═══════════════════════════════════════════════════════════════════

class NoRegressionGate(QualityGate):
    """
    Grade did not decrease from before.
    """
    name = "NoRegressionGate"
    phase = 3
    severity = "blocking"
    description = "Final grade must not regress from the previous run"

    GRADE_ORDER = {"F": 0, "D": 1, "C": 2, "B": 3, "A": 4, "S": 5, "S+": 6}

    def check(self, context: Dict[str, Any]) -> GateResult:
        before_grade = context.get("before_grade", "")
        after_grade = context.get("after_grade", "")
        before_score = context.get("before_score", 0.0)
        after_score = context.get("after_score", 0.0)

        if not before_grade:
            return self._result(
                True,
                "No previous grade to compare against; regression gate passed"
            )

        # Check grade order
        before_val = self.GRADE_ORDER.get(before_grade, 0)
        after_val = self.GRADE_ORDER.get(after_grade, 0)
        grade_ok = after_val >= before_val

        # Also check numeric score
        score_ok = after_score >= before_score - 0.001  # tiny epsilon for float

        passed = grade_ok and score_ok

        return self._result(
            passed,
            f"Grade: {before_grade} -> {after_grade} | "
            f"Score: {before_score:.4f} -> {after_score:.4f} | "
            f"{'No regression' if passed else 'REGRESSION DETECTED'}",
            rationale="CMMI ML5: Zero regression is mandatory; continuous improvement is expected"
        )


class ImprovementGate(QualityGate):
    """
    At least one parameter improved.
    """
    name = "ImprovementGate"
    phase = 3
    severity = "warning"
    description = "At least one formula parameter must show improvement"

    def check(self, context: Dict[str, Any]) -> GateResult:
        param_changes = context.get("param_changes", {})

        if not param_changes:
            return self._result(
                False,
                "No parameter change data available",
                rationale="CMMI ML5: Quantitative improvement must be demonstrated"
            )

        improved = []
        for param, change in param_changes.items():
            if isinstance(change, dict):
                delta = change.get("delta", 0)
            elif isinstance(change, (int, float)):
                delta = change
            else:
                delta = 0
            if delta > 0:
                improved.append(param)

        passed = len(improved) > 0
        return self._result(
            passed,
            f"Improved parameters: {improved if improved else 'NONE'} "
            f"(out of {list(param_changes.keys())})",
            rationale="CMMI ML5: Continuous improvement requires measurable parameter gains"
        )


class FullTestSuiteGate(QualityGate):
    """
    All tests (existing + new) pass together.
    """
    name = "FullTestSuiteGate"
    phase = 3
    severity = "blocking"
    description = "Full test suite (Rust + Python) must pass with 0 failures"

    def check(self, context: Dict[str, Any]) -> GateResult:
        rust_failures = context.get("rust_test_failures", 0)
        python_failures = context.get("python_test_failures", 0)
        rust_total = context.get("rust_test_total", 0)
        python_total = context.get("python_test_total", 0)

        total_failures = rust_failures + python_failures
        total_tests = rust_total + python_total

        passed = total_failures == 0

        return self._result(
            passed,
            f"Full suite: {total_tests - total_failures}/{total_tests} passed "
            f"(Rust: {rust_total - rust_failures}/{rust_total}, "
            f"Python: {python_total - python_failures}/{python_total})",
            rationale="CMMI ML5: Integration verification requires all tests passing together"
        )


class CommitMessageGate(QualityGate):
    """
    Commit follows Conventional Commits format.
    """
    name = "CommitMessageGate"
    phase = 3
    severity = "blocking"
    description = "Commit message must follow Conventional Commits specification"

    CONVENTIONAL_TYPES = {
        "feat", "fix", "docs", "style", "refactor", "perf",
        "test", "build", "ci", "chore", "revert"
    }

    def check(self, context: Dict[str, Any]) -> GateResult:
        commit_message = context.get("commit_message", "")

        if not commit_message:
            return self._result(
                False,
                "No commit message provided",
                rationale="CMMI ML5: All changes must be traceable via structured commit messages"
            )

        # Conventional Commits pattern: type(scope): description
        pattern = rf"^({'|'.join(self.CONVENTIONAL_TYPES)})(\(.+\))?!?:\s.+"
        matches = bool(re.match(pattern, commit_message, re.MULTILINE))

        return self._result(
            matches,
            f"Commit message: '{commit_message[:80]}...' | "
            f"{'Follows Conventional Commits' if matches else 'Does NOT follow Conventional Commits'}",
            rationale="CMMI ML5: Conventional Commits enable automated changelog and traceability"
        )


class GitHubPushGate(QualityGate):
    """
    Push succeeded (if --push enabled).
    """
    name = "GitHubPushGate"
    phase = 3
    severity = "info"
    description = "GitHub push succeeded (only checked when --push is enabled)"

    def check(self, context: Dict[str, Any]) -> GateResult:
        push_enabled = context.get("push_enabled", False)
        push_result = context.get("push_result", {})

        if not push_enabled:
            return self._result(
                True,
                "Push not enabled; gate skipped",
                rationale="Gate only applies when --push flag is used"
            )

        success = push_result.get("success", False)
        url = push_result.get("url", "N/A")

        return self._result(
            success,
            f"Push {'succeeded' if success else 'FAILED'} | URL: {url}",
            rationale="CMMI ML5: Successful delivery confirms end-to-end pipeline integrity"
        )


# ═══════════════════════════════════════════════════════════════════
# Quality Gate Runner
# ═══════════════════════════════════════════════════════════════════

class QualityGateRunner:
    """
    Runs all quality gates for a given phase and returns a summary.

    Usage:
        runner = QualityGateRunner()
        summary = runner.run_phase(phase=1, context={...})
        if summary["blocking_failed"]:
            print("Pipeline blocked!")
    """

    def __init__(self):
        self._gates: Dict[int, List[QualityGate]] = {
            1: [
                PlanCompletenessGate(),
                PlanMinImprovementsGate(),
                PlanMinTestsGate(),
                FormulaSensitivityGate(),
            ],
            2: [
                CompilationGate(),
                RustTestGate(),
                PythonTestGate(),
                NewTestCountGate(),
            ],
            3: [
                NoRegressionGate(),
                ImprovementGate(),
                FullTestSuiteGate(),
                CommitMessageGate(),
                GitHubPushGate(),
            ],
        }

    def run_phase(self, phase: int, context: Dict[str, Any]) -> Dict[str, Any]:
        """
        Run all gates for a given phase.

        Returns:
            {
                "phase": int,
                "total": int,
                "passed": int,
                "failed": int,
                "blocking_failed": bool,
                "results": [GateResult, ...],
                "summary": str,
            }
        """
        gates = self._gates.get(phase, [])
        results: List[GateResult] = []

        for gate in gates:
            try:
                result = gate.check(context)
            except Exception as e:
                result = GateResult(
                    passed=False,
                    severity=gate.severity,
                    details=f"Gate execution error: {e}",
                    gate_name=gate.name,
                )
            results.append(result)

        passed_count = sum(1 for r in results if r.passed)
        failed_count = len(results) - passed_count
        blocking_failed = any(
            r for r in results if not r.passed and r.severity == "blocking"
        )

        summary_lines = [
            f"Phase {phase} Quality Gates: {passed_count}/{len(results)} passed",
        ]
        for r in results:
            status = "PASS" if r.passed else "FAIL"
            sev = f"[{r.severity.upper()}]"
            summary_lines.append(f"  {status} {sev} {r.gate_name}: {r.details[:120]}")

        return {
            "phase": phase,
            "total": len(results),
            "passed": passed_count,
            "failed": failed_count,
            "blocking_failed": blocking_failed,
            "results": [r.to_dict() for r in results],
            "summary": "\n".join(summary_lines),
        }

    def run_all(self, context: Dict[str, Any]) -> Dict[str, Any]:
        """Run all gates for all phases."""
        all_results = {}
        overall_blocking = False

        for phase in sorted(self._gates.keys()):
            phase_result = self.run_phase(phase, context)
            all_results[f"phase_{phase}"] = phase_result
            if phase_result["blocking_failed"]:
                overall_blocking = True

        return {
            "overall_passed": not overall_blocking,
            "phases": all_results,
        }

    def get_gates_for_phase(self, phase: int) -> List[str]:
        """Return gate names for a given phase."""
        return [g.name for g in self._gates.get(phase, [])]
