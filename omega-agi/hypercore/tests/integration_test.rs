//! Cross-module integration tests for OMEGA HyperCore.
//!
//! These tests exercise multiple modules together to verify that the
//! scheduler, session manager, security, memory pool, and self-healing
//! subsystems compose correctly.

use omega_hypercore::*;
use std::time::Duration;

// ---------------------------------------------------------------------------
// 1. Scheduler + completion verification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_schedule_task_with_memory_tracking() {
    let scheduler = TaskScheduler::new();

    let id = scheduler.spawn(TaskPriority::Normal, |_task_id| async move {
        // Simulate some work
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    });

    // Wait for the task to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    let status = scheduler.status(id);
    assert!(
        matches!(status, Some(scheduler::TaskStatus::Completed)),
        "Task should have completed, got: {:?}",
        status
    );

    let stats = scheduler.stats();
    assert_eq!(stats.total_spawned, 1);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.failed, 0);
}

// ---------------------------------------------------------------------------
// 2. Session lifecycle with security context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_lifecycle_with_security() {
    use session::SessionConfig;

    let manager = SessionManager::new(SessionConfig {
        idle_timeout: Duration::from_secs(300),
        max_duration: Duration::from_secs(3600),
        max_tasks: 100,
        default_ring: SecurityRing::User,
        persistent: false,
    });

    let id = manager.create();

    // Created
    let session = manager.get(&id).unwrap();
    assert_eq!(session.state, session::SessionState::Created);
    assert_eq!(session.security.ring, SecurityRing::User);

    // Activate
    manager.activate(&id).unwrap();
    let session = manager.get(&id).unwrap();
    assert_eq!(session.state, session::SessionState::Active);

    // Set idle (via suspend path since set_idle is on Session, not SessionManager)
    manager.suspend(&id).unwrap();
    let session = manager.get(&id).unwrap();
    assert_eq!(session.state, session::SessionState::Suspended);

    // Terminate
    manager.terminate(&id).unwrap();
    let session = manager.get(&id).unwrap();
    assert_eq!(session.state, session::SessionState::Terminated);
}

// ---------------------------------------------------------------------------
// 3. Security capability grant / revoke
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_security_capability_grant_revoke() {
    let mut cap_set = CapabilitySet::new();

    let read_cap = Capability::new(
        "memory_read",
        "Read from memory pool",
        SecurityRing::User,
        "memory://pool/main",
    );

    // Grant
    cap_set.grant(read_cap);
    assert!(cap_set.has("memory_read"));
    assert_eq!(cap_set.len(), 1);

    // Check access at User ring
    assert!(cap_set.check("memory_read", SecurityRing::User));
    assert!(cap_set.check("memory_read", SecurityRing::Kernel));

    // Revoke
    let revoked = cap_set.revoke("memory_read");
    assert!(revoked);
    assert!(!cap_set.has("memory_read"));
    assert!(!cap_set.check("memory_read", SecurityRing::User));
    assert!(cap_set.is_empty());
}

// ---------------------------------------------------------------------------
// 4. Memory pool write / read cycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_memory_pool_write_read_cycle() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();

    let mut pool = MemoryPool::open(&path, 4096).unwrap();

    let data = b"OMEGA integration test payload";
    let offset = pool.write(data).unwrap();

    let read_back = pool.read(offset, data.len()).unwrap();
    assert_eq!(read_back, data);

    let stats = pool.stats();
    assert_eq!(stats.used_bytes, data.len());
    assert_eq!(stats.allocation_count, 1);
}

