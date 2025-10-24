//! Security configuration loading and parsing

use super::errors::SecurityError;
use super::limits::ResourceLimits;
use super::policy::{
    FileSystemPolicy, HttpPolicy, NetworkPolicy, SecurityPolicy, ToolSecurityPolicy,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;

/// Strongly-typed log level configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Trace => write!(f, "TRACE"),
        }
    }
}

/// Strongly-typed log format configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Structured,
    Json,
    Text,
    Compact,
}

impl Default for LogFormat {
    fn default() -> Self {
        Self::Structured
    }
}

/// Strongly-typed alert level configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AlertLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl AlertLevel {
    /// Get all alert levels as a vector
    pub fn all() -> Vec<Self> {
        vec![Self::Low, Self::Medium, Self::High, Self::Critical]
    }

    /// Check if this alert level is high priority
    pub fn is_high_priority(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

/// Strongly-typed lockdown trigger configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockdownTrigger {
    RepeatedViolations,
    ResourceExhaustion,
    SuspiciousPatterns,
    ManualOverride,
    ExternalThreat,
}

impl LockdownTrigger {
    /// Get default lockdown triggers
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::RepeatedViolations,
            Self::ResourceExhaustion,
            Self::SuspiciousPatterns,
        ]
    }
}

// ============================================================================
// Typestate Pattern Markers for Security Config
// ============================================================================

/// Marker for enabled state
#[derive(Debug, Clone, Copy)]
pub struct Enabled;

/// Marker for disabled state
#[derive(Debug, Clone, Copy)]
pub struct Disabled;

/// Marker for logging all operations
#[derive(Debug, Clone, Copy)]
pub struct LogAll;

/// Marker for selective logging
#[derive(Debug, Clone, Copy)]
pub struct LogSelective;

/// Marker for secret redaction enabled
#[derive(Debug, Clone, Copy)]
pub struct RedactSecrets;

/// Marker for no secret redaction
#[derive(Debug, Clone, Copy)]
pub struct NoRedaction;

/// Marker for stack traces included
#[derive(Debug, Clone, Copy)]
pub struct WithStackTraces;

/// Marker for stack traces excluded
#[derive(Debug, Clone, Copy)]
pub struct NoStackTraces;

/// Marker for environment-only secrets
#[derive(Debug, Clone, Copy)]
pub struct EnvironmentOnly;

/// Marker for flexible secret sources
#[derive(Debug, Clone, Copy)]
pub struct FlexibleSources;

/// Marker for auto-rotation enabled
#[derive(Debug, Clone, Copy)]
pub struct AutoRotate;

/// Marker for manual rotation
#[derive(Debug, Clone, Copy)]
pub struct ManualRotate;

/// Marker for production mode
#[derive(Debug, Clone, Copy)]
pub struct Production;

/// Marker for development mode
#[derive(Debug, Clone, Copy)]
pub struct Development;

/// Marker for lockdown active
#[derive(Debug, Clone, Copy)]
pub struct LockdownActive;

/// Marker for normal operations
#[derive(Debug, Clone, Copy)]
pub struct NormalOps;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub version: String,
    pub created: String,
    pub description: String,
}

/// Type-safe audit configuration with phantom types
#[derive(Debug, Clone)]
pub struct Audit<L, R, S> {
    pub secret_patterns: Vec<String>,
    pub retain_logs_days: u32,
    pub log_level: LogLevel,
    pub log_format: LogFormat,
    _logging: PhantomData<L>,
    _redaction: PhantomData<R>,
    _stack_traces: PhantomData<S>,
}

impl<L, R, S> Audit<L, R, S> {
    /// Get secret patterns
    pub fn secret_patterns(&self) -> &[String] {
        &self.secret_patterns
    }

    /// Get log retention period
    pub fn retain_logs_days(&self) -> u32 {
        self.retain_logs_days
    }

    /// Get log level
    pub fn log_level(&self) -> LogLevel {
        self.log_level
    }

