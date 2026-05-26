//! # HyperCore Error Types
//!
//! Typed error enum for the OMEGA HyperCore runtime.
//! Provides structured error information for memory, security, session,
//! scheduling, and I/O failures.

/// Primary error type for the HyperCore runtime.
#[derive(Debug, thiserror::Error)]
pub enum HyperCoreError {
    /// Memory allocation failed due to insufficient space.
    #[error("memory exhausted: requested {requested} bytes, {available} bytes available")]
    MemoryExhausted {
        requested: usize,
        available: usize,
    },

    /// Capacity mismatch between expected and actual sizes.
    #[error("capacity mismatch: expected {expected}, got {actual}")]
    CapacityMismatch {
        expected: usize,
        actual: usize,
    },

    /// A security capability was exercised at an insufficient ring level.
    #[error(
        "security violation: capability '{}' requires ring {required_ring}, current ring is {current_ring}",
        capability
    )]
    SecurityViolation {
        capability: String,
        current_ring: u8,
        required_ring: u8,
    },

    /// The referenced session has expired.
    #[error("session expired: {session_id}")]
    SessionExpired { session_id: String },

    /// A task exceeded its deadline.
    #[error("deadline exceeded for task {task_id}: {deadline_ms}ms deadline")]
    DeadlineExceeded { task_id: u64, deadline_ms: u64 },

    /// The requested task was not found.
    #[error("task not found: {task_id}")]
    TaskNotFound { task_id: u64 },

    /// An invalid session state transition was attempted.
    #[error("invalid state transition for session '{session_id}': {from} -> {to}")]
    InvalidStateTransition {
        session_id: String,
        from: String,
        to: String,
    },

    /// A wrapped I/O error.
    #[error("I/O error: {0}")]
    IoError(String),
}

impl From<std::io::Error> for HyperCoreError {
    fn from(err: std::io::Error) -> Self {
        HyperCoreError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for HyperCoreError {
    fn from(err: serde_json::Error) -> Self {
        HyperCoreError::IoError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_exhausted_display() {
        let err = HyperCoreError::MemoryExhausted {
            requested: 1024,
            available: 512,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("1024"));
        assert!(msg.contains("512"));
        assert!(msg.contains("memory exhausted"));
    }

    #[test]
    fn test_capacity_mismatch_display() {
        let err = HyperCoreError::CapacityMismatch {
            expected: 4096,
            actual: 2048,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("4096"));
        assert!(msg.contains("2048"));
        assert!(msg.contains("capacity mismatch"));
    }

    #[test]
    fn test_security_violation_display() {
        let err = HyperCoreError::SecurityViolation {
            capability: "admin_control".to_string(),
            current_ring: 3,
            required_ring: 0,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("admin_control"));
        assert!(msg.contains("security violation"));
    }

    #[test]
    fn test_session_expired_display() {
        let err = HyperCoreError::SessionExpired {
            session_id: "sess_abc123".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("sess_abc123"));
        assert!(msg.contains("session expired"));
    }

    #[test]
    fn test_deadline_exceeded_display() {
        let err = HyperCoreError::DeadlineExceeded {
            task_id: 42,
            deadline_ms: 5000,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("42"));
        assert!(msg.contains("5000"));
        assert!(msg.contains("deadline exceeded"));
    }

    #[test]
    fn test_task_not_found_display() {
        let err = HyperCoreError::TaskNotFound { task_id: 99 };
        let msg = format!("{}", err);
        assert!(msg.contains("99"));
        assert!(msg.contains("task not found"));
    }

    #[test]
    fn test_invalid_state_transition_display() {
        let err = HyperCoreError::InvalidStateTransition {
            session_id: "sess_transition".to_string(),
            from: "Active".to_string(),
            to: "Created".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("sess_transition"));
        assert!(msg.contains("Active"));
        assert!(msg.contains("Created"));
        assert!(msg.contains("invalid state transition"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let hc_err: HyperCoreError = io_err.into();
        match hc_err {
            HyperCoreError::IoError(msg) => assert!(msg.contains("file not found")),
            other => panic!("expected IoError, got {:?}", other),
        }
    }
}
