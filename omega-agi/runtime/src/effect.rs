//! # Effect System
//!
//! A side-effect tracking and management system. Effects represent observable
//! side-effects (I/O, state changes, external calls) that can be recorded,
//! replayed, and composed.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Unique effect identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EffectId(u64);

impl EffectId {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        EffectId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for EffectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Effect#{}", self.0)
    }
}

/// The result of executing an effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectResult {
    /// The effect that produced this result.
    pub effect_id: EffectId,
    /// Whether the effect succeeded.
    pub success: bool,
    /// Result data (JSON-serialized).
    pub data: Vec<u8>,
    /// Error message if the effect failed.
    pub error: Option<String>,
    /// Duration of the effect execution in microseconds.
    pub duration_us: u64,
    /// Timestamp when the result was produced.
    pub timestamp: DateTime<Utc>,
}

impl EffectResult {
    /// Create a successful effect result.
    pub fn ok(effect_id: EffectId, data: Vec<u8>, duration_us: u64) -> Self {
        Self {
            effect_id,
            success: true,
            data,
            error: None,
            duration_us,
            timestamp: Utc::now(),
        }
    }

    /// Create a failed effect result.
    pub fn err(effect_id: EffectId, error: impl Into<String>, duration_us: u64) -> Self {
        Self {
            effect_id,
            success: false,
            data: Vec::new(),
            error: Some(error.into()),
            duration_us,
            timestamp: Utc::now(),
        }
    }

    /// Deserialize the result data into a typed value.
    pub fn decode<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.data)
            .context("Failed to deserialize effect result data")
    }
}

/// An effect represents a side-effect that can be recorded and executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    /// Unique identifier.
    pub id: EffectId,
    /// Human-readable name of the effect.
    pub name: String,
    /// Effect type (e.g., "io_read", "state_write", "external_call").
    pub effect_type: String,
    /// Input parameters (JSON-serialized).
    pub input: Vec<u8>,
    /// Timestamp when the effect was created.
    pub timestamp: DateTime<Utc>,
    /// Whether this effect has been committed.
    pub committed: bool,
}

impl Effect {
    /// Create a new effect.
    pub fn new(name: impl Into<String>, effect_type: impl Into<String>, input: Vec<u8>) -> Self {
        Self {
            id: EffectId::new(),
            name: name.into(),
            effect_type: effect_type.into(),
            input,
            timestamp: Utc::now(),
            committed: false,
        }
    }

    /// Create an effect with a typed input.
    pub fn with_input<T: Serialize>(
        name: impl Into<String>,
        effect_type: impl Into<String>,
        input: &T,
    ) -> Result<Self> {
        let data = serde_json::to_vec(input).context("Failed to serialize effect input")?;
        Ok(Self::new(name, effect_type, data))
    }

    /// Deserialize the input into a typed value.
    pub fn decode_input<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.input)
            .context("Failed to deserialize effect input")
    }

    /// Mark this effect as committed.
    pub fn commit(&mut self) {
        self.committed = true;
    }
}

/// Context provided during effect execution.
pub struct EffectContext {
    /// The current effect being executed.
    pub effect: Effect,
    /// Metadata attached to the context.
    pub metadata: HashMap<String, String>,
}

impl EffectContext {
    /// Create a new effect context.
    pub fn new(effect: Effect) -> Self {
        Self {
            effect,
            metadata: HashMap::new(),
        }
    }

    /// Set a metadata key-value pair.
    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get a metadata value.
    pub fn get_meta(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Statistics about the effect system.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EffectSystemStats {
    pub total_effects: u64,
    pub committed_effects: u64,
    pub failed_effects: u64,
    pub total_duration_us: u64,
}

/// The effect system manages effect recording, execution, and history.
pub struct EffectSystem {
    effects: Arc<RwLock<HashMap<EffectId, Effect>>>,
    results: Arc<RwLock<HashMap<EffectId, EffectResult>>>,
    stats: Arc<RwLock<EffectSystemStats>>,
}

impl EffectSystem {
    /// Create a new effect system.
    pub fn new() -> Self {
        Self {
            effects: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(EffectSystemStats::default())),
        }
    }

    /// Record a new effect.
    pub fn record(&self, effect: Effect) -> EffectId {
        let id = effect.id;
        {
            let mut effects = self.effects.write();
            effects.insert(id, effect);
        }
        {
            let mut stats = self.stats.write();
            stats.total_effects += 1;
        }
        debug!(effect_id = %id, "Effect recorded");
        id
    }

    /// Record a result for an effect.
    pub fn record_result(&self, result: EffectResult) {
        let effect_id = result.effect_id;
        {
            let mut results = self.results.write();
            results.insert(effect_id, result.clone());
        }
        {
            let mut stats = self.stats.write();
            stats.total_duration_us += result.duration_us;
            if result.success {
                stats.committed_effects += 1;
            } else {
                stats.failed_effects += 1;
            }
        }
        debug!(effect_id = %effect_id, success = result.success, "Effect result recorded");
    }

    /// Execute a synchronous effect handler and record the result.
    pub fn execute<F>(&self, effect: Effect, handler: F) -> EffectResult
    where
        F: FnOnce(&EffectContext) -> Result<Vec<u8>>,
    {
        let id = effect.id;
        self.record(effect.clone());

        let ctx = EffectContext::new(effect);
        let start = std::time::Instant::now();

        match handler(&ctx) {
            Ok(data) => {
                let duration_us = start.elapsed().as_micros() as u64;
                let result = EffectResult::ok(id, data, duration_us);
                self.record_result(result.clone());
                result
            }
            Err(e) => {
                let duration_us = start.elapsed().as_micros() as u64;
                let result = EffectResult::err(id, e.to_string(), duration_us);
                self.record_result(result.clone());
                result
            }
        }
    }

    /// Get an effect by ID.
    pub fn get_effect(&self, id: EffectId) -> Option<Effect> {
        self.effects.read().get(&id).cloned()
    }

    /// Get a result by effect ID.
    pub fn get_result(&self, id: EffectId) -> Option<EffectResult> {
        self.results.read().get(&id).cloned()
    }

    /// Get current statistics.
    pub fn stats(&self) -> EffectSystemStats {
        self.stats.read().clone()
    }

    /// Get the total number of recorded effects.
    pub fn len(&self) -> usize {
        self.effects.read().len()
    }

    /// Check if no effects have been recorded.
    pub fn is_empty(&self) -> bool {
        self.effects.read().is_empty()
    }

    /// Clear all recorded effects and results.
    pub fn clear(&self) {
        self.effects.write().clear();
        self.results.write().clear();
        info!("Effect system cleared");
    }
}

impl Default for EffectSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_id_unique() {
        let id1 = EffectId::new();
        let id2 = EffectId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_effect_id_display() {
        let id = EffectId::new();
        let display = format!("{}", id);
        assert!(display.starts_with("Effect#"));
    }

