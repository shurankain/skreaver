//! Secure tool registry with RBAC enforcement
//!
//! This module provides a wrapper around any `ToolRegistry` that enforces
//! role-based access control (RBAC) by checking security policies before
//! dispatching tool calls.

use super::{ExecutionResult, ToolCall, ToolRegistry};
use skreaver_core::collections::NonEmptyVec;
use skreaver_core::security::config::SecurityConfig;
use std::sync::Arc;

/// A secure tool registry wrapper that enforces RBAC policies
///
/// `SecureToolRegistry` wraps any `ToolRegistry` implementation and adds
/// permission checking before tool dispatch. It checks the security policy
/// for each tool and blocks execution if the tool is not allowed.
///
/// # Security Model
///
/// - Each tool call is checked against the security configuration
/// - Tools can be completely disabled via policy (fs_enabled, http_enabled, network_enabled)
/// - Failed permission checks return `ExecutionResult::Failure` with a clear error message
/// - The underlying registry is never called if permissions are denied
///
/// # Example
///
/// ```rust
/// use skreaver_tools::{InMemoryToolRegistry, SecureToolRegistry, ToolRegistry};
/// use skreaver_core::security::SecurityConfig;
/// use std::sync::Arc;
///
/// let registry = InMemoryToolRegistry::new();
/// let security_config = Arc::new(SecurityConfig::create_default());
/// let secure_registry = SecureToolRegistry::new(registry, security_config);
///
/// // Tool calls will now be checked against security policies
/// ```
#[derive(Clone)]
pub struct SecureToolRegistry<T: ToolRegistry> {
    inner: T,
    security_config: Arc<SecurityConfig>,
}

impl<T: ToolRegistry> SecureToolRegistry<T> {
    /// Create a new secure tool registry
    ///
    /// # Parameters
    ///
    /// * `inner` - The underlying tool registry to wrap
    /// * `security_config` - The security configuration containing RBAC policies
    ///
    /// # Returns
    ///
    /// A new `SecureToolRegistry` that enforces RBAC
    pub fn new(inner: T, security_config: Arc<SecurityConfig>) -> Self {
        Self {
            inner,
            security_config,
        }
    }

    /// Check if a tool is allowed to execute based on security policy
    ///
    /// This method checks the tool-specific security policy to determine
    /// if the tool should be allowed to execute. It checks:
    /// - File system access permissions
    /// - HTTP access permissions
    /// - Network access permissions
    ///
    /// # Parameters
    ///
    /// * `tool_name` - The name of the tool to check
    ///
    /// # Returns
    ///
    /// `Ok(())` if the tool is allowed, `Err(String)` with error message if denied
    fn check_permissions(&self, tool_name: &str) -> Result<(), String> {
        let policy = self.security_config.get_tool_policy(tool_name);

        // Check if the tool requires capabilities that are disabled
        // For now, we check if at least one capability is enabled
        // In the future, this could be more sophisticated based on tool requirements

        let has_any_capability = policy.fs_policy.enabled
            || policy.http_policy.enabled
            || policy.network_policy.enabled;

        if !has_any_capability {
            return Err(format!(
                "Permission denied: Tool '{}' is not allowed by security policy. \
                 All capabilities (filesystem, HTTP, network) are disabled.",
                tool_name
            ));
        }

        // Check for emergency lockdown mode
        if self.security_config.emergency.lockdown_enabled {
            let allowed_tools = &self.security_config.emergency.lockdown_allowed_tools;
            if !allowed_tools.contains(&tool_name.to_string()) {
                return Err(format!(
                    "Permission denied: System is in emergency lockdown mode. \
                     Tool '{}' is not in the allowed list.",
                    tool_name
                ));
            }
        }

        Ok(())
    }
}

