//! OMEGA AGI - Adapters Module
//! 
//! Universal adapter layer for OMEGA AGI compatibility with multiple agent systems.
//! This module provides adapters for OpenClaw, Hermes, and OpenHuman protocols.

pub mod openclaw_adapter;
pub mod hermes_adapter;
pub mod openhuman_adapter;

// Re-export commonly used types
pub use openclaw_adapter::{
    OpenClawAdapter,
    OpenClawAdapterTrait,
    OpenClawMessage,
    OpenClawAgentMessage,
    OpenClawSkill,
    OpenClawSkillLoader,
    OpenClawCard,
    OpenClawCardElement,
    AdapterInfo,
};

pub use hermes_adapter::{
    HermesAdapter,
    HermesAdapterTrait,
    HermesMessage,
    HermesWorkflow,
    HermesWorkflowStep,
    HermesTask,
    HermesApiClient,
    HermesWorkflowEngine,
    HermesAdapterInfo,
};

pub use openhuman_adapter::{
    OpenHumanAdapter,
    OpenHumanAdapterTrait,
    OpenHumanMessage,
    OpenHumanAgent,
    OpenHumanWorkflow,
    OpenHumanWorkflowExecutor,
    OpenHumanApiClient,
    OpenHumanAdapterInfo,
};

use serde::{Deserialize, Serialize};

/// Unified adapter manager for all supported protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterManager {
    pub openclaw: Option<OpenClawAdapter>,
    pub hermes: Option<HermesAdapter>,
    pub openhuman: Option<OpenHumanAdapter>,
    pub active_adapter: String,
}

impl AdapterManager {
    /// Create new adapter manager with default configurations
    pub fn new() -> Self {
        Self {
            openclaw: Some(OpenClawAdapter::new()),
            hermes: Some(HermesAdapter::default()),
            openhuman: Some(OpenHumanAdapter::default()),
            active_adapter: "openclaw".to_string(),
        }
    }

    /// Get active adapter info
    pub fn get_active_info(&self) -> AdapterInfo {
        match self.active_adapter.as_str() {
            "hermes" => {
                AdapterInfo {
                    name: "Hermes Adapter".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    protocol_version: "1.0".to_string(),
                    capabilities: vec!["workflow_execution".to_string()],
                }
            }
            "openhuman" => {
                AdapterInfo {
                    name: "OpenHuman Adapter".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    protocol_version: "1.0".to_string(),
                    capabilities: vec!["agent_communication".to_string()],
                }
            }
            _ => {
                if let Some(ref o) = self.openclaw {
                    o.adapter_info()
                } else {
                    AdapterInfo::default()
                }
            }
        }
    }

    /// List all available adapters
    pub fn list_adapters(&self) -> Vec<String> {
        let mut adapters = Vec::new();
        if self.openclaw.is_some() {
            adapters.push("openclaw".to_string());
        }
        if self.hermes.is_some() {
            adapters.push("hermes".to_string());
        }
        if self.openhuman.is_some() {
            adapters.push("openhuman".to_string());
        }
        adapters
    }

    /// Set active adapter
    pub fn set_active(&mut self, adapter: &str) -> Result<(), String> {
        match adapter {
            "openclaw" | "hermes" | "openhuman" => {
                self.active_adapter = adapter.to_string();
                Ok(())
            }
            _ => Err(format!("Unknown adapter: {}", adapter)),
        }
    }
}

impl Default for AdapterManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Protocol version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
}

impl ProtocolVersion {
    pub fn new(name: &str, version: &str, features: Vec<&str>) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            features: features.into_iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::new("OMEGA AGI", "1.0.0", vec![
            "multi_protocol_support",
            "workflow_execution",
            "skill_loading",
            "message_transform",
        ])
    }
}

/// Compatibility matrix showing supported features per protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityMatrix {
    pub protocol: String,
    pub message_sending: bool,
    pub skill_loading: bool,
    pub workflow_execution: bool,
    pub agent_protocol: bool,
}

impl CompatibilityMatrix {
    pub fn openclaw() -> Self {
        Self {
            protocol: "OpenClaw".to_string(),
            message_sending: true,
            skill_loading: true,
            workflow_execution: false,
            agent_protocol: true,
        }
    }

    pub fn hermes() -> Self {
        Self {
            protocol: "Hermes".to_string(),
            message_sending: true,
            skill_loading: false,
            workflow_execution: true,
            agent_protocol: true,
        }
    }

    pub fn openhuman() -> Self {
        Self {
            protocol: "OpenHuman".to_string(),
            message_sending: true,
            skill_loading: false,
            workflow_execution: true,
            agent_protocol: true,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::openclaw(),
            Self::hermes(),
            Self::openhuman(),
        ]
    }
}

/// Unified error type for adapter operations
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("OpenClaw adapter error: {0}")]
    OpenClaw(String),
    
    #[error("Hermes adapter error: {0}")]
    Hermes(String),
    
    #[error("OpenHuman adapter error: {0}")]
    OpenHuman(String),
    
    #[error("Protocol mismatch: expected {expected}, got {actual}")]
    ProtocolMismatch { expected: String, actual: String },
    
    #[error("Adapter not available: {0}")]
    NotAvailable(String),
}

impl From<anyhow::Error> for AdapterError {
    fn from(e: anyhow::Error) -> Self {
        AdapterError::OpenClaw(e.to_string())
    }
}