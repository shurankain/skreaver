//! Security-aware tool wrapper
//!
//! This module provides a secure wrapper that can be applied to any tool to enforce
//! security policies and audit all operations.

use super::{SecurityContext, SecurityError, SecurityManager};
use crate::{ExecutionResult, Tool};
use std::sync::Arc;

/// A security-aware wrapper around any tool implementation
pub struct SecureTool<T: Tool> {
    inner: T,
    security_manager: Arc<SecurityManager>,
}

impl<T: Tool> SecureTool<T> {
    /// Create a new secure tool wrapper
    pub fn new(tool: T, security_manager: Arc<SecurityManager>) -> Self {
        Self {
            inner: tool,
            security_manager,
        }
    }

    /// Get a reference to the inner tool
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Execute a tool call with full security enforcement
    pub fn secure_call(&self, input: String, context: SecurityContext) -> ExecutionResult {
        // 1. Validate input against security policies
        if let Err(e) = self.security_manager.validate_operation(&context, &input) {
            let error_msg = format!("Security validation failed: {}", e);
            return ExecutionResult::failure(error_msg);
        }

        // 2. Execute the tool (simplified - timeout enforcement would be added later)
        let execution_result = self.inner.call(input);
        let result = Ok(execution_result);

        match result {
            Ok(execution_result) => {
                // 3. Scan output for sensitive data if needed
                if let ExecutionResult::Success { ref output } = execution_result
                    && let Err(scan_error) = self.scan_output_for_secrets(output)
                {
                    return ExecutionResult::failure(format!(
                        "Output security scan failed: {}",
                        scan_error
                    ));
                }

                execution_result
            }
            Err(SecurityError::TimeoutExceeded { timeout_ms }) => {
                ExecutionResult::failure(format!("Tool execution timed out after {}ms", timeout_ms))
            }
            Err(e) => ExecutionResult::failure(format!("Security error: {}", e)),
        }
    }

    fn scan_output_for_secrets(&self, output: &str) -> Result<(), SecurityError> {
        use super::validation::ContentScanner;

        let scanner = ContentScanner::new();
        let scan_result = scanner.scan_content(output.as_bytes()).map_err(|_| {
            SecurityError::ValidationFailed {
                reason: "Failed to scan output content".to_string(),
            }
        })?;

        if !scan_result.is_safe() {
            return Err(SecurityError::SecretInInput);
        }

        Ok(())
    }
}

impl<T: Tool> Tool for SecureTool<T> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn call(&self, input: String) -> ExecutionResult {
        // For the regular call method, we create a default security context
        let context = self.security_manager.create_context(
            crate::identifiers::AgentId::new_unchecked("default_agent"),
            crate::identifiers::ToolId::new_unchecked(self.inner.name()),
        );

        self.secure_call(input, context)
    }
}

/// Factory for creating secure versions of standard tools
pub struct SecureToolFactory {
    security_manager: Arc<SecurityManager>,
}

impl SecureToolFactory {
    pub fn new(security_manager: Arc<SecurityManager>) -> Self {
        Self { security_manager }
    }

    /// Wrap any tool with security enforcement
    pub fn secure<T: Tool>(&self, tool: T) -> SecureTool<T> {
        SecureTool::new(tool, Arc::clone(&self.security_manager))
    }

    // Note: Specific tool factory methods would be added here when the tools are integrated
    // For now, users can use the generic `secure()` method with any tool implementation
}

/// Trait for tools that can provide security-specific validation
pub trait SecureToolExt: Tool {
    /// Get the security policy requirements for this tool
    fn security_requirements(&self) -> super::policy::SecurityPolicy {
        // Default implementation - allow all operations
        super::policy::SecurityPolicy {
            fs_policy: super::policy::FileSystemPolicy::default(),
            http_policy: super::policy::HttpPolicy::default(),
            network_policy: super::policy::NetworkPolicy::default(),
        }
    }

    /// Validate tool-specific input constraints
    fn validate_input(&self, input: &str) -> Result<(), SecurityError> {
        // Default implementation - basic length check
        if input.len() > 100_000 {
            return Err(SecurityError::ValidationFailed {
                reason: "Input too long".to_string(),
            });
        }
        Ok(())
    }

    /// Post-process output to ensure it's safe
    fn sanitize_output(&self, output: String) -> String {
        use crate::sanitization::ContentSanitizer;
        // Use unified sanitizer to remove control chars and ANSI escapes
        ContentSanitizer::sanitize_output(&output)
    }
}

// Blanket implementation for all tools
impl<T: Tool> SecureToolExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::config::SecurityConfig;
    use std::sync::Arc;

    // Mock tool for testing
    struct MockTool {
        name: String,
        response: String,
    }

    impl MockTool {
        fn new(name: String, response: String) -> Self {
            Self { name, response }
        }
    }

    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn call(&self, _input: String) -> ExecutionResult {
            ExecutionResult::success(self.response.clone())
        }
    }

    #[test]
    fn test_secure_tool_wrapper() {
        let config = SecurityConfig::default();
        let manager = Arc::new(SecurityManager::new(config));
        let factory = SecureToolFactory::new(manager);

        let mock_tool = MockTool::new("test_tool".to_string(), "safe output".to_string());
        let secure_tool = factory.secure(mock_tool);

        // Test normal operation
        let result = secure_tool.call("safe input".to_string());
        assert!(result.is_success());
        if let ExecutionResult::Success { output } = result {
            assert_eq!(output, "safe output");
        }
    }

    #[test]
    fn test_secure_tool_blocks_secrets() {
        let config = SecurityConfig::default();
        let manager = Arc::new(SecurityManager::new(config));
        let factory = SecureToolFactory::new(manager);

        // Tool that returns a secret in output
        let mock_tool = MockTool::new(
            "test_tool".to_string(),
            "api_key=fake123test456mock".to_string(),
        );
        let secure_tool = factory.secure(mock_tool);

        let result = secure_tool.call("safe input".to_string());
        assert!(result.is_failure());
        if let ExecutionResult::Failure { reason } = result {
            assert!(reason.message().contains("security scan failed"));
        }
    }

    #[test]
    fn test_secure_tool_validates_input() {
        let config = SecurityConfig::default();
        let manager = Arc::new(SecurityManager::new(config));
        let factory = SecureToolFactory::new(manager);

        let mock_tool = MockTool::new("test_tool".to_string(), "output".to_string());
        let secure_tool = factory.secure(mock_tool);

        // Test with input containing secrets
        let result = secure_tool.call("api_key=fake123test456mock".to_string());
        assert!(result.is_failure());
        if let ExecutionResult::Failure { reason } = result {
            assert!(reason.message().contains("Security validation failed"));
        }
    }
}
