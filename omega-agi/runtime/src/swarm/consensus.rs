//! Raft共识引擎 - Agent间冲突解决

use super::{generate_id, SwarmError};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

/// 提案
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Proposal {
    pub id: String,
    pub task_id: String,
    pub proposer: String,
    pub content: ProposalContent,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProposalContent {
    CodeChange { file: String, diff: String },
    TaskAssignment { agent_id: String, task_id: String },
    ArchitectureDecision { decision: String, rationale: String },
    MergeRequest { source_branch: String, target_branch: String },
}

/// 投票
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Vote {
    pub proposal_id: String,
    pub voter: String,
    pub decision: VoteDecision,
    pub timestamp: u64,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum VoteDecision {
    Approve,
    Reject,
    Abstain,
}

/// 共识状态
#[derive(Clone, Debug, PartialEq)]
pub enum ConsensusState {
    Pending,
    Accepted,
    Rejected,
    Expired,
}

/// 共识引擎
pub struct ConsensusEngine {
    proposals: Arc<RwLock<HashMap<String, ProposalState>>>,
    votes: Arc<RwLock<HashMap<String, Vec<Vote>>>>,
    /// 共识阈值 (需要多少比例的同意)
    threshold: f64,
    /// 提案超时时间 (秒)
    timeout: u64,
}

struct ProposalState {
    proposal: Proposal,
    state: ConsensusState,
    created_at: u64,
}

impl ConsensusEngine {
    pub fn new() -> Self {
        Self {
            proposals: Arc::new(RwLock::new(HashMap::new())),
            votes: Arc::new(RwLock::new(HashMap::new())),
            threshold: 0.66, // 2/3 多数
            timeout: 300,    // 5分钟超时
        }
    }
    
    /// 提交提案
    pub async fn propose(&self, proposal: Proposal) -> Result<String, SwarmError> {
        let proposal_id = proposal.id.clone();
        let state = ProposalState {
            proposal: proposal.clone(),
            state: ConsensusState::Pending,
            created_at: current_timestamp(),
        };
        
        let mut proposals = self.proposals.write().await;
        proposals.insert(proposal_id.clone(), state);
        
        let mut votes = self.votes.write().await;
        votes.insert(proposal_id.clone(), Vec::new());
        
        Ok(proposal_id)
    }
    
    /// 投票
    pub async fn vote(&self, vote: Vote, total_agents: usize) -> Result<ConsensusState, SwarmError> {
        let mut votes = self.votes.write().await;
        let vote_list = votes.get_mut(&vote.proposal_id)
            .ok_or_else(|| SwarmError::ConsensusFailed("Proposal not found".to_string()))?;
        
        // 检查是否已投票
        if vote_list.iter().any(|v| v.voter == vote.voter) {
            return Err(SwarmError::ConsensusFailed("Already voted".to_string()));
        }
        
        vote_list.push(vote.clone());
        
        // 检查是否达到共识
        let state = self.check_consensus(&vote.proposal_id, total_agents).await?;
        
        Ok(state)
    }
    
    /// 检查共识状态
    async fn check_consensus(&self, proposal_id: &str, total_agents: usize) -> Result<ConsensusState, SwarmError> {
        let votes = self.votes.read().await;
        let vote_list = votes.get(proposal_id)
            .ok_or_else(|| SwarmError::ConsensusFailed("Proposal not found".to_string()))?;
        
        let proposals = self.proposals.read().await;
        let state = proposals.get(proposal_id)
            .ok_or_else(|| SwarmError::ConsensusFailed("Proposal not found".to_string()))?;
        
        // 检查超时
        if current_timestamp() - state.created_at > self.timeout {
            return Ok(ConsensusState::Expired);
        }
        
        let approve_count = vote_list.iter().filter(|v| matches!(v.decision, VoteDecision::Approve)).count();
        let reject_count = vote_list.iter().filter(|v| matches!(v.decision, VoteDecision::Reject)).count();
        
        let approve_ratio = approve_count as f64 / total_agents as f64;
        let reject_ratio = reject_count as f64 / total_agents as f64;
        
        if approve_ratio >= self.threshold {
            return Ok(ConsensusState::Accepted);
        }
        
        if reject_ratio >= self.threshold {
            return Ok(ConsensusState::Rejected);
        }
        
        Ok(ConsensusState::Pending)
    }
    
    /// 获取提案状态
    pub async fn get_proposal_state(&self, proposal_id: &str) -> Option<ConsensusState> {
        let proposals = self.proposals.read().await;
        proposals.get(proposal_id).map(|s| s.state.clone())
    }
    
    /// 获取提案详情
    pub async fn get_proposal(&self, proposal_id: &str) -> Option<Proposal> {
        let proposals = self.proposals.read().await;
        proposals.get(proposal_id).map(|s| s.proposal.clone())
    }
    
    /// 获取投票详情
    pub async fn get_votes(&self, proposal_id: &str) -> Vec<Vote> {
        let votes = self.votes.read().await;
        votes.get(proposal_id).cloned().unwrap_or_default()
    }
    
    /// 清理过期提案
    pub async fn cleanup_expired(&self) -> usize {
        let mut proposals = self.proposals.write().await;
        let mut votes = self.votes.write().await;
        
        let now = current_timestamp();
        let expired: Vec<String> = proposals
            .iter()
            .filter(|(_, state)| now - state.created_at > self.timeout)
            .map(|(id, _)| id.clone())
            .collect();
        
        for id in &expired {
            proposals.remove(id);
            votes.remove(id);
        }
        
        expired.len()
    }
    
    /// 多阶段提交 (2PC简化版)
    pub async fn two_phase_commit(
        &self,
        proposal: Proposal,
        participants: Vec<String>,
    ) -> Result<bool, SwarmError> {
        // Phase 1: Prepare
        let prepare_votes = self.collect_prepare_votes(&proposal, &participants).await?;
        
        let all_prepared = prepare_votes.iter().all(|v| matches!(v.decision, VoteDecision::Approve));
        
        if !all_prepared {
            return Ok(false); // 有参与者未准备好
        }
        
        // Phase 2: Commit
        let proposal_id = self.propose(proposal).await?;
        
        for participant in participants {
            let vote = Vote {
                proposal_id: proposal_id.clone(),
                voter: participant,
                decision: VoteDecision::Approve,
                timestamp: current_timestamp(),
                comment: None,
            };
            self.vote(vote, participants.len()).await?;
        }
        
        Ok(true)
    }
    
    async fn collect_prepare_votes(
        &self,
        _proposal: &Proposal,
        participants: &[String],
    ) -> Result<Vec<Vote>, SwarmError> {
        // 模拟收集准备投票
        // 实际实现会发送准备请求给所有参与者
        let votes: Vec<Vote> = participants.iter().map(|p| Vote {
            proposal_id: "prepare".to_string(),
            voter: p.clone(),
            decision: VoteDecision::Approve,
            timestamp: current_timestamp(),
            comment: Some("Prepared".to_string()),
        }).collect();
        
        Ok(votes)
    }
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_propose_and_vote() {
        let engine = ConsensusEngine::new();
        
        let proposal = Proposal {
            id: generate_id(),
            task_id: "task_1".to_string(),
            proposer: "agent_1".to_string(),
            content: ProposalContent::CodeChange {
                file: "test.rs".to_string(),
                diff: "+fn test() {}".to_string(),
            },
            timestamp: current_timestamp(),
        };
        
        let proposal_id = engine.propose(proposal).await.unwrap();
        
        // 添加投票
        for i in 0..3 {
            let vote = Vote {
                proposal_id: proposal_id.clone(),
                voter: format!("agent_{}", i),
                decision: VoteDecision::Approve,
                timestamp: current_timestamp(),
                comment: None,
            };
            let state = engine.vote(vote, 3).await.unwrap();
            
            if i == 2 {
                assert_eq!(state, ConsensusState::Accepted);
            } else {
                assert_eq!(state, ConsensusState::Pending);
            }
        }
    }

    #[tokio::test]
    async fn test_reject_consensus() {
        let engine = ConsensusEngine::new();
        
        let proposal = Proposal {
            id: generate_id(),
            task_id: "task_1".to_string(),
            proposer: "agent_1".to_string(),
            content: ProposalContent::CodeChange {
                file: "test.rs".to_string(),
                diff: "+fn test() {}".to_string(),
            },
            timestamp: current_timestamp(),
        };
        
        let proposal_id = engine.propose(proposal).await.unwrap();
        
        // 全部拒绝
        for i in 0..3 {
            let vote = Vote {
                proposal_id: proposal_id.clone(),
                voter: format!("agent_{}", i),
                decision: VoteDecision::Reject,
                timestamp: current_timestamp(),
                comment: Some("Bad code".to_string()),
            };
            engine.vote(vote, 3).await.unwrap();
        }
        
        let state = engine.get_proposal_state(&proposal_id).await.unwrap();
        assert_eq!(state, ConsensusState::Rejected);
    }
}
