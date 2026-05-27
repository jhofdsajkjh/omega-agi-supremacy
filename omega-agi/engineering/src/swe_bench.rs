//! SWE-bench Lite Integration
//!
//! Implements SWE-bench benchmark evaluation for code repair tasks.
//! Each instance runs inside an isolated Docker container, receives
//! a problem statement, applies a candidate patch, and runs the test suite.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum SWEError {
    #[error("Docker error: {0}")]
    DockerError(String),

    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    #[error("Setup failed: {0}")]
    SetupFailed(String),

    #[error("Patch failed to apply")]
    PatchApplyFailed,

    #[error("Execution timeout after {0}s")]
    Timeout(u64),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("SWE-bench dataset not found at: {0}")]
    DatasetNotFound(String),
}

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single SWE-bench instance (one code-repair task).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SWEBench {
    /// Unique identifier, e.g. "django__django-11099"
    pub instance_id: String,
    /// Repository identifier, e.g. "django/django"
    pub repo: String,
    /// The git commit this instance was captured at.
    pub commit: String,
    /// The problem statement / bug description.
    pub problem_statement: String,
    /// The golden test patch that should make tests pass.
    pub test_patch: String,
    /// Reference patch (not always provided).
    pub reference_patch: Option<String>,
    /// Docker image used to build the environment.
    pub environment_image: String,
    /// Version of the framework / language.
    pub version: Option<String>,
    /// Test command to run.
    pub test_cmd: Option<String>,
    /// Expected failure tests (may need adjustment).
    pub failing_tests: Option<Vec<String>>,
}

impl SWEBench {
    /// Load instances from a SWE-bench JSON dataset file.
    pub fn load_dataset(path: &Path) -> Result<Vec<SWEBench>, SWEError> {
        if !path.exists() {
            return Err(SWEError::DatasetNotFound(path.display().to_string()));
        }
        let text = std::fs::read_to_string(path)?;
        let instances: Vec<SWEBench> = serde_json::from_str(&text)?;
        Ok(instances)
    }
}

/// Resolution outcome for one instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceStatus {
    Resolved,
    Unresolved,
    Failed,
    Timeout,
}

/// Details of what happened when the patch was applied and tests run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    /// Whether the candidate patch applied without merge conflicts.
    pub patch_applied: bool,
    /// Whether the test suite passed after applying the patch.
    pub tests_passed: bool,
    /// Logs from environment setup.
    pub setup_logs: String,
    /// stdout + stderr from the test run.
    pub test_logs: String,
    /// stderr from the patch application step.
    pub patch_error: Option<String>,
}

/// Per-instance performance metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceMetrics {
    /// Wall-clock time from start to test completion (ms).
    pub resolution_time_ms: u64,
    /// Peak memory usage observed (MB).
    pub memory_peak_mb: u64,
    /// Total CPU time consumed (ms).
    pub cpu_time_ms: u64,
    /// Maximum RSS at end of execution (MB).
    pub max_rss_mb: Option<u64>,
}

/// Result of evaluating one SWE-bench instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SWEBenchResult {
    pub instance_id: String,
    pub status: InstanceStatus,
    pub resolution: Resolution,
    pub metrics: InstanceMetrics,
    /// Timestamp when evaluation completed.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub evaluated_at: DateTime<Utc>,
}

impl SWEBenchResult {
    /// Quick pass/fail check.
    pub fn is_resolved(&self) -> bool {
        matches!(self.status, InstanceStatus::Resolved)
    }
}

/// Summary across a batch of results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvaluationSummary {
    pub total: usize,
    pub resolved: usize,
    pub unresolved: usize,
    pub failed: usize,
    pub timeout: usize,
    pub resolution_rate: f64,
}

