// OMEGA Engineering Library
// Layer 3 - Code Engineering & Quality Assurance

pub mod code_generator;
pub mod test_harness;
pub mod pr_manager;
pub mod swe_bench;
pub mod reviewer;
pub mod quality_gates;

pub use code_generator::{CodeGenerator, GeneratedCode, Language, CodeQuality, CodeContext, GenError};
pub use test_harness::{TestHarness, TestResult, RustTestCase, PythonTestCase, TestSummary, TestError, TimeoutConfig};
pub use pr_manager::{PRManager, PRState, PRStatus, Review, ReviewState, CheckRun, PRError};
pub use swe_bench::{SWEBench, SWEBenchResult, InstanceStatus, Resolution, InstanceMetrics, EvaluationSummary, run_swe_bench, evaluate_instances, SWEError};
pub use reviewer::{Reviewer, Severity, ReviewRule, FileReview, ReviewSummary, ReviewError};
pub use quality_gates::{QualityGate, GateResult, GateContext, QualityGateRunner, PhaseResult};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
