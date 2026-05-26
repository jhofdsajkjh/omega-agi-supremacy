//! # ML Inference Engine
//!
//! A lightweight ML inference engine for running models within the OMEGA runtime.
//! Supports model registration, configuration, and simulated inference.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Handle to a registered model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ModelHandle(u64);

impl ModelHandle {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        ModelHandle(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for ModelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Model#{}", self.0)
    }
}

/// Configuration for an inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Temperature for sampling (0.0 = deterministic, higher = more random).
    pub temperature: f64,
    /// Maximum number of tokens to generate.
    pub max_tokens: u32,
    /// Top-p (nucleus) sampling parameter.
    pub top_p: f64,
    /// Whether to use greedy decoding.
    pub greedy: bool,
    /// Random seed for reproducibility.
    pub seed: Option<u64>,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 512,
            top_p: 0.9,
            greedy: false,
            seed: None,
        }
    }
}

impl InferenceConfig {
    /// Create a deterministic (greedy) configuration.
    pub fn deterministic() -> Self {
        Self {
            temperature: 0.0,
            greedy: true,
            top_p: 1.0,
            ..Default::default()
        }
    }

    /// Create a creative (high temperature) configuration.
    pub fn creative() -> Self {
        Self {
            temperature: 1.2,
            top_p: 0.95,
            ..Default::default()
        }
    }
}

/// The result of an inference request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    /// The generated output text.
    pub output: String,
    /// Number of tokens generated.
    pub tokens_generated: u32,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Wall-clock inference time in microseconds.
    pub duration_us: u64,
    /// Model handle that produced this result.
    pub model: ModelHandle,
    /// Whether the inference was truncated due to max_tokens.
    pub truncated: bool,
}

/// Metadata about a registered model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model handle.
    pub handle: ModelHandle,
    /// Model name.
    pub name: String,
    /// Model version string.
    pub version: String,
    /// Parameter count (in millions).
    pub parameter_count_millions: u64,
    /// Model type (e.g., "llm", "embedding", "classification").
    pub model_type: String,
    /// Number of times this model has been used for inference.
    pub inference_count: u64,
    /// Total inference time in microseconds.
    pub total_inference_us: u64,
}

/// Statistics for the inference engine.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InferenceStats {
    pub models_loaded: usize,
    pub total_inferences: u64,
    pub total_tokens_generated: u64,
    pub total_inference_time_us: u64,
    pub total_errors: u64,
}

/// The ML inference engine manages models and executes inference requests.
pub struct InferenceEngine {
    models: Arc<RwLock<HashMap<ModelHandle, ModelInfo>>>,
    stats: Arc<RwLock<InferenceStats>>,
}