    /// Get log format
    pub fn log_format(&self) -> LogFormat {
        self.log_format
    }
}

impl Audit<LogAll, RedactSecrets, NoStackTraces> {
    /// Create new audit config with all logging, secret redaction, no stack traces
    pub fn new_secure() -> Self {
        Self {
            secret_patterns: vec![
                r"(?i)(password|pwd|secret|key|token).*[:=]\s*['\x22]?([^\x22\s]{8,})".to_string(),
                r"(?i)(api[_-]?key|apikey).*[:=]\s*['\x22]?([^\x22\s]{16,})".to_string(),
                r"(?i)(bearer|authorization).*[:=]\s*['\x22]?([^\x22\s]{20,})".to_string(),
            ],
            retain_logs_days: 90,
            log_level: LogLevel::Info,
            log_format: LogFormat::Structured,
            _logging: PhantomData,
            _redaction: PhantomData,
            _stack_traces: PhantomData,
        }
    }
}

impl<R, S> Audit<LogAll, R, S> {
    /// Check if all operations should be logged
    pub fn logs_all_operations(&self) -> bool {
        true
    }
}

impl<R, S> Audit<LogSelective, R, S> {
    /// Check if all operations should be logged
    pub fn logs_all_operations(&self) -> bool {
        false
    }
}

impl<L, S> Audit<L, RedactSecrets, S> {
    /// Check if secrets should be redacted
    pub fn redacts_secrets(&self) -> bool {
        true
    }
}

impl<L, S> Audit<L, NoRedaction, S> {
    /// Check if secrets should be redacted
    pub fn redacts_secrets(&self) -> bool {
        false
    }
}

impl<L, R> Audit<L, R, WithStackTraces> {
    /// Check if stack traces should be included
    pub fn includes_stack_traces(&self) -> bool {
        true
    }
}

impl<L, R> Audit<L, R, NoStackTraces> {
    /// Check if stack traces should be included
    pub fn includes_stack_traces(&self) -> bool {
        false
    }
}

impl<L, R, S> Audit<L, R, S> {
    /// Enable stack traces
    pub fn with_stack_traces(self) -> Audit<L, R, WithStackTraces> {
        Audit {
            secret_patterns: self.secret_patterns,
            retain_logs_days: self.retain_logs_days,
            log_level: self.log_level,
            log_format: self.log_format,
            _logging: PhantomData,
            _redaction: PhantomData,
            _stack_traces: PhantomData,
        }
    }

    /// Disable stack traces
    pub fn without_stack_traces(self) -> Audit<L, R, NoStackTraces> {
        Audit {
            secret_patterns: self.secret_patterns,
            retain_logs_days: self.retain_logs_days,
            log_level: self.log_level,
            log_format: self.log_format,
            _logging: PhantomData,
            _redaction: PhantomData,
            _stack_traces: PhantomData,
        }
    }

    /// Enable secret redaction
    pub fn with_redaction(self) -> Audit<L, RedactSecrets, S> {
        Audit {
            secret_patterns: self.secret_patterns,
            retain_logs_days: self.retain_logs_days,
            log_level: self.log_level,
            log_format: self.log_format,
            _logging: PhantomData,
            _redaction: PhantomData,
            _stack_traces: PhantomData,
        }
    }

    /// Disable secret redaction
    pub fn without_redaction(self) -> Audit<L, NoRedaction, S> {
        Audit {
            secret_patterns: self.secret_patterns,
            retain_logs_days: self.retain_logs_days,
            log_level: self.log_level,
            log_format: self.log_format,
            _logging: PhantomData,
            _redaction: PhantomData,
            _stack_traces: PhantomData,
        }
    }

    /// Enable all operation logging
    pub fn log_all(self) -> Audit<LogAll, R, S> {
        Audit {
            secret_patterns: self.secret_patterns,
            retain_logs_days: self.retain_logs_days,
            log_level: self.log_level,
            log_format: self.log_format,
            _logging: PhantomData,
            _redaction: PhantomData,
            _stack_traces: PhantomData,
        }
    }

