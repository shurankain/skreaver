//! # Rate Limiting
//!
//! This module provides rate limiting middleware for the HTTP runtime,
//! protecting against abuse and ensuring fair usage of agent resources.

use governor::{
    Quota, RateLimiter,
    clock::{Clock, DefaultClock},
    state::{InMemoryState, NotKeyed, keyed::DefaultKeyedStateStore},
};
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Rate limiter for global requests
pub type GlobalRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Rate limiter for per-IP requests  
pub type IpRateLimiter =
    RateLimiter<std::net::IpAddr, DefaultKeyedStateStore<std::net::IpAddr>, DefaultClock>;

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per minute globally
    pub global_rpm: u32,
    /// Maximum requests per minute per IP
    pub per_ip_rpm: u32,
    /// Maximum requests per minute per authenticated user
    pub per_user_rpm: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            global_rpm: 1000,  // 1000 requests per minute globally
            per_ip_rpm: 60,    // 60 requests per minute per IP
            per_user_rpm: 120, // 120 requests per minute per authenticated user
        }
    }
}

/// Rate limiting state
pub struct RateLimitState {
    pub global_limiter: GlobalRateLimiter,
    pub ip_limiter: IpRateLimiter,
    pub user_limiters: Arc<RwLock<HashMap<String, Arc<GlobalRateLimiter>>>>,
    pub config: RateLimitConfig,
}

/// Rate limit error response
#[derive(Debug, Serialize)]
pub struct RateLimitError {
    pub error: String,
    pub message: String,
    pub retry_after: u64, // Seconds until next request is allowed
}

impl RateLimitState {
    /// Create a new rate limit state with the given configuration
    ///
    /// # Panics
    ///
    /// Panics if any rate limit value is 0. Use `try_new` for fallible construction.
    pub fn new(config: RateLimitConfig) -> Self {
        Self::try_new(config).expect("Rate limit configuration must have non-zero values")
    }

    /// Try to create a new rate limit state with the given configuration
    ///
    /// Returns None if any rate limit value is 0.
    pub fn try_new(config: RateLimitConfig) -> Option<Self> {
        // Create quota for global rate limiting (ensure non-zero)
        let global_quota = Quota::per_minute(std::num::NonZeroU32::new(config.global_rpm)?);
        let global_limiter = RateLimiter::direct(global_quota);

        // Create quota for per-IP rate limiting (ensure non-zero)
        let ip_quota = Quota::per_minute(std::num::NonZeroU32::new(config.per_ip_rpm)?);
        let ip_limiter = RateLimiter::keyed(ip_quota);

        Some(Self {
            global_limiter,
            ip_limiter,
            user_limiters: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }

    /// Get or create a rate limiter for a specific user
    async fn get_user_limiter(&self, user_id: &str) -> Arc<GlobalRateLimiter> {
        let mut user_limiters = self.user_limiters.write().await;

        user_limiters
            .entry(user_id.to_string())
            .or_insert_with(|| {
                // Safe: per_user_rpm was already validated in new()/try_new()
                let quota = Quota::per_minute(
                    std::num::NonZeroU32::new(self.config.per_user_rpm)
                        .expect("per_user_rpm must be non-zero (validated at construction)"),
                );
                Arc::new(RateLimiter::direct(quota))
            })
            .clone()
    }

    /// Check if a request should be rate limited
    pub async fn check_rate_limit(
        &self,
        client_ip: std::net::IpAddr,
        user_id: Option<&str>,
    ) -> Result<(), RateLimitError> {
        // Check global rate limit
        if let Err(not_until) = self.global_limiter.check() {
            // Record global rate limit exceeded metric
            if let Some(registry) = skreaver_observability::get_metrics_registry() {
                registry
                    .core_metrics()
                    .security_rate_limit_exceeded_total
                    .with_label_values(&["global"])
                    .inc();
            }

            let retry_after = not_until
                .wait_time_from(DefaultClock::default().now())
                .as_secs();
            return Err(RateLimitError {
                error: "global_rate_limit_exceeded".to_string(),
                message: "Global rate limit exceeded. Please try again later.".to_string(),
                retry_after,
            });
        }

        // Check per-IP rate limit
        if let Err(not_until) = self.ip_limiter.check_key(&client_ip) {
            // Record IP rate limit exceeded metric
            if let Some(registry) = skreaver_observability::get_metrics_registry() {
                registry
                    .core_metrics()
                    .security_rate_limit_exceeded_total
                    .with_label_values(&["ip"])
                    .inc();
            }

            let retry_after = not_until
                .wait_time_from(DefaultClock::default().now())
                .as_secs();
            return Err(RateLimitError {
                error: "ip_rate_limit_exceeded".to_string(),
                message: "IP rate limit exceeded. Please try again later.".to_string(),
                retry_after,
            });
        }

        // Check per-user rate limit if authenticated
        if let Some(user_id) = user_id {
            let user_limiter = self.get_user_limiter(user_id).await;
            if let Err(not_until) = user_limiter.check() {
                // Record user rate limit exceeded metric
                if let Some(registry) = skreaver_observability::get_metrics_registry() {
                    registry
                        .core_metrics()
                        .security_rate_limit_exceeded_total
                        .with_label_values(&["user"])
                        .inc();
                }

                let retry_after = not_until
                    .wait_time_from(DefaultClock::default().now())
                    .as_secs();
                return Err(RateLimitError {
                    error: "user_rate_limit_exceeded".to_string(),
                    message: "User rate limit exceeded. Please try again later.".to_string(),
                    retry_after,
                });
            }
        }

        Ok(())
    }
}

/// Create a configured rate limit state
pub fn create_rate_limit_state(config: RateLimitConfig) -> Arc<RateLimitState> {
    Arc::new(RateLimitState::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_rate_limit_creation() {
        let config = RateLimitConfig {
            global_rpm: 100,
            per_ip_rpm: 10,
            per_user_rpm: 20,
        };

        let state = RateLimitState::new(config);

        // Test successful request
        let client_ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let result = state.check_rate_limit(client_ip, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_user_limiter_creation() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        // Create a user limiter
        let user_limiter = state.get_user_limiter("test-user").await;

        // Should be able to make a request
        assert!(user_limiter.check().is_ok());
    }
}
