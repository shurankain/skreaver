//! Health Check Components
//!
//! Provides health monitoring for Skreaver components with detailed status
//! information for observability and monitoring systems.

use crate::ObservabilityError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Health check status levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// System is healthy
    Healthy,
    /// System is degraded but functional
    Degraded { reason: String },
    /// System is unhealthy
    Unhealthy { reason: String },
}

impl HealthStatus {
    /// Check if status is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Get status as HTTP status code equivalent
    pub fn as_http_status(&self) -> u16 {
        match self {
            HealthStatus::Healthy => 200,
            HealthStatus::Degraded { .. } => 503,
            HealthStatus::Unhealthy { .. } => 503,
        }
    }

    /// Get status as string
    pub fn as_str(&self) -> &str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded { .. } => "degraded",
            HealthStatus::Unhealthy { .. } => "unhealthy",
        }
    }
}

/// Individual component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Current health status
    pub status: HealthStatus,
    /// Last check timestamp
    pub last_check: SystemTime,
    /// Response time for the health check
    pub response_time: Duration,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ComponentHealth {
    /// Create new healthy component
    pub fn healthy(name: String) -> Self {
        Self {
            name,
            status: HealthStatus::Healthy,
            last_check: SystemTime::now(),
            response_time: Duration::from_millis(0),
            metadata: HashMap::new(),
        }
    }

    /// Create degraded component
    pub fn degraded(name: String, reason: String) -> Self {
        Self {
            name,
            status: HealthStatus::Degraded { reason },
            last_check: SystemTime::now(),
            response_time: Duration::from_millis(0),
            metadata: HashMap::new(),
        }
    }

    /// Create unhealthy component
    pub fn unhealthy(name: String, reason: String) -> Self {
        Self {
            name,
            status: HealthStatus::Unhealthy { reason },
            last_check: SystemTime::now(),
            response_time: Duration::from_millis(0),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to component health
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Overall system health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    /// Overall system status
    pub status: HealthStatus,
    /// Individual component health
    pub components: HashMap<String, ComponentHealth>,
    /// Overall check timestamp
    pub timestamp: SystemTime,
    /// System uptime
    pub uptime: Duration,
}

impl SystemHealth {
    /// Create new system health from components
    pub fn from_components(components: HashMap<String, ComponentHealth>) -> Self {
        let status = Self::calculate_overall_status(&components);

        Self {
            status,
            components,
            timestamp: SystemTime::now(),
            uptime: Duration::from_secs(0), // TODO: Calculate actual uptime
        }
    }

    /// Calculate overall system status from component statuses
    fn calculate_overall_status(components: &HashMap<String, ComponentHealth>) -> HealthStatus {
        if components.is_empty() {
            return HealthStatus::Healthy;
        }

        let mut has_unhealthy = false;
        let mut has_degraded = false;
        let mut reasons = Vec::new();

        for component in components.values() {
            match &component.status {
                HealthStatus::Unhealthy { reason } => {
                    has_unhealthy = true;
                    reasons.push(format!("{}: {}", component.name, reason));
                }
                HealthStatus::Degraded { reason } => {
                    has_degraded = true;
                    reasons.push(format!("{}: {}", component.name, reason));
                }
                HealthStatus::Healthy => {}
            }
        }

        if has_unhealthy {
            HealthStatus::Unhealthy {
                reason: reasons.join(", "),
            }
        } else if has_degraded {
            HealthStatus::Degraded {
                reason: reasons.join(", "),
            }
        } else {
            HealthStatus::Healthy
        }
    }
}

/// Health checker for system components
pub struct HealthChecker {
    components: HashMap<String, Box<dyn HealthCheck + Send + Sync>>,
}

impl HealthChecker {
    /// Create new health checker
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    /// Register a health check for a component
    pub fn register<T>(&mut self, name: String, check: T)
    where
        T: HealthCheck + Send + Sync + 'static,
    {
        self.components.insert(name, Box::new(check));
    }

    /// Perform health checks on all registered components
    pub async fn check_all(&self) -> SystemHealth {
        let mut components = HashMap::new();

        for (name, checker) in &self.components {
            let start = std::time::Instant::now();
            let status = checker.check().await;
            let response_time = start.elapsed();

            let mut component = match status {
                Ok(()) => ComponentHealth::healthy(name.clone()),
                Err(reason) => ComponentHealth::unhealthy(name.clone(), reason),
            };
            component.response_time = response_time;
            components.insert(name.clone(), component);
        }

        SystemHealth::from_components(components)
    }

    /// Check specific component by name
    pub async fn check_component(&self, name: &str) -> Option<ComponentHealth> {
        let checker = self.components.get(name)?;
        let start = std::time::Instant::now();
        let status = checker.check().await;
        let response_time = start.elapsed();

        let mut component = match status {
            Ok(()) => ComponentHealth::healthy(name.to_string()),
            Err(reason) => ComponentHealth::unhealthy(name.to_string(), reason),
        };
        component.response_time = response_time;
        Some(component)
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HealthChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthChecker")
            .field(
                "components",
                &format!("{} registered health checks", self.components.len()),
            )
            .finish()
    }
}

/// Trait for health check implementations
#[async_trait::async_trait]
pub trait HealthCheck {
    /// Perform health check and return result
    async fn check(&self) -> Result<(), String>;
}

/// Basic health check that always passes
pub struct AlwaysHealthy;

#[async_trait::async_trait]
impl HealthCheck for AlwaysHealthy {
    async fn check(&self) -> Result<(), String> {
        Ok(())
    }
}

/// Initialize health check system
pub fn init_health_checks() -> Result<(), ObservabilityError> {
    // Basic initialization - in a real implementation this would
    // set up global health check registry
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(
            !HealthStatus::Degraded {
                reason: "test".to_string()
            }
            .is_healthy()
        );
        assert!(
            !HealthStatus::Unhealthy {
                reason: "test".to_string()
            }
            .is_healthy()
        );
    }

    #[test]
    fn test_component_health_creation() {
        let healthy = ComponentHealth::healthy("test".to_string());
        assert!(healthy.status.is_healthy());

        let degraded = ComponentHealth::degraded("test".to_string(), "reason".to_string());
        assert!(!degraded.status.is_healthy());
        assert_eq!(degraded.status.as_str(), "degraded");
    }

    #[test]
    fn test_system_health_calculation() {
        let mut components = HashMap::new();
        components.insert(
            "comp1".to_string(),
            ComponentHealth::healthy("comp1".to_string()),
        );
        components.insert(
            "comp2".to_string(),
            ComponentHealth::healthy("comp2".to_string()),
        );

        let system = SystemHealth::from_components(components);
        assert!(system.status.is_healthy());
    }

    #[test]
    fn test_system_health_with_degraded() {
        let mut components = HashMap::new();
        components.insert(
            "comp1".to_string(),
            ComponentHealth::healthy("comp1".to_string()),
        );
        components.insert(
            "comp2".to_string(),
            ComponentHealth::degraded("comp2".to_string(), "slow".to_string()),
        );

        let system = SystemHealth::from_components(components);
        assert!(!system.status.is_healthy());
        assert_eq!(system.status.as_str(), "degraded");
    }

    #[tokio::test]
    async fn test_health_checker() {
        let mut checker = HealthChecker::new();
        checker.register("always_healthy".to_string(), AlwaysHealthy);

        let health = checker.check_all().await;
        assert!(health.status.is_healthy());
        assert_eq!(health.components.len(), 1);
    }
}
