//! Swarm智能体群协调层 (Layer 2)
//! 
//! 超越特性:
//! - 多Agent同时协作编码 (类似Google Docs)
//! - 自动任务分解与分配
//! - Raft共识机制解决冲突
//! - 实时状态同步
//!
//! 对比优势:
//! - OpenHuman: 单Agent架构
//! - Hermes-Agent: 单Agent架构  
//! - OMEGA: 多Agent Swarm协调

pub mod coordinator;
pub mod consensus;
pub mod crdt;
pub mod task_router;
pub mod health_monitor;

pub use coordinator::{SwarmCoordinator, AgentHandle, SwarmTask};
pub use consensus::{ConsensusEngine, Proposal, Vote};
pub use crdt::{CrdtDoc, TextChange, CollaborativeText};
pub use task_router::{TaskRouter, RoutingStrategy};
pub use health_monitor::{HealthMonitor, AgentHealth};

use std::time::{SystemTime, UNIX_EPOCH};

/// 生成唯一ID
pub fn generate_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("swarm_{}", timestamp)
}

/// Swarm错误类型
#[derive(Debug, Clone)]
pub enum SwarmError {
    AgentNotFound(String),
    TaskNotFound(String),
    ConsensusFailed(String),
    RoutingFailed(String),
    HealthCheckFailed(String),
    CrdtConflict(String),
    NetworkError(String),
}

impl std::fmt::Display for SwarmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwarmError::AgentNotFound(id) => write!(f, "Agent not found: {}", id),
            SwarmError::TaskNotFound(id) => write!(f, "Task not found: {}", id),
            SwarmError::ConsensusFailed(msg) => write!(f, "Consensus failed: {}", msg),
            SwarmError::RoutingFailed(msg) => write!(f, "Routing failed: {}", msg),
            SwarmError::HealthCheckFailed(msg) => write!(f, "Health check failed: {}", msg),
            SwarmError::CrdtConflict(msg) => write!(f, "CRDT conflict: {}", msg),
            SwarmError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for SwarmError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
        assert!(id1.starts_with("swarm_"));
    }

    #[test]
    fn test_swarm_error_display() {
        let err = SwarmError::AgentNotFound("agent_123".to_string());
        assert_eq!(err.to_string(), "Agent not found: agent_123");
    }
}