impl InferenceEngine {
    /// Create a new inference engine.
    pub fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(InferenceStats::default())),
        }
    }

    /// Register a model.
    pub fn register_model(
        &self,
        name: impl Into<String>,
        version: impl Into<String>,
        parameter_count_millions: u64,
        model_type: impl Into<String>,
    ) -> ModelHandle {
        let handle = ModelHandle::new();
        let info = ModelInfo {
            handle,
            name: name.into(),
            version: version.into(),
            parameter_count_millions,
            model_type: model_type.into(),
            inference_count: 0,
            total_inference_us: 0,
        };

        let count = {
            let mut models = self.models.write();
            models.insert(handle, info);
            models.len()
        };

        {
            let mut stats = self.stats.write();
            stats.models_loaded = count;
        }

        info!(
            model = %handle,
            name = %self.models.read().get(&handle).unwrap().name,
            "Model registered"
        );
        handle
    }

    /// Run inference on a model.
    ///
    /// This is a simulated implementation that produces deterministic output
    /// based on the input and configuration.
    pub fn infer(
        &self,
        model: ModelHandle,
        input: &str,
        config: &InferenceConfig,
    ) -> Result<InferenceResult> {
        let start = Instant::now();

        // Check model exists
        {
            let models = self.models.read();
            if !models.contains_key(&model) {
                return Err(anyhow::anyhow!("Model {} not found", model));
            }
        }

        // Simulate inference: generate output based on input
        let output = self.simulate_inference(input, config);

        let duration_us = start.elapsed().as_micros() as u64;
        let tokens_generated = output.split_whitespace().count() as u32;
        let truncated = tokens_generated >= config.max_tokens;

        // Compute a simulated confidence score
        let confidence = if config.greedy {
            0.95
        } else {
            (0.7 + 0.25 * (1.0 - config.temperature / 2.0)).max(0.1)
        };

        let result = InferenceResult {
            output,
            tokens_generated,
            confidence,
            duration_us,
            model,
            truncated,
        };

        // Update stats
        {
            let mut models = self.models.write();
            if let Some(info) = models.get_mut(&model) {
                info.inference_count += 1;
                info.total_inference_us += duration_us;
            }
        }
        {
            let mut stats = self.stats.write();
            stats.total_inferences += 1;
            stats.total_tokens_generated += tokens_generated as u64;
            stats.total_inference_time_us += duration_us;
        }

        debug!(
            model = %model,
            tokens = tokens_generated,
            duration_us,
            confidence,
            "Inference completed"
        );

        Ok(result)
    }

    /// Simulate inference by transforming the input.
    fn simulate_inference(&self, input: &str, config: &InferenceConfig) -> String {
        // Simple simulation: echo input with some transformation
        let words: Vec<&str> = input.split_whitespace().collect();
        let max_words = config.max_tokens as usize;

        if words.is_empty() {
            return String::new();
        }

        // Simulate generating a response based on input
        let response_words: Vec<String> = words
            .iter()
            .take(max_words)
            .enumerate()
            .map(|(i, word)| {
                if config.greedy {
                    format!("{}[{}]", word, i)
                } else {
                    // Add some variation based on temperature
                    let variation = if config.temperature > 0.5 { "_var" } else { "" };
                    format!("{}{}[{}]", word, variation, i)
                }
            })
            .collect();

        response_words.join(" ")
    }

    /// Get information about a registered model.
    pub fn get_model_info(&self, handle: ModelHandle) -> Option<ModelInfo> {
        self.models.read().get(&handle).cloned()
    }

    /// Unregister a model.
    pub fn unregister_model(&self, handle: ModelHandle) -> bool {
        let removed = self.models.write().remove(&handle).is_some();
        if removed {
            let mut stats = self.stats.write();
            stats.models_loaded = self.models.read().len();
            info!(model = %handle, "Model unregistered");
        }
        removed
    }

    /// Get engine statistics.
    pub fn stats(&self) -> InferenceStats {
        self.stats.read().clone()
    }

    /// List all registered model handles.
    pub fn list_models(&self) -> Vec<ModelHandle> {
        self.models.read().keys().copied().collect()
    }
}