    /// Enable selective operation logging
    pub fn log_selective(self) -> Audit<LogSelective, R, S> {
        Audit {
            secret_patterns: self.secret_patterns,
            retain_logs_days: self.retain_logs_days,
            log_level: self.log_level,
            log_format: self.log_format,
            _logging: PhantomData,
            _redaction: PhantomData,
            _stack_traces: PhantomData,
        }
    }
}

/// Backward compatible audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub log_all_operations: bool,
    pub redact_secrets: bool,
    pub secret_patterns: Vec<String>,
    pub retain_logs_days: u32,
    pub log_level: LogLevel,
    pub include_stack_traces: bool,
    pub log_format: LogFormat,
}

impl From<Audit<LogAll, RedactSecrets, NoStackTraces>> for AuditConfig {
    fn from(audit: Audit<LogAll, RedactSecrets, NoStackTraces>) -> Self {
        Self {
            log_all_operations: true,
            redact_secrets: true,
            include_stack_traces: false,
            secret_patterns: audit.secret_patterns,
            retain_logs_days: audit.retain_logs_days,
            log_level: audit.log_level,
            log_format: audit.log_format,
        }
    }
}

impl From<Audit<LogAll, RedactSecrets, WithStackTraces>> for AuditConfig {
    fn from(audit: Audit<LogAll, RedactSecrets, WithStackTraces>) -> Self {
        Self {
            log_all_operations: true,
            redact_secrets: true,
            include_stack_traces: true,
            secret_patterns: audit.secret_patterns,
            retain_logs_days: audit.retain_logs_days,
            log_level: audit.log_level,
            log_format: audit.log_format,
        }
    }
}

impl From<AuditConfig> for Audit<LogAll, RedactSecrets, NoStackTraces> {
    fn from(config: AuditConfig) -> Self {
        Self {
            secret_patterns: config.secret_patterns,
            retain_logs_days: config.retain_logs_days,
            log_level: config.log_level,
            log_format: config.log_format,
            _logging: PhantomData,
            _redaction: PhantomData,
            _stack_traces: PhantomData,
        }
    }
}

/// Type-safe secret configuration with phantom types
#[derive(Debug, Clone)]
pub struct Secret<E, R> {
    pub env_prefix: String,
    pub min_secret_length: usize,
    _environment: PhantomData<E>,
    _rotation: PhantomData<R>,
}

impl<E, R> Secret<E, R> {
    /// Get environment prefix
    pub fn env_prefix(&self) -> &str {
        &self.env_prefix
    }

    /// Get minimum secret length
    pub fn min_secret_length(&self) -> usize {
        self.min_secret_length
    }
}

impl Secret<EnvironmentOnly, ManualRotate> {
    /// Create new secure secret config (environment only, manual rotation)
    pub fn new_secure() -> Self {
        Self {
            env_prefix: "SKREAVER_SECRET_".to_string(),
            min_secret_length: 16,
            _environment: PhantomData,
            _rotation: PhantomData,
        }
    }
}

impl<R> Secret<EnvironmentOnly, R> {
    /// Check if secrets are environment-only
    pub fn is_environment_only(&self) -> bool {
        true
    }
}

impl<R> Secret<FlexibleSources, R> {
    /// Check if secrets are environment-only
    pub fn is_environment_only(&self) -> bool {
        false
    }
}

impl<E> Secret<E, AutoRotate> {
    /// Check if auto-rotation is enabled
    pub fn auto_rotates(&self) -> bool {
        true
    }
}

impl<E> Secret<E, ManualRotate> {
    /// Check if auto-rotation is enabled
    pub fn auto_rotates(&self) -> bool {
        false
    }
}

