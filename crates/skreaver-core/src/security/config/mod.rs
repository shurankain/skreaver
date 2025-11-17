//! Security configuration loading and parsing
//!
//! This module provides comprehensive security configuration management with:
//! - Type-safe configuration using phantom types for compile-time guarantees
//! - Backward-compatible serialization/deserialization
//! - Configuration validation with fail-fast on critical errors
//! - Flexible policy enforcement for tools and operations

// Module declarations
pub mod alerts;
pub mod audit;
pub mod emergency;
pub mod logging;
pub mod secrets;
pub mod types;

// Re-export commonly used types for backward compatibility
pub use alerts::{AlertLevel, Alerting, AlertingConfig, LockdownTrigger};
pub use audit::{Audit, AuditConfig};
pub use emergency::{DevelopmentConfig, DevelopmentMode, Emergency, EmergencyConfig};
pub use logging::{LogFormat, LogLevel};
pub use secrets::{Secret, SecretConfig};
pub use types::{
    AutoRotate, Development, Disabled, Enabled, EnvironmentOnly, FlexibleSources, LockdownActive,
    LogAll, LogSelective, ManualRotate, NoRedaction, NoStackTraces, NormalOps, Production,
    RedactSecrets, WithStackTraces,
};

use super::errors::SecurityError;
use super::limits::ResourceLimits;
use super::policy::{
    DomainFilter, FileSystemAccess, FileSystemPolicy, HttpAccess, HttpPolicy, NetworkPolicy,
    SecurityPolicy, SymlinkBehavior, ToolSecurityPolicy,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Main security configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub metadata: ConfigMetadata,
    pub fs: FileSystemPolicy,
    pub http: HttpPolicy,
    pub network: NetworkPolicy,
    pub resources: ResourceLimits,
    pub audit: AuditConfig,
    pub secrets: SecretConfig,
    pub tools: HashMap<String, ToolSecurityPolicy>,
    pub alerting: AlertingConfig,
    pub development: DevelopmentConfig,
    pub emergency: EmergencyConfig,
}

/// Configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub version: String,
    pub created: String,
    pub description: String,
}

impl SecurityConfig {
    /// Load security configuration from TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, SecurityError> {
        let content = std::fs::read_to_string(path).map_err(|e| SecurityError::ConfigError {
            message: format!("Failed to read config file: {}", e),
        })?;

