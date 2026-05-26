//! 健康监控 - Agent健康状态监控

use super::SwarmError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Agent健康状态
#[derive(Clone, Debug, PartialEq)]
pub enum AgentHealth {
    Healthy,
    Warning(String),
    Critical(String),
    Offline,
}

/// 健康指标
#[derive(Clone, Debug)]
pub struct HealthMetrics {
    pub cpu_usage: f32,        // 0-100
    pub memory_usage: f32,     // 0-100
    pub task_success_rate: f32, // 0-1
    pub avg_response_time_ms: u64,
    pub last_heartbeat: u64,
    pub consecutive_failures: u32,
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            task_success_rate: 1.0,
            avg_response_time_ms: 0,
            last_heartbeat: current_timestamp(),
            consecutive_failures: 0,
        }
    }
}

/// 健康监控器
pub struct HealthMonitor {
    metrics: Arc<RwLock<HashMap<String, HealthMetrics>>>,
    health_thresholds: HealthThresholds,
    check_interval: Duration,
}

#[derive(Clone, Debug)]
pub struct HealthThresholds {
    pub cpu_warning: f32,
    pub cpu_critical: f32,
    pub memory_warning: f32,
    pub memory_critical: f32,
    pub heartbeat_timeout_secs: u64,
    pub max_consecutive_failures: u32,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            cpu_warning: 70.0,
            cpu_critical: 90.0,
            memory_warning: 80.0,
            memory_critical: 95.0,
            heartbeat_timeout_secs: 60,
            max_consecutive_failures: 3,
        }
    }
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            health_thresholds: HealthThresholds::default(),
            check_interval: Duration::from_secs(30),
        }
    }
    
    pub fn with_thresholds(thresholds: HealthThresholds) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            health_thresholds: thresholds,
            check_interval: Duration::from_secs(30),
        }
    }
    
    /// 注册Agent监控
    pub async fn register_agent(&self, agent_id: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.insert(agent_id.to_string(), HealthMetrics::default());
    }
    
    /// 更新心跳
    pub async fn update_heartbeat(&self, agent_id: &str) -> Result<(), SwarmError> {
        let mut metrics = self.metrics.write().await;
        
        if let Some(m) = metrics.get_mut(agent_id) {
            m.last_heartbeat = current_timestamp();
            m.consecutive_failures = 0;
            Ok(())
        } else {
            Err(SwarmError::HealthCheckFailed(format!("Agent {} not registered", agent_id)))
        }
    }
    
    /// 更新指标
    pub async fn update_metrics(&self, agent_id: &str, new_metrics: HealthMetrics) -> Result<(), SwarmError> {
        let mut metrics = self.metrics.write().await;
        
        if metrics.contains_key(agent_id) {
            metrics.insert(agent_id.to_string(), new_metrics);
            Ok(())
        } else {
            Err(SwarmError::HealthCheckFailed(format!("Agent {} not registered", agent_id)))
        }
    }
    
    /// 记录任务结果
    pub async fn record_task_result(&self, agent_id: &str, success: bool) -> Result<(), SwarmError> {
        let mut metrics = self.metrics.write().await;
        
        if let Some(m) = metrics.get_mut(agent_id) {
            if success {
                m.consecutive_failures = 0;
                // 更新成功率 (指数移动平均)
                m.task_success_rate = m.task_success_rate * 0.9 + 0.1;
            } else {
                m.consecutive_failures += 1;
                m.task_success_rate = m.task_success_rate * 0.9;
            }
            Ok(())
        } else {
            Err(SwarmError::HealthCheckFailed(format!("Agent {} not registered", agent_id)))
        }
    }
    
    /// 检查Agent健康状态
    pub async fn check_health(&self, agent_id: &str) -> AgentHealth {
        let metrics = self.metrics.read().await;
        
        let Some(m) = metrics.get(agent_id) else {
            return AgentHealth::Offline;
        };
        
        let now = current_timestamp();
        let heartbeat_age = now - m.last_heartbeat;
        
        // 检查心跳超时
        if heartbeat_age > self.health_thresholds.heartbeat_timeout_secs {
            return AgentHealth::Offline;
        }
        
        // 检查连续失败
        if m.consecutive_failures >= self.health_thresholds.max_consecutive_failures {
            return AgentHealth::Critical(format!(
                "{} consecutive failures",
                m.consecutive_failures
            ));
        }
        
        // 检查CPU
        if m.cpu_usage >= self.health_thresholds.cpu_critical {
            return AgentHealth::Critical(format!("CPU usage: {:.1}%", m.cpu_usage));
        } else if m.cpu_usage >= self.health_thresholds.cpu_warning {
            return AgentHealth::Warning(format!("CPU usage: {:.1}%", m.cpu_usage));
        }
        
        // 检查内存
        if m.memory_usage >= self.health_thresholds.memory_critical {
            return AgentHealth::Critical(format!("Memory usage: {:.1}%", m.memory_usage));
        } else if m.memory_usage >= self.health_thresholds.memory_warning {
            return AgentHealth::Warning(format!("Memory usage: {:.1}%", m.memory_usage));
        }
        
        // 检查成功率
        if m.task_success_rate < 0.5 {
            return AgentHealth::Critical(format!("Success rate: {:.1}%", m.task_success_rate * 100.0));
        } else if m.task_success_rate < 0.8 {
            return AgentHealth::Warning(format!("Success rate: {:.1}%", m.task_success_rate * 100.0));
        }
        
        AgentHealth::Healthy
    }
    
    /// 检查所有Agent健康
    pub async fn check_all_health(&self) -> HashMap<String, AgentHealth> {
        let metrics = self.metrics.read().await;
        let mut results = HashMap::new();
        
        for agent_id in metrics.keys() {
            let health = self.check_health(agent_id).await;
            results.insert(agent_id.clone(), health);
        }
        
        results
    }
    
    /// 获取不健康的Agent
    pub async fn get_unhealthy_agents(&self) -> Vec<(String, AgentHealth)> {
        let all_health = self.check_all_health().await;
        
        all_health
            .into_iter()
            .filter(|(_, health)| !matches!(health, AgentHealth::Healthy))
            .collect()
    }
    
    /// 启动监控循环
    pub async fn start_monitoring(&self) {
        let metrics = self.metrics.clone();
        let thresholds = self.health_thresholds.clone();
        let interval = self.check_interval;
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                
                let mut metrics_guard = metrics.write().await;
                let now = current_timestamp();
                
                for (agent_id, m) in metrics_guard.iter_mut() {
                    // 检查心跳超时
                    let heartbeat_age = now - m.last_heartbeat;
                    if heartbeat_age > thresholds.heartbeat_timeout_secs {
                        tracing::warn!("Agent {} heartbeat timeout", agent_id);
                    }
                    
                    // 模拟收集系统指标 (实际应调用系统API)
                    // m.cpu_usage = collect_cpu_usage();
                    // m.memory_usage = collect_memory_usage();
                }
            }
        });
    }
    
    /// 注销Agent
    pub async fn unregister_agent(&self, agent_id: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.remove(agent_id);
    }
    
    /// 获取监控统计
    pub async fn get_statistics(&self) -> HealthStatistics {
        let metrics = self.metrics.read().await;
        let total = metrics.len();
        
        let mut healthy = 0;
        let mut warning = 0;
        let mut critical = 0;
        let mut offline = 0;
        
        for agent_id in metrics.keys() {
            match self.check_health(agent_id).await {
                AgentHealth::Healthy => healthy += 1,
                AgentHealth::Warning(_) => warning += 1,
                AgentHealth::Critical(_) => critical += 1,
                AgentHealth::Offline => offline += 1,
            }
        }
        
        HealthStatistics {
            total_agents: total,
            healthy,
            warning,
            critical,
            offline,
            avg_cpu_usage: metrics.values().map(|m| m.cpu_usage).sum::<f32>() / total.max(1) as f32,
            avg_memory_usage: metrics.values().map(|m| m.memory_usage).sum::<f32>() / total.max(1) as f32,
        }
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// 健康统计
#[derive(Clone, Debug)]
pub struct HealthStatistics {
    pub total_agents: usize,
    pub healthy: usize,
    pub warning: usize,
    pub critical: usize,
    pub offline: usize,
    pub avg_cpu_usage: f32,
    pub avg_memory_usage: f32,
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
    async fn test_register_and_heartbeat() {
        let monitor = HealthMonitor::new();
        
        monitor.register_agent("agent_1").await;
        monitor.update_heartbeat("agent_1").await.unwrap();
        
        let health = monitor.check_health("agent_1").await;
        assert_eq!(health, AgentHealth::Healthy);
    }

    #[tokio::test]
    async fn test_consecutive_failures() {
        let monitor = HealthMonitor::new();
        
        monitor.register_agent("agent_1").await;
        
        // 记录3次失败
        for _ in 0..3 {
            monitor.record_task_result("agent_1", false).await.unwrap();
        }
        
        let health = monitor.check_health("agent_1").await;
        assert!(matches!(health, AgentHealth::Critical(_)));
    }

    #[tokio::test]
    async fn test_statistics() {
        let monitor = HealthMonitor::new();
        
        monitor.register_agent("agent_1").await;
        monitor.register_agent("agent_2").await;
        
        let stats = monitor.get_statistics().await;
        assert_eq!(stats.total_agents, 2);
        assert_eq!(stats.healthy, 2);
    }
}
