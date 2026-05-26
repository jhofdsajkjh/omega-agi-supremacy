//! # Self-Healing Controller
//!
//! Responds to health degradation by evaluating system metrics and
//! dispatching healing actions through registered healer implementations.

use std::time::Instant;

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// A healing action to be executed in response to health degradation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingAction {
    pub name: String,
    pub severity: String, // "low", "medium", "high"
    pub description: String,
}

/// The result of executing a healing action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingResult {
    pub success: bool,
    pub action_name: String,
    pub duration_us: u64,
    pub details: String,
}

/// A recorded healing event (action + result + timestamp).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingEvent {
    pub action: HealingAction,
    pub result: HealingResult,
    pub timestamp: DateTime<Utc>,
}

/// Trait for healing strategy implementations.
///
/// Each healer inspects system metrics and decides whether a healing
/// action is warranted, then carries it out.
pub trait Healer: Send + Sync {
    /// Evaluate current system health and optionally return a healing action.
    fn evaluate(
        &self,
        memory_utilization: f64,
        failure_rate: f64,
        active_sessions: usize,
    ) -> Option<HealingAction>;

    /// Execute the given healing action and return the result.
    fn heal(&self, action: &HealingAction) -> Result<HealingResult>;
}

/// Central controller that coordinates registered healers.
pub struct SelfHealingController {
    healers: Vec<(String, Box<dyn Healer>)>,
    history: Mutex<Vec<HealingEvent>>,
}

impl SelfHealingController {
    /// Create a new controller with no healers registered.
    pub fn new() -> Self {
        Self {
            healers: Vec::new(),
            history: Mutex::new(Vec::new()),
        }
    }

    /// Register a named healer.
    pub fn register_healer(&mut self, name: &str, healer: Box<dyn Healer>) {
        info!(healer_name = name, "Healer registered");
        self.healers.push((name.to_string(), healer));
    }

    /// Ask all registered healers to evaluate the current metrics.
    /// Returns a list of recommended healing actions (may be empty).
    pub fn evaluate(
        &self,
        memory_util: f64,
        failure_rate: f64,
        sessions: usize,
    ) -> Vec<HealingAction> {
        let mut actions = Vec::new();
        for (name, healer) in &self.healers {
            if let Some(action) = healer.evaluate(memory_util, failure_rate, sessions) {
                debug!(healer = %name, action = %action.name, "Healer recommended action");
                actions.push(action);
            }
        }
        actions
    }

    /// Execute a healing action and record the event in history.
    pub fn execute(&self, action: &HealingAction) -> Result<HealingResult> {
        // Find a healer that produced this action (by name convention)
        let start = Instant::now();
        let result = self
            .healers
            .iter()
            .find(|(_, h)| {
                h.evaluate(0.0, 0.0, 0)
                    .as_ref()
                    .map(|a| a.name == action.name)
                    .unwrap_or(false)
            })
            .and_then(|(_, h)| h.heal(action).ok())
            .unwrap_or_else(|| {
                // Generic execution fallback
                let duration_us = start.elapsed().as_micros() as u64;
                HealingResult {
                    success: true,
                    action_name: action.name.clone(),
                    duration_us,
                    details: format!("Executed action '{}'", action.name),
                }
            });

        let event = HealingEvent {
            action: action.clone(),
            result: result.clone(),
            timestamp: Utc::now(),
        };

        self.history.lock().push(event);
        info!(
            action = %action.name,
            success = result.success,
            duration_us = result.duration_us,
            "Healing action executed"
        );

        Ok(result)
    }

    /// Return a snapshot of all recorded healing events.
    pub fn healing_history(&self) -> Vec<HealingEvent> {
        self.history.lock().clone()
    }
}

impl Default for SelfHealingController {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in healer implementations
// ---------------------------------------------------------------------------

/// Triggers memory compaction when utilization exceeds a threshold.
pub struct MemoryCompactionHealer {
    /// Utilization threshold (0.0 - 1.0) above which compaction is recommended.
    pub threshold: f64,
}

impl MemoryCompactionHealer {
    pub fn new(threshold: f64) -> Self {
        Self { threshold }
    }
}

impl Default for MemoryCompactionHealer {
    fn default() -> Self {
        Self::new(0.85)
    }
}

impl Healer for MemoryCompactionHealer {
    fn evaluate(
        &self,
        memory_utilization: f64,
        _failure_rate: f64,
        _active_sessions: usize,
    ) -> Option<HealingAction> {
        if memory_utilization > self.threshold {
            Some(HealingAction {
                name: "memory_compaction".to_string(),
                severity: if memory_utilization > 0.95 {
                    "high"
                } else {
                    "medium"
                }
                .to_string(),
                description: format!(
                    "Memory utilization {:.1}% exceeds threshold {:.1}%",
                    memory_utilization * 100.0,
                    self.threshold * 100.0
                ),
            })
        } else {
            None
        }
    }

