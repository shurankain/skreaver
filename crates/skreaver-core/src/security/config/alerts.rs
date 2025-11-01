//! Alert configuration types

use super::types::{Disabled, Enabled};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

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

/// Type-safe alerting configuration with phantom type for enable/disable state
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

impl Default for AlertingConfig {
    fn default() -> Self {
        AlertingConfig::from(Alerting::<Enabled>::new_enabled())
    }
}