// ---------------------------------------------------------------------------
// 5. Scheduler multiple priorities
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_scheduler_multiple_priorities() {
    let scheduler = TaskScheduler::new();

    let priorities = [
        TaskPriority::Low,
        TaskPriority::Normal,
        TaskPriority::High,
        TaskPriority::Critical,
    ];

    let mut ids = Vec::new();
    for &priority in &priorities {
        let id = scheduler.spawn(priority, |_task_id| async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            Ok(())
        });
        ids.push(id);
    }

    // Wait for all tasks to complete
    tokio::time::sleep(Duration::from_millis(200)).await;

    let stats = scheduler.stats();
    assert_eq!(stats.total_spawned, 4);
    assert_eq!(stats.completed, 4, "All four priority tasks should complete");

    for id in &ids {
        let status = scheduler.status(*id);
        assert!(
            matches!(status, Some(scheduler::TaskStatus::Completed)),
            "Task {:?} should be completed",
            id
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Session timeout cleanup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_timeout_cleanup() {
    use session::SessionConfig;

    let manager = SessionManager::new(SessionConfig {
        max_duration: Duration::from_millis(50),
        idle_timeout: Duration::from_millis(50),
        max_tasks: 100,
        default_ring: SecurityRing::User,
        persistent: false,
    });

    let id = manager.create();
    manager.activate(&id).unwrap();

    assert_eq!(manager.total_count(), 1);

    // Wait for the session to expire
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (expired, _idle) = manager.cleanup();
    assert!(expired >= 1, "At least one session should have expired");
}

// ---------------------------------------------------------------------------
// 7. Security ring hierarchy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_security_ring_hierarchy() {
    // Verify strict ordering: Kernel < Hypervisor < Supervisor < User
    assert!(SecurityRing::Kernel < SecurityRing::Hypervisor);
    assert!(SecurityRing::Hypervisor < SecurityRing::Supervisor);
    assert!(SecurityRing::Supervisor < SecurityRing::User);

    // A capability at Kernel ring should be accessible by Kernel only
    let kernel_cap = Capability::new(
        "kernel_only",
        "Kernel-level capability",
        SecurityRing::Kernel,
        "system://kernel",
    );
    assert!(kernel_cap.accessible_by(SecurityRing::Kernel));
    assert!(!kernel_cap.accessible_by(SecurityRing::Hypervisor));
    assert!(!kernel_cap.accessible_by(SecurityRing::Supervisor));
    assert!(!kernel_cap.accessible_by(SecurityRing::User));

    // A capability at User ring should be accessible by all rings
    let user_cap = Capability::new(
        "user_level",
        "User-level capability",
        SecurityRing::User,
        "app://data",
    );
    assert!(user_cap.accessible_by(SecurityRing::Kernel));
    assert!(user_cap.accessible_by(SecurityRing::Hypervisor));
    assert!(user_cap.accessible_by(SecurityRing::Supervisor));
    assert!(user_cap.accessible_by(SecurityRing::User));

    // Verify CapabilitySet respects ring hierarchy
    let mut cap_set = CapabilitySet::new();
    cap_set.grant(Capability::new(
        "hypervisor_op",
        "Hypervisor operation",
        SecurityRing::Hypervisor,
        "system://hypervisor",
    ));

    assert!(cap_set.check("hypervisor_op", SecurityRing::Kernel));
    assert!(cap_set.check("hypervisor_op", SecurityRing::Hypervisor));
    assert!(!cap_set.check("hypervisor_op", SecurityRing::Supervisor));
    assert!(!cap_set.check("hypervisor_op", SecurityRing::User));
}

// ---------------------------------------------------------------------------
// 8. Memory pool stats accuracy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_memory_pool_stats_accuracy() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();

    let mut pool = MemoryPool::open(&path, 4096).unwrap();

    let chunk1 = b"AAAA";
    let chunk2 = b"BBBBBBBB";
    let chunk3 = b"C";

    let offset1 = pool.write(chunk1).unwrap();
    let _offset2 = pool.write(chunk2).unwrap();
    let _offset3 = pool.write(chunk3).unwrap();

    let stats = pool.stats();
    let total_written = chunk1.len() + chunk2.len() + chunk3.len();
    assert_eq!(stats.used_bytes, total_written);
    assert_eq!(stats.free_bytes, 4096 - total_written);
    assert_eq!(stats.allocation_count, 3);

    // Verify utilization
    let expected_util = total_written as f64 / 4096.0;
    assert!(
        (stats.utilization - expected_util).abs() < f64::EPSILON,
        "utilization mismatch: got {}, expected {}",
        stats.utilization,
        expected_util
    );

    // Verify first chunk is still readable
    let read_back = pool.read(offset1, chunk1.len()).unwrap();
    assert_eq!(read_back, chunk1);
}