impl Default for InferenceEngine {
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
    fn test_model_handle_unique() {
        let h1 = ModelHandle::new();
        let h2 = ModelHandle::new();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_model_handle_display() {
        let handle = ModelHandle::new();
        let display = format!("{}", handle);
        assert!(display.starts_with("Model#"));
    }

    #[test]
    fn test_inference_config_default() {
        let config = InferenceConfig::default();
        assert!((config.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(config.max_tokens, 512);
        assert!((config.top_p - 0.9).abs() < f64::EPSILON);
        assert!(!config.greedy);
        assert!(config.seed.is_none());
    }

    #[test]
    fn test_inference_config_deterministic() {
        let config = InferenceConfig::deterministic();
        assert!(config.greedy);
        assert!((config.temperature - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_inference_config_creative() {
        let config = InferenceConfig::creative();
        assert!((config.temperature - 1.2).abs() < f64::EPSILON);
        assert!((config.top_p - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_register_model() {
        let engine = InferenceEngine::new();
        let handle = engine.register_model("gpt-omega", "1.0", 7000, "llm");

        let info = engine.get_model_info(handle).unwrap();
        assert_eq!(info.name, "gpt-omega");
        assert_eq!(info.version, "1.0");
        assert_eq!(info.parameter_count_millions, 7000);
        assert_eq!(info.model_type, "llm");
        assert_eq!(info.inference_count, 0);
    }

    #[test]
    fn test_unregister_model() {
        let engine = InferenceEngine::new();
        let handle = engine.register_model("temp", "1.0", 100, "test");
        assert!(engine.unregister_model(handle));
        assert!(engine.get_model_info(handle).is_none());
    }

    #[test]
    fn test_infer_basic() {
        let engine = InferenceEngine::new();
        let handle = engine.register_model("test-model", "1.0", 100, "llm");

        let result = engine
            .infer(handle, "hello world", &InferenceConfig::default())
            .unwrap();

        assert!(!result.output.is_empty());
        assert!(result.tokens_generated > 0);
        assert!(result.confidence > 0.0);
        assert!(result.duration_us > 0);
        assert_eq!(result.model, handle);
        assert!(!result.truncated);
    }

    #[test]
    fn test_infer_deterministic() {
        let engine = InferenceEngine::new();
        let handle = engine.register_model("det", "1.0", 100, "llm");

        let config = InferenceConfig::deterministic();
        let result = engine.infer(handle, "test input", &config).unwrap();

        assert!(result.confidence > 0.9);
        // Deterministic output should be reproducible
        let result2 = engine.infer(handle, "test input", &config).unwrap();
        assert_eq!(result.output, result2.output);
    }

    #[test]
    fn test_infer_nonexistent_model() {
        let engine = InferenceEngine::new();
        let fake_handle = ModelHandle::new();
        let result = engine.infer(fake_handle, "test", &InferenceConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_infer_empty_input() {
        let engine = InferenceEngine::new();
        let handle = engine.register_model("empty", "1.0", 100, "llm");

        let result = engine
            .infer(handle, "", &InferenceConfig::default())
            .unwrap();
        assert!(result.output.is_empty());
        assert_eq!(result.tokens_generated, 0);
    }

    #[test]
    fn test_infer_truncation() {
        let engine = InferenceEngine::new();
        let handle = engine.register_model("trunc", "1.0", 100, "llm");

        let config = InferenceConfig {
            max_tokens: 2,
            ..Default::default()
        };
        let result = engine
            .infer(handle, "one two three four five", &config)
            .unwrap();
        assert!(result.truncated);
    }

    #[test]
    fn test_engine_stats() {
        let engine = InferenceEngine::new();
        let h1 = engine.register_model("m1", "1.0", 100, "llm");
        let h2 = engine.register_model("m2", "1.0", 200, "embedding");

        engine.infer(h1, "hello", &InferenceConfig::default()).unwrap();
        engine.infer(h1, "world", &InferenceConfig::default()).unwrap();
        engine.infer(h2, "test", &InferenceConfig::default()).unwrap();

        let stats = engine.stats();
        assert_eq!(stats.models_loaded, 2);
        assert_eq!(stats.total_inferences, 3);
        assert!(stats.total_tokens_generated > 0);
        assert!(stats.total_inference_time_us > 0);
        assert_eq!(stats.total_errors, 0);
    }

    #[test]
    fn test_list_models() {
        let engine = InferenceEngine::new();
        let h1 = engine.register_model("a", "1.0", 100, "llm");
        let h2 = engine.register_model("b", "1.0", 200, "llm");

        let mut models = engine.list_models();
        models.sort();
        assert_eq!(models, vec![h1, h2]);
    }

    #[test]
    fn test_model_info_updated_after_inference() {
        let engine = InferenceEngine::new();
        let handle = engine.register_model("counter", "1.0", 100, "llm");

        engine.infer(handle, "test", &InferenceConfig::default()).unwrap();
        engine.infer(handle, "test2", &InferenceConfig::default()).unwrap();

        let info = engine.get_model_info(handle).unwrap();
        assert_eq!(info.inference_count, 2);
        assert!(info.total_inference_us > 0);
    }
}
