//! Emergency and development mode configuration types
//!
//! This module provides type-safe development and emergency configuration using phantom types
//! to enforce compile-time guarantees about development mode and lockdown states.

use super::alerts::LockdownTrigger;
use super::types::{Development, LockdownActive, NormalOps, Production};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Type-safe development configuration with phantom type
///
/// Generic parameter `S` represents the mode state (Development or Production)
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

/// Type-safe emergency configuration with phantom type
///
/// Generic parameter `S` represents the lockdown state (LockdownActive or NormalOps)
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
