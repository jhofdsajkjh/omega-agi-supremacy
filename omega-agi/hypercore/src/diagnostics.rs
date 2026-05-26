//! # Diagnostic Engine
//!
//! Probes all subsystems (scheduler, session manager) and produces a
//! unified [`SystemHealthReport`] with per-subsystem scores and an
//! overall geometric-mean score.

use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::scheduler::TaskScheduler;
use crate::session::SessionManager;

/// Health snapshot for a single subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemHealth {
    /// Name of the subsystem (e.g. "scheduler", "session_manager").
    pub name: String,
    /// `true` when the subsystem is considered healthy.
    pub healthy: bool,
    /// Normalised health score in `[0.0, 1.0]`.
    pub score: f64,
    /// Human-readable details.
    pub details: String,
}

/// Aggregate health report covering every known subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    /// Per-subsystem health entries.
    pub subsystems: Vec<SubsystemHealth>,
    /// Geometric mean of all subsystem scores.
    pub overall_score: f64,
    /// Timestamp at which the diagnostic was collected.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Actionable recommendations based on the diagnostic.
    pub recommendations: Vec<String>,
}

/// Probes scheduler and session subsystems to produce health reports.
pub struct DiagnosticEngine {
    scheduler: Arc<TaskScheduler>,
    session_manager: Arc<SessionManager>,
}

impl DiagnosticEngine {
    /// Create a new diagnostic engine wrapping the given subsystems.
    pub fn new(scheduler: Arc<TaskScheduler>, session_manager: Arc<SessionManager>) -> Self {
        Self {
            scheduler,
            session_manager,
        }
    }

    /// Run a full diagnostic across all subsystems.
    ///
    /// Returns a [`SystemHealthReport`] whose `overall_score` is the
    /// geometric mean of the individual subsystem scores.
    pub fn run_full_diagnostic(&self) -> SystemHealthReport {
        let subsystems = vec![
            self.check_subsystem("scheduler"),
            self.check_subsystem("session_manager"),
        ];

        let overall_score = geometric_mean(subsystems.iter().map(|s| s.score));

        let mut recommendations = Vec::new();
        for sub in &subsystems {
            if !sub.healthy {
                recommendations.push(format!(
                    "Subsystem '{}' is unhealthy: {}",
                    sub.name, sub.details
                ));
            } else if sub.score < 0.8 {
                recommendations.push(format!(
                    "Subsystem '{}' score is below 0.8: {}",
                    sub.name, sub.details
                ));
            }
        }

        SystemHealthReport {
            subsystems,
            overall_score,
            timestamp: Utc::now(),
            recommendations,
        }
    }

    /// Check the health of a named subsystem.
    ///
    /// Supported names: `"scheduler"`, `"session_manager"`.
    /// Returns a default unhealthy entry for unknown names.
    pub fn check_subsystem(&self, name: &str) -> SubsystemHealth {
        match name {
            "scheduler" => {
                let stats = self.scheduler.stats();
                let total = stats.total_spawned;
                if total == 0 {
                    SubsystemHealth {
                        name: "scheduler".to_string(),
                        healthy: true,
                        score: 1.0,
                        details: "No tasks spawned yet; scheduler is idle and healthy.".to_string(),
                    }
                } else {
                    let success_rate = if total > 0 {
                        stats.completed as f64 / total as f64
                    } else {
                        1.0
                    };
                    let failure_impact = if stats.failed > 0 {
                        stats.failed as f64 / total as f64
                    } else {
                        0.0
                    };
                    let score = (success_rate * (1.0 - failure_impact * 0.5)).clamp(0.0, 1.0);
                    let healthy = score >= 0.5;
                    SubsystemHealth {
                        name: "scheduler".to_string(),
                        healthy,
                        score,
                        details: format!(
                            "spawned={} completed={} failed={} success_rate={:.2}",
                            stats.total_spawned,
                            stats.completed,
                            stats.failed,
                            success_rate
                        ),
                    }
                }
            }
            "session_manager" => {
                let total = self.session_manager.total_count();
                let active = self.session_manager.active_count();
                if total == 0 {
                    SubsystemHealth {
                        name: "session_manager".to_string(),
                        healthy: true,
                        score: 1.0,
                        details: "No sessions created yet; session manager is idle.".to_string(),
                    }
                } else {
                    let active_ratio = active as f64 / total as f64;
                    let score = active_ratio.clamp(0.0, 1.0);
                    let healthy = score >= 0.1 || active > 0;
                    SubsystemHealth {
                        name: "session_manager".to_string(),
                        healthy,
                        score,
                        details: format!("total={} active={} ratio={:.2}", total, active, active_ratio),
                    }
                }
            }
            _ => SubsystemHealth {
                name: name.to_string(),
                healthy: false,
                score: 0.0,
                details: format!("Unknown subsystem: '{}'", name),
            },
        }
    }

    /// Convenience shortcut: run a full diagnostic and return only the overall score.
    pub fn health_score(&self) -> f64 {
        self.run_full_diagnostic().overall_score
    }
}

/// Compute the geometric mean of an iterator of non-negative f64 values.
/// Returns 0.0 for an empty iterator.
fn geometric_mean(values: impl Iterator<Item = f64>) -> f64 {
    let mut count = 0u64;
    let mut log_sum = 0.0_f64;
    for v in values {
        if v <= 0.0 {
            return 0.0;
        }
        log_sum += v.ln();
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        (log_sum / count as f64).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionConfig;

    fn make_engine() -> DiagnosticEngine {
        let scheduler = Arc::new(TaskScheduler::new());
        let session_manager = Arc::new(SessionManager::new(SessionConfig::default()));
        DiagnosticEngine::new(scheduler, session_manager)
    }

    #[test]
    fn test_geometric_mean_all_ones() {
        let mean = geometric_mean([1.0, 1.0, 1.0].into_iter());
        assert!((mean - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_geometric_mean_mixed() {
        // sqrt(0.5 * 2.0) = 1.0
        let mean = geometric_mean([0.5, 2.0].into_iter());
        assert!((mean - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_geometric_mean_empty() {
        let mean = geometric_mean([].into_iter());
        assert!((mean - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_geometric_mean_zero() {
        let mean = geometric_mean([1.0, 0.0, 1.0].into_iter());
        assert!((mean - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_check_subsystem_scheduler_idle() {
        let engine = make_engine();
        let health = engine.check_subsystem("scheduler");
        assert!(health.healthy);
        assert!((health.score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_check_subsystem_session_idle() {
        let engine = make_engine();
        let health = engine.check_subsystem("session_manager");
        assert!(health.healthy);
        assert!((health.score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_check_subsystem_unknown() {
        let engine = make_engine();
        let health = engine.check_subsystem("nonexistent");
        assert!(!health.healthy);
        assert!((health.score - 0.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_run_full_diagnostic_idle() {
        let engine = make_engine();
        let report = engine.run_full_diagnostic();
        assert!((report.overall_score - 1.0).abs() < 1e-9);
        assert_eq!(report.subsystems.len(), 2);
        assert!(report.recommendations.is_empty());
    }
}
