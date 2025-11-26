//! Security framework for Skreaver agents and tools
//!
//! This module provides security controls, policy enforcement, and audit logging
//! to enable secure deployment of AI agents in production environments.

#[cfg(feature = "security-audit")]
pub mod audit;
pub mod config;
pub mod errors;
#[cfg(feature = "security-basic")]
pub mod fs;
pub mod limits;
pub mod policy;
pub mod secret;
#[cfg(feature = "security-basic")]
pub mod secure_tool;
#[cfg(feature = "security-basic")]
pub mod validated_fd;
#[cfg(feature = "security-basic")]
pub mod validated_url;
#[cfg(feature = "security-basic")]
pub mod validation;

#[cfg(feature = "security-audit")]
pub use audit::{AuditLogger, SecurityAuditLog, SecurityEvent, SecurityResult};
pub use config::{
    AlertLevel, Alerting, AlertingConfig, Audit, AuditConfig, AutoRotate, Development,
    DevelopmentConfig, DevelopmentMode, Disabled, Emergency, EmergencyConfig, Enabled,
    EnvironmentOnly, FlexibleSources, LockdownActive, LockdownTrigger, LogAll, LogFormat, LogLevel,
    LogSelective, ManualRotate, NoRedaction, NoStackTraces, NormalOps, Production, RedactSecrets,
    Secret, SecretConfig, SecurityConfig, WithStackTraces,
};
pub use errors::{SecurityError, SecurityViolation};
#[cfg(feature = "security-basic")]
pub use fs::{SecureFileSystem, ValidatedPath};
pub use limits::{CpuPercent, ResourceLimits, ResourceTracker, ResourceUsage};
pub use policy::{
    ContentScanning, DomainFilter, FileCountLimit, FileSizeLimit, FileSystemAccess,
    FileSystemPolicy, HttpAccess, HttpAccessConfig, HttpPolicy, NetworkAccess, NetworkPolicy,
    NetworkPort, RedirectLimit, ResponseSizeLimit, SecurityPolicy, SymlinkBehavior, TimeoutSeconds,
    ToolSecurityPolicy,
};
#[cfg(feature = "security-basic")]
pub use validated_fd::ValidatedFileDescriptor;
// Re-export secret types - note: config::Secret is a different type (config marker)
pub use secret::{Secret as SecretValue, SecretBytes, SecretString};
#[cfg(feature = "security-basic")]
pub use secure_tool::{SecureTool, SecureToolExt, SecureToolFactory};
#[cfg(feature = "security-basic")]
pub use validated_url::ValidatedUrl;
#[cfg(feature = "security-basic")]
pub use validation::{DomainValidator, InputValidator, PathValidator};

use crate::identifiers::{AgentId, PrincipalId, SessionId, ToolId};
use serde::{Deserialize, Serialize};

/// Security context for operations
///
/// **Security**: As of v0.6.0, uses validated `PrincipalId` instead of raw `String`
/// to prevent injection attacks and audit log corruption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    /// Unique session identifier
    pub session_id: SessionId,
    /// Agent identifier
    pub agent_id: AgentId,
    /// Tool being used
    pub tool_name: ToolId,
    /// User or service identity (validated to prevent injection attacks)
    pub principal: Option<PrincipalId>,
    /// Security policy to apply
    pub policy: SecurityPolicy,
    /// Resource limits for this context
    pub limits: ResourceLimits,
}

impl SecurityContext {
    pub fn new(agent_id: AgentId, tool_name: ToolId, policy: SecurityPolicy) -> Self {
        Self {
            session_id: SessionId::generate(),
            agent_id,
            tool_name,
            principal: None,
            policy,
            limits: ResourceLimits::default(),
        }
    }

    /// Set the principal (user/service identity) for this security context
    ///
    /// # Security
    ///
    /// As of v0.6.0, requires a validated `PrincipalId` to prevent injection attacks.
    /// The principal is validated to block:
    /// - SQL injection patterns
    /// - Path traversal sequences
    /// - Shell metacharacters
    /// - LDAP injection attempts
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_core::{SecurityContext, PrincipalId, AgentId, ToolId, SecurityPolicy};
    ///
    /// let policy = SecurityPolicy {
    ///     fs_policy: Default::default(),
    ///     http_policy: Default::default(),
    ///     network_policy: Default::default(),
    /// };
    ///
    /// let context = SecurityContext::new(
    ///     AgentId::new_unchecked("agent-1"),
    ///     ToolId::new_unchecked("calculator"),
    ///     policy
    /// ).with_principal(PrincipalId::new_unchecked("alice@example.com"));
    /// ```
    pub fn with_principal(mut self, principal: PrincipalId) -> Self {
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
    #[cfg(feature = "security-audit")]
    audit_log: audit::AuditLogger,
    resource_tracker: limits::ResourceTracker,
}

impl SecurityManager {
    pub fn new(config: SecurityConfig) -> Self {
        #[cfg(feature = "security-audit")]
        let audit_log = audit::AuditLogger::new(&config.audit);
        let resource_tracker = limits::ResourceTracker::new(&config.resources);

        Self {
            config,
            #[cfg(feature = "security-audit")]
            audit_log,
            resource_tracker,
        }
    }

    pub fn create_context(&self, agent_id: AgentId, tool_name: ToolId) -> SecurityContext {
        let policy = self.config.get_tool_policy(tool_name.as_str());
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
        #[cfg(feature = "security-basic")]
        {
            let validator = validation::InputValidator::new(&context.policy);
            validator.validate(input)?;
        }

        // Log the validation attempt
        #[cfg(feature = "security-audit")]
        {
            let event = audit::SecurityEvent::ValidationAttempt {
                context: context.clone(),
                input_hash: self.hash_input(input),
                result: audit::SecurityResult::Allowed,
            };
            self.audit_log.log_event(event);
        }

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

    #[cfg(feature = "security-audit")]
    fn hash_input(&self, input: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
