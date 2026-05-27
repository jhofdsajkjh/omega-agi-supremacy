//! OMEGA AGI - OpenClaw Adapter
//! 
//! Adapter for integrating with OpenClaw agent system.
//! Provides compatibility with OpenClaw message protocol, skill loading,
//! and agent communication standards.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Generate a simple UUID-like ID
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("id_{}_{}", duration.as_nanos(), std::process::id())
}

/// OpenClaw message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OpenClawMessage {
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "image")]
    Image { image_key: String, caption: Option<String> },
    #[serde(rename = "file")]
    File { file_key: String, name: String },
    #[serde(rename = "post")]
    Post { content: OpenClawPostContent },
    #[serde(rename = "interactive")]
    Interactive { card: OpenClawCard },
}

/// OpenClaw rich text content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawPostContent {
    pub zh_cn: Option<OpenClawPost>,
    pub en_us: Option<OpenClawPost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawPost {
    pub title: String,
    pub content: Vec<Vec<OpenClawElement>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tag")]
pub enum OpenClawElement {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "mention")]
    Mention { open_id: String, name: String },
    #[serde(rename = "link")]
    Link { text: String, url: String },
}

/// OpenClaw interactive card
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawCard {
    pub config: Option<OpenClawCardConfig>,
    pub elements: Vec<OpenClawCardElement>,
    pub header: Option<OpenClawCardHeader>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawCardConfig {
    pub wide_screen_mode: Option<bool>,
    pub enable_forward: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawCardHeader {
    pub title: OpenClawCardTitle,
    pub subtitle: Option<String>,
    pub image_url: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawCardTitle {
    pub tag: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tag")]
pub enum OpenClawCardElement {
    #[serde(rename = "divider")]
    Divider,
    #[serde(rename = "markdown")]
    Markdown { content: String },
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "hr")]
    Hr,
}

/// OpenClaw sender info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawSender {
    pub sender_id: OpenClawSenderId,
    pub sender_type: String,
    pub tenant_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawSenderId {
    pub open_id: Option<String>,
    pub union_id: Option<String>,
    pub user_id: Option<String>,
}

/// OpenClaw skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct OpenClawSkill {
    pub name: String,
    pub description: String,
    pub location: String,
    pub trigger: Vec<String>,
}



/// OpenClaw agent protocol message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawAgentMessage {
    pub message_id: String,
    pub message_type: String,
    pub create_time: String,
    pub chat_id: String,
    pub chat_type: String,
    pub content: String,
    pub sender: OpenClawSender,
}

/// OpenClaw skill loader
pub struct OpenClawSkillLoader {
    skills: Arc<RwLock<HashMap<String, OpenClawSkill>>>,
}

impl OpenClawSkillLoader {
    pub fn new() -> Self {
        Self {
            skills: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new skill
    pub async fn register(&self, skill: OpenClawSkill) -> Result<()> {
        let mut skills = self.skills.write().await;
        skills.insert(skill.name.clone(), skill);
        Ok(())
    }

    /// Find skill by trigger keyword
    pub async fn find_by_trigger(&self, keyword: &str) -> Option<OpenClawSkill> {
        let skills = self.skills.read().await;
        for skill in skills.values() {
            if skill.trigger.iter().any(|t| t.contains(keyword)) {
                return Some(skill.clone());
            }
        }
        None
    }

    /// List all registered skills
    pub async fn list_skills(&self) -> Vec<OpenClawSkill> {
        let skills = self.skills.read().await;
        skills.values().cloned().collect()
    }
}

impl Default for OpenClawSkillLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// OpenClaw message adapter trait
pub trait OpenClawAdapterTrait: Send + Sync {
    /// Process incoming OpenClaw message
    fn process_message(&self, message: OpenClawAgentMessage) -> impl std::future::Future<Output = Result<OpenClawMessage>> + Send;
    
    /// Send message through OpenClaw
    fn send_message(&self, message: OpenClawMessage, chat_id: &str) -> impl std::future::Future<Output = Result<String>> + Send;
    
    /// Load skills from OpenClaw skill directory
    fn load_skills(&self, skill_dir: &str) -> impl std::future::Future<Output = Result<Vec<OpenClawSkill>>> + Send;
    
    /// Get adapter info
    fn adapter_info(&self) -> AdapterInfo;
}

/// Adapter metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub name: String,
    pub version: String,
    pub protocol_version: String,
    pub capabilities: Vec<String>,
}

impl Default for AdapterInfo {
    fn default() -> Self {
        Self {
            name: "OpenClaw Adapter".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: "1.0".to_string(),
            capabilities: vec![
                "message_sending".to_string(),
                "skill_loading".to_string(),
                "agent_protocol".to_string(),
                "interactive_cards".to_string(),
            ],
        }
    }
}

/// OpenClaw adapter implementation
pub struct OpenClawAdapter {
    #[allow(unused)] skill_loader: OpenClawSkillLoader,
    info: AdapterInfo,
}

impl OpenClawAdapter {
    pub fn new() -> Self {
        Self {
            skill_loader: OpenClawSkillLoader::new(),
            info: AdapterInfo::default(),
        }
    }

