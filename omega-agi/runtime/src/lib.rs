//! # OMEGA Runtime
//!
//! Layer 1 execution engine for the OMEGA AGI system.
//! Provides actor system, effect system, WASM sandbox, ML inference, and graph execution.
//!
//! Built on top of `omega-hypercore` for scheduling, memory, security, and session management.

pub mod actor;
pub mod effect;
pub mod wasm_sandbox;
pub mod ml_inference;
pub mod graph_executor;

pub use actor::{Actor, ActorId, ActorRef, ActorSystem, Message};
pub use effect::{Effect, EffectContext, EffectId, EffectResult, EffectSystem};
pub use wasm_sandbox::{WasmError, WasmModule, WasmSandbox, WasmSandboxConfig};
pub use ml_inference::{InferenceConfig, InferenceEngine, InferenceResult, ModelHandle};
pub use graph_executor::{GraphExecutor, GraphExecutorError, NodeId, NodeResult, TaskGraph};

/// Runtime version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