impl EvaluationSummary {
    pub fn from_results(results: &[SWEBenchResult]) -> Self {
        let total = results.len();
        let resolved = results.iter().filter(|r| r.is_resolved()).count();
        let unresolved = results
            .iter()
            .filter(|r| matches!(r.status, InstanceStatus::Unresolved))
            .count();
        let failed = results
            .iter()
            .filter(|r| matches!(r.status, InstanceStatus::Failed))
            .count();
        let timeout = results
            .iter()
            .filter(|r| matches!(r.status, InstanceStatus::Timeout))
            .count();
        let resolution_rate = if total > 0 {
            resolved as f64 / total as f64
        } else {
            0.0
        };

        Self { total, resolved, unresolved, failed, timeout, resolution_rate }
    }
}

// ---------------------------------------------------------------------------
// Docker executor
// ---------------------------------------------------------------------------

/// Runs a SWE-bench instance inside a Docker container.
pub struct DockerExecutor {
    /// Timeout per instance in seconds.
    pub timeout_secs: u64,
    /// Path to the local SWE-bench dataset / workspace.
    pub workspace: std::path::PathBuf,
}

impl DockerExecutor {
    /// Create a new executor.
    pub fn new(timeout_secs: u64, workspace: impl Into<std::path::PathBuf>) -> Self {
        Self { timeout_secs, workspace: workspace.into() }
    }

    /// Evaluate one SWE-bench instance.
    pub fn execute(&self, instance: &SWEBench) -> Result<SWEBenchResult, SWEError> {
        let start = Instant::now();
        let output = self.run_docker(instance)?;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Parse exit code into status and resolution details
        let (status, resolution) = match output.status.code() {
            Some(0) => (
                InstanceStatus::Resolved,
                Resolution {
                    patch_applied: true,
                    tests_passed: true,
                    setup_logs: String::new(),
                    test_logs: String::from_utf8_lossy(&output.stdout).to_string(),
                    patch_error: None,
                },
            ),
            // Tests ran but failed -- patch applied, wrong answer
            Some(1) => (
                InstanceStatus::Unresolved,
                Resolution {
                    patch_applied: true,
                    tests_passed: false,
                    setup_logs: String::new(),
                    test_logs: String::from_utf8_lossy(&output.stdout).to_string(),
                    patch_error: None,
                },
            ),
            // Exit 124 = timeout from wrapper
            Some(124) => (
                InstanceStatus::Timeout,
                Resolution {
                    patch_applied: false,
                    tests_passed: false,
                    setup_logs: String::new(),
                    test_logs: String::from_utf8_lossy(&output.stdout).to_string(),
                    patch_error: Some("Execution timed out".into()),
                },
            ),
            _ => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                (
                    InstanceStatus::Failed,
                    Resolution {
                        patch_applied: false,
                        tests_passed: false,
                        setup_logs: stderr.clone(),
                        test_logs: String::from_utf8_lossy(&output.stdout).to_string(),
                        patch_error: Some(stderr),
                    },
                )
            }
        };

        let metrics = InstanceMetrics {
            resolution_time_ms: elapsed_ms,
            memory_peak_mb: 0,
            cpu_time_ms: elapsed_ms,
            max_rss_mb: None,
        };

        Ok(SWEBenchResult {
            instance_id: instance.instance_id.clone(),
            status,
            resolution,
            metrics,
            evaluated_at: Utc::now(),
        })
    }

    /// Build the shell script to run inside Docker.
    fn build_script(&self, instance: &SWEBench) -> String {
        let test_cmd = instance.test_cmd.as_deref().unwrap_or("python -m pytest tests/ -x");
        format!(
            "set -e\n\
             cd /workspace\n\
             git clone --depth 1 https://github.com/{repo}.git /tmp/repo 2>/dev/null || true\n\
             cd /tmp/repo\n\
             git fetch origin {commit} --depth 1\n\
             git checkout {commit}\n\
             printf '%s' '{patch}' > /tmp/candidate.patch\n\
             git apply /tmp/candidate.patch 2>&1 || echo PATCH_APPLY_FAILED >&2\n\
             echo '---TESTS---'\n\
             {test_cmd}\n\
             exit $?\n",
            repo = instance.repo,
            commit = instance.commit,
            patch = instance.test_patch.replace('\'', "'\\''"),
            test_cmd = test_cmd,
        )
    }

    /// Build a Docker run command and execute it.
    fn run_docker(&self, instance: &SWEBench) -> Result<std::process::Output, SWEError> {
        let container_name = format!(
            "swebench-{}-{}",
            instance.repo.replace('/', "-"),
            instance.instance_id.replace('|', "-")
        );

        let script = self.build_script(instance);

        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--name",
                &container_name,
                "--network=none",
                "--memory=2g",
                "--cpus=2",
                "-i",
                instance.environment_image.as_str(),
                "sh",
                "-c",
                &script,
            ])
            .output()
            .map_err(|e| SWEError::DockerError(e.to_string()))?;

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// High-level API
// ---------------------------------------------------------------------------