    #[test]
    fn test_effect_creation() {
        let effect = Effect::new("read_file", "io_read", b"/path/to/file".to_vec());
        assert_eq!(effect.name, "read_file");
        assert_eq!(effect.effect_type, "io_read");
        assert!(!effect.committed);
    }

    #[test]
    fn test_effect_with_input() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Params {
            path: String,
            offset: u64,
        }
        let params = Params {
            path: "/data".to_string(),
            offset: 42,
        };
        let effect = Effect::with_input("read", "io", &params).unwrap();
        let decoded: Params = effect.decode_input().unwrap();
        assert_eq!(decoded, params);
    }

    #[test]
    fn test_effect_commit() {
        let mut effect = Effect::new("write", "io_write", b"data".to_vec());
        assert!(!effect.committed);
        effect.commit();
        assert!(effect.committed);
    }

    #[test]
    fn test_effect_result_ok() {
        let id = EffectId::new();
        let result = EffectResult::ok(id, b"output".to_vec(), 100);
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.duration_us, 100);
    }

    #[test]
    fn test_effect_result_err() {
        let id = EffectId::new();
        let result = EffectResult::err(id, "something failed", 50);
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("something failed"));
    }

    #[test]
    fn test_effect_result_decode() {
        let id = EffectId::new();
        let data = serde_json::to_vec(&vec!["a", "b"]).unwrap();
        let result = EffectResult::ok(id, data, 10);
        let decoded: Vec<String> = result.decode().unwrap();
        assert_eq!(decoded, vec!["a", "b"]);
    }

    #[test]
    fn test_effect_system_record() {
        let system = EffectSystem::new();
        let effect = Effect::new("test", "test_type", b"".to_vec());
        let id = effect.id;
        system.record(effect);

        assert_eq!(system.len(), 1);
        let retrieved = system.get_effect(id).unwrap();
        assert_eq!(retrieved.name, "test");
    }

    #[test]
    fn test_effect_system_execute_success() {
        let system = EffectSystem::new();
        let effect = Effect::new("double", "compute", serde_json::to_vec(&42).unwrap());

        let result = system.execute(effect, |ctx| {
            let input: i32 = ctx.effect.decode_input()?;
            Ok(serde_json::to_vec(&(input * 2))?)
        });

        assert!(result.success);
        let output: i32 = result.decode().unwrap();
        assert_eq!(output, 84);
    }

    #[test]
    fn test_effect_system_execute_failure() {
        let system = EffectSystem::new();
        let effect = Effect::new("fail", "test", b"".to_vec());

        let result = system.execute(effect, |_ctx| {
            anyhow::bail!("intentional failure")
        });

        assert!(!result.success);
        assert!(result.error.unwrap().contains("intentional failure"));
    }

    #[test]
    fn test_effect_system_stats() {
        let system = EffectSystem::new();

        let e1 = Effect::new("ok1", "test", b"".to_vec());
        system.execute(e1, |_ctx| Ok(b"ok".to_vec()));

        let e2 = Effect::new("fail1", "test", b"".to_vec());
        system.execute(e2, |_ctx| anyhow::bail!("fail"));

        let stats = system.stats();
        assert_eq!(stats.total_effects, 2);
        assert_eq!(stats.committed_effects, 1);
        assert_eq!(stats.failed_effects, 1);
    }

    #[test]
    fn test_effect_system_clear() {
        let system = EffectSystem::new();
        system.record(Effect::new("a", "t", b"".to_vec()));
        system.record(Effect::new("b", "t", b"".to_vec()));
        assert_eq!(system.len(), 2);

        system.clear();
        assert!(system.is_empty());
    }

    #[test]
    fn test_effect_context_metadata() {
        let effect = Effect::new("test", "test", b"".to_vec());
        let mut ctx = EffectContext::new(effect);
        ctx.set_meta("key1", "value1");
        ctx.set_meta("key2", "value2");

        assert_eq!(ctx.get_meta("key1"), Some(&"value1".to_string()));
        assert_eq!(ctx.get_meta("key2"), Some(&"value2".to_string()));
        assert_eq!(ctx.get_meta("nonexistent"), None);
    }
}
