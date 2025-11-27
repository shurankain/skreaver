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

/// Development level for type-safe configuration
///
/// Represents different levels of development mode with clear security implications.
/// Each level enables specific validation skips appropriate for that use case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "level")]
pub enum DevelopmentLevel {
    /// Production mode - all validations enabled, no skips
    Production,

    /// Light development - only domain validation relaxed (useful for local testing)
    /// - Skips domain validation for dev_allow_domains
    /// - All other validations active
    Light,

    /// Standard development - common validations relaxed
    /// - Skips domain validation
    /// - Skips path validation
    /// - Resource limits still enforced
    Standard,

    /// Full development - all validations disabled (maximum flexibility, use with caution)
    /// - Skips domain validation
    /// - Skips path validation
    /// - Skips resource limits
    Full,
}

impl DevelopmentLevel {
    /// Check if development mode is enabled (any level except Production)
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Production)
    }

    /// Check if domain validation should be skipped
    pub fn skip_domain_validation(&self) -> bool {
        matches!(self, Self::Light | Self::Standard | Self::Full)
    }

    /// Check if path validation should be skipped
    pub fn skip_path_validation(&self) -> bool {
        matches!(self, Self::Standard | Self::Full)
    }

    /// Check if resource limits should be skipped
    pub fn skip_resource_limits(&self) -> bool {
        matches!(self, Self::Full)
    }
}

impl Default for DevelopmentLevel {
    fn default() -> Self {
        Self::Production
    }
}

/// Backward compatible development configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentConfig {
    #[serde(flatten)]
    pub level: DevelopmentLevel,
    pub dev_allow_domains: Vec<String>,
}

impl DevelopmentConfig {
    /// Check if development mode is enabled
    pub fn is_enabled(&self) -> bool {
        self.level.is_enabled()
    }

    /// Check if domain validation should be skipped
    pub fn skip_domain_validation(&self) -> bool {
        self.level.skip_domain_validation()
    }

    /// Check if path validation should be skipped
    pub fn skip_path_validation(&self) -> bool {
        self.level.skip_path_validation()
    }

    /// Check if resource limits should be skipped
    pub fn skip_resource_limits(&self) -> bool {
        self.level.skip_resource_limits()
    }

    /// Get development allowed domains
    pub fn dev_allow_domains(&self) -> &[String] {
        &self.dev_allow_domains
    }
}

impl From<DevelopmentMode<Development>> for DevelopmentConfig {
    fn from(dev: DevelopmentMode<Development>) -> Self {
        // Map boolean flags to appropriate level
        let level = match (
            dev.skip_domain_validation,
            dev.skip_path_validation,
            dev.skip_resource_limits,
        ) {
            (false, false, false) => DevelopmentLevel::Production,
            (true, false, false) => DevelopmentLevel::Light,
            (true, true, false) => DevelopmentLevel::Standard,
            (true, true, true) => DevelopmentLevel::Full,
            // Invalid combinations map to closest safe level
            _ => DevelopmentLevel::Production,
        };

        Self {
            level,
            dev_allow_domains: dev.dev_allow_domains,
        }
    }
}

impl From<DevelopmentMode<Production>> for DevelopmentConfig {
    fn from(dev: DevelopmentMode<Production>) -> Self {
        Self {
            level: DevelopmentLevel::Production,
            dev_allow_domains: dev.dev_allow_domains,
        }
    }
}

impl Default for DevelopmentConfig {
    fn default() -> Self {
        Self {
            level: DevelopmentLevel::Production,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_development_level_production() {
        let level = DevelopmentLevel::Production;
        assert!(!level.is_enabled());
        assert!(!level.skip_domain_validation());
        assert!(!level.skip_path_validation());
        assert!(!level.skip_resource_limits());
    }

    #[test]
    fn test_development_level_light() {
        let level = DevelopmentLevel::Light;
        assert!(level.is_enabled());
        assert!(level.skip_domain_validation());
        assert!(!level.skip_path_validation());
        assert!(!level.skip_resource_limits());
    }

    #[test]
    fn test_development_level_standard() {
        let level = DevelopmentLevel::Standard;
        assert!(level.is_enabled());
        assert!(level.skip_domain_validation());
        assert!(level.skip_path_validation());
        assert!(!level.skip_resource_limits());
    }

    #[test]
    fn test_development_level_full() {
        let level = DevelopmentLevel::Full;
        assert!(level.is_enabled());
        assert!(level.skip_domain_validation());
        assert!(level.skip_path_validation());
        assert!(level.skip_resource_limits());
    }

    #[test]
    fn test_development_config_helper_methods() {
        let config = DevelopmentConfig {
            level: DevelopmentLevel::Standard,
            dev_allow_domains: vec!["localhost".to_string()],
        };

        assert!(config.is_enabled());
        assert!(config.skip_domain_validation());
        assert!(config.skip_path_validation());
        assert!(!config.skip_resource_limits());
        assert_eq!(config.dev_allow_domains(), &["localhost"]);
    }

    #[test]
    fn test_development_config_default() {
        let config = DevelopmentConfig::default();
        assert_eq!(config.level, DevelopmentLevel::Production);
        assert!(!config.is_enabled());
        assert_eq!(
            config.dev_allow_domains,
            vec!["localhost".to_string(), "127.0.0.1".to_string()]
        );
    }

    #[test]
    fn test_development_level_serialization() {
        let level = DevelopmentLevel::Standard;
        let json = serde_json::to_string(&level).unwrap();
        assert!(json.contains("Standard"));

        let deserialized: DevelopmentLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, level);
    }

    #[test]
    fn test_development_config_serialization() {
        let config = DevelopmentConfig {
            level: DevelopmentLevel::Light,
            dev_allow_domains: vec!["example.com".to_string()],
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DevelopmentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.level, config.level);
        assert_eq!(deserialized.dev_allow_domains, config.dev_allow_domains);
    }

    #[test]
    fn test_development_mode_to_config_conversion() {
        // Test Production mode conversion
        let prod_mode = DevelopmentMode::<Production>::new_production();
        let config: DevelopmentConfig = prod_mode.into();
        assert_eq!(config.level, DevelopmentLevel::Production);

        // Test Development mode with all flags enabled
        let mut dev_mode = DevelopmentMode::<Development>::new_development();
        dev_mode.skip_domain_validation = true;
        dev_mode.skip_path_validation = true;
        dev_mode.skip_resource_limits = true;
        let config: DevelopmentConfig = dev_mode.into();
        assert_eq!(config.level, DevelopmentLevel::Full);
    }

    #[test]
    fn test_emergency_lockdown_active() {
        let emergency = Emergency::<LockdownActive>::new_lockdown();
        assert!(emergency.is_lockdown_enabled());
        assert!(emergency.is_tool_allowed("memory"));
        assert!(!emergency.is_tool_allowed("http"));
    }

    #[test]
    fn test_emergency_normal_ops() {
        let emergency = Emergency::<NormalOps>::new_normal();
        assert!(!emergency.is_lockdown_enabled());
    }

    #[test]
    fn test_emergency_config_default() {
        let config = EmergencyConfig::default();
        assert!(!config.lockdown_enabled);
        assert_eq!(config.security_contact, "security@example.com");
        assert!(!config.lockdown_allowed_tools.is_empty());
    }
}
