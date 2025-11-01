//! Marker types for typestate pattern in security configuration
//!
//! These zero-sized types are used as phantom type parameters to enforce
//! compile-time guarantees about security configuration states.

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
