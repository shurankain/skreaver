//! Security configuration loading and parsing

use super::errors::SecurityError;
use super::limits::ResourceLimits;
use super::policy::{FileSystemPolicy, HttpPolicy, NetworkPolicy, SecurityPolicy, ToolPolicy};
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
    pub tools: HashMap<String, ToolPolicy>,
    pub alerting: AlertingConfig,
    pub development: DevelopmentConfig,
    pub emergency: EmergencyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub version: String,
    pub created: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub log_all_operations: bool,
    pub redact_secrets: bool,
    pub secret_patterns: Vec<String>,
    pub retain_logs_days: u32,
    pub log_level: String,
    pub include_stack_traces: bool,
    pub log_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretConfig {
    pub environment_only: bool,
    pub env_prefix: String,
    pub auto_rotate: bool,
    pub min_secret_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    pub enabled: bool,
    pub violation_threshold: u32,
    pub violation_window_minutes: u32,
    pub webhook_url: Option<String>,
    pub email_recipients: Vec<String>,
    pub alert_levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentConfig {
    pub enabled: bool,
    pub skip_domain_validation: bool,
    pub skip_path_validation: bool,
    pub skip_resource_limits: bool,
    pub dev_allow_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyConfig {
    pub lockdown_enabled: bool,
    pub lockdown_allowed_tools: Vec<String>,
    pub security_contact: String,
    pub auto_lockdown_triggers: Vec<String>,
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
                created: time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
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

        SecurityPolicy {
            fs_policy: if tool_policy.fs_enabled.unwrap_or(self.fs.enabled) {
                self.fs.clone()
            } else {
                FileSystemPolicy::disabled()
            },
            http_policy: if tool_policy.http_enabled.unwrap_or(self.http.enabled) {
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

    /// Validate configuration for security issues
    pub fn validate(&self) -> Result<(), SecurityError> {
        // Check for development mode in production
        if self.development.enabled {
            tracing::warn!("Development mode is enabled - this should not be used in production");
        }

        // Validate resource limits
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

        // Validate file system policies
        if self.fs.enabled && self.fs.allow_paths.is_empty() {
            return Err(SecurityError::ConfigError {
                message: "File system enabled but no allowed paths configured".to_string(),
            });
        }

        // Validate HTTP policies
        if self.http.enabled && self.http.allow_domains.is_empty() && !self.development.enabled {
            tracing::warn!("HTTP enabled but no allowed domains configured");
        }

        // Check for overly permissive settings
        if self.http.allow_local && !self.development.enabled {
            tracing::warn!("HTTP requests to localhost are allowed - this may be a security risk");
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
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            log_all_operations: true,
            redact_secrets: true,
            secret_patterns: vec![
                r"(?i)(password|pwd|secret|key|token).*[:=]\s*['\x22]?([^\x22\s]{8,})".to_string(),
                r"(?i)(api[_-]?key|apikey).*[:=]\s*['\x22]?([^\x22\s]{16,})".to_string(),
                r"(?i)(bearer|authorization).*[:=]\s*['\x22]?([^\x22\s]{20,})".to_string(),
            ],
            retain_logs_days: 90,
            log_level: "INFO".to_string(),
            include_stack_traces: false,
            log_format: "structured".to_string(),
        }
    }
}

impl Default for SecretConfig {
    fn default() -> Self {
        Self {
            environment_only: true,
            env_prefix: "SKREAVER_SECRET_".to_string(),
            auto_rotate: false,
            min_secret_length: 16,
        }
    }
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            violation_threshold: 5,
            violation_window_minutes: 15,
            webhook_url: None,
            email_recipients: Vec::new(),
            alert_levels: vec!["HIGH".to_string(), "CRITICAL".to_string()],
        }
    }
}

impl Default for DevelopmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            skip_domain_validation: false,
            skip_path_validation: false,
            skip_resource_limits: false,
            dev_allow_domains: vec!["localhost".to_string(), "127.0.0.1".to_string()],
        }
    }
}

impl Default for EmergencyConfig {
    fn default() -> Self {
        Self {
            lockdown_enabled: false,
            lockdown_allowed_tools: vec!["memory".to_string(), "logging".to_string()],
            security_contact: "security@example.com".to_string(),
            auto_lockdown_triggers: vec![
                "repeated_violations".to_string(),
                "resource_exhaustion".to_string(),
                "suspicious_patterns".to_string(),
            ],
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self::create_default()
    }
}