impl<E, R> Secret<E, R> {
    /// Enable auto-rotation
    pub fn with_auto_rotate(self) -> Secret<E, AutoRotate> {
        Secret {
            env_prefix: self.env_prefix,
            min_secret_length: self.min_secret_length,
            _environment: PhantomData,
            _rotation: PhantomData,
        }
    }

    /// Disable auto-rotation
    pub fn without_auto_rotate(self) -> Secret<E, ManualRotate> {
        Secret {
            env_prefix: self.env_prefix,
            min_secret_length: self.min_secret_length,
            _environment: PhantomData,
            _rotation: PhantomData,
        }
    }

    /// Restrict to environment-only secrets
    pub fn environment_only(self) -> Secret<EnvironmentOnly, R> {
        Secret {
            env_prefix: self.env_prefix,
            min_secret_length: self.min_secret_length,
            _environment: PhantomData,
            _rotation: PhantomData,
        }
    }

    /// Allow flexible secret sources
    pub fn flexible_sources(self) -> Secret<FlexibleSources, R> {
        Secret {
            env_prefix: self.env_prefix,
            min_secret_length: self.min_secret_length,
            _environment: PhantomData,
            _rotation: PhantomData,
        }
    }
}

/// Backward compatible secret configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretConfig {
    pub environment_only: bool,
    pub env_prefix: String,
    pub auto_rotate: bool,
    pub min_secret_length: usize,
}

impl From<Secret<EnvironmentOnly, ManualRotate>> for SecretConfig {
    fn from(secret: Secret<EnvironmentOnly, ManualRotate>) -> Self {
        Self {
            environment_only: true,
            auto_rotate: false,
            env_prefix: secret.env_prefix,
            min_secret_length: secret.min_secret_length,
        }
    }
}

impl From<Secret<EnvironmentOnly, AutoRotate>> for SecretConfig {
    fn from(secret: Secret<EnvironmentOnly, AutoRotate>) -> Self {
        Self {
            environment_only: true,
            auto_rotate: true,
            env_prefix: secret.env_prefix,
            min_secret_length: secret.min_secret_length,
        }
    }
}

impl From<SecretConfig> for Secret<EnvironmentOnly, ManualRotate> {
    fn from(config: SecretConfig) -> Self {
        Self {
            env_prefix: config.env_prefix,
            min_secret_length: config.min_secret_length,
            _environment: PhantomData,
            _rotation: PhantomData,
        }
    }
}

/// Type-safe alerting configuration with phantom type
#[derive(Debug, Clone)]
pub struct Alerting<S> {
    pub violation_threshold: u32,
    pub violation_window_minutes: u32,
    pub webhook_url: Option<String>,
    pub email_recipients: Vec<String>,
    pub alert_levels: Vec<AlertLevel>,
    _state: PhantomData<S>,
}

impl<S> Alerting<S> {
    /// Get violation threshold
    pub fn violation_threshold(&self) -> u32 {
        self.violation_threshold
    }

    /// Get violation window in minutes
    pub fn violation_window_minutes(&self) -> u32 {
        self.violation_window_minutes
    }

    /// Get webhook URL
    pub fn webhook_url(&self) -> Option<&str> {
        self.webhook_url.as_deref()
    }

    /// Get email recipients
    pub fn email_recipients(&self) -> &[String] {
        &self.email_recipients
    }

    /// Get alert levels
    pub fn alert_levels(&self) -> &[AlertLevel] {
        &self.alert_levels
    }
}

impl Alerting<Enabled> {
    /// Create new enabled alerting config
    pub fn new_enabled() -> Self {
        Self {
            violation_threshold: 5,
            violation_window_minutes: 15,
            webhook_url: None,
            email_recipients: Vec::new(),
            alert_levels: vec![AlertLevel::High, AlertLevel::Critical],
            _state: PhantomData,
        }
    }

    /// Check if alerting is enabled
    pub fn is_enabled(&self) -> bool {
        true
    }

