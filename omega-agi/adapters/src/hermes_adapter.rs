//! OMEGA AGI - Hermes Adapter
//! 
//! Adapter for integrating with Hermes-Agent system.
//! Provides compatibility with Hermes message format, workflow execution,
//! and API adaptation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Generate a simple unique ID
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{}_{}", duration.as_nanos(), std::process::id())
}

/// Hermes message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HermesMessage {
    #[serde(rename = "request")]
    Request {
        id: String,
        action: String,
        params: HermesParams,
        context: Option<HermesContext>,
    },
    #[serde(rename = "response")]
    Response {
        id: String,
        status: HermesStatus,
        result: Option<serde_json::Value>,
        error: Option<HermesError>,
    },
    #[serde(rename = "event")]
    Event {
        event_type: String,
        data: serde_json::Value,
        timestamp: String,
    },
}

/// Hermes request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesParams {
    #[serde(flatten)]
    pub data: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for HermesParams {
    fn default() -> Self {
        HermesParams { data: std::collections::HashMap::new() }
    }
}

/// Hermes execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesContext {
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub workspace: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl Default for HermesContext {
    fn default() -> Self {
        Self {
            session_id: None,
            user_id: None,
            workspace: None,
            metadata: HashMap::new(),
        }
    }
}

/// Hermes status codes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesStatus {
    pub code: u32,
    pub message: String,
    pub details: Option<HashMap<String, String>>,
}

/// Hermes error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesError {
    pub code: String,
    pub message: String,
    pub stack: Option<String>,
}

/// Hermes workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesWorkflow {
    pub id: String,
    pub name: String,
    pub steps: Vec<HermesWorkflowStep>,
    pub metadata: HermesWorkflowMetadata,
}

/// Hermes workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesWorkflowStep {
    pub id: String,
    pub action: String,
    pub params: HermesParams,
    pub retry: Option<HermesRetryConfig>,
    pub timeout_ms: Option<u64>,
}

/// Hermes retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesRetryConfig {
    pub max_attempts: u32,
    pub backoff_ms: u64,
    pub retry_on: Vec<String>,
}

/// Hermes workflow metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesWorkflowMetadata {
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

impl Default for HermesWorkflowMetadata {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            author: None,
            description: None,
            tags: Vec::new(),
        }
    }
}

/// Hermes task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesTask {
    pub task_id: String,
    pub workflow_id: String,
    pub status: HermesTaskStatus,
    pub result: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// Hermes task status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HermesTaskStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// Hermes workflow engine
pub struct HermesWorkflowEngine {
    workflows: Arc<RwLock<HashMap<String, HermesWorkflow>>>,
    tasks: Arc<RwLock<HashMap<String, HermesTask>>>,
}

impl HermesWorkflowEngine {
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a workflow
    pub async fn register_workflow(&self, workflow: HermesWorkflow) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        workflows.insert(workflow.id.clone(), workflow);
        Ok(())
    }

    /// Get workflow by ID
    pub async fn get_workflow(&self, id: &str) -> Option<HermesWorkflow> {
        let workflows = self.workflows.read().await;
        workflows.get(id).cloned()
    }

    /// Create a new task from workflow
    pub async fn create_task(&self, workflow_id: &str) -> Result<HermesTask> {
        let workflow = self.get_workflow(workflow_id).await
            .ok_or_else(|| anyhow::anyhow!("Workflow not found: {}", workflow_id))?;
        
        let now = chrono::Utc::now().to_rfc3339();
        let task = HermesTask {
            task_id: generate_id(),
            workflow_id: workflow_id.to_string(),
            status: HermesTaskStatus::Pending,
            result: None,
            created_at: now.clone(),
            updated_at: now,
        };
        
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.task_id.clone(), task.clone());
        
        Ok(task)
    }

    /// Execute workflow step
    pub async fn execute_step(&self, task: &mut HermesTask, step: &HermesWorkflowStep) -> Result<()> {
        task.status = HermesTaskStatus::Running;
        task.updated_at = chrono::Utc::now().to_rfc3339();
        
        // Simulate step execution
        tracing::info!("Executing step: {} - {}", step.id, step.action);
        
        // In real implementation, execute the action
        task.result = Some(serde_json::json!({
            "step_id": step.id,
            "status": "completed"
        }));
        task.status = HermesTaskStatus::Completed;
        task.updated_at = chrono::Utc::now().to_rfc3339();
        
        Ok(())
    }
}

impl Default for HermesWorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Hermes API client
pub struct HermesApiClient {
    base_url: String,
    api_key: Option<String>,
}

impl HermesApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            api_key: None,
        }
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }

    /// Build API request
    pub fn build_request(&self, action: &str, params: HermesParams) -> Result<HermesMessage> {
        Ok(HermesMessage::Request {
            id: generate_id(),
            action: action.to_string(),
            params,
            context: None,
        })
    }

    /// Parse API response
    pub fn parse_response(&self, data: &[u8]) -> Result<HermesMessage> {
        serde_json::from_slice(data)
            .map_err(|e| anyhow::anyhow!("Failed to parse Hermes response: {}", e))
    }
}

