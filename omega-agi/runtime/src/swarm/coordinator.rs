//! Swarm协调器 - 多Agent协作核心

use super::{generate_id, SwarmError};
use super::{ConsensusEngine, CrdtDoc, TaskRouter, HealthMonitor, AgentHealth};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use serde::{Deserialize, Serialize};

/// Agent句柄
#[derive(Clone, Debug)]
pub struct AgentHandle {
    pub id: String,
    pub capabilities: Vec<String>,
    pub current_task: Option<String>,
    pub health: AgentHealth,
    pub last_heartbeat: u64,
}

impl AgentHandle {
    pub fn new(id: String, capabilities: Vec<String>) -> Self {
        Self {
            id,
            capabilities,
            current_task: None,
            health: AgentHealth::Healthy,
            last_heartbeat: current_timestamp(),
        }
    }
    
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(&capability.to_string())
    }
}

/// Swarm任务
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmTask {
    pub id: String,
    pub task_type: TaskType,
    pub priority: u8, // 1-10, 10为最高
    pub description: String,
    pub assigned_agents: Vec<String>,
    pub status: TaskStatus,
    pub created_at: u64,
    pub deadline: Option<u64>,
    pub dependencies: Vec<String>, // 依赖的其他任务ID
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TaskType {
    CodeGeneration,
    CodeReview,
    Testing,
    Documentation,
    Analysis,
    Refactoring,
    SecurityAudit,
    Custom(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Assigned,
    InProgress,
    UnderReview,
    Completed,
    Failed,
}

/// Swarm事件
#[derive(Clone, Debug)]
pub enum SwarmEvent {
    AgentJoined(String),
    AgentLeft(String),
    TaskCreated(String),
    TaskAssigned { task_id: String, agent_id: String },
    TaskCompleted(String),
    TaskFailed { task_id: String, reason: String },
    ConsensusReached(String),
    ConflictDetected { task_id: String, agents: Vec<String> },
}

/// Swarm协调器
pub struct SwarmCoordinator {
    agents: Arc<RwLock<HashMap<String, AgentHandle>>>,
    task_queue: Arc<RwLock<VecDeque<SwarmTask>>>,
    active_tasks: Arc<RwLock<HashMap<String, SwarmTask>>>,
    consensus_engine: ConsensusEngine,
    crdt_docs: Arc<RwLock<HashMap<String, CrdtDoc>>>,
    task_router: TaskRouter,
    health_monitor: HealthMonitor,
    event_tx: mpsc::Sender<SwarmEvent>,
    event_rx: Arc<RwLock<mpsc::Receiver<SwarmEvent>>>,
}

impl SwarmCoordinator {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            task_queue: Arc::new(RwLock::new(VecDeque::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            consensus_engine: ConsensusEngine::new(),
            crdt_docs: Arc::new(RwLock::new(HashMap::new())),
            task_router: TaskRouter::new(),
            health_monitor: HealthMonitor::new(),
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
        }
    }
    
    /// 注册Agent
    pub async fn register_agent(&self, capabilities: Vec<String>) -> Result<String, SwarmError> {
        let agent_id = generate_id();
        let agent = AgentHandle::new(agent_id.clone(), capabilities);
        
        let mut agents = self.agents.write().await;
        agents.insert(agent_id.clone(), agent);
        
        let _ = self.event_tx.send(SwarmEvent::AgentJoined(agent_id.clone())).await;
        
        Ok(agent_id)
    }
    
    /// 注销Agent
    pub async fn unregister_agent(&self, agent_id: &str) -> Result<(), SwarmError> {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id)
            .ok_or_else(|| SwarmError::AgentNotFound(agent_id.to_string()))?;
        
        let _ = self.event_tx.send(SwarmEvent::AgentLeft(agent_id.to_string())).await;
        
        Ok(())
    }
    
    /// 创建任务
    pub async fn create_task(
        &self,
        task_type: TaskType,
        description: String,
        priority: u8,
        deadline: Option<u64>,
    ) -> Result<String, SwarmError> {
        let task_id = generate_id();
        let task = SwarmTask {
            id: task_id.clone(),
            task_type,
            priority: priority.min(10),
            description,
            assigned_agents: Vec::new(),
            status: TaskStatus::Pending,
            created_at: current_timestamp(),
            deadline,
            dependencies: Vec::new(),
        };
        
        let mut queue = self.task_queue.write().await;
        queue.push_back(task);
        
        let _ = self.event_tx.send(SwarmEvent::TaskCreated(task_id.clone())).await;
        
        Ok(task_id)
    }
    
    /// 分解任务并分配给Agent群
    pub async fn decompose_and_assign(&self, goal: &str) -> Result<Vec<String>, SwarmError> {
        // 1. 使用LLM分解任务 (模拟)
        let subtasks = self.llm_decompose(goal).await?;
        
        let mut task_ids = Vec::new();
        let agents = self.agents.read().await;
        
        for subtask in subtasks {
            // 2. 根据Agent能力匹配分配
            let best_agents = self.find_best_agents(&subtask, &agents).await?;
            
            let task_id = self.create_task(
                subtask.task_type,
                subtask.description,
                subtask.priority,
                subtask.deadline,
            ).await?;
            
            // 3. 分配任务给Agent群
            for agent_id in best_agents {
                self.assign_task_to_agent(&task_id, &agent_id).await?;
            }
            
            task_ids.push(task_id);
        }
        
        Ok(task_ids)
    }
    
    /// LLM任务分解 (模拟实现)
    async fn llm_decompose(&self, goal: &str) -> Result<Vec<Subtask>, SwarmError> {
        // 实际实现会调用LLM API
        // 这里返回模拟的子任务
        let subtasks = vec![
            Subtask {
                task_type: TaskType::Analysis,
                description: format!("分析需求: {}", goal),
                priority: 10,
                deadline: None,
                required_capabilities: vec!["analysis".to_string()],
            },
            Subtask {
                task_type: TaskType::CodeGeneration,
                description: format!("生成代码实现: {}", goal),
                priority: 9,
                deadline: None,
                required_capabilities: vec!["coding".to_string(), "rust".to_string()],
            },
            Subtask {
                task_type: TaskType::Testing,
                description: "生成并运行测试".to_string(),
                priority: 8,
                deadline: None,
                required_capabilities: vec!["testing".to_string()],
            },
        ];
        
        Ok(subtasks)
    }
    
    /// 查找最适合的Agents
    async fn find_best_agents(
        &self,
        subtask: &Subtask,
        agents: &HashMap<String, AgentHandle>,
    ) -> Result<Vec<String>, SwarmError> {
        let mut matches: Vec<(String, usize)> = agents
            .iter()
            .filter(|(_, agent)| agent.current_task.is_none())
            .filter(|(_, agent)| {
                subtask.required_capabilities.iter()
                    .all(|cap| agent.has_capability(cap))
            })
            .map(|(id, _)| (id.clone(), 1usize))
            .collect();
        
        // 按负载排序
        matches.sort_by_key(|(_, score)| *score);
        
        // 返回前3个最佳匹配
        Ok(matches.into_iter().take(3).map(|(id, _)| id).collect())
    }
    
    /// 分配任务给Agent
    async fn assign_task_to_agent(&self, task_id: &str, agent_id: &str) -> Result<(), SwarmError> {
        let mut agents = self.agents.write().await;
        let agent = agents.get_mut(agent_id)
            .ok_or_else(|| SwarmError::AgentNotFound(agent_id.to_string()))?;
        
        agent.current_task = Some(task_id.to_string());
        
        let mut tasks = self.active_tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.assigned_agents.push(agent_id.to_string());
            task.status = TaskStatus::Assigned;
        }
        
        let _ = self.event_tx.send(SwarmEvent::TaskAssigned {
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
        }).await;
        
        Ok(())
    }
    
    /// 获取下一个待处理任务
    pub async fn next_task(&self, agent_id: &str) -> Option<SwarmTask> {
        let mut queue = self.task_queue.write().await;
        let mut agents = self.agents.write().await;
        
        if let Some(agent) = agents.get_mut(agent_id) {
            // 找到优先级最高的匹配任务
            if let Some(pos) = queue.iter().position(|task| {
                agent.capabilities.iter().any(|cap| {
                    matches!(task.task_type, TaskType::CodeGeneration) && cap == "coding" ||
                    matches!(task.task_type, TaskType::Testing) && cap == "testing" ||
                    matches!(task.task_type, TaskType::Analysis) && cap == "analysis"
                })
            }) {
                let mut task = queue.remove(pos).unwrap();
                task.status = TaskStatus::InProgress;
                task.assigned_agents.push(agent_id.to_string());
                agent.current_task = Some(task.id.clone());
                
                let task_clone = task.clone();
                let mut active = self.active_tasks.write().await;
                active.insert(task.id.clone(), task);
                
                return Some(task_clone);
            }
        }
        
        None
    }
    
    /// 完成任务
    pub async fn complete_task(&self, task_id: &str, result: TaskResult) -> Result<(), SwarmError> {
        let mut tasks = self.active_tasks.write().await;
        let task = tasks.get_mut(task_id)
            .ok_or_else(|| SwarmError::TaskNotFound(task_id.to_string()))?;
        
        // 释放Agents
        let mut agents = self.agents.write().await;
        for agent_id in &task.assigned_agents {
            if let Some(agent) = agents.get_mut(agent_id) {
                agent.current_task = None;
            }
        }
        
        match result {
            TaskResult::Success => {
                task.status = TaskStatus::Completed;
                let _ = self.event_tx.send(SwarmEvent::TaskCompleted(task_id.to_string())).await;
            }
            TaskResult::Failure(reason) => {
                task.status = TaskStatus::Failed;
                let _ = self.event_tx.send(SwarmEvent::TaskFailed {
                    task_id: task_id.to_string(),
                    reason,
                }).await;
            }
        }
        
        Ok(())
    }
    
    /// 获取系统状态
    pub async fn get_status(&self) -> SwarmStatus {
        let agents = self.agents.read().await;
        let tasks = self.active_tasks.read().await;
        let queue = self.task_queue.read().await;
        
        SwarmStatus {
            agent_count: agents.len(),
            active_task_count: tasks.len(),
            pending_task_count: queue.len(),
            healthy_agents: agents.values().filter(|a| matches!(a.health, AgentHealth::Healthy)).count(),
        }
    }
}

