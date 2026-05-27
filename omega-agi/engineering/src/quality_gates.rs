//! CMMI Level 5 Quality Gates - Rust Implementation
//! Ported from Python quality_gates.py with full trait-based design.

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════
// GateResult
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub passed: bool,
    pub severity: String,
    pub details: String,
    pub gate_name: String,
    pub timestamp: String,
    pub rationale: String,
}

impl GateResult {
    pub fn new(passed: bool, severity: &str, details: &str, gate_name: &str, rationale: &str) -> Self {
        Self {
            passed,
            severity: severity.to_string(),
            details: details.to_string(),
            gate_name: gate_name.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            rationale: rationale.to_string(),
        }
    }
    pub fn is_blocking_failure(&self) -> bool {
        !self.passed && self.severity == "blocking"
    }
}

// ═══════════════════════════════════════════════════════════════════
// QualityGate trait
// ═══════════════════════════════════════════════════════════════════

pub trait QualityGate: Send + Sync {
    fn name(&self) -> &'static str;
    fn phase(&self) -> u8;
    fn severity(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn check(&self, context: &GateContext) -> GateResult;
    fn pass(&self, details: &str) -> GateResult {
        GateResult::new(true, self.severity(), details, self.name(), self.description())
    }
    fn fail(&self, details: &str, rationale: &str) -> GateResult {
        GateResult::new(false, self.severity(), details, self.name(), rationale)
    }
}

// ═══════════════════════════════════════════════════════════════════
// GateContext and data types
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateContext {
    pub improvements: Vec<Improvement>,
    pub sensitivity_analysis: HashMap<String, SensitivityData>,
    pub compilation: Option<CompilationData>,
    pub rust_tests: Option<TestData>,
    pub python_tests: Option<TestData>,
    pub planned_test_count: usize,
    pub actual_test_count: usize,
    pub before_grade: String,
    pub after_grade: String,
    pub before_score: f64,
    pub after_score: f64,
    pub param_changes: HashMap<String, f64>,
    pub rust_test_failures: usize,
    pub rust_test_total: usize,
    pub python_test_failures: usize,
    pub python_test_total: usize,
    pub commit_message: String,
    pub push_enabled: bool,
    pub push_result: Option<PushResult>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Improvement {
    pub id: Option<String>,
    pub param: Option<String>,
    pub file: Option<String>,
    pub tests_added: Option<usize>,
    pub phase: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensitivityData {
    pub sensitivity: Option<f64>,
    pub weakest_param: Option<String>,
    pub current: Option<f64>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationData {
    pub errors: i32,
    pub warnings: Option<i32>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestData {
    pub failures: usize,
    pub total: usize,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResult {
    pub success: bool,
    pub url: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ═══════════════════════════════════════════════════════════════════
// Helper utilities
// ═══════════════════════════════════════════════════════════════════

fn format_score(score: f64) -> String {
    format!("{:.4}", score)
}

// ═══════════════════════════════════════════════════════════════════
// Phase 1 Gates (Planning)
// ═══════════════════════════════════════════════════════════════════

const REQUIRED_IMPROVEMENT_FIELDS: [&str; 5] = ["id", "param", "file", "tests_added", "phase"];

pub struct PlanCompletenessGate;
impl QualityGate for PlanCompletenessGate {
    fn name(&self) -> &'static str { "PlanCompletenessGate" }
    fn phase(&self) -> u8 { 1 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str {
        "Every improvement in the plan must have: id, param, file, tests_added, phase"
    }
    fn check(&self, ctx: &GateContext) -> GateResult {
        if ctx.improvements.is_empty() {
            return self.fail("Plan has no improvements",
                "CMMI ML5: All artifacts must be fully specified before execution");
        }
        let mut missing_reports = Vec::new();
        for (i, imp) in ctx.improvements.iter().enumerate() {
            let mut missing = Vec::new();
            for field in REQUIRED_IMPROVEMENT_FIELDS {
                let has = match field {
                    "id" => imp.id.is_some(),
                    "param" => imp.param.is_some(),
                    "file" => imp.file.is_some(),
                    "tests_added" => imp.tests_added.is_some(),
                    "phase" => imp.phase.is_some(),
                    _ => false,
                };
                if !has { missing.push(field); }
            }
            if !missing.is_empty() {
                missing_reports.push(format!("Improvement[{}] missing: {{{}}}", i, missing.join(", ")));
            }
        }
        if !missing_reports.is_empty() {
            return self.fail(
                &format!("Completeness check failed: {}", missing_reports.join("; ")),
                "CMMI ML5: All artifacts must be fully specified before execution");
        }
        self.pass(&format!("All {} improvements have required fields", ctx.improvements.len()))
    }
}

pub struct PlanMinImprovementsGate;
impl QualityGate for PlanMinImprovementsGate {
    fn name(&self) -> &'static str { "PlanMinImprovementsGate" }
    fn phase(&self) -> u8 { 1 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str {
        "Plan must contain at least 3 improvements for meaningful impact"
    }
    fn check(&self, ctx: &GateContext) -> GateResult {
        let count = ctx.improvements.len();
        if count >= 3 {
            self.pass(&format!("Improvement count: {} (minimum: 3)", count))
        } else {
            self.fail(&format!("Improvement count: {} (minimum: 3)", count),
                "CMMI ML5: Minimum 3 improvements ensure statistical significance")
        }
    }
}

pub struct PlanMinTestsGate;
impl QualityGate for PlanMinTestsGate {
    fn name(&self) -> &'static str { "PlanMinTestsGate" }
    fn phase(&self) -> u8 { 1 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str { "Plan must specify at least 10 total new tests" }
    fn check(&self, ctx: &GateContext) -> GateResult {
        let total: usize = ctx.improvements.iter().filter_map(|i| i.tests_added).sum();
        if total >= 10 {
            self.pass(&format!("Total planned tests: {} (minimum: 10)", total))
        } else {
            self.fail(&format!("Total planned tests: {} (minimum: 10)", total),
                "CMMI ML5: Minimum 10 tests ensure adequate verification coverage")
        }
    }
}

pub struct FormulaSensitivityGate;
impl QualityGate for FormulaSensitivityGate {
    fn name(&self) -> &'static str { "FormulaSensitivityGate" }
    fn phase(&self) -> u8 { 1 }
    fn severity(&self) -> &'static str { "warning" }
    fn description(&self) -> &'static str {
        "Plan should target the weakest parameter(s) identified by sensitivity analysis"
    }
    fn check(&self, ctx: &GateContext) -> GateResult {
        if ctx.improvements.is_empty() {
            return self.pass("No improvements planned; gate skipped (non-blocking)");
        }
        let weakest = ctx.sensitivity_analysis.get("weakest_param")
            .and_then(|v| v.weakest_param.clone())
            .or_else(|| ctx.sensitivity_analysis.get("sensitivities")
                .and_then(|v| v.weakest_param.clone()));
        let Some(target) = weakest else {
            return self.pass("Sensitivity analysis not available; gate skipped (non-blocking)");
        };
        let targets_weakest = ctx.improvements.iter()
            .any(|imp| imp.param.as_ref().map_or(false, |p| p == &target));
        if targets_weakest {
            self.pass(&format!("Plan targets weakest parameter '{}' — good targeting", target))
        } else {
            self.fail(&format!("Plan does NOT target weakest parameter '{}'", target),
                "CMMI ML5: Targeting weakest params maximizes improvement leverage")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase 2 Gates (Execution)
// ═══════════════════════════════════════════════════════════════════

pub struct CompilationGate;
impl QualityGate for CompilationGate {
    fn name(&self) -> &'static str { "CompilationGate" }
    fn phase(&self) -> u8 { 2 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str { "Rust compilation must succeed with 0 errors" }
    fn check(&self, ctx: &GateContext) -> GateResult {
        let Some(comp) = ctx.compilation.as_ref() else {
            return self.pass("Compilation check skipped (no Rust project detected)");
        };
        if comp.errors < 0 {
            return self.pass("Compilation check skipped (no Rust project detected)");
        }
        if comp.errors == 0 {
            self.pass(&format!("Compilation errors: {} (PASS)", comp.errors))
        } else {
            self.fail(&format!("Compilation errors: {} (FAIL)", comp.errors),
                "CMMI ML5: Zero-defect compilation is mandatory for production code")
        }
    }
}

pub struct RustTestGate;
impl QualityGate for RustTestGate {
    fn name(&self) -> &'static str { "RustTestGate" }
    fn phase(&self) -> u8 { 2 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str { "All Rust tests must pass with 0 failures" }
    fn check(&self, ctx: &GateContext) -> GateResult {
        let Some(tests) = ctx.rust_tests.as_ref() else {
            return self.pass("Rust test gate skipped (no Rust tests detected)");
        };
        if tests.total == 0 {
            return self.pass("Rust test gate skipped (no Rust tests detected)");
        }
        if tests.failures == 0 {
            self.pass(&format!("Rust tests: {}/{} passed, {} failures",
                tests.total - tests.failures, tests.total, tests.failures))
        } else {
            self.fail(&format!("Rust tests: {}/{} passed, {} failures (FAIL)",
                tests.total - tests.failures, tests.total, tests.failures),
                "CMMI ML5: All unit tests must pass before proceeding")
        }
    }
}

pub struct PythonTestGate;
impl QualityGate for PythonTestGate {
    fn name(&self) -> &'static str { "PythonTestGate" }
    fn phase(&self) -> u8 { 2 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str { "All Python tests must pass with 0 failures" }
    fn check(&self, ctx: &GateContext) -> GateResult {
        let Some(tests) = ctx.python_tests.as_ref() else {
            return self.pass("Python test gate skipped (no Python tests detected)");
        };
        if tests.total == 0 {
            return self.pass("Python test gate skipped (no Python tests detected)");
        }
        if tests.failures == 0 {
            self.pass(&format!("Python tests: {}/{} passed, {} failures",
                tests.total - tests.failures, tests.total, tests.failures))
        } else {
            self.fail(&format!("Python tests: {}/{} passed, {} failures (FAIL)",
                tests.total - tests.failures, tests.total, tests.failures),
                "CMMI ML5: All unit tests must pass before proceeding")
        }
    }
}

pub struct NewTestCountGate;
impl QualityGate for NewTestCountGate {
    fn name(&self) -> &'static str { "NewTestCountGate" }
    fn phase(&self) -> u8 { 2 }
    fn severity(&self) -> &'static str { "warning" }
    fn description(&self) -> &'static str {
        "At least 80% of planned tests must be actually created and passing"
    }
    fn check(&self, ctx: &GateContext) -> GateResult {
        let planned = ctx.planned_test_count;
        let actual = ctx.actual_test_count;
        if planned == 0 {
            return self.pass("Test count gate skipped (no tests planned)");
        }
        let ratio = actual as f64 / planned as f64;
        if ratio >= 0.80 {
            self.pass(&format!("Test creation ratio: {}/{} = {:.1}% (minimum: 80%)",
                actual, planned, ratio * 100.0))
        } else {
            self.fail(&format!("Test creation ratio: {}/{} = {:.1}% (minimum: 80%)",
                actual, planned, ratio * 100.0),
                "CMMI ML5: 80% test realization ensures plan integrity")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase 3 Gates (Audit)
// ═══════════════════════════════════════════════════════════════════

const GRADE_ORDER: [&str; 7] = ["F", "D", "C", "B", "A", "S", "S+"];

pub struct NoRegressionGate;
impl QualityGate for NoRegressionGate {
    fn name(&self) -> &'static str { "NoRegressionGate" }
    fn phase(&self) -> u8 { 3 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str { "Final grade must not regress from the previous run" }
    fn check(&self, ctx: &GateContext) -> GateResult {
        if ctx.before_grade.is_empty() {
            return self.pass("No previous grade to compare against; regression gate passed");
        }
        let before_val = GRADE_ORDER.iter().position(|&g| g == ctx.before_grade).unwrap_or(0) as i32;
        let after_val = GRADE_ORDER.iter().position(|&g| g == ctx.after_grade).unwrap_or(0) as i32;
        let grade_ok = after_val >= before_val;
        let score_ok = ctx.after_score >= ctx.before_score - 0.001;
        let passed = grade_ok && score_ok;
        let details = format!(
            "Grade: {} -> {} | Score: {} -> {} | {}",
            ctx.before_grade, ctx.after_grade,
            format_score(ctx.before_score), format_score(ctx.after_score),
            if passed { "No regression" } else { "REGRESSION DETECTED" }
        );
        if passed {
            self.pass(&details)
        } else {
            self.fail(&details,
                "CMMI ML5: Zero regression is mandatory; continuous improvement is expected")
        }
    }
}

pub struct ImprovementGate;
impl QualityGate for ImprovementGate {
    fn name(&self) -> &'static str { "ImprovementGate" }
    fn phase(&self) -> u8 { 3 }
    fn severity(&self) -> &'static str { "warning" }
    fn description(&self) -> &'static str { "At least one formula parameter must show improvement" }
    fn check(&self, ctx: &GateContext) -> GateResult {
        if ctx.param_changes.is_empty() {
            return self.fail("No parameter change data available",
                "CMMI ML5: Quantitative improvement must be demonstrated");
        }
        let improved: Vec<&String> = ctx.param_changes.iter()
            .filter(|(_, &delta)| delta > 0.0).map(|(k, _)| k).collect();
        let passed = !improved.is_empty();
        let details = format!("Improved parameters: {} (out of {})",
            if improved.is_empty() { "NONE".to_string() }
            else { improved.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ") },
            ctx.param_changes.len());
        if passed {
            self.pass(&details)
        } else {
            self.fail(&details,
                "CMMI ML5: Continuous improvement requires measurable parameter gains")
        }
    }
}

pub struct FullTestSuiteGate;
impl QualityGate for FullTestSuiteGate {
    fn name(&self) -> &'static str { "FullTestSuiteGate" }
    fn phase(&self) -> u8 { 3 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str {
        "Full test suite (Rust + Python) must pass with 0 failures"
    }
    fn check(&self, ctx: &GateContext) -> GateResult {
        let total_failures = ctx.rust_test_failures + ctx.python_test_failures;
        let total_tests = ctx.rust_test_total + ctx.python_test_total;
        let passed = total_failures == 0;
        let details = format!(
            "Full suite: {}/{} passed (Rust: {}/{}, Python: {}/{})",
            total_tests.saturating_sub(total_failures), total_tests,
            ctx.rust_test_total.saturating_sub(ctx.rust_test_failures), ctx.rust_test_total,
            ctx.python_test_total.saturating_sub(ctx.python_test_failures), ctx.python_test_total
        );
        if passed {
            self.pass(&details)
        } else {
            self.fail(&details,
                "CMMI ML5: Integration verification requires all tests passing together")
        }
    }
}

pub struct CommitMessageGate;
impl QualityGate for CommitMessageGate {
    fn name(&self) -> &'static str { "CommitMessageGate" }
    fn phase(&self) -> u8 { 3 }
    fn severity(&self) -> &'static str { "blocking" }
    fn description(&self) -> &'static str {
        "Commit message must follow Conventional Commits specification"
    }
    fn check(&self, ctx: &GateContext) -> GateResult {
        if ctx.commit_message.is_empty() {
            return self.fail("No commit message provided",
                "CMMI ML5: All changes must be traceable via structured commit messages");
        }
        let types = ["feat","fix","docs","style","refactor","perf","test","build","ci","chore","revert"];
        let pattern = format!("^({})(\\(.*\\))?!?:\\s.+", types.join("|"));
        let re = Regex::new(&pattern).expect("valid regex");
        let matches = re.is_match(&ctx.commit_message);
        let details = format!(
            "Commit message: '{}' | {}",
            &ctx.commit_message[..ctx.commit_message.len().min(80)],
            if matches { "Follows Conventional Commits" } else { "Does NOT follow Conventional Commits" }
        );
        if matches {
            self.pass(&details)
        } else {
            self.fail(&details,
                "CMMI ML5: Conventional Commits enable automated changelog and traceability")
        }
    }
}

pub struct GitHubPushGate;
impl QualityGate for GitHubPushGate {
    fn name(&self) -> &'static str { "GitHubPushGate" }
    fn phase(&self) -> u8 { 3 }
    fn severity(&self) -> &'static str { "info" }
    fn description(&self) -> &'static str {
        "GitHub push succeeded (only checked when --push is enabled)"
    }
    fn check(&self, ctx: &GateContext) -> GateResult {
        if !ctx.push_enabled {
            return self.pass("Push not enabled; gate skipped");
        }
        let Some(result) = ctx.push_result.as_ref() else {
            return self.fail("Push enabled but no result data available",
                "CMMI ML5: Successful delivery confirms end-to-end pipeline integrity");
        };
        let url = result.url.as_deref().unwrap_or("N/A");
        let details = format!("Push {} | URL: {}",
            if result.success { "succeeded" } else { "FAILED" }, url);
        if result.success {
            self.pass(&details)
        } else {
            self.fail(&details,
                "CMMI ML5: Successful delivery confirms end-to-end pipeline integrity")
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// QualityGateRunner
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: u8,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub blocking_failed: bool,
    pub results: Vec<GateResult>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllPhasesResult {
    pub overall_passed: bool,
    pub phases: HashMap<String, PhaseResult>,
}

impl Default for AllPhasesResult {
    fn default() -> Self { Self { overall_passed: true, phases: HashMap::new() } }
}

pub struct QualityGateRunner {
    phase1_gates: Vec<Box<dyn QualityGate>>,
    phase2_gates: Vec<Box<dyn QualityGate>>,
    phase3_gates: Vec<Box<dyn QualityGate>>,
}

impl Default for QualityGateRunner {
    fn default() -> Self { Self::new() }
}

impl QualityGateRunner {
    pub fn new() -> Self {
        Self {
            phase1_gates: vec![
                Box::new(PlanCompletenessGate),
                Box::new(PlanMinImprovementsGate),
                Box::new(PlanMinTestsGate),
                Box::new(FormulaSensitivityGate),
            ],
            phase2_gates: vec![
                Box::new(CompilationGate),
                Box::new(RustTestGate),
                Box::new(PythonTestGate),
                Box::new(NewTestCountGate),
            ],
            phase3_gates: vec![
                Box::new(NoRegressionGate),
                Box::new(ImprovementGate),
                Box::new(FullTestSuiteGate),
                Box::new(CommitMessageGate),
                Box::new(GitHubPushGate),
            ],
        }
    }

    fn gate_refs(&self, phase: u8) -> Vec<&dyn QualityGate> {
        match phase {
            1 => self.phase1_gates.iter().map(|b| b.as_ref()).collect(),
            2 => self.phase2_gates.iter().map(|b| b.as_ref()).collect(),
            3 => self.phase3_gates.iter().map(|b| b.as_ref()).collect(),
            _ => vec![],
        }
    }

    pub fn run_phase(&self, phase: u8, context: &GateContext) -> PhaseResult {
        let gates = self.gate_refs(phase);
        let results: Vec<GateResult> = gates.iter().map(|g| g.check(context)).collect();
        let passed_count = results.iter().filter(|r| r.passed).count();
        let blocking_failed = results.iter().any(|r| r.is_blocking_failure());
        let summary = format!("Phase {} Quality Gates: {}/{} passed",
            phase, passed_count, results.len());
        PhaseResult {
            phase,
            total: results.len(),
            passed: passed_count,
            failed: results.len() - passed_count,
            blocking_failed,
            results,
            summary,
        }
    }

    pub fn run_all(&self, context: &GateContext) -> AllPhasesResult {
        let mut result = AllPhasesResult::default();
        for phase in [1u8, 2, 3] {
            let phase_result = self.run_phase(phase, context);
            result.phases.insert(format!("phase_{}", phase), phase_result);
        }
        result.overall_passed = result.phases.values().all(|p| !p.blocking_failed);
        result
    }

    pub fn gate_names(&self, phase: u8) -> Vec<&'static str> {
        self.gate_refs(phase).iter().map(|g| g.name()).collect()
    }

    pub fn run_named(&self, name: &str, context: &GateContext) -> Option<GateResult> {
        let all: Vec<&dyn QualityGate> = self.phase1_gates.iter()
            .chain(self.phase2_gates.iter())
            .chain(self.phase3_gates.iter())
            .map(|b| b.as_ref()).collect();
        all.into_iter().find(|g| g.name() == name).map(|g| g.check(context))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_imp(id: &str, param: &str, tests: usize) -> Improvement {
        Improvement {
            id: Some(id.to_string()),
            param: Some(param.to_string()),
            file: Some("formula.rs".to_string()),
            tests_added: Some(tests),
            phase: Some("1".to_string()),
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_plan_completeness_pass() {
        let ctx = GateContext {
            improvements: vec![make_imp("imp1", "alpha", 5), make_imp("imp2", "beta", 3)],
            ..Default::default()
        };
        assert!(PlanCompletenessGate.check(&ctx).passed);
    }

    #[test]
    fn test_plan_completeness_fail() {
        let ctx = GateContext {
            improvements: vec![Improvement {
                id: Some("imp1".to_string()),
                param: None,
                file: None,
                tests_added: None,
                phase: None,
                extra: HashMap::new(),
            }],
            ..Default::default()
        };
        assert!(!PlanCompletenessGate.check(&ctx).passed);
    }

    #[test]
    fn test_plan_min_improvements_pass() {
        let ctx = GateContext {
            improvements: vec![make_imp("1","a",1), make_imp("2","b",1), make_imp("3","c",1)],
            ..Default::default()
        };
        assert!(PlanMinImprovementsGate.check(&ctx).passed);
    }

    #[test]
    fn test_plan_min_improvements_fail() {
        let ctx = GateContext {
            improvements: vec![make_imp("1","a",1)],
            ..Default::default()
        };
        assert!(!PlanMinImprovementsGate.check(&ctx).passed);
    }

    #[test]
    fn test_plan_min_tests_pass() {
        let ctx = GateContext {
            improvements: vec![make_imp("1","a",6), make_imp("2","b",5)],
            ..Default::default()
        };
        assert!(PlanMinTestsGate.check(&ctx).passed);
    }

    #[test]
    fn test_plan_min_tests_fail() {
        let ctx = GateContext {
            improvements: vec![make_imp("1","a",5)],
            ..Default::default()
        };
        assert!(!PlanMinTestsGate.check(&ctx).passed);
    }

    #[test]
    fn test_no_regression_pass() {
        let ctx = GateContext {
            before_grade: "B".to_string(),
            after_grade: "A".to_string(),
            before_score: 0.8,
            after_score: 0.85,
            ..Default::default()
        };
        assert!(NoRegressionGate.check(&ctx).passed);
    }

    #[test]
    fn test_no_regression_fail() {
        let ctx = GateContext {
            before_grade: "A".to_string(),
            after_grade: "B".to_string(),
            before_score: 0.85,
            after_score: 0.8,
            ..Default::default()
        };
        assert!(!NoRegressionGate.check(&ctx).passed);
    }

    #[test]
    fn test_improvement_gate_pass() {
        let ctx = GateContext {
            param_changes: HashMap::from([
                ("alpha".to_string(), 0.05),
                ("beta".to_string(), -0.02),
            ]),
            ..Default::default()
        };
        assert!(ImprovementGate.check(&ctx).passed);
    }

    #[test]
    fn test_improvement_gate_fail() {
        let ctx = GateContext {
            param_changes: HashMap::from([
                ("alpha".to_string(), -0.05),
                ("beta".to_string(), -0.02),
            ]),
            ..Default::default()
        };
        assert!(!ImprovementGate.check(&ctx).passed);
    }

    #[test]
    fn test_full_test_suite_pass() {
        let ctx = GateContext {
            rust_test_failures: 0,
            rust_test_total: 10,
            python_test_failures: 0,
            python_test_total: 20,
            ..Default::default()
        };
        assert!(FullTestSuiteGate.check(&ctx).passed);
    }

    #[test]
    fn test_full_test_suite_fail() {
        let ctx = GateContext {
            rust_test_failures: 0,
            rust_test_total: 10,
            python_test_failures: 2,
            python_test_total: 20,
            ..Default::default()
        };
        assert!(!FullTestSuiteGate.check(&ctx).passed);
    }

    #[test]
    fn test_commit_message_valid() {
        let ctx = GateContext {
            commit_message: "feat(formula): add new optimization".to_string(),
            ..Default::default()
        };
        assert!(CommitMessageGate.check(&ctx).passed);
    }

    #[test]
    fn test_commit_message_invalid() {
        let ctx = GateContext {
            commit_message: "Updated stuff".to_string(),
            ..Default::default()
        };
        assert!(!CommitMessageGate.check(&ctx).passed);
    }

    #[test]
    fn test_runner_run_all() {
        let ctx = GateContext {
            improvements: vec![make_imp("1","a",6), make_imp("2","b",5)],
            rust_test_failures: 0,
            rust_test_total: 10,
            python_test_failures: 0,
            python_test_total: 20,
            commit_message: "feat(formula): add optimization improvements".to_string(),
            param_changes: HashMap::from([("alpha".to_string(), 0.05)]),
            ..Default::default()
        };
        let runner = QualityGateRunner::new();
        let result = runner.run_all(&ctx);
        assert!(result.overall_passed);
    }
}