    /// Disable alerting
    pub fn disable(self) -> Alerting<Disabled> {
        Alerting {
            violation_threshold: self.violation_threshold,
            violation_window_minutes: self.violation_window_minutes,
            webhook_url: self.webhook_url,
            email_recipients: self.email_recipients,
            alert_levels: self.alert_levels,
            _state: PhantomData,
        }
    }
}

impl Alerting<Disabled> {
    /// Create new disabled alerting config
    pub fn new_disabled() -> Self {
        Self {
            violation_threshold: 5,
            violation_window_minutes: 15,
            webhook_url: None,
            email_recipients: Vec::new(),
            alert_levels: vec![AlertLevel::High, AlertLevel::Critical],
            _state: PhantomData,
        }
    }

    /// Check if alerting is enabled
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// Enable alerting
    pub fn enable(self) -> Alerting<Enabled> {
        Alerting {
            violation_threshold: self.violation_threshold,
            violation_window_minutes: self.violation_window_minutes,
            webhook_url: self.webhook_url,
            email_recipients: self.email_recipients,
            alert_levels: self.alert_levels,
            _state: PhantomData,
        }
    }
}

/// Backward compatible alerting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    pub enabled: bool,
    pub violation_threshold: u32,
    pub violation_window_minutes: u32,
    pub webhook_url: Option<String>,
    pub email_recipients: Vec<String>,
    pub alert_levels: Vec<AlertLevel>,
}

impl From<Alerting<Enabled>> for AlertingConfig {
    fn from(alerting: Alerting<Enabled>) -> Self {
        Self {
            enabled: true,
            violation_threshold: alerting.violation_threshold,
            violation_window_minutes: alerting.violation_window_minutes,
            webhook_url: alerting.webhook_url,
            email_recipients: alerting.email_recipients,
            alert_levels: alerting.alert_levels,
        }
    }
}

impl From<Alerting<Disabled>> for AlertingConfig {
    fn from(alerting: Alerting<Disabled>) -> Self {
        Self {
            enabled: false,
            violation_threshold: alerting.violation_threshold,
            violation_window_minutes: alerting.violation_window_minutes,
            webhook_url: alerting.webhook_url,
            email_recipients: alerting.email_recipients,
            alert_levels: alerting.alert_levels,
        }
    }
}

/// Type-safe development configuration with phantom type
#[derive(Debug, Clone)]
pub struct DevelopmentMode<S> {
    pub skip_domain_validation: bool,
    pub skip_path_validation: bool,
    pub skip_resource_limits: bool,
    pub dev_allow_domains: Vec<String>,
    _state: PhantomData<S>,
}

impl<S> DevelopmentMode<S> {
    /// Check if domain validation is skipped
    pub fn skips_domain_validation(&self) -> bool {
        self.skip_domain_validation
    }

    /// Check if path validation is skipped
    pub fn skips_path_validation(&self) -> bool {
        self.skip_path_validation
    }

    /// Check if resource limits are skipped
    pub fn skips_resource_limits(&self) -> bool {
        self.skip_resource_limits
    }

    /// Get development allowed domains
    pub fn dev_allow_domains(&self) -> &[String] {
        &self.dev_allow_domains
    }
}

impl DevelopmentMode<Development> {
    /// Create new development mode config
    pub fn new_development() -> Self {
        Self {
            skip_domain_validation: false,
            skip_path_validation: false,
            skip_resource_limits: false,
            dev_allow_domains: vec!["localhost".to_string(), "127.0.0.1".to_string()],
            _state: PhantomData,
        }
    }

    /// Check if development mode is enabled
    pub fn is_enabled(&self) -> bool {
        true
    }

    /// Switch to production mode
    pub fn to_production(self) -> DevelopmentMode<Production> {
        DevelopmentMode {
            skip_domain_validation: false,
            skip_path_validation: false,
            skip_resource_limits: false,
            dev_allow_domains: self.dev_allow_domains,
            _state: PhantomData,
        }
    }
}

