//! Optimization Module for Omega-AGI
//!
//! Performance self-optimization with automatic bottleneck detection,
//! analysis, and repair capabilities.

pub mod performance_optimizer;

pub use performance_optimizer::{
    AppliedOptimization, Bottleneck, CallSite, CodeTransform, CpuMetrics,
    IoMetrics, MemoryMetrics, MetricsCollector, OptError, OptimizationRules,
    OptimizationStrategy, OptimizationSuggestion, OptimizerConfig,
    OptimizerSummary, PerformanceOptimizer, ProfilingResult, RiskLevel,
};