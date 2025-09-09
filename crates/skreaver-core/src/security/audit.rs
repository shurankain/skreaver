//! Security audit logging and monitoring

use super::SecurityContext;
use super::errors::{SecurityViolation, ViolationSeverity};
#[cfg(feature = "security-audit")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "security-audit")]
use time::{Duration, OffsetDateTime};
use time::format_description::well_known::Rfc3339;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Security events for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum SecurityEvent {
    ValidationAttempt {
        context: SecurityContext,
        input_hash: String,
        result: SecurityResult,
    },
    ResourceLimitCheck {
        context: SecurityContext,
        resource_type: String,
        current_usage: u64,
        limit: u64,
        result: SecurityResult,
    },
    PolicyViolation {
        context: SecurityContext,
        violation: SecurityViolation,
        action_taken: String,
    },
    AuthenticationAttempt {
        principal: Option<String>,
        method: String,
        source_ip: Option<String>,
        result: SecurityResult,
        timestamp: OffsetDateTime,
    },
    AuthorizationCheck {
        context: SecurityContext,
        resource: String,
        permission: String,
        result: SecurityResult,
    },
    SuspiciousActivity {
        context: SecurityContext,
        activity_type: String,
        confidence_score: f64,
        indicators: Vec<String>,
    },
    EmergencyAction {
        trigger: String,
        action: String,
        affected_agents: Vec<String>,
        timestamp: OffsetDateTime,
    },
}

/// Results of security operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result_type", rename_all = "snake_case")]
pub enum SecurityResult {
    Allowed,
    Denied {
        reason: String,
    },
    LimitExceeded {
        limit_type: String,
        requested: u64,
        limit: u64,
    },
    Error {
        error_type: String,
        message: String,
    },
}

/// Audit log entry with structured metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditLog {
    pub id: Uuid,
    pub timestamp: OffsetDateTime,
    pub event: SecurityEvent,
    pub severity: LogSeverity,
    pub session_id: Option<Uuid>,
    pub agent_id: Option<String>,
    pub tool_name: Option<String>,
    pub correlation_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl From<&SecurityResult> for LogSeverity {
    fn from(result: &SecurityResult) -> Self {
        match result {
            SecurityResult::Allowed => LogSeverity::Info,
            SecurityResult::Denied { .. } => LogSeverity::Warning,
            SecurityResult::LimitExceeded { .. } => LogSeverity::Error,
            SecurityResult::Error { .. } => LogSeverity::Critical,
        }
    }
}

impl From<&ViolationSeverity> for LogSeverity {
    fn from(severity: &ViolationSeverity) -> Self {
        match severity {
            ViolationSeverity::Low => LogSeverity::Info,
            ViolationSeverity::Medium => LogSeverity::Warning,
            ViolationSeverity::High => LogSeverity::Error,
            ViolationSeverity::Critical => LogSeverity::Critical,
        }
    }
}

/// Audit logger for security events
pub struct AuditLogger {
    config: AuditConfig,
    violation_tracker: Arc<Mutex<ViolationTracker>>,
    redactor: SecretRedactor,
}

#[derive(Debug, Clone)]
pub struct AuditConfig {
    pub log_all_operations: bool,
    pub redact_secrets: bool,
    pub secret_patterns: Vec<String>,
    pub retain_logs_days: u32,
    pub log_level: LogLevel,
    pub include_stack_traces: bool,
    pub log_format: LogFormat,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone)]
pub enum LogFormat {
    Json,
    Text,
    Structured,
}

impl AuditLogger {
    pub fn new(config: &super::config::AuditConfig) -> Self {
        let audit_config = AuditConfig {
            log_all_operations: config.log_all_operations,
            redact_secrets: config.redact_secrets,
            secret_patterns: config.secret_patterns.clone(),
            retain_logs_days: config.retain_logs_days,
            log_level: match config.log_level.as_str() {
                "DEBUG" => LogLevel::Debug,
                "INFO" => LogLevel::Info,
                "WARNING" => LogLevel::Warning,
                "ERROR" => LogLevel::Error,
                "CRITICAL" => LogLevel::Critical,
                _ => LogLevel::Info,
            },
            include_stack_traces: config.include_stack_traces,
            log_format: match config.log_format.as_str() {
                "json" => LogFormat::Json,
                "text" => LogFormat::Text,
                "structured" => LogFormat::Structured,
                _ => LogFormat::Structured,
            },
        };

        Self {
            config: audit_config.clone(),
            violation_tracker: Arc::new(Mutex::new(ViolationTracker::new())),
            redactor: SecretRedactor::new(&audit_config.secret_patterns),
        }
    }