/// Run evaluation on a single instance.
pub fn run_swe_bench(instance: &SWEBench) -> Result<SWEBenchResult, SWEError> {
    let executor = DockerExecutor::new(300, "/workspace/swebench");
    executor.execute(instance)
}

/// Run evaluation on a batch of instances, returning results in input order.
/// Individual failures are captured as Failed results rather than propagating.
pub fn evaluate_instances(instances: &[SWEBench]) -> Vec<SWEBenchResult> {
    let mut results = Vec::with_capacity(instances.len());

    for instance in instances {
        let result = run_swe_bench(instance);
        results.push(result.unwrap_or_else(|e| SWEBenchResult {
            instance_id: instance.instance_id.clone(),
            status: InstanceStatus::Failed,
            resolution: Resolution {
                patch_applied: false,
                tests_passed: false,
                setup_logs: e.to_string(),
                test_logs: String::new(),
                patch_error: None,
            },
            metrics: InstanceMetrics {
                resolution_time_ms: 0,
                memory_peak_mb: 0,
                cpu_time_ms: 0,
                max_rss_mb: None,
            },
            evaluated_at: Utc::now(),
        }));
    }

    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary() {
        let results = vec![
            SWEBenchResult {
                instance_id: "a".into(),
                status: InstanceStatus::Resolved,
                resolution: Resolution {
                    patch_applied: true,
                    tests_passed: true,
                    setup_logs: String::new(),
                    test_logs: String::new(),
                    patch_error: None,
                },
                metrics: InstanceMetrics {
                    resolution_time_ms: 100,
                    memory_peak_mb: 0,
                    cpu_time_ms: 0,
                    max_rss_mb: None,
                },
                evaluated_at: Utc::now(),
            },
            SWEBenchResult {
                instance_id: "b".into(),
                status: InstanceStatus::Unresolved,
                resolution: Resolution {
                    patch_applied: true,
                    tests_passed: false,
                    setup_logs: String::new(),
                    test_logs: String::new(),
                    patch_error: None,
                },
                metrics: InstanceMetrics {
                    resolution_time_ms: 100,
                    memory_peak_mb: 0,
                    cpu_time_ms: 0,
                    max_rss_mb: None,
                },
                evaluated_at: Utc::now(),
            },
        ];
        let s = EvaluationSummary::from_results(&results);
        assert_eq!(s.total, 2);
        assert_eq!(s.resolved, 1);
        assert!((s.resolution_rate - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_resolved_check() {
        let r = SWEBenchResult {
            instance_id: "test".into(),
            status: InstanceStatus::Resolved,
            resolution: Resolution {
                patch_applied: true,
                tests_passed: true,
                setup_logs: String::new(),
                test_logs: String::new(),
                patch_error: None,
            },
            metrics: InstanceMetrics {
                resolution_time_ms: 100,
                memory_peak_mb: 0,
                cpu_time_ms: 0,
                max_rss_mb: None,
            },
            evaluated_at: Utc::now(),
        };
        assert!(r.is_resolved());
    }
}