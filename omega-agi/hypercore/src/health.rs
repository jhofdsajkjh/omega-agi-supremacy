//! # Health Monitor
//!
//! Samples subsystem health across scheduler, memory, sessions, and security.
//! Produces `HealthSnapshot` structs with utilization metrics and alerts.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use tracing::{debug, info};

use crate::memory::MemoryPool;
use crate::scheduler::TaskScheduler;

/// A point-in-time snapshot of subsystem health.
#[derive(Debug, Clone)]
pub struct HealthSnapshot {
    /// Tasks completed per second (estimated from scheduler stats).
    pub scheduler_throughput: f64,
    /// Memory utilization ratio (0.0 = empty, 1.0 = full).
    pub memory_utilization: f64,
    /// Number of currently active sessions.
    pub active_sessions: usize,
    /// Number of active security alerts.
    pub security_alerts: usize,
    /// Timestamp when this snapshot was taken.
    pub timestamp: DateTime<Utc>,
    /// Human-readable alert messages, if any.
    pub alerts: Vec<String>,
}

impl Default for HealthSnapshot {
    fn default() -> Self {
        Self {
            scheduler_throughput: 0.0,
            memory_utilization: 0.0,
            active_sessions: 0,
            security_alerts: 0,
            timestamp: Utc::now(),
            alerts: Vec::new(),
        }
    }
}

/// Monitors the health of HyperCore subsystems.
pub struct HealthMonitor {
    scheduler: Arc<TaskScheduler>,
    /// Cached security alert count (updated externally or via snapshot).
    security_alerts: usize,
}

impl HealthMonitor {
    /// Create a new health monitor referencing the given scheduler.
    pub fn new(scheduler: Arc<TaskScheduler>) -> Self {
        info!("HealthMonitor initialized");
        Self {
            scheduler,
            security_alerts: 0,
        }
    }

    /// Take a health snapshot from the scheduler's current statistics.
    pub fn snapshot(&self) -> HealthSnapshot {
        let stats = self.scheduler.stats();

        // Estimate throughput: completed tasks / a normalised window.
        // We use avg_latency_us to derive an approximate tasks/sec figure.
        let throughput = if stats.avg_latency_us > 0 {
            1_000_000.0 / stats.avg_latency_us as f64
        } else {
            0.0
        };

        let mut alerts = Vec::new();

        if stats.failed > 0 {
            alerts.push(format!("scheduler has {} failed tasks", stats.failed));
        }

        if stats.running > 1000 {
            alerts.push(format!(
                "scheduler running {} tasks (high concurrency)",
                stats.running
            ));
        }

        debug!(
            throughput,
            failed = stats.failed,
            running = stats.running,
            "Health snapshot taken"
        );

        HealthSnapshot {
            scheduler_throughput: throughput,
            memory_utilization: 0.0, // not available without a memory pool reference
            active_sessions: 0,
            security_alerts: self.security_alerts,
            timestamp: Utc::now(),
            alerts,
        }
    }

    /// Returns true if the system is healthy: no alerts and utilization < 0.95.
    pub fn is_healthy(&self) -> bool {
        let snap = self.snapshot();
        snap.alerts.is_empty() && snap.memory_utilization < 0.95
    }

    /// Take a health snapshot that incorporates memory pool statistics.
    pub fn memory_stats(&self, pool: &MemoryPool) -> HealthSnapshot {
        let mut snap = self.snapshot();
        let mem = pool.stats();

        snap.memory_utilization = mem.utilization;
        snap.active_sessions = 0; // sessions tracked separately

        if mem.utilization > 0.9 {
            snap.alerts.push(format!(
                "memory utilization high: {:.1}%",
                mem.utilization * 100.0
            ));
        }

        if mem.utilization > 0.95 {
            snap.alerts.push(format!(
                "memory critically low: {:.1}% utilized",
                mem.utilization * 100.0
            ));
        }

        debug!(
            utilization = mem.utilization,
            used = mem.used_bytes,
            capacity = mem.capacity_bytes,
            "Memory health snapshot taken"
        );

        snap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a TaskScheduler wrapped in Arc for tests.
    fn make_scheduler() -> Arc<TaskScheduler> {
        Arc::new(TaskScheduler::new())
    }

    #[test]
    fn test_new_health_monitor() {
        let scheduler = make_scheduler();
        let monitor = HealthMonitor::new(scheduler);
        assert_eq!(monitor.security_alerts, 0);
    }

    #[test]
    fn test_snapshot_default_values() {
        let scheduler = make_scheduler();
        let monitor = HealthMonitor::new(scheduler);
        let snap = monitor.snapshot();

        // Fresh scheduler has no completed tasks, so throughput should be 0.0
        assert_eq!(snap.scheduler_throughput, 0.0);
        assert_eq!(snap.memory_utilization, 0.0);
        assert_eq!(snap.active_sessions, 0);
        assert_eq!(snap.security_alerts, 0);
        assert!(snap.alerts.is_empty());
    }

    #[test]
    fn test_snapshot_has_timestamp() {
        let scheduler = make_scheduler();
        let monitor = HealthMonitor::new(scheduler);
        let snap = monitor.snapshot();

        // Timestamp should be close to now (within 1 second).
        let now = Utc::now();
        let diff = (now - snap.timestamp).num_seconds().abs();
        assert!(diff <= 1, "snapshot timestamp should be close to now");
    }

    #[test]
    fn test_is_healthy_fresh() {
        let scheduler = make_scheduler();
        let monitor = HealthMonitor::new(scheduler);
        assert!(monitor.is_healthy());
    }

    #[test]
    fn test_health_snapshot_default() {
        let snap = HealthSnapshot::default();
        assert_eq!(snap.scheduler_throughput, 0.0);
        assert_eq!(snap.memory_utilization, 0.0);
        assert_eq!(snap.active_sessions, 0);
        assert!(snap.alerts.is_empty());
    }

    #[test]
    fn test_memory_stats_incorporates_pool() {
        let scheduler = make_scheduler();
        let monitor = HealthMonitor::new(scheduler);

        // Create a real memory pool via tempfile
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let mut pool = MemoryPool::open(&path, 4096).unwrap();
        pool.write(b"test data").unwrap();

        let snap = monitor.memory_stats(&pool);
        assert!(snap.memory_utilization > 0.0);
    }

    #[test]
    fn test_memory_stats_high_utilization_alert() {
        let scheduler = make_scheduler();
        let monitor = HealthMonitor::new(scheduler);

        // Create a tiny pool and fill it to trigger alerts
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let mut pool = MemoryPool::open(&path, 64).unwrap();
        pool.write(&[0u8; 60]).unwrap();

        let snap = monitor.memory_stats(&pool);
        // Utilization should be > 0.9, triggering at least one alert
        assert!(
            snap.memory_utilization > 0.9,
            "expected high utilization, got {:.4}",
            snap.memory_utilization
        );
        assert!(!snap.alerts.is_empty(), "expected alerts for high utilization");
    }
}
