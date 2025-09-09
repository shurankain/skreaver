//! Security framework for Skreaver agents and tools
//!
//! This module provides security controls, policy enforcement, and audit logging
//! to enable secure deployment of AI agents in production environments.

pub mod audit;
pub mod config;
pub mod errors;
pub mod limits;
pub mod policy;
pub mod secure_tool;
pub mod validation;

pub use audit::{SecurityAuditLog, SecurityEvent, SecurityResult};
pub use config::SecurityConfig;
pub use errors::{SecurityError, SecurityViolation};
pub use limits::{ResourceLimits, ResourceTracker, ResourceUsage};
pub use policy::{SecurityPolicy, ToolPolicy};
pub use secure_tool::{SecureTool, SecureToolExt, SecureToolFactory};
pub use validation::{DomainValidator, InputValidator, PathValidator};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Security context for operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    /// Unique session identifier
    pub session_id: Uuid,
    /// Agent identifier
    pub agent_id: String,
    /// Tool being used
    pub tool_name: String,
    /// User or service identity
    pub principal: Option<String>,
    /// Security policy to apply
    pub policy: SecurityPolicy,
    /// Resource limits for this context
    pub limits: ResourceLimits,
}

impl SecurityContext {
    pub fn new(agent_id: String, tool_name: String, policy: SecurityPolicy) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            agent_id,
            tool_name,
            principal: None,
            policy,
            limits: ResourceLimits::default(),
        }
    }

    pub fn with_principal(mut self, principal: String) -> Self {
        self.principal = Some(principal);
        self
    }

    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }
}

/// Security manager for coordinating security operations
pub struct SecurityManager {
    config: SecurityConfig,
    audit_log: audit::AuditLogger,
    resource_tracker: limits::ResourceTracker,
}

impl SecurityManager {
    pub fn new(config: SecurityConfig) -> Self {
        let audit_log = audit::AuditLogger::new(&config.audit);
        let resource_tracker = limits::ResourceTracker::new(&config.resources);

        Self {
            config,
            audit_log,
            resource_tracker,
        }
    }

    pub fn create_context(&self, agent_id: String, tool_name: String) -> SecurityContext {
        let policy = self.config.get_tool_policy(&tool_name);
        let limits = self.config.resources.clone();

        SecurityContext::new(agent_id, tool_name, policy).with_limits(limits)
    }

    pub fn validate_operation(
        &self,
        context: &SecurityContext,
        input: &str,
    ) -> Result<(), SecurityError> {
        // Resource limit check
        self.resource_tracker.check_limits(context)?;

        // Input validation
        let validator = InputValidator::new(&context.policy);
        validator.validate(input)?;

        // Log the validation attempt
        let event = SecurityEvent::ValidationAttempt {
            context: context.clone(),
            input_hash: self.hash_input(input),
            result: SecurityResult::Allowed,
        };
        self.audit_log.log_event(event);

        Ok(())
    }

    pub fn enforce_timeout<T>(
        &self,
        context: &SecurityContext,
        operation: impl std::future::Future<Output = T>,
    ) -> impl std::future::Future<Output = Result<T, SecurityError>> {
        let timeout = context.limits.max_execution_time;
        async move {
            tokio::time::timeout(timeout, operation).await.map_err(|_| {
                SecurityError::TimeoutExceeded {
                    timeout_ms: timeout.as_millis() as u64,
                }
            })
        }
    }

    fn hash_input(&self, input: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
