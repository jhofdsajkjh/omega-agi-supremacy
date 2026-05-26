//! # Session Manager
//!
//! Manages agent sessions with state tracking, timeout, and lifecycle.
//! Each session has its own security context and memory scope.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::security::{SecurityContext, SecurityRing};

/// Session state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Session created but not yet active
    Created,
    /// Session is actively processing
    Active,
    /// Session is idle (waiting for input)
    Idle,
    /// Session is suspended (can be resumed)
    Suspended,
    /// Session terminated
    Terminated,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionState::Created => write!(f, "Created"),
            SessionState::Active => write!(f, "Active"),
            SessionState::Idle => write!(f, "Idle"),
            SessionState::Suspended => write!(f, "Suspended"),
            SessionState::Terminated => write!(f, "Terminated"),
        }
    }
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Maximum session duration
    pub max_duration: Duration,
    /// Idle timeout before auto-suspend
    pub idle_timeout: Duration,
    /// Maximum number of tasks per session
    pub max_tasks: usize,
    /// Default security ring for the session
    pub default_ring: SecurityRing,
    /// Whether to persist session state
    pub persistent: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_duration: Duration::from_secs(3600), // 1 hour
            idle_timeout: Duration::from_secs(300),   // 5 minutes
            max_tasks: 1000,
            default_ring: SecurityRing::User,
            persistent: false,
        }
    }
}

/// A single session instance
pub struct Session {
    /// Unique session identifier
    pub id: String,
    /// Session state
    pub state: SessionState,
    /// Security context
    pub security: SecurityContext,
    /// Session configuration
    pub config: SessionConfig,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_active: Instant,
    /// Number of tasks executed in this session
    pub task_count: usize,
    /// Session metadata
    pub metadata: HashMap<String, String>,
}