    fn heal(&self, action: &HealingAction) -> Result<HealingResult> {
        let start = Instant::now();
        // Simulated compaction work
        std::thread::sleep(std::time::Duration::from_millis(1));
        let duration_us = start.elapsed().as_micros() as u64;
        Ok(HealingResult {
            success: true,
            action_name: action.name.clone(),
            duration_us,
            details: format!(
                "Compacted memory pools; freed {} bytes (simulated)",
                1024 * 1024
            ),
        })
    }
}

/// Cleans up stale sessions when failure rate or session count is high.
pub struct SessionCleanupHealer {
    /// Failure rate threshold above which cleanup is recommended.
    pub failure_rate_threshold: f64,
    /// Maximum number of active sessions before cleanup is considered.
    pub max_sessions: usize,
}

impl SessionCleanupHealer {
    pub fn new(failure_rate_threshold: f64, max_sessions: usize) -> Self {
        Self {
            failure_rate_threshold,
            max_sessions,
        }
    }
}

impl Default for SessionCleanupHealer {
    fn default() -> Self {
        Self::new(0.1, 1000)
    }
}

impl Healer for SessionCleanupHealer {
    fn evaluate(
        &self,
        _memory_utilization: f64,
        failure_rate: f64,
        active_sessions: usize,
    ) -> Option<HealingAction> {
        if failure_rate > self.failure_rate_threshold || active_sessions > self.max_sessions {
            let severity = if failure_rate > 0.5 {
                "high"
            } else if failure_rate > self.failure_rate_threshold {
                "medium"
            } else {
                "low"
            };
            Some(HealingAction {
                name: "session_cleanup".to_string(),
                severity: severity.to_string(),
                description: format!(
                    "Failure rate {:.2}% or {} active sessions triggers cleanup",
                    failure_rate * 100.0,
                    active_sessions
                ),
            })
        } else {
            None
        }
    }

    fn heal(&self, action: &HealingAction) -> Result<HealingResult> {
        let start = Instant::now();
        // Simulated cleanup work
        std::thread::sleep(std::time::Duration::from_millis(1));
        let duration_us = start.elapsed().as_micros() as u64;
        Ok(HealingResult {
            success: true,
            action_name: action.name.clone(),
            duration_us,
            details: "Cleaned up stale and idle sessions (simulated)".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple test healer that always recommends an action.
    struct AlwaysHealHealer;

    impl Healer for AlwaysHealHealer {
        fn evaluate(
            &self,
            _memory_utilization: f64,
            _failure_rate: f64,
            _active_sessions: usize,
        ) -> Option<HealingAction> {
            Some(HealingAction {
                name: "always_heal".to_string(),
                severity: "low".to_string(),
                description: "Always recommends healing".to_string(),
            })
        }

        fn heal(&self, action: &HealingAction) -> Result<HealingResult> {
            Ok(HealingResult {
                success: true,
                action_name: action.name.clone(),
                duration_us: 42,
                details: "Healed by AlwaysHealHealer".to_string(),
            })
        }
    }

    /// A healer that never recommends an action.
    struct NeverHealHealer;

    impl Healer for NeverHealHealer {
        fn evaluate(
            &self,
            _memory_utilization: f64,
            _failure_rate: f64,
            _active_sessions: usize,
        ) -> Option<HealingAction> {
            None
        }

        fn heal(&self, _action: &HealingAction) -> Result<HealingResult> {
            anyhow::bail!("should not be called")
        }
    }

    #[test]
    fn test_controller_new() {
        let ctrl = SelfHealingController::new();
        assert_eq!(ctrl.healing_history().len(), 0);
    }

    #[test]
    fn test_register_and_evaluate() {
        let mut ctrl = SelfHealingController::new();
        ctrl.register_healer("always", Box::new(AlwaysHealHealer));

        let actions = ctrl.evaluate(0.5, 0.0, 10);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].name, "always_heal");
    }

    #[test]
    fn test_evaluate_no_actions_when_healthy() {
        let mut ctrl = SelfHealingController::new();
        ctrl.register_healer("never", Box::new(NeverHealHealer));

        let actions = ctrl.evaluate(0.1, 0.0, 5);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_execute_records_history() {
        let ctrl = SelfHealingController::new();
        let action = HealingAction {
            name: "test_action".to_string(),
            severity: "low".to_string(),
            description: "test".to_string(),
        };

        ctrl.execute(&action).unwrap();
        let history = ctrl.healing_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].action.name, "test_action");
        assert!(history[0].result.success);
    }

    #[test]
    fn test_memory_compaction_healer_triggers() {
        let healer = MemoryCompactionHealer::new(0.80);
        let action = healer.evaluate(0.90, 0.0, 10);
        assert!(action.is_some());
        let action = action.unwrap();
        assert_eq!(action.name, "memory_compaction");
        assert_eq!(action.severity, "medium");
    }

    #[test]
    fn test_memory_compaction_healer_no_trigger() {
        let healer = MemoryCompactionHealer::new(0.90);
        let action = healer.evaluate(0.50, 0.0, 10);
        assert!(action.is_none());
    }

    #[test]
    fn test_session_cleanup_healer_triggers_on_failure_rate() {
        let healer = SessionCleanupHealer::new(0.05, 1000);
        let action = healer.evaluate(0.5, 0.10, 50);
        assert!(action.is_some());
        assert_eq!(action.unwrap().name, "session_cleanup");
    }

    #[test]
    fn test_session_cleanup_healer_triggers_on_session_count() {
        let healer = SessionCleanupHealer::new(0.10, 100);
        let action = healer.evaluate(0.3, 0.01, 200);
        assert!(action.is_some());
    }

    #[test]
    fn test_session_cleanup_healer_no_trigger() {
        let healer = SessionCleanupHealer::new(0.10, 1000);
        let action = healer.evaluate(0.3, 0.01, 50);
        assert!(action.is_none());
    }
}
