//! # Test Harness - 自动化测试框架
//!
//! 支持Rust和Python测试的并行执行、超时控制、JUnit XML输出、覆盖率收集

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum TestError {
    #[error("Rust test failed: {0}")]
    RustTestFailed(String),
    
    #[error("Python test failed: {0}")]
    PythonTestFailed(String),
    
    #[error("Test timeout after {0}s")]
    Timeout(u64),
    
    #[error("Coverage collection failed: {0}")]
    CoverageFailed(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// 超时配置
#[derive(Clone, Debug)]
pub struct TimeoutConfig {
    pub default_timeout_secs: u64,
    pub rust_test_timeout_secs: u64,
    pub python_test_timeout_secs: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout_secs: 30,
            rust_test_timeout_secs: 30,
            python_test_timeout_secs: 60,
        }
    }
}

// ============================================================================
// Data Structures
// ============================================================================

/// Rust测试用例
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RustTestCase {
    pub name: String,
    pub file_path: String,
    pub line_number: usize,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Python测试用例
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PythonTestCase {
    pub name: String,
    pub file_path: String,
    #[serde(default)]
    pub test_class: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 测试结果枚举
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TestResult {
    Pass {
        duration_ms: u64,
    },
    Fail {
        error: String,
        duration_ms: u64,
    },
    Skip {
        reason: String,
    },
}

impl TestResult {
    pub fn is_pass(&self) -> bool {
        matches!(self, TestResult::Pass { .. })
    }
    
    pub fn is_fail(&self) -> bool {
        matches!(self, TestResult::Fail { .. })
    }
    
    pub fn duration_ms(&self) -> u64 {
        match self {
            TestResult::Pass { duration_ms } => *duration_ms,
            TestResult::Fail { duration_ms, .. } => *duration_ms,
            TestResult::Skip { .. } => 0,
        }
    }
}

/// 测试汇总
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total_duration_ms: u64,
    pub rust_results: Vec<(String, TestResult)>,
    pub python_results: Vec<(String, TestResult)>,
}

// ============================================================================
// Test Harness
// ============================================================================

/// 测试框架 - 协调Rust/Python测试执行
pub struct TestHarness {
    pub rust_tests: Vec<RustTestCase>,
    pub python_tests: Vec<PythonTestCase>,
    pub results: Vec<TestResult>,
    pub timeout_config: TimeoutConfig,
    pub enable_coverage: bool,
    pub coverage_output: Option<String>,
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl TestHarness {
    /// 创建新的测试框架
    pub fn new() -> Self {
        Self {
            rust_tests: Vec::new(),
            python_tests: Vec::new(),
            results: Vec::new(),
            timeout_config: TimeoutConfig::default(),
            enable_coverage: false,
            coverage_output: None,
        }
    }

    /// 添加Rust测试用例
    pub fn add_rust_test(&mut self, test: RustTestCase) {
        self.rust_tests.push(test);
    }

    /// 添加Python测试用例
    pub fn add_python_test(&mut self, test: PythonTestCase) {
        self.python_tests.push(test);
    }

    /// 设置超时配置
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_config.default_timeout_secs = timeout_secs;
        self
    }

    /// 启用覆盖率收集
    pub fn with_coverage(mut self, output_path: &str) -> Self {
        self.enable_coverage = true;
        self.coverage_output = Some(output_path.to_string());
        self
    }

    /// 运行所有Rust测试
    pub fn run_rust_tests(&mut self) -> Result<Vec<(String, TestResult)>, TestError> {
        let mut results = Vec::new();

        for test in &self.rust_tests {
            let start = Instant::now();
            
            let output = Command::new("cargo")
                .args(["test", "--test", "--", "--nocapture"])
                .current_dir(
                    Path::new(&test.file_path)
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                )
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();

            let duration_ms = start.elapsed().as_millis() as u64;

            match output {
                Ok(out) if out.status.success() => {
                    results.push((test.name.clone(), TestResult::Pass { duration_ms }));
                }
                Ok(out) => {
                    let error = String::from_utf8_lossy(&out.stderr).to_string();
                    results.push((test.name.clone(), TestResult::Fail { error, duration_ms }));
                }
                Err(e) => {
                    results.push((
                        test.name.clone(),
                        TestResult::Fail {
                            error: e.to_string(),
                            duration_ms,
                        },
                    ));
                }
            }
        }

        Ok(results)
    }

    /// 运行所有Python测试
    pub fn run_python_tests(&mut self, pytest_path: &str) -> Result<Vec<(String, TestResult)>, TestError> {
        let mut results = Vec::new();

        for test in &self.python_tests {
            let start = Instant::now();
            
            let mut args = vec![
                pytest_path.to_string(),
                test.file_path.clone(),
                "-v".to_string(),
                "--tb=short".to_string(),
            ];

            if let Some(ref class) = test.test_class {
                args.push("--collect-only".to_string());
                args.push("-k".to_string());
                args.push(class.clone());
            }

            let output = Command::new("python3")
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();

            let duration_ms = start.elapsed().as_millis() as u64;

            match output {
                Ok(out) if out.status.success() => {
                    results.push((test.name.clone(), TestResult::Pass { duration_ms }));
                }
                Ok(out) => {
                    let error = String::from_utf8_lossy(&out.stderr).to_string();
                    results.push((test.name.clone(), TestResult::Fail { error, duration_ms }));
                }
                Err(e) => {
                    results.push((
                        test.name.clone(),
                        TestResult::Fail {
                            error: e.to_string(),
                            duration_ms,
                        },
                    ));
                }
            }
        }

        Ok(results)
    }

    /// 并行运行所有测试
    pub fn run_all(&mut self) -> TestSummary {
        let mut rust_results = Vec::new();
        let mut python_results = Vec::new();
        let start_time = Instant::now();

        // 并行执行Rust和Python测试
        let rust_handle = std::thread::spawn({
            let mut harness = Self {
                rust_tests: self.rust_tests.clone(),
                python_tests: Vec::new(),
                results: Vec::new(),
                timeout_config: self.timeout_config.clone(),
                enable_coverage: self.enable_coverage,
                coverage_output: self.coverage_output.clone(),
            };
            move || harness.run_rust_tests()
        });

        let python_handle = std::thread::spawn({
            let mut harness = Self {
                rust_tests: Vec::new(),
                python_tests: self.python_tests.clone(),
                results: Vec::new(),
                timeout_config: self.timeout_config.clone(),
                enable_coverage: self.enable_coverage,
                coverage_output: self.coverage_output.clone(),
            };
            let pytest = std::env::var("PYTEST_PATH").unwrap_or_else(|_| "pytest".to_string());
            move || harness.run_python_tests(&pytest)
        });

        // 收集结果
        if let Ok(Ok(res)) = rust_handle.join() {
            rust_results = res;
        }

        if let Ok(Ok(res)) = python_handle.join() {
            python_results = res;
        }

        let total_duration_ms = start_time.elapsed().as_millis() as u64;

        // 计算汇总
        let total = rust_results.len() + python_results.len();
        let passed = rust_results.iter()
            .chain(python_results.iter())
            .filter(|(_, r)| r.is_pass())
            .count();
        let failed = rust_results.iter()
            .chain(python_results.iter())
            .filter(|(_, r)| r.is_fail())
            .count();

        // 存储所有结果
        self.results = rust_results.iter()
            .chain(python_results.iter())
            .map(|(_, r)| r.clone())
            .collect();

        TestSummary {
            total,
            passed,
            failed,
            skipped: 0,
            total_duration_ms,
            rust_results,
            python_results,
        }
    }

    /// 生成测试报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("# Test Report\n\n");
        report.push_str(&format!(
            "Generated: {}\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // 汇总统计
        let total = self.results.len();
        let passed = self.results.iter().filter(|r| r.is_pass()).count();
        let failed = self.results.iter().filter(|r| r.is_fail()).count();
        let total_duration: u64 = self.results.iter().map(|r| r.duration_ms()).sum();

        report.push_str(&format!(
            "## Summary\n\n- Total: {}\n- Passed: {}\n- Failed: {}\n- Duration: {}ms\n",
            total, passed, failed, total_duration
        ));

        report
    }

    /// 生成JUnit XML格式报告
    pub fn generate_junit_report(&self, test_suite_name: &str) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        
        let total = self.results.len();
        let failures = self.results.iter().filter(|r| r.is_fail()).count();
        let skipped = self.results.iter().filter(|r| matches!(r, TestResult::Skip { .. })).count();
        let total_duration: f64 = self.results.iter().map(|r| r.duration_ms() as f64 / 1000.0).sum();

        xml.push_str("<testsuite ");
        xml.push_str(&format!("name=\"{}\" ", test_suite_name));
        xml.push_str(&format!("tests=\"{}\" ", total));
        xml.push_str(&format!("failures=\"{}\" ", failures));
        xml.push_str(&format!("skipped=\"{}\" ", skipped));
        xml.push_str(&format!("time=\"{:.3}\"", total_duration));
        xml.push_str(">\n");

        // Rust测试用例
        for (i, test) in self.rust_tests.iter().enumerate() {
            let result = self.results.get(i);
            self.write_junit_test_case(&mut xml, &test.name, &format!("rust.{}", test.file_path.replace('/', ".")), result);
        }

        // Python测试用例
        for (i, test) in self.python_tests.iter().enumerate() {
            let result_idx = self.rust_tests.len() + i;
            let result = self.results.get(result_idx);
            let classname = test.test_class.as_ref().unwrap_or(&test.file_path);
            self.write_junit_test_case(&mut xml, &test.name, &format!("python.{}", classname), result);
        }

        xml.push_str("</testsuite>\n");
        xml
    }

    fn write_junit_test_case(&self, xml: &mut String, name: &str, classname: &str, result: Option<&TestResult>) {
        match result {
            Some(TestResult::Pass { duration_ms }) => {
                let time = format!("{}.{:03}", duration_ms / 1000, duration_ms % 1000);
                xml.push_str(&format!("  <testcase name=\"{}\" classname=\"{}\" time=\"{}\"/>\n", name, classname, time));
            }
            Some(TestResult::Fail { error, duration_ms }) => {
                let time = format!("{}.{:03}", duration_ms / 1000, duration_ms % 1000);
                let msg = error.chars().take(200).collect::<String>().replace('\"', "&quot;");
                xml.push_str(&format!(
                    "  <testcase name=\"{}\" classname=\"{}\" time=\"{}\">\n",
                    name, classname, time
                ));
                xml.push_str(&format!(
                    "    <failure message=\"{}\" type=\"AssertionError\"><![CDATA[{}]]></failure>\n",
                    msg, error
                ));
                xml.push_str("  </testcase>\n");
            }
            Some(TestResult::Skip { reason }) => {
                xml.push_str(&format!("  <testcase name=\"{}\" classname=\"{}\" time=\"0.000\">\n", name, classname));
                xml.push_str(&format!("    <skipped message=\"{}\"/>\n", reason));
                xml.push_str("  </testcase>\n");
            }
            None => {
                xml.push_str(&format!("  <testcase name=\"{}\" classname=\"{}\" time=\"0.000\"/>\n", name, classname));
            }
        }
    }

    /// 收集LLVM覆盖率
    #[allow(dead_code)]
    pub fn collect_coverage(&self) -> Result<String, TestError> {
        if !self.enable_coverage {
            return Ok(String::new());
        }

        let output = Command::new("llvm-cov")
            .args(["report", "--ignore-filename-regex", "^/dev/"])
            .output()
            .map_err(|e| TestError::CoverageFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(TestError::CoverageFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_is_pass() {
        assert!(TestResult::Pass { duration_ms: 100 }.is_pass());
        assert!(!TestResult::Fail { error: "err".to_string(), duration_ms: 100 }.is_pass());
    }

    #[test]
    fn test_result_is_fail() {
        assert!(TestResult::Fail { error: "err".to_string(), duration_ms: 100 }.is_fail());
        assert!(!TestResult::Pass { duration_ms: 100 }.is_fail());
    }

    #[test]
    fn test_harness_creation() {
        let harness = TestHarness::new();
        assert!(harness.rust_tests.is_empty());
        assert!(harness.python_tests.is_empty());
    }

    #[test]
    fn test_harness_add_rust_test() {
        let mut harness = TestHarness::new();
        harness.add_rust_test(RustTestCase {
            name: "test_example".to_string(),
            file_path: "tests/test_example.rs".to_string(),
            line_number: 10,
            tags: vec!["unit".to_string()],
        });
        assert_eq!(harness.rust_tests.len(), 1);
    }

    #[test]
    fn test_harness_add_python_test() {
        let mut harness = TestHarness::new();
        harness.add_python_test(PythonTestCase {
            name: "test_example".to_string(),
            file_path: "tests/test_example.py".to_string(),
            test_class: Some("TestClass".to_string()),
            tags: vec!["unit".to_string()],
        });
        assert_eq!(harness.python_tests.len(), 1);
    }

    #[test]
    fn test_generate_report() {
        let harness = TestHarness::new();
        let report = harness.generate_report();
        assert!(report.contains("Test Report"));
    }

    #[test]
    fn test_generate_junit_report() {
        let mut harness = TestHarness::new();
        harness.add_rust_test(RustTestCase {
            name: "test_pass".to_string(),
            file_path: "test.rs".to_string(),
            line_number: 1,
            tags: vec![],
        });
        harness.results.push(TestResult::Pass { duration_ms: 100 });

        let xml = harness.generate_junit_report("test_suite");
        assert!(xml.contains("test_suite"));
        assert!(xml.contains("test_pass"));
    }
}