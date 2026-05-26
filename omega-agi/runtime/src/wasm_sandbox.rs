//! # WASM Sandbox
//!
//! A lightweight WASM sandbox for executing WebAssembly modules in isolation.
//! Provides resource limits, memory isolation, and module management.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Errors specific to the WASM sandbox.
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    /// The module could not be compiled.
    #[error("compilation failed: {0}")]
    CompilationFailed(String),

    /// Instantiation failed.
    #[error("instantiation failed: {0}")]
    InstantiationFailed(String),

    /// A function was not found in the module.
    #[error("function not found: {0}")]
    FunctionNotFound(String),

    /// Execution timed out.
    #[error("execution timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Memory limit exceeded.
    #[error("memory limit exceeded: used {used} bytes, limit {limit} bytes")]
    MemoryLimitExceeded { used: usize, limit: usize },

    /// Invalid input provided to the module.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// Module not loaded.
    #[error("module not found: {0}")]
    ModuleNotFound(String),
}

/// Configuration for the WASM sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmSandboxConfig {
    /// Maximum execution time per call in milliseconds.
    pub max_execution_time_ms: u64,
    /// Maximum memory in bytes per module instance.
    pub max_memory_bytes: usize,
    /// Whether to allow module imports.
    pub allow_imports: bool,
    /// Maximum number of concurrent instances.
    pub max_instances: usize,
}

impl Default for WasmSandboxConfig {
    fn default() -> Self {
        Self {
            max_execution_time_ms: 5000,
            max_memory_bytes: 64 * 1024 * 1024, // 64 MB
            allow_imports: false,
            max_instances: 100,
        }
    }
}

/// A compiled WASM module.
#[derive(Debug, Clone)]
pub struct WasmModule {
    /// Module name/identifier.
    pub name: String,
    /// The raw WASM bytecode.
    pub bytecode: Vec<u8>,
    /// Size of the bytecode in bytes.
    pub bytecode_size: usize,
    /// Hash of the bytecode for integrity checking.
    pub hash: String,
    /// List of exported function names.
    pub exports: Vec<String>,
    /// Timestamp when the module was registered.
    pub registered_at: chrono::DateTime<chrono::Utc>,
    /// Number of times this module has been instantiated.
    pub instantiation_count: u64,
}

impl WasmModule {
    /// Create a new WASM module from bytecode.
    pub fn new(name: impl Into<String>, bytecode: Vec<u8>) -> Result<Self> {
        if bytecode.is_empty() {
            anyhow::bail!("Bytecode cannot be empty");
        }

        let hash = format!("{:016x}", simple_hash(&bytecode));

        // Parse exports from bytecode (simplified: look for known patterns)
        let exports = Self::parse_exports(&bytecode);

        Ok(Self {
            name: name.into(),
            bytecode_size: bytecode.len(),
            hash,
            exports,
            registered_at: chrono::Utc::now(),
            instantiation_count: 0,
            bytecode,
        })
    }

    /// Parse exported function names from WASM bytecode.
    /// This is a simplified parser that looks for export section entries.
    fn parse_exports(bytecode: &[u8]) -> Vec<String> {
        let mut exports = Vec::new();

        // Simple heuristic: look for UTF-8 strings that could be function names
        // in the export section of the WASM binary
        let mut i = 0;
        while i < bytecode.len().saturating_sub(2) {
            // Look for length-prefixed strings (common in WASM sections)
            let len = bytecode[i] as usize;
            if len > 0 && len < 100 && i + 1 + len <= bytecode.len() {
                let candidate = &bytecode[i + 1..i + 1 + len];
                if let Ok(s) = std::str::from_utf8(candidate) {
                    // Filter for plausible function names
                    if s.chars().all(|c| c.is_alphanumeric() || c == '_') && s.len() >= 2 {
                        exports.push(s.to_string());
                    }
                }
            }
            i += 1;
        }

        // Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        exports.retain(|e| seen.insert(e.clone()));

        exports
    }

    /// Increment the instantiation count.
    pub fn record_instantiation(&mut self) {
        self.instantiation_count += 1;
    }
}

/// Simple hash function for bytecode integrity.
fn simple_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Statistics about a sandbox instance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandboxStats {
    pub modules_loaded: usize,
    pub total_instantiations: u64,
    pub total_executions: u64,
    pub total_errors: u64,
    pub total_execution_time_us: u64,
}

/// The WASM sandbox manages module loading, instantiation, and execution.
pub struct WasmSandbox {
    config: WasmSandboxConfig,
    modules: Arc<RwLock<HashMap<String, WasmModule>>>,
    stats: Arc<RwLock<SandboxStats>>,
}