    pub fn log_event(&self, event: SecurityEvent) {
        let severity = self.determine_severity(&event);

        // Check if we should log based on severity
        if !self.should_log_severity(&severity) {
            return;
        }

        let mut audit_log = SecurityAuditLog {
            id: Uuid::new_v4(),
            timestamp: OffsetDateTime::now_utc(),
            event: event.clone(),
            severity,
            session_id: self.extract_session_id(&event),
            agent_id: self.extract_agent_id(&event),
            tool_name: self.extract_tool_name(&event),
            correlation_id: None,
            metadata: HashMap::new(),
        };

        // Redact secrets if enabled
        if self.config.redact_secrets {
            audit_log = self.redactor.redact_log(audit_log);
        }

        // Track violations for pattern detection
        if let SecurityEvent::PolicyViolation { violation, .. } = &event {
            let mut tracker = self.violation_tracker.lock().unwrap();
            tracker.record_violation(violation.clone());
        }

        // Log the event
        self.write_log_entry(&audit_log);

        // Update metrics
        self.update_security_metrics(&audit_log);
    }

    pub fn log_access_attempt(&self, context: &SecurityContext, result: SecurityResult) {
        let event = SecurityEvent::ValidationAttempt {
            context: context.clone(),
            input_hash: "hash_placeholder".to_string(), // Would be calculated by caller
            result,
        };

        self.log_event(event);
    }

    pub fn log_resource_check(
        &self,
        context: &SecurityContext,
        resource_type: String,
        current: u64,
        limit: u64,
        result: SecurityResult,
    ) {
        let event = SecurityEvent::ResourceLimitCheck {
            context: context.clone(),
            resource_type,
            current_usage: current,
            limit,
            result,
        };

        self.log_event(event);
    }

    pub fn log_violation(
        &self,
        context: &SecurityContext,
        violation: SecurityViolation,
        action: String,
    ) {
        let event = SecurityEvent::PolicyViolation {
            context: context.clone(),
            violation,
            action_taken: action,
        };

        self.log_event(event);
    }

    fn determine_severity(&self, event: &SecurityEvent) -> LogSeverity {
        match event {
            SecurityEvent::ValidationAttempt { result, .. } => LogSeverity::from(result),
            SecurityEvent::ResourceLimitCheck { result, .. } => LogSeverity::from(result),
            SecurityEvent::PolicyViolation { violation, .. } => {
                LogSeverity::from(&violation.severity)
            }
            SecurityEvent::AuthenticationAttempt { result, .. } => LogSeverity::from(result),
            SecurityEvent::AuthorizationCheck { result, .. } => LogSeverity::from(result),
            SecurityEvent::SuspiciousActivity {
                confidence_score, ..
            } => {
                if *confidence_score > 0.8 {
                    LogSeverity::Critical
                } else if *confidence_score > 0.6 {
                    LogSeverity::Error
                } else {
                    LogSeverity::Warning
                }
            }
            SecurityEvent::EmergencyAction { .. } => LogSeverity::Critical,
        }
    }

    fn should_log_severity(&self, severity: &LogSeverity) -> bool {
        let min_level = &self.config.log_level;

        matches!(
            (min_level, severity),
            (LogLevel::Debug, _)
                | (
                    LogLevel::Info,
                    LogSeverity::Info
                        | LogSeverity::Warning
                        | LogSeverity::Error
                        | LogSeverity::Critical,
                )
                | (
                    LogLevel::Warning,
                    LogSeverity::Warning | LogSeverity::Error | LogSeverity::Critical,
                )
                | (LogLevel::Error, LogSeverity::Error | LogSeverity::Critical)
                | (LogLevel::Critical, LogSeverity::Critical)
        )
    }

    fn extract_session_id(&self, event: &SecurityEvent) -> Option<Uuid> {
        match event {
            SecurityEvent::ValidationAttempt { context, .. }
            | SecurityEvent::ResourceLimitCheck { context, .. }
            | SecurityEvent::PolicyViolation { context, .. }
            | SecurityEvent::AuthorizationCheck { context, .. }
            | SecurityEvent::SuspiciousActivity { context, .. } => Some(context.session_id),
            _ => None,
        }
    }

    fn extract_agent_id(&self, event: &SecurityEvent) -> Option<String> {
        match event {
            SecurityEvent::ValidationAttempt { context, .. }
            | SecurityEvent::ResourceLimitCheck { context, .. }
            | SecurityEvent::PolicyViolation { context, .. }
            | SecurityEvent::AuthorizationCheck { context, .. }
            | SecurityEvent::SuspiciousActivity { context, .. } => Some(context.agent_id.clone()),
            _ => None,
        }
    }

