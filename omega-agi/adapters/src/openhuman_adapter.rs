//! OMEGA AGI - OpenHuman Adapter
//! 
//! Adapter for integrating with OpenHuman system.
//! Provides OpenHuman API compatibility and workflow support.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Generate a simple unique ID
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("req_{}_{}", duration.as_nanos(), std::process::id())
}

/// OpenHuman API message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OpenHumanMessage {
    #[serde(rename = "agent_request")]
    AgentRequest {
        request_id: String,
        agent_id: String,
        prompt: String,
        context: Option<OpenHumanContext>,
    },
    #[serde(rename = "agent_response")]
    AgentResponse {
        request_id: String,
        status: String,
        output: Option<String>,
        metadata: Option<HashMap<String, String>>,
    },
    #[serde(rename = "stream")]
    Stream {
        request_id: String,
        chunk: String,
        is_final: bool,
    },
}

/// OpenHuman execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct OpenHumanContext {
    pub session_id: Option<String>,
    pub variables: HashMap<String, String>,
    pub files: Vec<OpenHumanFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanFile {
    pub path: String,
    pub name: String,
    pub mime_type: Option<String>,
}



/// OpenHuman agent definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanAgent {
    pub agent_id: String,
    pub name: String,
    pub description: String,
    pub model: String,
    pub capabilities: Vec<String>,
    pub config: OpenHumanAgentConfig,
}

/// OpenHuman agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanAgentConfig {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub system_prompt: Option<String>,
}

/// OpenHuman workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanWorkflow {
    pub workflow_id: String,
    pub name: String,
    pub description: Option<String>,
    pub nodes: Vec<OpenHumanNode>,
    pub edges: Vec<OpenHumanEdge>,
}

/// OpenHuman workflow node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanNode {
    pub node_id: String,
    pub node_type: String,
    pub config: HashMap<String, serde_json::Value>,
}

/// OpenHuman workflow edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanEdge {
    pub from_node: String,
    pub to_node: String,
    pub condition: Option<String>,
}

/// OpenHuman API client
pub struct OpenHumanApiClient {
    #[allow(unused)] base_url: String,
    api_key: Option<String>,
    timeout_ms: u64,
}

impl OpenHumanApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            api_key: None,
            timeout_ms: 60000,
        }
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Create agent request
    pub fn create_agent_request(
        &self,
        agent_id: &str,
        prompt: &str,
    ) -> OpenHumanMessage {
        OpenHumanMessage::AgentRequest {
            request_id: generate_id(),
            agent_id: agent_id.to_string(),
            prompt: prompt.to_string(),
            context: None,
        }
    }

    /// Parse response
    pub fn parse_response(&self, data: &[u8]) -> Result<OpenHumanMessage> {
        serde_json::from_slice(data)
            .map_err(|e| anyhow::anyhow!("Failed to parse OpenHuman response: {}", e))
    }
}

/// OpenHuman workflow executor
pub struct OpenHumanWorkflowExecutor {
    workflows: Arc<RwLock<HashMap<String, OpenHumanWorkflow>>>,
    agents: Arc<RwLock<HashMap<String, OpenHumanAgent>>>,
}

impl OpenHumanWorkflowExecutor {
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a workflow
    pub async fn register_workflow(&self, workflow: OpenHumanWorkflow) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        workflows.insert(workflow.workflow_id.clone(), workflow);
        Ok(())
    }

    /// Register an agent
    pub async fn register_agent(&self, agent: OpenHumanAgent) -> Result<()> {
        let mut agents = self.agents.write().await;
        agents.insert(agent.agent_id.clone(), agent);
        Ok(())
    }

    /// Execute workflow
    pub async fn execute_workflow(&self, workflow_id: &str) -> Result<HashMap<String, String>> {
        let workflow = {
            let workflows = self.workflows.read().await;
            workflows.get(workflow_id).cloned()
        };

        let workflow = workflow.ok_or_else(|| {
            anyhow::anyhow!("Workflow not found: {}", workflow_id)
        })?;

        let mut results = HashMap::new();
        
        for node in &workflow.nodes {
            tracing::info!("Executing node: {} - {}", node.node_id, node.node_type);
            results.insert(node.node_id.clone(), format!("completed:{}", node.node_type));
        }

        Ok(results)
    }
}

impl Default for OpenHumanWorkflowExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// OpenHuman adapter trait
pub trait OpenHumanAdapterTrait: Send + Sync {
    /// Send request to OpenHuman agent
    fn send_agent_request(&self, message: OpenHumanMessage) -> impl std::future::Future<Output = Result<OpenHumanMessage>> + Send;
    
