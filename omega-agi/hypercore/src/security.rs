//! # Capability-Based Security
//!
//! Ring-based security model with capability sets.
//! Inspired by seL4/CHERI capability hardware security.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Security ring levels (lower number = more privileged)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SecurityRing {
    /// Ring 0: Kernel-level access (full system control)
    Kernel = 0,
    /// Ring 1: Hypervisor-level (resource management)
    Hypervisor = 1,
    /// Ring 2: Supervisor-level (agent orchestration)
    Supervisor = 2,
    /// Ring 3: User-level (restricted task execution)
    User = 3,
}

impl Default for SecurityRing {
    fn default() -> Self {
        SecurityRing::User
    }
}

impl fmt::Display for SecurityRing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecurityRing::Kernel => write!(f, "Ring0::Kernel"),
            SecurityRing::Hypervisor => write!(f, "Ring1::Hypervisor"),
            SecurityRing::Supervisor => write!(f, "Ring2::Supervisor"),
            SecurityRing::User => write!(f, "Ring3::User"),
        }
    }
}

/// Individual capability token
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability {
    /// Unique capability identifier
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Minimum ring level required
    pub min_ring: SecurityRing,
    /// Resource scope (e.g., "memory://pool/main", "network://outbound")
    pub scope: String,
}

impl Capability {
    /// Create a new capability
    pub fn new(id: impl Into<String>, description: impl Into<String>, min_ring: SecurityRing, scope: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            min_ring,
            scope: scope.into(),
        }
    }

    /// Check if a given ring level can exercise this capability
    pub fn accessible_by(&self, ring: SecurityRing) -> bool {
        ring <= self.min_ring
    }
}

/// A set of capabilities with access control checks
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilitySet {
    capabilities: HashSet<Capability>,
}

impl CapabilitySet {
    /// Create an empty capability set
    pub fn new() -> Self {
        Self {
            capabilities: HashSet::new(),
        }
    }

    /// Add a capability to the set
    pub fn grant(&mut self, capability: Capability) {
        debug!(cap_id = %capability.id, scope = %capability.scope, "Capability granted");
        self.capabilities.insert(capability);
    }

    /// Remove a capability from the set
    pub fn revoke(&mut self, capability_id: &str) -> bool {
        let had = self.capabilities.iter().any(|c| c.id == capability_id);
        if had {
            self.capabilities.retain(|c| c.id != capability_id);
            info!(cap_id = capability_id, "Capability revoked");
        }
        had
    }

    /// Check if a specific capability is present
    pub fn has(&self, capability_id: &str) -> bool {
        self.capabilities.iter().any(|c| c.id == capability_id)
    }

    /// Check if a capability is accessible at a given ring level
    pub fn check(&self, capability_id: &str, ring: SecurityRing) -> bool {
        self.capabilities
            .iter()
            .any(|c| c.id == capability_id && c.accessible_by(ring))
    }

    /// Get all capabilities accessible at a given ring level
    pub fn accessible_at(&self, ring: SecurityRing) -> Vec<&Capability> {
        self.capabilities
            .iter()
            .filter(|c| c.accessible_by(ring))
            .collect()
    }

    /// Get all capabilities with a specific scope prefix
    pub fn by_scope(&self, scope_prefix: &str) -> Vec<&Capability> {
        self.capabilities
            .iter()
            .filter(|c| c.scope.starts_with(scope_prefix))
            .collect()
    }

    /// Get total number of capabilities
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Merge another capability set into this one
    pub fn merge(&mut self, other: &CapabilitySet) {
        for cap in &other.capabilities {
            self.capabilities.insert(cap.clone());
        }
    }
}

/// Security context for an agent or session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    /// Current ring level
    pub ring: SecurityRing,
    /// Granted capabilities
    pub capabilities: CapabilitySet,
    /// Session identifier
    pub session_id: String,
    /// Whether this context is sandboxed
    pub sandboxed: bool,
}

impl SecurityContext {
    /// Create a new security context
    pub fn new(session_id: impl Into<String>, ring: SecurityRing) -> Self {
        Self {
            ring,
            capabilities: CapabilitySet::new(),
            session_id: session_id.into(),
            sandboxed: ring >= SecurityRing::User,
        }
    }

    /// Create a kernel-level context (full access)
    pub fn kernel(session_id: impl Into<String>) -> Self {
        let mut ctx = Self::new(session_id, SecurityRing::Kernel);
        ctx.sandboxed = false;
        ctx
    }

    /// Check if a capability can be exercised
    pub fn can(&self, capability_id: &str) -> bool {
        self.capabilities.check(capability_id, self.ring)
    }

    /// Elevate to a higher privilege ring (requires capability check)
    pub fn elevate(&mut self, new_ring: SecurityRing) -> Result<(), String> {
        if new_ring < self.ring {
            // Elevating privilege (lower ring number = more privileged)
            warn!(
                from = %self.ring,
                to = %new_ring,
                session = %self.session_id,
                "Privilege elevation attempted"
            );
            self.ring = new_ring;
            self.sandboxed = new_ring >= SecurityRing::User;
            Ok(())
        } else {
            Err(format!(
                "Cannot elevate from {} to {} (would be demotion)",
                self.ring, new_ring
            ))
        }
    }