impl WasmSandbox {
    /// Create a new WASM sandbox with the given configuration.
    pub fn new(config: WasmSandboxConfig) -> Self {
        Self {
            config,
            modules: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(SandboxStats::default())),
        }
    }

    /// Create a sandbox with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(WasmSandboxConfig::default())
    }

    /// Register a WASM module.
    pub fn register_module(&self, module: WasmModule) -> Result<()> {
        let name = module.name.clone();
        let bytecode_size = module.bytecode_size;

        if bytecode_size > self.config.max_memory_bytes {
            return Err(WasmError::MemoryLimitExceeded {
                used: bytecode_size,
                limit: self.config.max_memory_bytes,
            }
            .into());
        }

        let count = {
            let mut modules = self.modules.write();
            modules.insert(name.clone(), module);
            modules.len()
        };

        {
            let mut stats = self.stats.write();
            stats.modules_loaded = count;
        }

        info!(module_name = %name, bytecode_size, "WASM module registered");
        Ok(())
    }

    /// Get a registered module by name.
    pub fn get_module(&self, name: &str) -> Option<WasmModule> {
        self.modules.read().get(name).cloned()
    }

    /// Check if a module is registered.
    pub fn has_module(&self, name: &str) -> bool {
        self.modules.read().contains_key(name)
    }

    /// Simulate executing a function in a WASM module.
    /// In a real implementation, this would invoke the WASM runtime.
    pub fn execute(
        &self,
        module_name: &str,
        function_name: &str,
        input: &[u8],
    ) -> Result<Vec<u8>> {
        let start = Instant::now();

        // Check module exists
        let module = self
            .modules
            .read()
            .get(module_name)
            .cloned()
            .ok_or_else(|| WasmError::ModuleNotFound(module_name.to_string()))?;

        // Check function exists
        if !module.exports.contains(&function_name.to_string()) {
            return Err(WasmError::FunctionNotFound(function_name.to_string()).into());
        }

        // Check timeout
        let timeout = Duration::from_millis(self.config.max_execution_time_ms);

        // Simulated execution: echo the input back with a prefix
        let execution_time = start.elapsed();
        if execution_time > timeout {
            {
                let mut stats = self.stats.write();
                stats.total_errors += 1;
            }
            return Err(WasmError::Timeout {
                timeout_ms: self.config.max_execution_time_ms,
            }
            .into());
        }

        // Simulate processing
        let mut output = Vec::new();
        output.extend_from_slice(b"wasm_output:");
        output.extend_from_slice(input);

        let duration_us = start.elapsed().as_micros() as u64;

        {
            let mut stats = self.stats.write();
            stats.total_executions += 1;
            stats.total_execution_time_us += duration_us;
        }

        debug!(
            module = %module_name,
            function = %function_name,
            duration_us,
            "WASM function executed"
        );

        Ok(output)
    }

    /// Unregister a module.
    pub fn unregister_module(&self, name: &str) -> bool {
        let removed = self.modules.write().remove(name).is_some();
        if removed {
            let mut stats = self.stats.write();
            stats.modules_loaded = self.modules.read().len();
            info!(module_name = %name, "WASM module unregistered");
        }
        removed
    }

    /// Get sandbox statistics.
    pub fn stats(&self) -> SandboxStats {
        self.stats.read().clone()
    }

    /// Get the number of registered modules.
    pub fn module_count(&self) -> usize {
        self.modules.read().len()
    }

    /// List all registered module names.
    pub fn list_modules(&self) -> Vec<String> {
        self.modules.read().keys().cloned().collect()
    }
}

impl Default for WasmSandbox {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_bytecode() -> Vec<u8> {
        // Minimal valid-looking WASM bytecode with some export-like strings
        let mut code = vec![
            0x00, 0x61, 0x73, 0x6d, // WASM magic
            0x01, 0x00, 0x00, 0x00, // version
        ];
        // Add some export-like name strings
        let func_name = b"compute";
        code.push(func_name.len() as u8);
        code.extend_from_slice(func_name);
        let another = b"process";
        code.push(another.len() as u8);
        code.extend_from_slice(another);
        code
    }

    #[test]
    fn test_wasm_module_creation() {
        let module = WasmModule::new("test_module", make_test_bytecode()).unwrap();
        assert_eq!(module.name, "test_module");
        assert!(!module.bytecode.is_empty());
        assert!(!module.hash.is_empty());
    }