impl<T: ToolRegistry> ToolRegistry for SecureToolRegistry<T> {
    fn dispatch(&self, call: ToolCall) -> Option<ExecutionResult> {
        // Check permissions before dispatching
        if let Err(error) = self.check_permissions(call.name()) {
            tracing::warn!(
                tool_name = call.name(),
                error = %error,
                "Tool execution blocked by RBAC policy"
            );

            // Record RBAC denial metric
            if let Some(registry) = skreaver_observability::get_metrics_registry() {
                registry
                    .core_metrics()
                    .security_rbac_checks_total
                    .with_label_values(&["denied", call.name()])
                    .inc();
            }

            return Some(ExecutionResult::Failure { error });
        }

        // Record RBAC allowed metric
        if let Some(registry) = skreaver_observability::get_metrics_registry() {
            registry
                .core_metrics()
                .security_rbac_checks_total
                .with_label_values(&["allowed", call.name()])
                .inc();
        }

        // Permissions OK - dispatch to inner registry
        self.inner.dispatch(call)
    }

    fn dispatch_ref(&self, call: &ToolCall) -> Option<ExecutionResult> {
        // Check permissions before dispatching
        if let Err(error) = self.check_permissions(call.name()) {
            tracing::warn!(
                tool_name = call.name(),
                error = %error,
                "Tool execution blocked by RBAC policy"
            );

            // Record RBAC denial metric
            if let Some(registry) = skreaver_observability::get_metrics_registry() {
                registry
                    .core_metrics()
                    .security_rbac_checks_total
                    .with_label_values(&["denied", call.name()])
                    .inc();
            }

            return Some(ExecutionResult::Failure { error });
        }

        // Record RBAC allowed metric
        if let Some(registry) = skreaver_observability::get_metrics_registry() {
            registry
                .core_metrics()
                .security_rbac_checks_total
                .with_label_values(&["allowed", call.name()])
                .inc();
        }

        // Permissions OK - dispatch to inner registry
        self.inner.dispatch_ref(call)
    }

    fn try_dispatch(&self, call: &ToolCall) -> Result<ExecutionResult, String> {
        // Check permissions before dispatching
        if let Err(error) = self.check_permissions(call.name()) {
            tracing::warn!(
                tool_name = call.name(),
                error = %error,
                "Tool execution blocked by RBAC policy"
            );

            // Record RBAC denial metric
            if let Some(registry) = skreaver_observability::get_metrics_registry() {
                registry
                    .core_metrics()
                    .security_rbac_checks_total
                    .with_label_values(&["denied", call.name()])
                    .inc();
            }

            return Ok(ExecutionResult::Failure { error });
        }

        // Record RBAC allowed metric
        if let Some(registry) = skreaver_observability::get_metrics_registry() {
            registry
                .core_metrics()
                .security_rbac_checks_total
                .with_label_values(&["allowed", call.name()])
                .inc();
        }

        // Permissions OK - dispatch to inner registry
        self.inner.try_dispatch(call)
    }

