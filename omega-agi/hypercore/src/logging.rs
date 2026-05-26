//! # APEX Structured Logging
//!
//! Utilities for structured, correlation-aware logging in the OMEGA HyperCore.
//! Provides initialization helpers, correlation-ID scoping, and log quality scoring.

use std::fmt;

use tracing::subscriber::DefaultGuard;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

/// Initialize the APEX structured logging subsystem.
///
/// Returns a `DefaultGuard` that, when dropped, restores the previous global
/// subscriber. The component name is attached as a fixed field to all log lines.
pub fn init_apex_logging(component: &str) -> DefaultGuard {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!("{}=debug,omega_hypercore=debug", component))
    });

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_target(true)
        .finish();

    tracing::subscriber::set_default(subscriber)
}

/// RAII guard that pushes a correlation ID onto the current tracing span.
///
/// While the guard is alive, all log lines emitted within its scope carry
/// the `correlation_id` field.
pub struct CorrelationGuard {
    _span: tracing::span::EnteredSpan,
}

impl fmt::Debug for CorrelationGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CorrelationGuard").finish_non_exhaustive()
    }
}

/// Enter a correlation scope. All log lines emitted while the returned guard
/// is alive will include the given correlation ID.
pub fn with_correlation(id: Uuid) -> CorrelationGuard {
    let span = tracing::info_span!(
        "correlation",
        correlation_id = %id,
    );
    CorrelationGuard {
        _span: span.entered(),
    }
}

/// A deserialized log entry used for quality analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    /// The component that produced this log entry.
    pub component: String,
    /// The operation being performed.
    pub operation: String,
    /// Duration of the operation in microseconds.
    pub duration_us: u64,
    /// Correlation ID associated with the entry.
    pub correlation_id: String,
    /// Log level (e.g., "info", "warn", "error").
    pub level: String,
    /// The log message body.
    pub message: String,
}

/// Score the quality of a set of log entries on a 0.0--1.0 scale.
///
/// The scoring criteria are:
/// - All entries must have a **non-empty** `component`.
/// - All entries must have a **non-empty** `operation`.
/// - All entries must have a **non-empty** `correlation_id`.
/// - All entries must have `duration_us > 0`.
///
/// Each criterion contributes 0.25 to the final score.
pub fn score_log_quality(entries: &[LogEntry]) -> f64 {
    if entries.is_empty() {
        return 0.0;
    }

    let total = entries.len() as f64;

    let has_component = entries.iter().filter(|e| !e.component.is_empty()).count() as f64 / total;
    let has_operation = entries.iter().filter(|e| !e.operation.is_empty()).count() as f64 / total;
    let has_correlation = entries.iter().filter(|e| !e.correlation_id.is_empty()).count() as f64 / total;
    let has_duration = entries.iter().filter(|e| e.duration_us > 0).count() as f64 / total;

    (has_component + has_operation + has_correlation + has_duration) / 4.0
}

/// Helper to build a `LogEntry` for testing.
#[cfg(test)]
fn make_entry(
    component: &str,
    operation: &str,
    duration_us: u64,
    correlation_id: &str,
    level: &str,
    message: &str,
) -> LogEntry {
    LogEntry {
        component: component.to_string(),
        operation: operation.to_string(),
        duration_us,
        correlation_id: correlation_id.to_string(),
        level: level.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_perfect_entries() {
        let entries = vec![
            make_entry("scheduler", "spawn", 120, "corr-001", "info", "task spawned"),
            make_entry("memory", "alloc", 45, "corr-002", "debug", "allocated 4KB"),
        ];
        let score = score_log_quality(&entries);
        assert!(
            (score - 1.0).abs() < f64::EPSILON,
            "expected 1.0, got {}",
            score
        );
    }

    #[test]
    fn test_score_empty_entries() {
        let entries: Vec<LogEntry> = Vec::new();
        let score = score_log_quality(&entries);
        assert!(
            (score - 0.0).abs() < f64::EPSILON,
            "expected 0.0 for empty, got {}",
            score
        );
    }

    #[test]
    fn test_score_missing_component() {
        let entries = vec![
            make_entry("", "op", 100, "corr", "info", "msg"),
        ];
        let score = score_log_quality(&entries);
        // Only 3 of 4 criteria met -> 0.75
        assert!(
            (score - 0.75).abs() < f64::EPSILON,
            "expected 0.75, got {}",
            score
        );
    }

    #[test]
    fn test_score_zero_duration() {
        let entries = vec![
            make_entry("comp", "op", 0, "corr", "info", "msg"),
        ];
        let score = score_log_quality(&entries);
        // Only 3 of 4 criteria met -> 0.75
        assert!(
            (score - 0.75).abs() < f64::EPSILON,
            "expected 0.75, got {}",
            score
        );
    }

    #[test]
    fn test_score_all_fields_empty() {
        let entries = vec![
            LogEntry {
                component: String::new(),
                operation: String::new(),
                duration_us: 0,
                correlation_id: String::new(),
                level: String::new(),
                message: String::new(),
            },
        ];
        let score = score_log_quality(&entries);
        assert!(
            (score - 0.0).abs() < f64::EPSILON,
            "expected 0.0, got {}",
            score
        );
    }
}