impl Session {
    /// Create a new session
    pub fn new(id: impl Into<String>, config: SessionConfig) -> Self {
        let id = id.into();
        let security = SecurityContext::new(&id, config.default_ring);
        Self {
            id,
            state: SessionState::Created,
            security,
            config,
            created_at: Utc::now(),
            last_active: Instant::now(),
            task_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// Activate the session
    pub fn activate(&mut self) -> Result<()> {
        match self.state {
            SessionState::Created | SessionState::Idle | SessionState::Suspended => {
                self.state = SessionState::Active;
                self.last_active = Instant::now();
                debug!(session_id = %self.id, "Session activated");
                Ok(())
            }
            SessionState::Active => Ok(()),
            SessionState::Terminated => anyhow::bail!("Cannot activate a terminated session"),
        }
    }

    /// Mark session as idle
    pub fn set_idle(&mut self) {
        if self.state == SessionState::Active {
            self.state = SessionState::Idle;
            self.last_active = Instant::now();
            debug!(session_id = %self.id, "Session set to idle");
        }
    }

    /// Suspend the session
    pub fn suspend(&mut self) -> Result<()> {
        match self.state {
            SessionState::Active | SessionState::Idle => {
                self.state = SessionState::Suspended;
                debug!(session_id = %self.id, "Session suspended");
                Ok(())
            }
            SessionState::Suspended => Ok(()),
            SessionState::Terminated => anyhow::bail!("Cannot suspend a terminated session"),
            SessionState::Created => anyhow::bail!("Cannot suspend a session that hasn't been activated"),
        }
    }

    /// Terminate the session
    pub fn terminate(&mut self) {
        self.state = SessionState::Terminated;
        info!(session_id = %self.id, tasks = self.task_count, "Session terminated");
    }

    /// Record a task execution
    pub fn record_task(&mut self) -> Result<()> {
        if self.task_count >= self.config.max_tasks {
            anyhow::bail!(
                "Session task limit reached: {}/{}",
                self.task_count,
                self.config.max_tasks
            );
        }
        self.task_count += 1;
        self.last_active = Instant::now();
        Ok(())
    }

    /// Check if the session has exceeded its maximum duration
    pub fn is_expired(&self) -> bool {
        self.last_active.elapsed() > self.config.max_duration
    }

    /// Check if the session has been idle too long
    pub fn is_idle_expired(&self) -> bool {
        self.last_active.elapsed() > self.config.idle_timeout
    }

    /// Set metadata key-value pair
    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get metadata value
    pub fn get_meta(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Manages all sessions with automatic cleanup
pub struct SessionManager {
    sessions: DashMap<String, Session>,
    config: SessionConfig,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(config: SessionConfig) -> Self {
        Self {
            sessions: DashMap::new(),
            config,
        }
    }

    /// Create a new session with auto-generated ID
    pub fn create(&self) -> String {
        let id = format!("sess_{}", Uuid::new_v4().to_string()[..8].to_string());
        let session = Session::new(&id, self.config.clone());
        self.sessions.insert(id.clone(), session);
        info!(session_id = %id, "Session created");
        id
    }

    /// Create a session with a specific ID
    pub fn create_with_id(&self, id: impl Into<String>) -> Result<String> {
        let id = id.into();
        if self.sessions.contains_key(&id) {
            anyhow::bail!("Session '{}' already exists", id);
        }
        let session = Session::new(&id, self.config.clone());
        self.sessions.insert(id.clone(), session);
        info!(session_id = %id, "Session created with specific ID");
        Ok(id)
    }

    /// Get a session by ID
    pub fn get(&self, id: &str) -> Option<Session> {
        self.sessions.get(id).map(|s| {
            let s = s.value();
            Session {
                id: s.id.clone(),
                state: s.state,
                security: SecurityContext {
                    ring: s.security.ring,
                    capabilities: s.security.capabilities.clone(),
                    session_id: s.security.session_id.clone(),
                    sandboxed: s.security.sandboxed,
                },
                config: s.config.clone(),
                created_at: s.created_at,
                last_active: s.last_active,
                task_count: s.task_count,
                metadata: s.metadata.clone(),
            }
        })
    }

    /// Activate a session
    pub fn activate(&self, id: &str) -> Result<()> {
        let mut session = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", id))?;
        session.activate()
    }

    /// Suspend a session
    pub fn suspend(&self, id: &str) -> Result<()> {
        let mut session = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", id))?;
        session.suspend()
    }

    /// Terminate a session
    pub fn terminate(&self, id: &str) -> Result<()> {
        let mut session = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", id))?;
        session.terminate();
        Ok(())
    }

    /// Record a task in a session
    pub fn record_task(&self, id: &str) -> Result<()> {
        let mut session = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", id))?;
        session.record_task()
    }

    /// Set session metadata
    pub fn set_meta(&self, id: &str, key: impl Into<String>, value: impl Into<String>) -> Result<()> {
        let mut session = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", id))?;
        session.set_meta(key, value);
        Ok(())
    }

    /// Clean up expired and idle sessions
    pub fn cleanup(&self) -> (usize, usize) {
        let mut expired = 0;
        let mut idle = 0;

        self.sessions.retain(|id, session| {
            if session.state == SessionState::Terminated {
                return false;
            }
            if session.is_expired() {
                warn!(session_id = %id, "Session expired, removing");
                expired += 1;
                return false;
            }
            if session.is_idle_expired() && session.state == SessionState::Idle {
                info!(session_id = %id, "Session idle timeout, suspending");
                session.state = SessionState::Suspended;
                idle += 1;
            }
            true
        });

        (expired, idle)
    }

    /// Get count of active sessions
    pub fn active_count(&self) -> usize {
        self.sessions
            .iter()
            .filter(|s| s.state == SessionState::Active)
            .count()
    }

    /// Get total session count
    pub fn total_count(&self) -> usize {
        self.sessions.len()
    }

    /// List all session IDs and their states
    pub fn list(&self) -> Vec<(String, SessionState)> {
        self.sessions
            .iter()
            .map(|s| (s.id.clone(), s.state))
            .collect()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new(SessionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle() {
        let mut session = Session::new("test-1", SessionConfig::default());
        assert_eq!(session.state, SessionState::Created);

        session.activate().unwrap();
        assert_eq!(session.state, SessionState::Active);

        session.set_idle();
        assert_eq!(session.state, SessionState::Idle);

        session.suspend().unwrap();
        assert_eq!(session.state, SessionState::Suspended);

        session.activate().unwrap();
        assert_eq!(session.state, SessionState::Active);

        session.terminate();
        assert_eq!(session.state, SessionState::Terminated);
    }

    #[test]
    fn test_session_terminate_cannot_activate() {
        let mut session = Session::new("test-2", SessionConfig::default());
        session.activate().unwrap();
        session.terminate();

        let result = session.activate();
        assert!(result.is_err());
    }

    #[test]
    fn test_session_task_recording() {
        let mut session = Session::new("test-3", SessionConfig {
            max_tasks: 3,
            ..Default::default()
        });

        session.record_task().unwrap();
        session.record_task().unwrap();
        session.record_task().unwrap();

        assert_eq!(session.task_count, 3);

        let result = session.record_task();
        assert!(result.is_err());
    }

    #[test]
    fn test_session_metadata() {
        let mut session = Session::new("test-4", SessionConfig::default());
        session.set_meta("agent", "claude");
        session.set_meta("model", "opus");

        assert_eq!(session.get_meta("agent"), Some(&"claude".to_string()));
        assert_eq!(session.get_meta("model"), Some(&"opus".to_string()));
        assert_eq!(session.get_meta("nonexistent"), None);
    }

    #[test]
    fn test_session_idle_timeout() {
        let mut session = Session::new("test-5", SessionConfig {
            idle_timeout: Duration::from_millis(10),
            ..Default::default()
        });
        session.activate().unwrap();
        session.set_idle();

        std::thread::sleep(Duration::from_millis(20));
        assert!(session.is_idle_expired());
    }

    #[test]
    fn test_session_manager_create_and_get() {
        let manager = SessionManager::default();
        let id = manager.create();

        let session = manager.get(&id).unwrap();
        assert_eq!(session.id, id);
        assert_eq!(session.state, SessionState::Created);
    }

    #[test]
    fn test_session_manager_create_with_id() {
        let manager = SessionManager::default();
        manager.create_with_id("custom-id").unwrap();

        let result = manager.create_with_id("custom-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_session_manager_lifecycle() {
        let manager = SessionManager::default();
        let id = manager.create();

        manager.activate(&id).unwrap();
        let session = manager.get(&id).unwrap();
        assert_eq!(session.state, SessionState::Active);

        manager.suspend(&id).unwrap();
        let session = manager.get(&id).unwrap();
        assert_eq!(session.state, SessionState::Suspended);

        manager.terminate(&id).unwrap();
        let session = manager.get(&id).unwrap();
        assert_eq!(session.state, SessionState::Terminated);
    }

    #[test]
    fn test_session_manager_cleanup() {
        let manager = SessionManager::new(SessionConfig {
            max_duration: Duration::from_millis(10),
            idle_timeout: Duration::from_millis(10),
            ..Default::default()
        });

        let id1 = manager.create();
        manager.activate(&id1).unwrap();
        manager.set_meta(&id1, "type", "expiring").unwrap();

        std::thread::sleep(Duration::from_millis(20));

        let (expired, idle) = manager.cleanup();
        assert!(expired >= 1);
    }

    #[test]
    fn test_session_manager_record_task() {
        let manager = SessionManager::default();
        let id = manager.create();

        manager.record_task(&id).unwrap();
        manager.record_task(&id).unwrap();

        let session = manager.get(&id).unwrap();
        assert_eq!(session.task_count, 2);
    }

    #[test]
    fn test_session_manager_list() {
        let manager = SessionManager::default();
        let id1 = manager.create();
        let id2 = manager.create();

        let list = manager.list();
        assert_eq!(list.len(), 2);

        let ids: Vec<&str> = list.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
    }

    #[test]
    fn test_session_manager_nonexistent() {
        let manager = SessionManager::default();
        assert!(manager.get("nonexistent").is_none());
        assert!(manager.activate("nonexistent").is_err());
        assert!(manager.terminate("nonexistent").is_err());
    }

    #[test]
    fn test_session_state_display() {
        assert_eq!(format!("{}", SessionState::Active), "Active");
        assert_eq!(format!("{}", SessionState::Terminated), "Terminated");
    }
}