/// Hermes adapter trait
pub trait HermesAdapterTrait: Send + Sync {
    /// Send request to Hermes
    fn send_request(&self, message: HermesMessage) -> impl std::future::Future<Output = Result<HermesMessage>> + Send;
    
    /// Execute workflow
    fn execute_workflow(&self, workflow: HermesWorkflow) -> impl std::future::Future<Output = Result<HermesTask>> + Send;
    
    /// Get adapter info
    fn adapter_info(&self) -> HermesAdapterInfo;
}

/// Hermes adapter info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesAdapterInfo {
    pub name: String,
    pub version: String,
    pub supported_actions: Vec<String>,
}

impl Default for HermesAdapterInfo {
    fn default() -> Self {
        Self {
            name: "Hermes Adapter".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_actions: vec![
                "execute".to_string(),
                "query".to_string(),
                "workflow_run".to_string(),
            ],
        }
    }
}

/// Hermes adapter implementation
pub struct HermesAdapter {
    api_client: HermesApiClient,
    workflow_engine: HermesWorkflowEngine,
    info: HermesAdapterInfo,
}

impl HermesAdapter {
    pub fn new(api_url: &str) -> Self {
        Self {
            api_client: HermesApiClient::new(api_url),
            workflow_engine: HermesWorkflowEngine::new(),
            info: HermesAdapterInfo::default(),
        }
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_client = self.api_client.with_api_key(api_key);
        self
    }
}

impl Default for HermesAdapter {
    fn default() -> Self {
        Self::new("http://localhost:8080")
    }
}

impl HermesAdapterTrait for HermesAdapter {
    async fn send_request(&self, message: HermesMessage) -> Result<HermesMessage> {
        match message {
            HermesMessage::Request { id, action, params, context: _ } => {
                tracing::info!("Hermes request: {} - {}", id, action);
                
                // Build response
                Ok(HermesMessage::Response {
                    id,
                    status: HermesStatus {
                        code: 200,
                        message: "OK".to_string(),
                        details: None,
                    },
                    result: Some(serde_json::json!({ "action": action, "params": params })),
                    error: None,
                })
            }
            _ => Err(anyhow::anyhow!("Expected Request message type")),
        }
    }

    async fn execute_workflow(&self, workflow: HermesWorkflow) -> Result<HermesTask> {
        // Register workflow
        self.workflow_engine.register_workflow(workflow.clone()).await?;
        
        // Create and execute task
        let mut task = self.workflow_engine.create_task(&workflow.id).await?;
        
        for step in &workflow.steps {
            self.workflow_engine.execute_step(&mut task, step).await?;
        }
        
        Ok(task)
    }

    fn adapter_info(&self) -> HermesAdapterInfo {
        self.info.clone()
    }
}

/// Hermes protocol constants
pub mod protocol {
    pub const HERMES_API_VERSION: &str = "v1";
    pub const DEFAULT_TIMEOUT_MS: u64 = 30000;
    pub const MAX_RETRY_ATTEMPTS: u32 = 3;
    
    // Status codes
    pub const STATUS_OK: u32 = 200;
    pub const STATUS_BAD_REQUEST: u32 = 400;
    pub const STATUS_NOT_FOUND: u32 = 404;
    pub const STATUS_SERVER_ERROR: u32 = 500;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hermes_message_serialization() {
        let msg = HermesMessage::Request {
            id: "req_123".to_string(),
            action: "test".to_string(),
            params: HermesParams::default(),
            context: None,
        };
        
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("req_123"));
    }

    #[test]
    fn test_workflow_creation() {
        let workflow = HermesWorkflow {
            id: "wf_001".to_string(),
            name: "Test Workflow".to_string(),
            steps: vec![
                HermesWorkflowStep {
                    id: "step_1".to_string(),
                    action: "init".to_string(),
                    params: HermesParams::default(),
                    retry: None,
                    timeout_ms: Some(5000),
                },
            ],
            metadata: HermesWorkflowMetadata {
                version: "1.0".to_string(),
                author: Some("Test".to_string()),
                description: None,
                tags: vec!["test".to_string()],
            },
        };
        
        assert_eq!(workflow.id, "wf_001");
        assert_eq!(workflow.steps.len(), 1);
    }

    #[tokio::test]
    async fn test_workflow_engine() {
        let engine = HermesWorkflowEngine::new();
        
        let workflow = HermesWorkflow {
            id: "wf_test".to_string(),
            name: "Test".to_string(),
            steps: vec![],
            metadata: HermesWorkflowMetadata::default(),
        };
        
        engine.register_workflow(workflow).await.unwrap();
        let found = engine.get_workflow("wf_test").await;
        
        assert!(found.is_some());
    }
}