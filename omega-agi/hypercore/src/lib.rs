//! # OMEGA HyperCore
//!
//! Zero-allocation async runtime with persistent memory and capability security.
//! Layer 0 of the OMEGA AGI system.

pub mod scheduler;
pub mod memory;
pub mod security;
pub mod session;
pub mod errors;
pub mod health;
pub mod logging;
pub mod pipeline;
pub mod diagnostics;
pub mod self_heal;

pub use scheduler::{TaskScheduler, TaskPriority, TaskId};
pub use memory::{MemoryPool, MemoryStats};
pub use security::{Capability, CapabilitySet, SecurityRing};
pub use session::SessionManager;
pub use errors::HyperCoreError;
pub use health::{HealthMonitor, HealthSnapshot};
pub use pipeline::{PipelineOrchestrator, PipelineResult, HealthCheck};
pub use diagnostics::{DiagnosticEngine, SystemHealthReport, SubsystemHealth};
pub use self_heal::{SelfHealingController, HealingAction, HealingResult, HealingEvent, Healer};

/// HyperCore version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
