//! Engineering层 (Layer 3) - 代码工程与质量保证
//!
//! 功能模块:
//! - 代码生成器: 自动生成高质量代码
//! - 测试框架: 自动化测试执行
//! - PR管理: 完整的PR生命周期
//! - 质量门禁: CMMI Level 5标准
//! - SWE-bench集成: 软件工程基准测试
//! - 代码审查: 自动代码审查

pub mod code_generator;
pub mod test_harness;
pub mod pr_manager;
pub mod quality_gates;
pub mod swe_bench;
pub mod reviewer;

pub use code_generator::{CodeGenerator, GeneratedCode};
pub use test_harness::{TestHarness, TestResult};
pub use pr_manager::{PRManager, PullRequest, PRStatus};
pub use quality_gates::{QualityGates, QualityReport};
pub use swe_bench::{SWEBenchRunner, BenchmarkResult};
pub use reviewer::{CodeReviewer, ReviewComment};