impl DevelopmentMode<Production> {
    /// Create new production mode config
    pub fn new_production() -> Self {
        Self {
            skip_domain_validation: false,
            skip_path_validation: false,
            skip_resource_limits: false,
            dev_allow_domains: vec!["localhost".to_string(), "127.0.0.1".to_string()],
            _state: PhantomData,
        }
    }

    /// Check if development mode is enabled
    pub fn is_enabled(&self) -> bool {
        false
    }

    /// Switch to development mode
    pub fn to_development(self) -> DevelopmentMode<Development> {
        DevelopmentMode {
            skip_domain_validation: self.skip_domain_validation,
            skip_path_validation: self.skip_path_validation,
            skip_resource_limits: self.skip_resource_limits,
            dev_allow_domains: self.dev_allow_domains,
            _state: PhantomData,
        }
    }
}

/// Backward compatible development configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentConfig {
    pub enabled: bool,
    pub skip_domain_validation: bool,
    pub skip_path_validation: bool,
    pub skip_resource_limits: bool,
    pub dev_allow_domains: Vec<String>,
}

impl From<DevelopmentMode<Development>> for DevelopmentConfig {
    fn from(dev: DevelopmentMode<Development>) -> Self {
        Self {
            enabled: true,
            skip_domain_validation: dev.skip_domain_validation,
            skip_path_validation: dev.skip_path_validation,
            skip_resource_limits: dev.skip_resource_limits,
            dev_allow_domains: dev.dev_allow_domains,
        }
    }
}

impl From<DevelopmentMode<Production>> for DevelopmentConfig {
    fn from(dev: DevelopmentMode<Production>) -> Self {
        Self {
            enabled: false,
            skip_domain_validation: dev.skip_domain_validation,
            skip_path_validation: dev.skip_path_validation,
            skip_resource_limits: dev.skip_resource_limits,
            dev_allow_domains: dev.dev_allow_domains,
        }
    }
}

/// Type-safe emergency configuration with phantom type
#[derive(Debug, Clone)]
pub struct Emergency<S> {
    pub lockdown_allowed_tools: Vec<String>,
    pub security_contact: String,
    pub auto_lockdown_triggers: Vec<LockdownTrigger>,
    _state: PhantomData<S>,
}

impl<S> Emergency<S> {
    /// Get lockdown allowed tools
    pub fn lockdown_allowed_tools(&self) -> &[String] {
        &self.lockdown_allowed_tools
    }

    /// Get security contact
    pub fn security_contact(&self) -> &str {
        &self.security_contact
    }

    /// Get auto-lockdown triggers
    pub fn auto_lockdown_triggers(&self) -> &[LockdownTrigger] {
        &self.auto_lockdown_triggers
    }
}

impl Emergency<LockdownActive> {
    /// Create new lockdown-active config
    pub fn new_lockdown() -> Self {
        Self {
            lockdown_allowed_tools: vec!["memory".to_string(), "logging".to_string()],
            security_contact: "security@example.com".to_string(),
            auto_lockdown_triggers: LockdownTrigger::defaults(),
            _state: PhantomData,
        }
    }

    /// Check if lockdown is enabled
    pub fn is_lockdown_enabled(&self) -> bool {
        true
    }

    /// Check if tool is allowed during lockdown
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.lockdown_allowed_tools.contains(&tool_name.to_string())
    }

    /// Deactivate lockdown
    pub fn deactivate_lockdown(self) -> Emergency<NormalOps> {
        Emergency {
            lockdown_allowed_tools: self.lockdown_allowed_tools,
            security_contact: self.security_contact,
            auto_lockdown_triggers: self.auto_lockdown_triggers,
            _state: PhantomData,
        }
    }
}

impl Emergency<NormalOps> {
    /// Create new normal operations config
    pub fn new_normal() -> Self {
        Self {
            lockdown_allowed_tools: vec!["memory".to_string(), "logging".to_string()],
            security_contact: "security@example.com".to_string(),
            auto_lockdown_triggers: LockdownTrigger::defaults(),
            _state: PhantomData,
        }
    }