    #[test]
    fn test_wasm_module_empty_bytecode_fails() {
        let result = WasmModule::new("empty", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_wasm_module_instantiation_count() {
        let mut module = WasmModule::new("test", make_test_bytecode()).unwrap();
        assert_eq!(module.instantiation_count, 0);
        module.record_instantiation();
        assert_eq!(module.instantiation_count, 1);
        module.record_instantiation();
        assert_eq!(module.instantiation_count, 2);
    }

    #[test]
    fn test_wasm_module_hash_deterministic() {
        let code = make_test_bytecode();
        let m1 = WasmModule::new("a", code.clone()).unwrap();
        let m2 = WasmModule::new("b", code).unwrap();
        assert_eq!(m1.hash, m2.hash);
    }

    #[test]
    fn test_wasm_module_exports() {
        let module = WasmModule::new("test", make_test_bytecode()).unwrap();
        // The bytecode contains "compute" and "process" strings
        assert!(module.exports.contains(&"compute".to_string()));
        assert!(module.exports.contains(&"process".to_string()));
    }

    #[test]
    fn test_sandbox_register_module() {
        let sandbox = WasmSandbox::with_defaults();
        let module = WasmModule::new("math", make_test_bytecode()).unwrap();
        sandbox.register_module(module).unwrap();

        assert_eq!(sandbox.module_count(), 1);
        assert!(sandbox.has_module("math"));
    }

    #[test]
    fn test_sandbox_register_oversized_module() {
        let sandbox = WasmSandbox::new(WasmSandboxConfig {
            max_memory_bytes: 10,
            ..Default::default()
        });
        let module = WasmModule::new("big", make_test_bytecode()).unwrap();
        let result = sandbox.register_module(module);
        assert!(result.is_err());
        let binding = result.unwrap_err();
        let err = binding.downcast_ref::<WasmError>().unwrap();
        assert!(matches!(err, WasmError::MemoryLimitExceeded { .. }));
    }

    #[test]
    fn test_sandbox_unregister_module() {
        let sandbox = WasmSandbox::with_defaults();
        let module = WasmModule::new("temp", make_test_bytecode()).unwrap();
        sandbox.register_module(module).unwrap();

        assert!(sandbox.unregister_module("temp"));
        assert!(!sandbox.has_module("temp"));
        assert_eq!(sandbox.module_count(), 0);
    }

    #[test]
    fn test_sandbox_execute_function() {
        let sandbox = WasmSandbox::with_defaults();
        let module = WasmModule::new("calc", make_test_bytecode()).unwrap();
        sandbox.register_module(module).unwrap();

        let result = sandbox.execute("calc", "compute", b"input_data").unwrap();
        assert!(result.starts_with(b"wasm_output:"));
        assert!(result.ends_with(b"input_data"));
    }

    #[test]
    fn test_sandbox_execute_nonexistent_module() {
        let sandbox = WasmSandbox::with_defaults();
        let result = sandbox.execute("missing", "func", b"");
        assert!(result.is_err());
        let binding = result.unwrap_err();
        let err = binding.downcast_ref::<WasmError>().unwrap();
        assert!(matches!(err, WasmError::ModuleNotFound(_)));
    }

    #[test]
    fn test_sandbox_execute_nonexistent_function() {
        let sandbox = WasmSandbox::with_defaults();
        let module = WasmModule::new("mod", make_test_bytecode()).unwrap();
        sandbox.register_module(module).unwrap();

        let result = sandbox.execute("mod", "nonexistent_func", b"");
        assert!(result.is_err());
        let binding = result.unwrap_err();
        let err = binding.downcast_ref::<WasmError>().unwrap();
        assert!(matches!(err, WasmError::FunctionNotFound(_)));
    }

    #[test]
    fn test_sandbox_stats() {
        let sandbox = WasmSandbox::with_defaults();
        let module = WasmModule::new("stats_mod", make_test_bytecode()).unwrap();
        sandbox.register_module(module).unwrap();

        sandbox.execute("stats_mod", "compute", b"data").unwrap();
        sandbox.execute("stats_mod", "process", b"data").unwrap();

        let stats = sandbox.stats();
        assert_eq!(stats.modules_loaded, 1);
        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.total_errors, 0);
    }

    #[test]
    fn test_sandbox_list_modules() {
        let sandbox = WasmSandbox::with_defaults();
        sandbox
            .register_module(WasmModule::new("a", make_test_bytecode()).unwrap())
            .unwrap();
        sandbox
            .register_module(WasmModule::new("b", make_test_bytecode()).unwrap())
            .unwrap();

        let mut modules = sandbox.list_modules();
        modules.sort();
        assert_eq!(modules, vec!["a", "b"]);
    }

    #[test]
    fn test_sandbox_config_default() {
        let config = WasmSandboxConfig::default();
        assert_eq!(config.max_execution_time_ms, 5000);
        assert_eq!(config.max_memory_bytes, 64 * 1024 * 1024);
        assert!(!config.allow_imports);
        assert_eq!(config.max_instances, 100);
    }

    #[test]
    fn test_wasm_error_display() {
        let err = WasmError::FunctionNotFound("my_func".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("my_func"));
        assert!(msg.contains("function not found"));

        let err = WasmError::Timeout { timeout_ms: 1000 };
        let msg = format!("{}", err);
        assert!(msg.contains("1000"));
        assert!(msg.contains("timeout"));
    }
}