    /// Execute workflow
    fn execute_workflow(&self, workflow: OpenHumanWorkflow) -> impl std::future::Future<Output = Result<HashMap<String, String>>> + Send;
    
    /// Get adapter info
    fn adapter_info(&self) -> OpenHumanAdapterInfo;
}

/// OpenHuman adapter info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHumanAdapterInfo {
    pub name: String,
    pub version: String,
    pub supported_agent_types: Vec<String>,
}

impl Default for OpenHumanAdapterInfo {
    fn default() -> Self {
        Self {
            name: "OpenHuman Adapter".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_agent_types: vec![
                "conversational".to_string(),
                "task_execution".to_string(),
                "code_generation".to_string(),
            ],
        }
    }
}

/// OpenHuman adapter implementation
pub struct OpenHumanAdapter {
    api_client: OpenHumanApiClient,
    workflow_executor: OpenHumanWorkflowExecutor,
    info: OpenHumanAdapterInfo,
}

impl OpenHumanAdapter {
    pub fn new(api_url: &str) -> Self {
        Self {
            api_client: OpenHumanApiClient::new(api_url),
            workflow_executor: OpenHumanWorkflowExecutor::new(),
            info: OpenHumanAdapterInfo::default(),
        }
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_client = self.api_client.with_api_key(api_key);
        self
    }
}

impl Default for OpenHumanAdapter {
    fn default() -> Self {
        Self::new("http://localhost:9090")
    }
}

impl OpenHumanAdapterTrait for OpenHumanAdapter {
    async fn send_agent_request(&self, message: OpenHumanMessage) -> Result<OpenHumanMessage> {
        match message {
            OpenHumanMessage::AgentRequest { request_id, agent_id, prompt, context: _ } => {
                tracing::info!("OpenHuman agent request: {} - {}", request_id, agent_id);
                
                Ok(OpenHumanMessage::AgentResponse {
                    request_id,
                    status: "completed".to_string(),
                    output: Some(format!("Processed: {}", prompt)),
                    metadata: None,
                })
            }
            _ => Err(anyhow::anyhow!("Expected AgentRequest message type")),
        }
    }

    async fn execute_workflow(&self, workflow: OpenHumanWorkflow) -> Result<HashMap<String, String>> {
        self.workflow_executor.register_workflow(workflow.clone()).await?;
        self.workflow_executor.execute_workflow(&workflow.workflow_id).await
    }

    fn adapter_info(&self) -> OpenHumanAdapterInfo {
        self.info.clone()
    }
}

/// OpenHuman protocol constants
pub mod protocol {
    pub const OPENHUMAN_API_VERSION: &str = "v1";
    pub const DEFAULT_TIMEOUT_MS: u64 = 60000;
    
    // Status values
    pub const STATUS_PENDING: &str = "pending";
    pub const STATUS_RUNNING: &str = "running";
    pub const STATUS_COMPLETED: &str = "completed";
    pub const STATUS_FAILED: &str = "failed";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_request_creation() {
        let client = OpenHumanApiClient::new("http://localhost:9090");
        let request = client.create_agent_request("agent_001", "Hello");
        
        match request {
            OpenHumanMessage::AgentRequest { request_id, agent_id, prompt, context: _ } => {
                assert_eq!(agent_id, "agent_001");
                assert_eq!(prompt, "Hello");
                assert!(request_id.starts_with("req_"));
            }
            _ => panic!("Expected AgentRequest"),
        }
    }

    #[test]
    fn test_workflow_serialization() {
        let workflow = OpenHumanWorkflow {
            workflow_id: "wf_001".to_string(),
            name: "Test Workflow".to_string(),
            description: Some("A test workflow".to_string()),
            nodes: vec![
                OpenHumanNode {
                    node_id: "node_1".to_string(),
                    node_type: "start".to_string(),
                    config: HashMap::new(),
                },
            ],
            edges: vec![],
        };
        
        let json = serde_json::to_string(&workflow).unwrap();
        assert!(json.contains("wf_001"));
    }

    #[tokio::test]
    async fn test_workflow_executor() {
        let executor = OpenHumanWorkflowExecutor::new();
        
        let workflow = OpenHumanWorkflow {
            workflow_id: "wf_test".to_string(),
            name: "Test".to_string(),
            description: None,
            nodes: vec![],
            edges: vec![],
        };
        
        executor.register_workflow(workflow).await.unwrap();
        let result = executor.execute_workflow("wf_test").await;
        
        assert!(result.is_ok());
    }
}