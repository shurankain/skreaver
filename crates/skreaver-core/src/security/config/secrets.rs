//! Secret management configuration types
//!
//! This module provides type-safe secret configuration using phantom types
//! to enforce compile-time guarantees about secret sources and rotation policies.

use super::types::{AutoRotate, EnvironmentOnly, FlexibleSources, ManualRotate};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Type-safe secret configuration with phantom types
///
/// Generic parameters:
/// - `E`: Environment mode (EnvironmentOnly or FlexibleSources)
/// - `R`: Rotation mode (AutoRotate or ManualRotate)
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