    /// Demote to a lower privilege ring
    pub fn demote(&mut self, new_ring: SecurityRing) -> Result<(), String> {
        if new_ring > self.ring {
            self.ring = new_ring;
            self.sandboxed = new_ring >= SecurityRing::User;
            info!(
                from = %self.ring,
                to = %new_ring,
                session = %self.session_id,
                "Privilege demoted"
            );
            Ok(())
        } else {
            Err(format!(
                "Cannot demote from {} to {} (would be elevation)",
                self.ring, new_ring
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cap(id: &str, ring: SecurityRing) -> Capability {
        Capability::new(id, format!("{} capability", id), ring, format!("test://{}", id))
    }

    #[test]
    fn test_capability_creation() {
        let cap = test_cap("read_memory", SecurityRing::Supervisor);
        assert_eq!(cap.id, "read_memory");
        assert!(cap.accessible_by(SecurityRing::Kernel));
        assert!(cap.accessible_by(SecurityRing::Supervisor));
        assert!(!cap.accessible_by(SecurityRing::User));
    }

    #[test]
    fn test_capability_set_grant_revoke() {
        let mut set = CapabilitySet::new();
        let cap = test_cap("write", SecurityRing::User);

        set.grant(cap);
        assert!(set.has("write"));
        assert_eq!(set.len(), 1);

        let revoked = set.revoke("write");
        assert!(revoked);
        assert!(!set.has("write"));
        assert!(set.is_empty());
    }

    #[test]
    fn test_capability_set_check() {
        let mut set = CapabilitySet::new();
        set.grant(test_cap("admin", SecurityRing::Kernel));
        set.grant(test_cap("read", SecurityRing::User));

        assert!(set.check("admin", SecurityRing::Kernel));
        assert!(!set.check("admin", SecurityRing::User));
        assert!(set.check("read", SecurityRing::User));
        assert!(set.check("read", SecurityRing::Kernel));
    }

    #[test]
    fn test_capability_set_by_scope() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::new("a", "", SecurityRing::User, "memory://pool/main"));
        set.grant(Capability::new("b", "", SecurityRing::User, "memory://pool/backup"));
        set.grant(Capability::new("c", "", SecurityRing::User, "network://outbound"));

        let memory_caps = set.by_scope("memory://");
        assert_eq!(memory_caps.len(), 2);
    }

    #[test]
    fn test_capability_set_merge() {
        let mut set_a = CapabilitySet::new();
        set_a.grant(test_cap("cap_a", SecurityRing::User));

        let mut set_b = CapabilitySet::new();
        set_b.grant(test_cap("cap_b", SecurityRing::User));

        set_a.merge(&set_b);
        assert!(set_a.has("cap_a"));
        assert!(set_a.has("cap_b"));
    }

    #[test]
    fn test_security_context_kernel() {
        let ctx = SecurityContext::kernel("test-session");
        assert_eq!(ctx.ring, SecurityRing::Kernel);
        assert!(!ctx.sandboxed);
    }

    #[test]
    fn test_security_context_can() {
        let mut ctx = SecurityContext::new("test", SecurityRing::User);
        ctx.capabilities.grant(test_cap("read", SecurityRing::User));
        ctx.capabilities.grant(test_cap("admin", SecurityRing::Kernel));

        assert!(ctx.can("read"));
        assert!(!ctx.can("admin"));
        assert!(!ctx.can("nonexistent"));
    }

    #[test]
    fn test_privilege_demote() {
        let mut ctx = SecurityContext::kernel("test");
        assert_eq!(ctx.ring, SecurityRing::Kernel);

        ctx.demote(SecurityRing::User).unwrap();
        assert_eq!(ctx.ring, SecurityRing::User);
        assert!(ctx.sandboxed);
    }

    #[test]
    fn test_privilege_elevate() {
        let mut ctx = SecurityContext::new("test", SecurityRing::User);
        ctx.elevate(SecurityRing::Kernel).unwrap();
        assert_eq!(ctx.ring, SecurityRing::Kernel);
        assert!(!ctx.sandboxed);
    }

    #[test]
    fn test_invalid_demote() {
        let mut ctx = SecurityContext::new("test", SecurityRing::User);
        let result = ctx.demote(SecurityRing::Kernel);
        assert!(result.is_err());
    }

    #[test]
    fn test_ring_ordering() {
        assert!(SecurityRing::Kernel < SecurityRing::Hypervisor);
        assert!(SecurityRing::Hypervisor < SecurityRing::Supervisor);
        assert!(SecurityRing::Supervisor < SecurityRing::User);
    }

    #[test]
    fn test_ring_display() {
        assert_eq!(format!("{}", SecurityRing::Kernel), "Ring0::Kernel");
        assert_eq!(format!("{}", SecurityRing::User), "Ring3::User");
    }
}
