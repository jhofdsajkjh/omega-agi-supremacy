//! # Actor System
//!
//! A lightweight actor system built on top of tokio channels and omega-hypercore scheduling.
//! Each actor has a unique ID, a mailbox, and processes messages asynchronously.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn};

use omega_hypercore::scheduler::TaskScheduler;

/// Unique actor identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(u64);

impl ActorId {
    /// Generate a new unique actor ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        ActorId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl std::fmt::Display for ActorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Actor#{}", self.0)
    }
}

/// A message that can be sent between actors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Source actor ID.
    pub from: ActorId,
    /// Target actor ID.
    pub to: ActorId,
    /// Message type identifier.
    pub msg_type: String,
    /// Message payload (JSON-serialized data).
    pub payload: Vec<u8>,
    /// Timestamp when the message was created.
    pub timestamp: DateTime<Utc>,
}

impl Message {
    /// Create a new message.
    pub fn new(from: ActorId, to: ActorId, msg_type: impl Into<String>, payload: Vec<u8>) -> Self {
        Self {
            from,
            to,
            msg_type: msg_type.into(),
            payload,
            timestamp: Utc::now(),
        }
    }

    /// Deserialize the payload into a typed value.
    pub fn decode<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.payload)
            .context("Failed to deserialize message payload")
    }

    /// Create a message with a typed payload.
    pub fn with_payload<T: Serialize>(
        from: ActorId,
        to: ActorId,
        msg_type: impl Into<String>,
        payload: &T,
    ) -> Result<Self> {
        let data = serde_json::to_vec(payload).context("Failed to serialize message payload")?;
        Ok(Self::new(from, to, msg_type, data))
    }
}

/// Statistics about an actor.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActorStats {
    pub messages_received: u64,
    pub messages_sent: u64,
    pub errors: u64,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

/// A reference to an actor that can be used to send messages.
#[derive(Clone)]
pub struct ActorRef {
    id: ActorId,
    sender: mpsc::UnboundedSender<Message>,
    stats: Arc<RwLock<ActorStats>>,
}

impl ActorRef {
    /// Get the actor's ID.
    pub fn id(&self) -> ActorId {
        self.id
    }

    /// Send a message to this actor.
    pub fn send(&self, msg: Message) -> Result<()> {
        self.sender
            .send(msg)
            .context("Failed to send message to actor (mailbox closed)")?;
        let mut stats = self.stats.write();
        stats.messages_sent += 1;
        stats.last_active = Utc::now();
        Ok(())
    }

    /// Get the actor's current statistics.
    pub fn stats(&self) -> ActorStats {
        self.stats.read().clone()
    }
}

/// Trait that all actors must implement.
#[async_trait::async_trait]
pub trait Actor: Send + Sync + 'static {
    /// Called when the actor receives a message.
    async fn handle(&mut self, msg: Message, ctx: &ActorContext) -> Result<()>;

    /// Called when the actor is started. Override to perform initialization.
    async fn on_start(&mut self, _ctx: &ActorContext) -> Result<()> {
        Ok(())
    }

    /// Called when the actor is stopped. Override to perform cleanup.
    async fn on_stop(&mut self, _ctx: &ActorContext) -> Result<()> {
        Ok(())
    }
}

/// Context provided to actors during message handling.
pub struct ActorContext {
    /// Reference to the actor system for spawning new actors or looking up refs.
    pub system: ActorSystemRef,
    /// This actor's ID.
    pub id: ActorId,
}

/// Internal reference to the actor system shared with actors.
#[derive(Clone)]
pub struct ActorSystemRef {
    actors: Arc<DashMap<ActorId, ActorRef>>,
    scheduler: Arc<TaskScheduler>,
}

impl ActorSystemRef {
    /// Look up an actor by ID and send it a message.
    pub fn send_to(&self, target: ActorId, msg: Message) -> Result<()> {
        let actor = self
            .actors
            .get(&target)
            .context(format!("Actor {} not found", target))?;
        actor.send(msg)
    }