    fn extract_tool_name(&self, event: &SecurityEvent) -> Option<String> {
        match event {
            SecurityEvent::ValidationAttempt { context, .. }
            | SecurityEvent::ResourceLimitCheck { context, .. }
            | SecurityEvent::PolicyViolation { context, .. }
            | SecurityEvent::AuthorizationCheck { context, .. }
            | SecurityEvent::SuspiciousActivity { context, .. } => Some(context.tool_name.clone()),
            _ => None,
        }
    }

    fn write_log_entry(&self, audit_log: &SecurityAuditLog) {
        match self.config.log_format {
            LogFormat::Json => {
                if let Ok(json) = serde_json::to_string(audit_log) {
                    match audit_log.severity {
                        LogSeverity::Critical => {
                            tracing::error!(target: "skreaver_security", "{}", json)
                        }
                        LogSeverity::Error => {
                            tracing::error!(target: "skreaver_security", "{}", json)
                        }
                        LogSeverity::Warning => {
                            tracing::warn!(target: "skreaver_security", "{}", json)
                        }
                        LogSeverity::Info => {
                            tracing::info!(target: "skreaver_security", "{}", json)
                        }
                        LogSeverity::Debug => {
                            tracing::debug!(target: "skreaver_security", "{}", json)
                        }
                    }
                }
            }
            LogFormat::Structured => {
                let agent_id = audit_log.agent_id.as_deref().unwrap_or("unknown");
                let tool_name = audit_log.tool_name.as_deref().unwrap_or("unknown");
                let session_id = audit_log
                    .session_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string());

                match audit_log.severity {
                    LogSeverity::Critical => tracing::error!(
                        target: "skreaver_security",
                        event_id = %audit_log.id,
                        session_id = %session_id,
                        agent_id = %agent_id,
                        tool_name = %tool_name,
                        "Security event: {:?}", audit_log.event
                    ),
                    LogSeverity::Error => tracing::error!(
                        target: "skreaver_security",
                        event_id = %audit_log.id,
                        session_id = %session_id,
                        agent_id = %agent_id,
                        tool_name = %tool_name,
                        "Security event: {:?}", audit_log.event
                    ),
                    LogSeverity::Warning => tracing::warn!(
                        target: "skreaver_security",
                        event_id = %audit_log.id,
                        session_id = %session_id,
                        agent_id = %agent_id,
                        tool_name = %tool_name,
                        "Security event: {:?}", audit_log.event
                    ),
                    LogSeverity::Info => tracing::info!(
                        target: "skreaver_security",
                        event_id = %audit_log.id,
                        session_id = %session_id,
                        agent_id = %agent_id,
                        tool_name = %tool_name,
                        "Security event: {:?}", audit_log.event
                    ),
                    LogSeverity::Debug => tracing::debug!(
                        target: "skreaver_security",
                        event_id = %audit_log.id,
                        session_id = %session_id,
                        agent_id = %agent_id,
                        tool_name = %tool_name,
                        "Security event: {:?}", audit_log.event
                    ),
                }
            }
            LogFormat::Text => {
                let message = format!(
                    "[{}] {} - Agent: {} Tool: {} - {:?}",
                    audit_log.timestamp.format(&Rfc3339).unwrap(),
                    match audit_log.severity {
                        LogSeverity::Critical => "CRITICAL",
                        LogSeverity::Error => "ERROR",
                        LogSeverity::Warning => "WARNING",
                        LogSeverity::Info => "INFO",
                        LogSeverity::Debug => "DEBUG",
                    },
                    audit_log.agent_id.as_deref().unwrap_or("unknown"),
                    audit_log.tool_name.as_deref().unwrap_or("unknown"),
                    audit_log.event
                );

                match audit_log.severity {
                    LogSeverity::Critical | LogSeverity::Error => {
                        tracing::error!(target: "skreaver_security", "{}", message)
                    }
                    LogSeverity::Warning => {
                        tracing::warn!(target: "skreaver_security", "{}", message)
                    }
                    LogSeverity::Info => tracing::info!(target: "skreaver_security", "{}", message),
                    LogSeverity::Debug => {
                        tracing::debug!(target: "skreaver_security", "{}", message)
                    }
                }
            }
        }
    }

    fn update_security_metrics(&self, audit_log: &SecurityAuditLog) {
        // TODO: Update Prometheus metrics when available
        // use crate::benchmarks::metrics::SECURITY_METRICS;

        // TODO: Implement metrics when SECURITY_METRICS is available
        match &audit_log.event {
            SecurityEvent::PolicyViolation { .. } => {
                // SECURITY_METRICS.security_violations_by_type.inc();
            }
            SecurityEvent::ResourceLimitCheck {
                result: SecurityResult::LimitExceeded { .. },
                ..
            } => {
                // SECURITY_METRICS.resource_limit_exceeded_total.inc();
            }
            SecurityEvent::ValidationAttempt {
                result: SecurityResult::Denied { .. },
                ..
            } => {
                // SECURITY_METRICS.access_denied_total.inc();
            }
            SecurityEvent::AuthenticationAttempt {
                result: SecurityResult::Error { .. },
                ..
            } => {
                // SECURITY_METRICS.authentication_failures.inc();
            }
            SecurityEvent::SuspiciousActivity { .. } => {
                // SECURITY_METRICS.suspicious_activity_score.set(*confidence_score);
            }
            _ => {}
        }
    }
}