        Self::load_from_toml(&content)
    }

    /// Load security configuration from TOML string
    pub fn load_from_toml(toml_content: &str) -> Result<Self, SecurityError> {
        toml::from_str(toml_content).map_err(|e| SecurityError::ConfigError {
            message: format!("Failed to parse TOML config: {}", e),
        })
    }

    /// Load default security configuration
    pub fn create_default() -> Self {
        Self {
            metadata: ConfigMetadata {
                version: "0.1.0".to_string(),
                created: time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string()),
                description: "Default Skreaver security configuration".to_string(),
            },
            fs: FileSystemPolicy::default(),
            http: HttpPolicy::default(),
            network: NetworkPolicy::default(),
            resources: ResourceLimits::default(),
            audit: AuditConfig::default(),
            secrets: SecretConfig::default(),
            tools: HashMap::new(),
            alerting: AlertingConfig::default(),
            development: DevelopmentConfig::default(),
            emergency: EmergencyConfig::default(),
        }
    }

    /// Get security policy for a specific tool
    pub fn get_tool_policy(&self, tool_name: &str) -> SecurityPolicy {
        let tool_policy = self.tools.get(tool_name).cloned().unwrap_or_default();

        let fs_enabled = tool_policy
            .fs_enabled
            .unwrap_or(!matches!(self.fs.access, FileSystemAccess::Disabled));
        let http_enabled = tool_policy
            .http_enabled
            .unwrap_or(!matches!(self.http.access, HttpAccess::Disabled));

        SecurityPolicy {
            fs_policy: if fs_enabled {
                self.fs.clone()
            } else {
                FileSystemPolicy::disabled()
            },
            http_policy: if http_enabled {
                self.http.clone()
            } else {
                HttpPolicy::disabled()
            },
            network_policy: if tool_policy.network_enabled.unwrap_or(self.network.enabled) {
                self.network.clone()
            } else {
                NetworkPolicy::disabled()
            },
        }
    }

    /// Validate configuration for security issues (fail-fast on critical errors)
    pub fn validate(&self) -> Result<(), SecurityError> {
        // Check for development mode in production
        if self.development.enabled {
            tracing::warn!("Development mode is enabled - this should not be used in production");
        }

        // Validate resource limits (CRITICAL - must fail)
        if self.resources.max_memory_mb == 0 {
            return Err(SecurityError::ConfigError {
                message: "Memory limit cannot be zero".to_string(),
            });
        }

        if self.resources.max_execution_time.is_zero() {
            return Err(SecurityError::ConfigError {
                message: "Execution timeout cannot be zero".to_string(),
            });
        }

        if self.resources.max_concurrent_operations == 0 {
            return Err(SecurityError::ConfigError {
                message: "max_concurrent_operations must be > 0".to_string(),
            });
        }

        if self.resources.max_open_files == 0 {
            return Err(SecurityError::ConfigError {
                message: "max_open_files must be > 0".to_string(),
            });
        }

        // CPU percentage is now validated at the type level via CpuPercent

        // Validate file system policies (CRITICAL - must fail)
        if matches!(self.fs.access, FileSystemAccess::Enabled { .. })
            && self.fs.allow_paths.is_empty()
        {
            return Err(SecurityError::ConfigError {
                message: "File system enabled but no allowed paths configured (security risk)"
                    .to_string(),
            });
        }

        // Check for path traversal in allow_paths
        for path in &self.fs.allow_paths {
            if let Some(path_str) = path.to_str()
                && path_str.contains("..")
            {
                return Err(SecurityError::ConfigError {
                    message: format!(
                        "Allowed path '{}' contains '..' (path traversal risk)",
                        path.display()
                    ),
                });
            }
        }

        // Note: File size, timeout, and redirect limits are validated by their newtypes (FileSizeLimit, TimeoutSeconds, RedirectLimit)
        // during deserialization, so no additional validation is needed here.

        // Validate HTTP policies
        if let HttpAccess::Internet {
            domain_filter: DomainFilter::AllowList { allow_list, .. },
            ..
        } = &self.http.access
            && allow_list.is_empty()
            && !self.development.enabled
        {
            tracing::warn!("HTTP enabled with empty allow list (all domains will be blocked)");
        }

        // Check for overly permissive settings (WARNINGS)
        if let HttpAccess::Internet {
            include_local: true,
            ..
        } = &self.http.access
            && !self.development.enabled
        {
            tracing::warn!(
                "HTTP requests to localhost are allowed - this may be a security risk in production"
            );
        }

        if self.network.allow_private_networks && !self.development.enabled {
            tracing::warn!(
                "Network access to private IP ranges is allowed - this may be a security risk"
            );
        }

        if let FileSystemAccess::Enabled {
            symlink_behavior: SymlinkBehavior::Follow,
            ..
        } = &self.fs.access
        {
            tracing::warn!("Following symbolic links is enabled - this may be a security risk");
        }

        // Validate network policies
        if self.network.allow_ports.is_empty() && self.network.enabled {
            tracing::warn!("Network enabled but no allowed ports configured");
        }

        // Check for common dangerous ports in allow_ports
        let dangerous_ports = [22, 23, 3389]; // SSH, Telnet, RDP
        for network_port in &self.network.allow_ports {
            let port = network_port.port();
            if dangerous_ports.contains(&port) {
                tracing::warn!(
                    "Port {} (potentially dangerous) is in allow_ports list",
                    port
                );
            }
        }

        // Validate audit configuration
        if self.audit.retain_logs_days == 0 {
            tracing::warn!("Log retention is 0 days - logs will not be retained");
        }

        // Validate secrets configuration
        if self.secrets.min_secret_length < 16 {
            tracing::warn!(
                "min_secret_length is {} - recommendation is 32+ for production",
                self.secrets.min_secret_length
            );
        }

        // Validate alerting configuration
        if self.alerting.enabled
            && self.alerting.email_recipients.is_empty()
            && self.alerting.webhook_url.is_none()
        {
            tracing::warn!("Alerting enabled but no recipients or webhook configured");
        }

        if self.alerting.violation_threshold == 0 {
            return Err(SecurityError::ConfigError {
                message: "Alerting violation_threshold must be > 0".to_string(),
            });
        }

        // Validate emergency configuration
        if self.emergency.lockdown_enabled && self.emergency.lockdown_allowed_tools.is_empty() {
            tracing::warn!(
                "Emergency lockdown is active but no tools are allowed - system may be inoperable"
            );
        }

        Ok(())
    }

    /// Check if emergency lockdown is active
    pub fn is_lockdown_active(&self) -> bool {
        self.emergency.lockdown_enabled
    }

    /// Check if tool is allowed during lockdown
    pub fn is_tool_allowed_in_lockdown(&self, tool_name: &str) -> bool {
        if !self.is_lockdown_active() {
            return true;
        }

        self.emergency
            .lockdown_allowed_tools
            .contains(&tool_name.to_string())
    }

    /// Check if a specific lockdown trigger is enabled
    pub fn has_lockdown_trigger(&self, trigger: LockdownTrigger) -> bool {
        self.emergency.auto_lockdown_triggers.contains(&trigger)
    }

    /// Check if alert level should trigger notification
    pub fn should_alert(&self, level: AlertLevel) -> bool {
        self.alerting.enabled && self.alerting.alert_levels.contains(&level)
    }

    /// Get effective log level for tool operations
    pub fn get_log_level(&self) -> LogLevel {
        self.audit.log_level
    }

    /// Get effective log format for tool operations
    pub fn get_log_format(&self) -> LogFormat {
        self.audit.log_format
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self::create_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_load_from_toml_valid_config() {
        let toml_content = r#"
[metadata]
version = "0.1.0"
created = "2025-09-08"
description = "Test configuration"

[fs]
access = { Enabled = { symlink_behavior = "NoFollow", content_scanning = true } }
allow_paths = ["/tmp"]
deny_patterns = [".."]
max_file_size_bytes = 16777216
max_files_per_operation = 100

[http]
access = { Internet = { config = { timeout = 30, max_response_size = 33554432 }, domain_filter = { AllowList = { allow_list = ["example.com"], deny_list = ["localhost"] } }, include_local = false, max_redirects = 3, user_agent = "test-agent" } }
allow_methods = ["GET", "POST"]
default_headers = []

[network]
enabled = false
allow_ports = []
deny_ports = [22, 23]
ttl_seconds = 300
allow_private_networks = false

[resources]
max_memory_mb = 128
max_cpu_percent = 50
max_execution_time_seconds = 300
max_concurrent_operations = 10
max_open_files = 100
max_disk_usage_mb = 512

[audit]
log_all_operations = true
redact_secrets = true
secret_patterns = ["pattern1"]
retain_logs_days = 90
log_level = "INFO"
include_stack_traces = false
log_format = "structured"

[secrets]
environment_only = true
env_prefix = "TEST_"
auto_rotate = false
min_secret_length = 16

[alerting]
enabled = true
violation_threshold = 5
violation_window_minutes = 15
alert_levels = ["HIGH", "CRITICAL"]
email_recipients = []

[development]
enabled = false
skip_domain_validation = false
skip_path_validation = false
skip_resource_limits = false
dev_allow_domains = ["localhost"]

[emergency]
lockdown_enabled = false
lockdown_allowed_tools = ["memory"]
security_contact = "security@example.com"
auto_lockdown_triggers = ["repeated_violations"]

[tools]
"#;

        let config = SecurityConfig::load_from_toml(toml_content);
        assert!(
            config.is_ok(),
            "Failed to parse valid TOML: {:?}",
            config.err()
        );

        let config = config.unwrap();
        assert_eq!(config.metadata.version, "0.1.0");
        assert!(matches!(config.fs.access, FileSystemAccess::Enabled { .. }));
        assert_eq!(config.fs.allow_paths, vec![PathBuf::from("/tmp")]);
        // HTTP timeout is now inside the access enum variant
        assert_eq!(config.resources.max_memory_mb, 128);
    }

    #[test]
    fn test_load_actual_security_toml() {
        // Test with actual skreaver-security.toml file
        let result = SecurityConfig::load_from_file("../../../skreaver-security.toml");

        // Should either load successfully or give clear error
        match result {
            Ok(config) => {
                assert_eq!(config.metadata.version, "0.1.0");
                assert!(matches!(config.fs.access, FileSystemAccess::Enabled { .. }));
                assert!(!config.fs.allow_paths.is_empty());
            }
            Err(e) => {
                // File might not exist in test environment, that's okay
                println!("Note: Could not load skreaver-security.toml: {}", e);
            }
        }
    }

    #[test]
    fn test_invalid_toml_syntax() {
        let invalid_toml = r#"
[metadata
version = "broken
"#;

        let result = SecurityConfig::load_from_toml(invalid_toml);
        assert!(result.is_err());

        if let Err(SecurityError::ConfigError { message }) = result {
            assert!(message.contains("parse"));
        }
    }

    #[test]
    fn test_missing_required_fields() {
        let incomplete_toml = r#"
[metadata]
version = "0.1.0"
"#;

        let result = SecurityConfig::load_from_toml(incomplete_toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_zero_memory_limit() {
        let mut config = SecurityConfig::create_default();
        config.resources.max_memory_mb = 0;

        let result = config.validate();
        assert!(result.is_err());

        if let Err(SecurityError::ConfigError { message }) = result {
            assert!(message.contains("Memory limit"));
        }
    }

    #[test]
    fn test_validate_zero_timeout() {
        let mut config = SecurityConfig::create_default();
        config.resources.max_execution_time = Duration::from_secs(0);

        let result = config.validate();
        assert!(result.is_err());

        if let Err(SecurityError::ConfigError { message }) = result {
            assert!(message.contains("timeout"));
        }
    }

    #[test]
    fn test_validate_fs_enabled_no_paths() {
        let mut config = SecurityConfig::create_default();
        config.fs.access = FileSystemAccess::Enabled {
            symlink_behavior: SymlinkBehavior::NoFollow,
            content_scanning: true,
        };
        config.fs.allow_paths.clear();

        let result = config.validate();
        assert!(result.is_err());

        if let Err(SecurityError::ConfigError { message }) = result {
            assert!(message.contains("allowed paths"));
        }
    }

    #[test]
    fn test_get_tool_policy() {
        let config = SecurityConfig::create_default();

        // Test with non-existent tool
        let policy = config.get_tool_policy("nonexistent");
        // Check that it matches the global config
        let fs_enabled = !matches!(config.fs.access, FileSystemAccess::Disabled);
        let policy_fs_enabled = !matches!(policy.fs_policy.access, FileSystemAccess::Disabled);
        assert_eq!(policy_fs_enabled, fs_enabled);

        // Test with tool that has custom settings
        let mut config_with_tools = config.clone();
        let tool_policy = ToolSecurityPolicy {
            fs_enabled: Some(false),
            ..Default::default()
        };
        config_with_tools
            .tools
            .insert("restricted_tool".to_string(), tool_policy);

        let policy = config_with_tools.get_tool_policy("restricted_tool");
        assert!(matches!(
            policy.fs_policy.access,
            FileSystemAccess::Disabled
        ));
    }

    #[test]
    fn test_lockdown_mode() {
        let mut config = SecurityConfig::create_default();
        assert!(!config.is_lockdown_active());

        config.emergency.lockdown_enabled = true;
        assert!(config.is_lockdown_active());

        assert!(config.is_tool_allowed_in_lockdown("memory"));
        assert!(!config.is_tool_allowed_in_lockdown("http"));
    }

    #[test]
    fn test_alert_levels() {
        let config = SecurityConfig::create_default();

        assert!(config.should_alert(AlertLevel::High));
        assert!(config.should_alert(AlertLevel::Critical));
        assert!(!config.should_alert(AlertLevel::Low));
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
    }

    #[test]
    fn test_lockdown_triggers() {
        let config = SecurityConfig::create_default();

        assert!(config.has_lockdown_trigger(LockdownTrigger::RepeatedViolations));
        assert!(config.has_lockdown_trigger(LockdownTrigger::ResourceExhaustion));
        assert!(!config.has_lockdown_trigger(LockdownTrigger::ManualOverride));
    }

    #[test]
    fn test_alert_level_priority() {
        assert!(AlertLevel::Critical.is_high_priority());
        assert!(AlertLevel::High.is_high_priority());
        assert!(!AlertLevel::Medium.is_high_priority());
        assert!(!AlertLevel::Low.is_high_priority());
    }

    #[test]
    fn test_default_configs() {
        let config = SecurityConfig::create_default();

        assert_eq!(config.audit.log_level, LogLevel::Info);
        assert_eq!(config.audit.log_format, LogFormat::Structured);
        assert!(config.audit.redact_secrets);
        assert_eq!(config.secrets.min_secret_length, 16);
        assert_eq!(config.alerting.violation_threshold, 5);
    }
}