    /// Check if an actor exists.
    pub fn exists(&self, id: ActorId) -> bool {
        self.actors.contains_key(&id)
    }
}

/// The actor system that manages actor lifecycle and message routing.
pub struct ActorSystem {
    actors: Arc<DashMap<ActorId, ActorRef>>,
    scheduler: Arc<TaskScheduler>,
    shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
}

impl ActorSystem {
    /// Create a new actor system.
    pub fn new() -> Self {
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);
        Self {
            actors: Arc::new(DashMap::new()),
            scheduler: Arc::new(TaskScheduler::new()),
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Get a reference to the system for passing to actors.
    pub fn system_ref(&self) -> ActorSystemRef {
        ActorSystemRef {
            actors: self.actors.clone(),
            scheduler: self.scheduler.clone(),
        }
    }

    /// Spawn a new actor and return its ActorRef.
    pub fn spawn<A: Actor>(&self, mut actor: A) -> Result<ActorRef> {
        let id = ActorId::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

        let stats = Arc::new(RwLock::new(ActorStats {
            created_at: Utc::now(),
            last_active: Utc::now(),
            ..Default::default()
        }));

        let actor_ref = ActorRef {
            id,
            sender: tx,
            stats: stats.clone(),
        };

        let ctx = ActorContext {
            system: self.system_ref(),
            id,
        };

        // Call on_start
        let start_stats = stats.clone();
        let _start_handle = tokio::spawn(async move {
            if let Err(e) = actor.on_start(&ctx).await {
                warn!(actor_id = %id, error = %e, "Actor on_start failed");
                let mut s = start_stats.write();
                s.errors += 1;
            }
            // Process messages
            while let Some(msg) = rx.recv().await {
                {
                    let mut s = stats.write();
                    s.messages_received += 1;
                    s.last_active = Utc::now();
                } // Drop guard before await

                if let Err(e) = actor.handle(msg, &ctx).await {
                    let mut s = stats.write();
                    s.errors += 1;
                    warn!(actor_id = %id, error = %e, "Actor message handling failed");
                }
            }
            // on_stop is called when the channel is closed
            let _ = actor.on_stop(&ctx).await;
        });

        self.actors.insert(id, actor_ref.clone());
        info!(actor_id = %id, "Actor spawned");
        Ok(actor_ref)
    }

    /// Look up an actor by ID.
    pub fn get(&self, id: ActorId) -> Option<ActorRef> {
        self.actors.get(&id).map(|r| r.value().clone())
    }

    /// Stop an actor by closing its mailbox.
    pub fn stop(&self, id: ActorId) -> bool {
        if let Some((_, actor)) = self.actors.remove(&id) {
            info!(actor_id = %id, "Actor stopped");
            true
        } else {
            false
        }
    }

    /// Get the number of registered actors.
    pub fn actor_count(&self) -> usize {
        self.actors.len()
    }

    /// Shut down all actors.
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
        }
        // Remove all actors, which closes their mailboxes
        self.actors.clear();
        self.scheduler.shutdown().await;
        info!("Actor system shutdown complete");
    }
}