    fn dispatch_batch(&self, calls: &NonEmptyVec<ToolCall>) -> NonEmptyVec<ExecutionResult> {
        // Check permissions for each call and dispatch or return failure
        let head_result = if let Err(error) = self.check_permissions(calls.head().name()) {
            tracing::warn!(
                tool_name = calls.head().name(),
                error = %error,
                "Tool execution blocked by RBAC policy"
            );
            ExecutionResult::Failure { error }
        } else {
            self.inner
                .dispatch_ref(calls.head())
                .unwrap_or_else(|| ExecutionResult::Failure {
                    error: format!("Tool not found: {}", calls.head().name()),
                })
        };

        let tail_results: Vec<ExecutionResult> = calls
            .tail()
            .iter()
            .map(|call| {
                if let Err(error) = self.check_permissions(call.name()) {
                    tracing::warn!(
                        tool_name = call.name(),
                        error = %error,
                        "Tool execution blocked by RBAC policy"
                    );
                    ExecutionResult::Failure { error }
                } else {
                    self.inner
                        .dispatch_ref(call)
                        .unwrap_or_else(|| ExecutionResult::Failure {
                            error: format!("Tool not found: {}", call.name()),
                        })
                }
            })
            .collect();

        NonEmptyVec::new(head_result, tail_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InMemoryToolRegistry, Tool};
    use skreaver_core::security::policy::ToolPolicy;
    use std::collections::HashMap;

    struct TestTool;

    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test_tool"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult::Success {
                output: format!("Executed: {}", input),
            }
        }
    }

    #[test]
    fn test_secure_registry_allows_enabled_tools() {
        let registry = InMemoryToolRegistry::new().with_tool("test_tool", Arc::new(TestTool));

        let config = SecurityConfig::create_default();
        // Default config has filesystem and HTTP enabled
        let secure_registry = SecureToolRegistry::new(registry, Arc::new(config));

        let result = secure_registry.dispatch(
            ToolCall::new("test_tool", "hello").expect("Valid tool name"),
        );

        assert!(result.is_some());
        match result.unwrap() {
            ExecutionResult::Success { output } => {
                assert_eq!(output, "Executed: hello");
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn test_secure_registry_blocks_disabled_tools() {
        let registry = InMemoryToolRegistry::new().with_tool("blocked_tool", Arc::new(TestTool));

        let mut config = SecurityConfig::create_default();
        // Create a tool policy that disables all capabilities
        let mut tool_policies = HashMap::new();
        tool_policies.insert(
            "blocked_tool".to_string(),
            ToolPolicy {
                fs_enabled: Some(false),
                http_enabled: Some(false),
                network_enabled: Some(false),
                rate_limit_per_minute: None,
                additional_restrictions: HashMap::new(),
            },
        );
        config.tools = tool_policies;

        let secure_registry = SecureToolRegistry::new(registry, Arc::new(config));

        let result = secure_registry.dispatch(
            ToolCall::new("blocked_tool", "hello").expect("Valid tool name"),
        );

        assert!(result.is_some());
        match result.unwrap() {
            ExecutionResult::Failure { error } => {
                assert!(error.contains("Permission denied"));
                assert!(error.contains("blocked_tool"));
            }
            _ => panic!("Expected failure due to permissions"),
        }
    }

    #[test]
    fn test_secure_registry_enforces_lockdown_mode() {
        let registry = InMemoryToolRegistry::new()
            .with_tool("allowed_tool", Arc::new(TestTool))
            .with_tool("blocked_tool", Arc::new(TestTool));

        let mut config = SecurityConfig::create_default();
        config.emergency.lockdown_enabled = true;
        config.emergency.lockdown_allowed_tools = vec!["allowed_tool".to_string()];

        let secure_registry = SecureToolRegistry::new(registry, Arc::new(config));

        // Allowed tool should work
        let allowed_result = secure_registry.dispatch(
            ToolCall::new("allowed_tool", "hello").expect("Valid tool name"),
        );
        assert!(matches!(
            allowed_result,
            Some(ExecutionResult::Success { .. })
        ));

        // Blocked tool should fail
        let blocked_result = secure_registry.dispatch(
            ToolCall::new("blocked_tool", "hello").expect("Valid tool name"),
        );
        match blocked_result.unwrap() {
            ExecutionResult::Failure { error } => {
                assert!(error.contains("emergency lockdown"));
                assert!(error.contains("blocked_tool"));
            }
            _ => panic!("Expected failure due to lockdown"),
        }
    }

    #[test]
    fn test_secure_registry_batch_mixed_permissions() {
        let registry = InMemoryToolRegistry::new()
            .with_tool("allowed_tool", Arc::new(TestTool))
            .with_tool("blocked_tool", Arc::new(TestTool));

        let mut config = SecurityConfig::create_default();
        let mut tool_policies = HashMap::new();
        tool_policies.insert(
            "blocked_tool".to_string(),
            ToolPolicy {
                fs_enabled: Some(false),
                http_enabled: Some(false),
                network_enabled: Some(false),
                rate_limit_per_minute: None,
                additional_restrictions: HashMap::new(),
            },
        );
        config.tools = tool_policies;

        let secure_registry = SecureToolRegistry::new(registry, Arc::new(config));

        let calls = NonEmptyVec::new(
            ToolCall::new("allowed_tool", "hello").expect("Valid tool name"),
            vec![ToolCall::new("blocked_tool", "world").expect("Valid tool name")],
        );

        let results = secure_registry.dispatch_batch(&calls);

        // First should succeed
        assert!(matches!(results.head(), ExecutionResult::Success { .. }));

        // Second should fail due to permissions
        match &results.tail()[0] {
            ExecutionResult::Failure { error } => {
                assert!(error.contains("Permission denied"));
            }
            _ => panic!("Expected permission failure for blocked_tool"),
        }
    }
}
