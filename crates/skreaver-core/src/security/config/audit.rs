//! Audit configuration types
//!
//! This module provides type-safe audit configuration using phantom types
//! to enforce compile-time guarantees about logging, redaction, and stack trace settings.

use super::logging::{LogFormat, LogLevel};
use super::types::{
    LogAll, LogSelective, NoRedaction, NoStackTraces, RedactSecrets, WithStackTraces,
};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Type-safe audit configuration with phantom types
///
/// Generic parameters:
/// - `L`: Logging mode (LogAll or LogSelective)
/// - `R`: Redaction mode (RedactSecrets or NoRedaction)
/// - `S`: Stack trace mode (WithStackTraces or NoStackTraces)
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
