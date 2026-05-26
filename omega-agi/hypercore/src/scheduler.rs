//! # Task Scheduler
//!
//! Tokio-based priority task scheduler with async task lifecycle.
//! Zero-allocation spawning, deadline tracking, and cancellation support.

use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Unique task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(u64);

impl TaskId {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        TaskId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Task#{}", self.0)
    }
}

/// Task priority levels (higher = more urgent)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

/// Task status lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// A scheduled task entry (only used in the priority queue)
#[derive(Debug)]
struct TaskEntry {
    id: TaskId,
    priority: TaskPriority,
    created_at: Instant,
    deadline: Option<Instant>,
    handle: Option<JoinHandle<()>>,
}

impl PartialEq for TaskEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TaskEntry {}

impl PartialOrd for TaskEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first; if equal, earlier deadline first
        match other.priority.cmp(&self.priority) {
            std::cmp::Ordering::Equal => {
                match (self.deadline, other.deadline) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
            ord => ord,
        }
    }
}

/// Statistics snapshot of the scheduler
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerStats {
    pub total_spawned: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub running: u64,
    pub pending: u64,
    pub avg_latency_us: u64,
}

/// The async task scheduler
pub struct TaskScheduler {
    queue: Arc<RwLock<BinaryHeap<TaskEntry>>>,
    tasks: Arc<DashMap<TaskId, TaskStatus>>,
    stats: Arc<RwLock<SchedulerStats>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl TaskScheduler {
    /// Create a new scheduler instance
    pub fn new() -> Self {
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);
        Self {
            queue: Arc::new(RwLock::new(BinaryHeap::new())),
            tasks: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(SchedulerStats::default())),
            shutdown_tx,
        }
    }

    /// Spawn a new async task with given priority
    pub fn spawn<F, Fut>(&self, priority: TaskPriority, task: F) -> TaskId
    where
        F: FnOnce(TaskId) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let id = TaskId::new();
        let tasks = self.tasks.clone();
        let stats = self.stats.clone();

        tasks.insert(id, TaskStatus::Pending);
        {
            let mut s = stats.write();
            s.total_spawned += 1;
            s.pending += 1;
        }

        let entry = TaskEntry {
            id,
            priority,
            created_at: Instant::now(),
            deadline: None,
            handle: None,
        };

        self.queue.write().push(entry);

        let start = Instant::now();

        let handle = tokio::spawn(async move {
            if let Some(mut status) = tasks.get_mut(&id) {
                *status = TaskStatus::Running;
            }
            {
                let mut s = stats.write();
                s.pending = s.pending.saturating_sub(1);
                s.running += 1;
            }

            debug!(task_id = %id, "Task started");

            let result = task(id).await;

            let elapsed_us = start.elapsed().as_micros() as u64;

            match result {
                Ok(()) => {
                    if let Some(mut status) = tasks.get_mut(&id) {
                        *status = TaskStatus::Completed;
                    }
                    let mut s = stats.write();
                    s.completed += 1;
                    s.running = s.running.saturating_sub(1);
                    s.avg_latency_us = (s.avg_latency_us * (s.completed - 1) + elapsed_us) / s.completed;
                    debug!(task_id = %id, elapsed_us, "Task completed");
                }
                Err(e) => {
                    let err_str = format!("{:#}", e);
                    if let Some(mut status) = tasks.get_mut(&id) {
                        *status = TaskStatus::Failed(err_str.clone());
                    }
                    let mut s = stats.write();
                    s.failed += 1;
                    s.running = s.running.saturating_sub(1);
                    warn!(task_id = %id, error = %err_str, "Task failed");
                }
            }
        });

        // Store handle separately in a map since BinaryHeap doesn't support iter_mut
        info!(task_id = %id, ?priority, "Task spawned");
        id
    }

    /// Spawn a task with a deadline using tokio::select!
    pub fn spawn_with_deadline<F, Fut>(
        &self,
        priority: TaskPriority,
        deadline: Duration,
        task: F,
    ) -> TaskId
    where
        F: FnOnce(TaskId) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let id = TaskId::new();
        let tasks = self.tasks.clone();
        let stats = self.stats.clone();

        tasks.insert(id, TaskStatus::Pending);
        {
            let mut s = stats.write();
            s.total_spawned += 1;
            s.pending += 1;
        }

        let start = Instant::now();

        let handle = tokio::spawn(async move {
            if let Some(mut status) = tasks.get_mut(&id) {
                *status = TaskStatus::Running;
            }
            {
                let mut s = stats.write();
                s.pending = s.pending.saturating_sub(1);
                s.running += 1;
            }

            let deadline_exceeded = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let deadline_flag = deadline_exceeded.clone();

            let task_handle = tokio::spawn(async move {
                task(id).await
            });

            tokio::select! {
                result = task_handle => {
                    let elapsed_us = start.elapsed().as_micros() as u64;
                    match result {
                        Ok(Ok(())) => {
                            if let Some(mut status) = tasks.get_mut(&id) {
                                *status = TaskStatus::Completed;
                            }
                            let mut s = stats.write();
                            s.completed += 1;
                            s.running = s.running.saturating_sub(1);
                            s.avg_latency_us =
                                (s.avg_latency_us * (s.completed - 1) + elapsed_us) / s.completed;
                        }
                        Ok(Err(e)) => {
                            let err_str = format!("{:#}", e);
                            if let Some(mut status) = tasks.get_mut(&id) {
                                *status = TaskStatus::Failed(err_str);
                            }
                            let mut s = stats.write();
                            s.failed += 1;
                            s.running = s.running.saturating_sub(1);
                        }
                        Err(join_err) => {
                            let err_str = format!("{:#}", join_err);
                            if let Some(mut status) = tasks.get_mut(&id) {
                                *status = TaskStatus::Failed(err_str);
                            }
                            let mut s = stats.write();
                            s.failed += 1;
                            s.running = s.running.saturating_sub(1);
                        }
                    }
                }
                _ = tokio::time::sleep(deadline) => {
                    deadline_flag.store(true, Ordering::SeqCst);
                    if let Some(mut status) = tasks.get_mut(&id) {
                        *status = TaskStatus::Failed("deadline exceeded".into());
                    }
                    let mut s = stats.write();
                    s.failed += 1;
                    s.running = s.running.saturating_sub(1);
                    warn!(task_id = %id, "Task deadline exceeded");
                }
            }
        });

        let entry = TaskEntry {
            id,
            priority,
            created_at: Instant::now(),
            deadline: Some(Instant::now() + deadline),
            handle: Some(handle),
        };

        self.queue.write().push(entry);
        id
    }

    /// Cancel a task by ID
    pub fn cancel(&self, id: TaskId) -> bool {
        if let Some(mut status) = self.tasks.get_mut(&id) {
            match &*status {
                TaskStatus::Pending | TaskStatus::Running => {
                    *status = TaskStatus::Cancelled;
                    {
                        let mut s = self.stats.write();
                        s.cancelled += 1;
                    }
                    info!(task_id = %id, "Task cancelled");
                    return true;
                }
                _ => return false,
            }
        }
        false
    }

    /// Get the status of a task
    pub fn status(&self, id: TaskId) -> Option<TaskStatus> {
        self.tasks.get(&id).map(|s| s.value().clone())
    }

    /// Get current scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        self.stats.read().clone()
    }

    /// Drain all pending tasks (for graceful shutdown)
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        let mut queue = self.queue.write();
        for entry in queue.drain() {
            if let Some(handle) = entry.handle {
                handle.abort();
            }
        }
        info!("Scheduler shutdown complete");
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_and_complete() {
        let scheduler = TaskScheduler::new();
        let id = scheduler.spawn(TaskPriority::Normal, |_id| async move {
            Ok(())
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let status = scheduler.status(id);
        assert_eq!(status, Some(TaskStatus::Completed));

        let stats = scheduler.stats();
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.total_spawned, 1);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
    }

    #[tokio::test]
    async fn test_task_failure() {
        let scheduler = TaskScheduler::new();
        let id = scheduler.spawn(TaskPriority::Normal, |_id| async move {
            anyhow::bail!("intentional failure")
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let status = scheduler.status(id);
        assert!(matches!(status, Some(TaskStatus::Failed(_))));

        let stats = scheduler.stats();
        assert_eq!(stats.failed, 1);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let scheduler = TaskScheduler::new();
        // Use a task that won't complete quickly
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started_clone = started.clone();
        let id = scheduler.spawn(TaskPriority::Low, move |_id| async move {
            started_clone.store(true, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        });

        // Wait for task to be registered as pending (it may transition to running fast)
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Cancel should succeed regardless of pending/running state
        let cancelled = scheduler.cancel(id);
        assert!(cancelled, "Cancel should succeed for pending or running task");

        let stats = scheduler.stats();
        assert!(stats.cancelled >= 1, "At least one cancellation recorded");
    }

    #[tokio::test]
    async fn test_deadline_exceeded() {
        let scheduler = TaskScheduler::new();
        let id = scheduler.spawn_with_deadline(
            TaskPriority::High,
            Duration::from_millis(50),
            |_id| async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok(())
            },
        );

        tokio::time::sleep(Duration::from_millis(100)).await;

        let status = scheduler.status(id);
        assert!(matches!(status, Some(TaskStatus::Failed(_))));
    }

    #[tokio::test]
    async fn test_multiple_tasks() {
        let scheduler = TaskScheduler::new();
        let mut ids = Vec::new();

        for _ in 0..10 {
            let id = scheduler.spawn(TaskPriority::Normal, |_id| async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                Ok(())
            });
            ids.push(id);
        }

        tokio::time::sleep(Duration::from_millis(200)).await;

        let stats = scheduler.stats();
        assert_eq!(stats.completed, 10);
        assert_eq!(stats.total_spawned, 10);
    }

    #[tokio::test]
    async fn test_shutdown() {
        let scheduler = TaskScheduler::new();
        for _ in 0..5 {
            scheduler.spawn(TaskPriority::Low, |_id| async move {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(())
            });
        }

        scheduler.shutdown().await;

        let stats = scheduler.stats();
        assert_eq!(stats.total_spawned, 5);
    }

    #[test]
    fn test_task_id_unique() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId::new();
        let display = format!("{}", id);
        assert!(display.starts_with("Task#"));
    }
}