    /// Check if lockdown is enabled
    pub fn is_lockdown_enabled(&self) -> bool {
        false
    }

    /// Activate lockdown
    pub fn activate_lockdown(self) -> Emergency<LockdownActive> {
        Emergency {
            lockdown_allowed_tools: self.lockdown_allowed_tools,
            security_contact: self.security_contact,
            auto_lockdown_triggers: self.auto_lockdown_triggers,
            _state: PhantomData,
        }
    }
}

/// Backward compatible emergency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyConfig {
    pub lockdown_enabled: bool,
    pub lockdown_allowed_tools: Vec<String>,
    pub security_contact: String,
    pub auto_lockdown_triggers: Vec<LockdownTrigger>,
}

impl From<Emergency<LockdownActive>> for EmergencyConfig {
    fn from(emergency: Emergency<LockdownActive>) -> Self {
        Self {
            lockdown_enabled: true,
            lockdown_allowed_tools: emergency.lockdown_allowed_tools,
            security_contact: emergency.security_contact,
            auto_lockdown_triggers: emergency.auto_lockdown_triggers,
        }
    }
}

impl From<Emergency<NormalOps>> for EmergencyConfig {
    fn from(emergency: Emergency<NormalOps>) -> Self {
        Self {
            lockdown_enabled: false,
            lockdown_allowed_tools: emergency.lockdown_allowed_tools,
            security_contact: emergency.security_contact,
            auto_lockdown_triggers: emergency.auto_lockdown_triggers,
        }
    }
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
                    .unwrap(),
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

        // Validate CPU percentage
        if self.resources.max_cpu_percent < 0.0 || self.resources.max_cpu_percent > 100.0 {
            return Err(SecurityError::ConfigError {
                message: format!(
                    "max_cpu_percent must be 0.0-100.0, got {}",
                    self.resources.max_cpu_percent
                ),
            });
        }

        // Validate file system policies (CRITICAL - must fail)
        if self.fs.enabled && self.fs.allow_paths.is_empty() {
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
        if self.http.enabled && self.http.allow_domains.is_empty() && !self.development.enabled {
            tracing::warn!(
                "HTTP enabled but no allowed domains configured (all domains will be blocked)"
            );
        }

        // Check for overly permissive settings (WARNINGS)
        if self.http.allow_local && !self.development.enabled {
            tracing::warn!(
                "HTTP requests to localhost are allowed - this may be a security risk in production"
            );
        }

        if self.network.allow_private_networks && !self.development.enabled {
            tracing::warn!(
                "Network access to private IP ranges is allowed - this may be a security risk"
            );
        }

        if self.fs.follow_symlinks {
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
            log_level: LogLevel::Info,
            include_stack_traces: false,
            log_format: LogFormat::Structured,
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
            alert_levels: vec![AlertLevel::High, AlertLevel::Critical],
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
            auto_lockdown_triggers: LockdownTrigger::defaults(),
        }
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
enabled = true
allow_paths = ["/tmp"]
deny_patterns = [".."]
max_file_size_bytes = 16777216
max_files_per_operation = 100
follow_symlinks = false
scan_content = true

[http]
enabled = true
allow_domains = ["example.com"]
deny_domains = ["localhost"]
allow_methods = ["GET", "POST"]
timeout_seconds = 30
max_response_bytes = 33554432
max_redirects = 3
user_agent = "test-agent"
allow_local = false
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
        assert!(config.fs.enabled);
        assert_eq!(config.fs.allow_paths, vec![PathBuf::from("/tmp")]);
        assert_eq!(config.http.timeout.seconds(), 30);
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
                assert!(config.fs.enabled);
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
        config.fs.enabled = true;
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
        assert_eq!(policy.fs_policy.enabled, config.fs.enabled);

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
        assert!(!policy.fs_policy.enabled);
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