impl Default for ActorSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple echo actor that replies to every message.
    struct EchoActor {
        reply_to: Option<ActorId>,
    }

    #[async_trait::async_trait]
    impl Actor for EchoActor {
        async fn handle(&mut self, msg: Message, _ctx: &ActorContext) -> Result<()> {
            if let Some(reply_to) = self.reply_to {
                let reply = Message::new(msg.to, reply_to, "echo_reply", msg.payload.clone());
                _ctx.system.send_to(reply_to, reply)?;
            }
            Ok(())
        }
    }

    /// A collector actor that stores received messages.
    struct CollectorActor {
        collected: Arc<RwLock<Vec<Message>>>,
    }

    impl CollectorActor {
        fn new(collected: Arc<RwLock<Vec<Message>>>) -> Self {
            Self { collected }
        }
    }

    #[async_trait::async_trait]
    impl Actor for CollectorActor {
        async fn handle(&mut self, msg: Message, _ctx: &ActorContext) -> Result<()> {
            self.collected.write().push(msg);
            Ok(())
        }
    }

    /// A counter actor that counts messages.
    struct CounterActor {
        count: u64,
    }

    impl CounterActor {
        fn new() -> Self {
            Self { count: 0 }
        }
    }

    #[async_trait::async_trait]
    impl Actor for CounterActor {
        async fn handle(&mut self, _msg: Message, _ctx: &ActorContext) -> Result<()> {
            self.count += 1;
            Ok(())
        }
    }

    #[test]
    fn test_actor_id_unique() {
        let id1 = ActorId::new();
        let id2 = ActorId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_actor_id_display() {
        let id = ActorId::new();
        let display = format!("{}", id);
        assert!(display.starts_with("Actor#"));
    }

    #[test]
    fn test_message_creation() {
        let from = ActorId::new();
        let to = ActorId::new();
        let msg = Message::new(from, to, "test", b"hello".to_vec());
        assert_eq!(msg.from, from);
        assert_eq!(msg.to, to);
        assert_eq!(msg.msg_type, "test");
        assert_eq!(msg.payload, b"hello");
    }

    #[test]
    fn test_message_with_payload() {
        let from = ActorId::new();
        let to = ActorId::new();
        let data = vec!["alpha", "beta"];
        let msg = Message::with_payload(from, to, "list", &data).unwrap();
        let decoded: Vec<String> = msg.decode().unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_message_decode_invalid() {
        let from = ActorId::new();
        let to = ActorId::new();
        let msg = Message::new(from, to, "bad", b"not json".to_vec());
        let result: Result<String> = msg.decode();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_spawn_actor() {
        let system = ActorSystem::new();
        let ref_ = system.spawn(EchoActor { reply_to: None }).unwrap();
        assert_eq!(system.actor_count(), 1);
        assert_eq!(ref_.id(), ref_.id());
    }

    #[tokio::test]
    async fn test_send_message() {
        let system = ActorSystem::new();
        let collected = Arc::new(RwLock::new(Vec::new()));
        let ref_ = system.spawn(CollectorActor::new(collected.clone())).unwrap();

        let msg = Message::new(ref_.id(), ref_.id(), "test", b"data".to_vec());
        ref_.send(msg).unwrap();

        // Give the actor time to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let items = collected.read();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].msg_type, "test");
    }

    #[tokio::test]
    async fn test_stop_actor() {
        let system = ActorSystem::new();
        let ref_ = system.spawn(EchoActor { reply_to: None }).unwrap();
        assert_eq!(system.actor_count(), 1);

        let stopped = system.stop(ref_.id());
        assert!(stopped);
        assert_eq!(system.actor_count(), 0);
    }

    #[tokio::test]
    async fn test_actor_stats() {
        let system = ActorSystem::new();
        let ref_ = system.spawn(EchoActor { reply_to: None }).unwrap();

        let msg = Message::new(ref_.id(), ref_.id(), "ping", b"".to_vec());
        ref_.send(msg).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let stats = ref_.stats();
        assert!(stats.messages_sent >= 1);
    }

    #[tokio::test]
    async fn test_actor_system_shutdown() {
        let mut system = ActorSystem::new();
        system.spawn(EchoActor { reply_to: None }).unwrap();
        system.spawn(EchoActor { reply_to: None }).unwrap();

        system.shutdown().await;
        assert_eq!(system.actor_count(), 0);
    }

    #[tokio::test]
    async fn test_send_to_nonexistent_actor() {
        let system = ActorSystem::new();
        let fake_id = ActorId::new();
        let ctx = system.system_ref();
        let msg = Message::new(fake_id, fake_id, "test", b"".to_vec());
        let result = ctx.send_to(fake_id, msg);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_actor_system_ref_exists() {
        let system = ActorSystem::new();
        let ref_ = system.spawn(EchoActor { reply_to: None }).unwrap();
        let ctx = system.system_ref();
        assert!(ctx.exists(ref_.id()));
        assert!(!ctx.exists(ActorId::new()));
    }
}