    /// Parse incoming message from OpenClaw webhook/event
    pub fn parse_event(&self, payload: &[u8]) -> Result<OpenClawAgentMessage> {
        serde_json::from_slice(payload).map_err(|e| anyhow::anyhow!("Failed to parse OpenClaw event: {}", e))
    }

    /// Build response message
    pub fn build_text_response(&self, content: &str) -> OpenClawMessage {
        OpenClawMessage::Text { content: content.to_string() }
    }

    /// Build interactive card response
    pub fn build_card_response(&self, title: &str, elements: Vec<OpenClawCardElement>) -> OpenClawMessage {
        OpenClawMessage::Interactive {
            card: OpenClawCard {
                config: Some(OpenClawCardConfig {
                    wide_screen_mode: Some(true),
                    enable_forward: Some(true),
                }),
                elements,
                header: Some(OpenClawCardHeader {
                    title: OpenClawCardTitle {
                        tag: "plain_text".to_string(),
                        content: title.to_string(),
                    },
                    subtitle: None,
                    image_url: None,
                    template: None,
                }),
            },
        }
    }
}

impl Default for OpenClawAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenClawAdapterTrait for OpenClawAdapter {
    async fn process_message(&self, message: OpenClawAgentMessage) -> Result<OpenClawMessage> {
        tracing::info!("Processing OpenClaw message: {}", message.message_id);
        
        // Parse content based on message type
        match message.message_type.as_str() {
            "text" => {
                Ok(OpenClawMessage::Text { content: message.content })
            }
            "post" => {
                let post: OpenClawPostContent = serde_json::from_str(&message.content)?;
                Ok(OpenClawMessage::Post { content: post })
            }
            _ => {
                Ok(OpenClawMessage::Text { content: message.content })
            }
        }
    }

    async fn send_message(&self, _message: OpenClawMessage, _chat_id: &str) -> Result<String> {
        // In real implementation, this would call OpenClaw API
        Ok(generate_id())
    }

    async fn load_skills(&self, _skill_dir: &str) -> Result<Vec<OpenClawSkill>> {
        // In real implementation, load skills from directory
        Ok(Vec::new())
    }

    fn adapter_info(&self) -> AdapterInfo {
        self.info.clone()
    }
}

/// OpenClaw protocol constants
pub mod protocol {
    pub const MESSAGE_TYPE_TEXT: &str = "text";
    pub const MESSAGE_TYPE_IMAGE: &str = "image";
    pub const MESSAGE_TYPE_FILE: &str = "file";
    pub const MESSAGE_TYPE_POST: &str = "post";
    pub const MESSAGE_TYPE_INTERACTIVE: &str = "interactive";
    
    pub const CHAT_TYPE_P2P: &str = "p2p";
    pub const CHAT_TYPE_GROUP: &str = "group";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_skill_loader() {
        let loader = OpenClawSkillLoader::new();
        
        let skill = OpenClawSkill {
            name: "test_skill".to_string(),
            description: "A test skill".to_string(),
            location: "/skills/test".to_string(),
            trigger: vec!["test".to_string()],
        };
        
        loader.register(skill.clone()).await.unwrap();
        let found = loader.find_by_trigger("test").await;
        
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test_skill");
    }

    #[test]
    fn test_message_serialization() {
        let msg = OpenClawMessage::Text {
            content: "Hello OpenClaw".to_string(),
        };
        
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Hello OpenClaw"));
    }

    #[test]
    fn test_adapter_info() {
        let adapter = OpenClawAdapter::new();
        let info = adapter.adapter_info();
        
        assert_eq!(info.name, "OpenClaw Adapter");
        assert!(info.capabilities.contains(&"message_sending".to_string()));
    }
}