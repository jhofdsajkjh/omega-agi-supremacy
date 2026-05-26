//! 任务路由器 - 智能任务分配策略

use super::{SwarmError, AgentHandle, SwarmTask};
use std::collections::HashMap;

/// 路由策略
#[derive(Clone, Debug)]
pub enum RoutingStrategy {
    /// 轮询
    RoundRobin,
    /// 最少连接
    LeastConnections,
    /// 能力匹配
    CapabilityMatch,
    /// 负载均衡
    LoadBalanced,
    /// 优先级
    Priority,
    /// 随机
    Random,
}

/// 任务路由器
pub struct TaskRouter {
    strategy: RoutingStrategy,
    round_robin_index: std::sync::atomic::AtomicUsize,
}

impl TaskRouter {
    pub fn new() -> Self {
        Self {
            strategy: RoutingStrategy::CapabilityMatch,
            round_robin_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    
    pub fn with_strategy(strategy: RoutingStrategy) -> Self {
        Self {
            strategy,
            round_robin_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    
    /// 选择最佳Agent
    pub fn select_agent(
        &self,
        task: &SwarmTask,
        agents: &HashMap<String, AgentHandle>,
    ) -> Result<String, SwarmError> {
        match self.strategy {
            RoutingStrategy::RoundRobin => self.round_robin(agents),
            RoutingStrategy::LeastConnections => self.least_connections(agents),
            RoutingStrategy::CapabilityMatch => self.capability_match(task, agents),
            RoutingStrategy::LoadBalanced => self.load_balanced(task, agents),
            RoutingStrategy::Priority => self.priority_based(task, agents),
            RoutingStrategy::Random => self.random_select(agents),
        }
    }
    
    /// 轮询选择
    fn round_robin(&self, agents: &HashMap<String, AgentHandle>) -> Result<String, SwarmError> {
        let available: Vec<&String> = agents
            .iter()
            .filter(|(_, a)| a.current_task.is_none())
            .map(|(id, _)| id)
            .collect();
        
        if available.is_empty() {
            return Err(SwarmError::RoutingFailed("No available agents".to_string()));
        }
        
        let index = self.round_robin_index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let selected = available[index % available.len()];
        
        Ok(selected.clone())
    }
    
    /// 最少连接
    fn least_connections(&self, agents: &HashMap<String, AgentHandle>) -> Result<String, SwarmError> {
        agents
            .iter()
            .filter(|(_, a)| a.current_task.is_none())
            .min_by_key(|(_, a)| a.last_heartbeat) // 使用last_heartbeat作为代理指标
            .map(|(id, _)| id.clone())
            .ok_or_else(|| SwarmError::RoutingFailed("No available agents".to_string()))
    }
    
    /// 能力匹配
    fn capability_match(
        &self,
        task: &SwarmTask,
        agents: &HashMap<String, AgentHandle>,
    ) -> Result<String, SwarmError> {
        let required_caps = Self::extract_required_capabilities(task);
        
        let mut matches: Vec<(String, f64)> = agents
            .iter()
            .filter(|(_, a)| a.current_task.is_none())
            .map(|(id, agent)| {
                let score = Self::calculate_match_score(&required_caps, &agent.capabilities);
                (id.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();
        
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        matches
            .first()
            .map(|(id, _)| id.clone())
            .ok_or_else(|| SwarmError::RoutingFailed("No capable agent found".to_string()))
    }
    
    /// 负载均衡
    fn load_balanced(
        &self,
        task: &SwarmTask,
        agents: &HashMap<String, AgentHandle>,
    ) -> Result<String, SwarmError> {
        // 结合能力匹配和负载
        let required_caps = Self::extract_required_capabilities(task);
        
        let mut matches: Vec<(String, f64)> = agents
            .iter()
            .filter(|(_, a)| a.current_task.is_none())
            .map(|(id, agent)| {
                let capability_score = Self::calculate_match_score(&required_caps, &agent.capabilities);
                let load_factor = 1.0; // 简化，实际应计算Agent历史负载
                let score = capability_score / load_factor;
                (id.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();
        
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        matches
            .first()
            .map(|(id, _)| id.clone())
            .ok_or_else(|| SwarmError::RoutingFailed("No suitable agent found".to_string()))
    }
    
    /// 基于优先级
    fn priority_based(
        &self,
        task: &SwarmTask,
        agents: &HashMap<String, AgentHandle>,
    ) -> Result<String, SwarmError> {
        // 高优先级任务分配给最可靠的Agent
        if task.priority >= 8 {
            // 选择最健康的Agent
            agents
                .iter()
                .filter(|(_, a)| matches!(a.health, super::AgentHealth::Healthy))
                .filter(|(_, a)| a.current_task.is_none())
                .max_by_key(|(_, a)| a.capabilities.len())
                .map(|(id, _)| id.clone())
                .ok_or_else(|| SwarmError::RoutingFailed("No healthy agent available".to_string()))
        } else {
            self.capability_match(task, agents)
        }
    }
    
    /// 随机选择
    fn random_select(&self, agents: &HashMap<String, AgentHandle>) -> Result<String, SwarmError> {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let available: Vec<&String> = agents
            .iter()
            .filter(|(_, a)| a.current_task.is_none())
            .map(|(id, _)| id)
            .collect();
        
        if available.is_empty() {
            return Err(SwarmError::RoutingFailed("No available agents".to_string()));
        }
        
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as usize;
        
        let index = seed % available.len();
        Ok(available[index].clone())
    }
    
    /// 提取任务所需能力
    fn extract_required_capabilities(task: &SwarmTask) -> Vec<String> {
        use super::TaskType;
        
        match &task.task_type {
            TaskType::CodeGeneration => vec!["coding".to_string()],
            TaskType::CodeReview => vec!["review".to_string(), "coding".to_string()],
            TaskType::Testing => vec!["testing".to_string()],
            TaskType::Documentation => vec!["writing".to_string()],
            TaskType::Analysis => vec!["analysis".to_string()],
            TaskType::Refactoring => vec!["coding".to_string(), "analysis".to_string()],
            TaskType::SecurityAudit => vec!["security".to_string()],
            TaskType::Custom(caps) => caps.split(',').map(|s| s.trim().to_string()).collect(),
        }
    }
    
    /// 计算匹配分数
    fn calculate_match_score(required: &[String], available: &[String]) -> f64 {
        if required.is_empty() {
            return 1.0;
        }
        
        let matches = required.iter()
            .filter(|r| available.contains(r))
            .count();
        
        matches as f64 / required.len() as f64
    }
    
    /// 设置路由策略
    pub fn set_strategy(&mut self, strategy: RoutingStrategy) {
        self.strategy = strategy;
    }
}

impl Default for TaskRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{TaskType, TaskStatus};

    fn create_test_task(task_type: TaskType) -> SwarmTask {
        SwarmTask {
            id: "task_1".to_string(),
            task_type,
            priority: 5,
            description: "Test task".to_string(),
            assigned_agents: vec![],
            status: TaskStatus::Pending,
            created_at: 0,
            deadline: None,
            dependencies: vec![],
        }
    }

    fn create_test_agents() -> HashMap<String, AgentHandle> {
        let mut agents = HashMap::new();
        
        agents.insert("agent_1".to_string(), AgentHandle {
            id: "agent_1".to_string(),
            capabilities: vec!["coding".to_string()],
            current_task: None,
            health: super::super::AgentHealth::Healthy,
            last_heartbeat: 0,
        });
        
        agents.insert("agent_2".to_string(), AgentHandle {
            id: "agent_2".to_string(),
            capabilities: vec!["testing".to_string()],
            current_task: None,
            health: super::super::AgentHealth::Healthy,
            last_heartbeat: 0,
        });
        
        agents
    }

    #[test]
    fn test_capability_match() {
        let router = TaskRouter::with_strategy(RoutingStrategy::CapabilityMatch);
        let task = create_test_task(TaskType::CodeGeneration);
        let agents = create_test_agents();
        
        let selected = router.select_agent(&task, &agents).unwrap();
        assert_eq!(selected, "agent_1");
    }

    #[test]
    fn test_round_robin() {
        let router = TaskRouter::with_strategy(RoutingStrategy::RoundRobin);
        let task = create_test_task(TaskType::CodeGeneration);
        let agents = create_test_agents();
        
        let selected1 = router.select_agent(&task, &agents).unwrap();
        let selected2 = router.select_agent(&task, &agents).unwrap();
        
        // 轮询应该选择不同的Agent
        assert_ne!(selected1, selected2);
    }
}