/// Track violation patterns for anomaly detection
struct ViolationTracker {
    violations: Vec<SecurityViolation>,
    patterns: HashMap<String, u32>,
}

impl ViolationTracker {
    fn new() -> Self {
        Self {
            violations: Vec::new(),
            patterns: HashMap::new(),
        }
    }

    fn record_violation(&mut self, violation: SecurityViolation) {
        // Track pattern frequency
        let pattern_key = format!("{}:{}", violation.violation_type, violation.tool_name);
        *self.patterns.entry(pattern_key).or_insert(0) += 1;

        // Keep only recent violations (sliding window)
        let cutoff = OffsetDateTime::now_utc() - Duration::hours(24);
        self.violations.retain(|v| v.timestamp > cutoff);

        self.violations.push(violation);
    }

    #[allow(dead_code)]
    fn get_suspicious_patterns(&self) -> Vec<String> {
        self.patterns
            .iter()
            .filter(|(_, count)| **count > 5) // Threshold for suspicious activity
            .map(|(pattern, count)| format!("{} ({}x)", pattern, count))
            .collect()
    }
}

/// Redact secrets from audit logs
struct SecretRedactor {
    patterns: Vec<regex::Regex>,
}

impl SecretRedactor {
    fn new(pattern_strings: &[String]) -> Self {
        let patterns = pattern_strings
            .iter()
            .filter_map(|p| regex::Regex::new(p).ok())
            .collect();

        Self { patterns }
    }

    fn redact_log(&self, mut audit_log: SecurityAuditLog) -> SecurityAuditLog {
        // Redact secrets in event data
        audit_log.event = self.redact_event(audit_log.event);

        // Redact secrets in metadata
        for (_, value) in audit_log.metadata.iter_mut() {
            *value = self.redact_string(value.clone());
        }

        audit_log
    }

    fn redact_event(&self, event: SecurityEvent) -> SecurityEvent {
        // This would need to be implemented for each event type
        // For now, return as-is since most sensitive data should be hashed
        event
    }

    fn redact_string(&self, input: String) -> String {
        let mut redacted = input;
        for pattern in &self.patterns {
            redacted = pattern.replace_all(&redacted, "[REDACTED]").to_string();
        }
        redacted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::config::AuditConfig as ConfigAuditConfig;

    #[test]
    fn test_audit_logger_creation() {
        let config = ConfigAuditConfig {
            log_all_operations: true,
            redact_secrets: true,
            secret_patterns: vec!["test_pattern".to_string()],
            retain_logs_days: 90,
            log_level: "INFO".to_string(),
            include_stack_traces: false,
            log_format: "structured".to_string(),
        };

        let logger = AuditLogger::new(&config);
        assert!(matches!(logger.config.log_format, LogFormat::Structured));
        assert!(matches!(logger.config.log_level, LogLevel::Info));
    }

    #[test]
    fn test_violation_tracker() {
        let mut tracker = ViolationTracker::new();

        let violation = SecurityViolation {
            violation_type: "test_violation".to_string(),
            severity: ViolationSeverity::Medium,
            description: "Test violation".to_string(),
            agent_id: "test_agent".to_string(),
            tool_name: "test_tool".to_string(),
            input_hash: None,
            timestamp: OffsetDateTime::now_utc(),
            remediation: None,
        };

        tracker.record_violation(violation);
        assert_eq!(tracker.violations.len(), 1);
        assert_eq!(
            *tracker.patterns.get("test_violation:test_tool").unwrap(),
            1
        );
    }

    #[test]
    fn test_secret_redactor() {
        let patterns = vec!["api_key=\\w+".to_string()];
        let redactor = SecretRedactor::new(&patterns);

        let input = "api_key=secret123 and some other text";
        let redacted = redactor.redact_string(input.to_string());

        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("secret123"));
    }
}