/// 子任务定义
#[derive(Debug)]
struct Subtask {
    task_type: TaskType,
    description: String,
    priority: u8,
    deadline: Option<u64>,
    required_capabilities: Vec<String>,
}

/// 任务结果
pub enum TaskResult {
    Success,
    Failure(String),
}

/// Swarm状态
#[derive(Debug, Clone)]
pub struct SwarmStatus {
    pub agent_count: usize,
    pub active_task_count: usize,
    pub pending_task_count: usize,
    pub healthy_agents: usize,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_registration() {
        let coordinator = SwarmCoordinator::new();
        let agent_id = coordinator.register_agent(vec!["coding".to_string()]).await.unwrap();
        assert!(!agent_id.is_empty());
        
        let status = coordinator.get_status().await;
        assert_eq!(status.agent_count, 1);
    }

    #[tokio::test]
    async fn test_task_creation() {
        let coordinator = SwarmCoordinator::new();
        let task_id = coordinator.create_task(
            TaskType::CodeGeneration,
            "Test task".to_string(),
            5,
            None,
        ).await.unwrap();
        
        assert!(!task_id.is_empty());
        
        let status = coordinator.get_status().await;
        assert_eq!(status.pending_task_count, 1);
    }

    #[tokio::test]
    async fn test_task_assignment() {
        let coordinator = SwarmCoordinator::new();
        let agent_id = coordinator.register_agent(vec!["coding".to_string()]).await.unwrap();
        let task_id = coordinator.create_task(
            TaskType::CodeGeneration,
            "Generate code".to_string(),
            5,
            None,
        ).await.unwrap();
        
        coordinator.assign_task_to_agent(&task_id, &agent_id).await.unwrap();
        
        let status = coordinator.get_status().await;
        assert_eq!(status.active_task_count, 1);
    }
}
