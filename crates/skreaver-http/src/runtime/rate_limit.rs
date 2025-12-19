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
use std::{collections::HashMap, num::NonZeroU32, sync::Arc, time::Instant};
use tokio::sync::RwLock;

/// Rate limiter for global requests
pub type GlobalRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Rate limiter for per-IP requests  
pub type IpRateLimiter =
    RateLimiter<std::net::IpAddr, DefaultKeyedStateStore<std::net::IpAddr>, DefaultClock>;

/// Rate limiting configuration with compile-time guarantees of non-zero values
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per minute globally (guaranteed non-zero)
    pub global_rpm: NonZeroU32,
    /// Maximum requests per minute per IP (guaranteed non-zero)
    pub per_ip_rpm: NonZeroU32,
    /// Maximum requests per minute per authenticated user (guaranteed non-zero)
    pub per_user_rpm: NonZeroU32,
    /// Maximum number of user limiters to keep in memory (prevents DoS via memory exhaustion)
    pub max_user_limiters: usize,
    /// Time after which inactive user limiters are cleaned up
    pub user_limiter_ttl_secs: u64,
}

impl RateLimitConfig {
    /// Create a new rate limit configuration with type-safe non-zero values
    pub const fn new(
        global_rpm: NonZeroU32,
        per_ip_rpm: NonZeroU32,
        per_user_rpm: NonZeroU32,
    ) -> Self {
        Self {
            global_rpm,
            per_ip_rpm,
            per_user_rpm,
            max_user_limiters: 10000,    // Default max users
            user_limiter_ttl_secs: 3600, // 1 hour default
        }
    }

    /// Create with custom user limiter bounds
    pub const fn with_user_bounds(
        global_rpm: NonZeroU32,
        per_ip_rpm: NonZeroU32,
        per_user_rpm: NonZeroU32,
        max_user_limiters: usize,
        user_limiter_ttl_secs: u64,
    ) -> Self {
        Self {
            global_rpm,
            per_ip_rpm,
            per_user_rpm,
            max_user_limiters,
            user_limiter_ttl_secs,
        }
    }
}

// MEDIUM-39: Safe const initialization for NonZeroU32 values
// These constants are verified at compile time, eliminating the need for unsafe
const DEFAULT_GLOBAL_RPM: NonZeroU32 = match NonZeroU32::new(1000) {
    Some(v) => v,
    None => panic!("DEFAULT_GLOBAL_RPM must be non-zero"),
};

const DEFAULT_PER_IP_RPM: NonZeroU32 = match NonZeroU32::new(60) {
    Some(v) => v,
    None => panic!("DEFAULT_PER_IP_RPM must be non-zero"),
};

const DEFAULT_PER_USER_RPM: NonZeroU32 = match NonZeroU32::new(120) {
    Some(v) => v,
    None => panic!("DEFAULT_PER_USER_RPM must be non-zero"),
};

impl Default for RateLimitConfig {
    fn default() -> Self {
        // MEDIUM-39: Use safe const values instead of unsafe { new_unchecked() }
        Self {
            global_rpm: DEFAULT_GLOBAL_RPM, // 1000 requests per minute globally
            per_ip_rpm: DEFAULT_PER_IP_RPM, // 60 requests per minute per IP
            per_user_rpm: DEFAULT_PER_USER_RPM, // 120 requests per minute per authenticated user
            max_user_limiters: 10000,       // SECURITY: Limit to prevent memory exhaustion DoS
            user_limiter_ttl_secs: 3600,    // Clean up after 1 hour of inactivity
        }
    }
}

/// Entry tracking a user rate limiter with last access time
struct UserLimiterEntry {
    limiter: Arc<GlobalRateLimiter>,
    last_access: Instant,
}

/// Rate limiting state
///
/// SECURITY: User limiters are bounded to prevent memory exhaustion DoS attacks.
/// Entries expire after `user_limiter_ttl_secs` of inactivity and are evicted
/// when the map exceeds `max_user_limiters`.
pub struct RateLimitState {
    pub global_limiter: GlobalRateLimiter,
    pub ip_limiter: IpRateLimiter,
    user_limiters: Arc<RwLock<HashMap<String, UserLimiterEntry>>>,
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
    /// This method cannot panic because RateLimitConfig guarantees non-zero values
    /// through the type system (using NonZeroU32).
    pub fn new(config: RateLimitConfig) -> Self {
        // Create quota for global rate limiting (guaranteed non-zero by type)
        let global_quota = Quota::per_minute(config.global_rpm);
        let global_limiter = RateLimiter::direct(global_quota);

        // Create quota for per-IP rate limiting (guaranteed non-zero by type)
        let ip_quota = Quota::per_minute(config.per_ip_rpm);
        let ip_limiter = RateLimiter::keyed(ip_quota);

        Self {
            global_limiter,
            ip_limiter,
            user_limiters: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get or create a rate limiter for a specific user
    ///
    /// SECURITY: This method enforces bounds on the user limiter map to prevent
    /// memory exhaustion DoS attacks. When the map exceeds `max_user_limiters`,
    /// expired entries are cleaned up. If still over limit, oldest entries are evicted.
    async fn get_user_limiter(&self, user_id: &str) -> Arc<GlobalRateLimiter> {
        let mut user_limiters = self.user_limiters.write().await;

        // Check if entry exists and update access time
        if let Some(entry) = user_limiters.get_mut(user_id) {
            entry.last_access = Instant::now();
            return Arc::clone(&entry.limiter);
        }

        // SECURITY: Enforce maximum user limiters to prevent memory exhaustion
        if user_limiters.len() >= self.config.max_user_limiters {
            // First, try to clean up expired entries
            let ttl = std::time::Duration::from_secs(self.config.user_limiter_ttl_secs);
            let now = Instant::now();
            user_limiters.retain(|_, entry| now.duration_since(entry.last_access) < ttl);

            // If still at capacity, evict oldest entries
            if user_limiters.len() >= self.config.max_user_limiters {
                // Find and remove oldest 10% of entries
                let to_remove = (self.config.max_user_limiters / 10).max(1);
                let mut entries: Vec<_> = user_limiters
                    .iter()
                    .map(|(k, v)| (k.clone(), v.last_access))
                    .collect();
                entries.sort_by_key(|(_, last_access)| *last_access);

                for (key, _) in entries.into_iter().take(to_remove) {
                    user_limiters.remove(&key);
                }

                tracing::warn!(
                    "Rate limiter map at capacity ({}), evicted {} oldest entries",
                    self.config.max_user_limiters,
                    to_remove
                );
            }
        }

        // Create new limiter entry
        let quota = Quota::per_minute(self.config.per_user_rpm);
        let entry = UserLimiterEntry {
            limiter: Arc::new(RateLimiter::direct(quota)),
            last_access: Instant::now(),
        };
        let limiter = Arc::clone(&entry.limiter);
        user_limiters.insert(user_id.to_string(), entry);

        limiter
    }

    /// Clean up expired user limiters (called periodically)
    pub async fn cleanup_expired_limiters(&self) {
        let mut user_limiters = self.user_limiters.write().await;
        let ttl = std::time::Duration::from_secs(self.config.user_limiter_ttl_secs);
        let now = Instant::now();
        let before = user_limiters.len();

        user_limiters.retain(|_, entry| now.duration_since(entry.last_access) < ttl);

        let removed = before - user_limiters.len();
        if removed > 0 {
            tracing::debug!("Cleaned up {} expired user rate limiters", removed);
        }
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
        let config = RateLimitConfig::new(
            NonZeroU32::new(100).unwrap(),
            NonZeroU32::new(10).unwrap(),
            NonZeroU32::new(20).unwrap(),
        );

        let state = RateLimitState::new(config);

        // Test successful request
        let client_ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let result = state.check_rate_limit(client_ip, None).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_type_safety() {
        // This test verifies that RateLimitConfig requires NonZeroU32 at compile time
        let config = RateLimitConfig::new(
            NonZeroU32::new(1000).unwrap(),
            NonZeroU32::new(60).unwrap(),
            NonZeroU32::new(120).unwrap(),
        );

        assert_eq!(config.global_rpm.get(), 1000);
        assert_eq!(config.per_ip_rpm.get(), 60);
        assert_eq!(config.per_user_rpm.get(), 120);

        // Default config should also have non-zero values
        let default_config = RateLimitConfig::default();
        assert_eq!(default_config.global_rpm.get(), 1000);
        assert_eq!(default_config.per_ip_rpm.get(), 60);
        assert_eq!(default_config.per_user_rpm.get(), 120);
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

    #[tokio::test]
    async fn test_user_limiter_bounds() {
        // SECURITY: Test that user limiters are bounded
        let config = RateLimitConfig::with_user_bounds(
            NonZeroU32::new(1000).unwrap(),
            NonZeroU32::new(60).unwrap(),
            NonZeroU32::new(120).unwrap(),
            5, // Only allow 5 user limiters
            1, // 1 second TTL for testing
        );
        let state = RateLimitState::new(config);

        // Create 5 user limiters (at limit)
        for i in 0..5 {
            let _ = state.get_user_limiter(&format!("user-{}", i)).await;
        }

        // Creating 6th should evict oldest
        let _ = state.get_user_limiter("user-new").await;

        // Verify we didn't exceed the limit
        let limiters = state.user_limiters.read().await;
        assert!(
            limiters.len() <= 5,
            "User limiters should be bounded at max_user_limiters"
        );
    }

    #[tokio::test]
    async fn test_user_limiter_cleanup() {
        let config = RateLimitConfig::with_user_bounds(
            NonZeroU32::new(1000).unwrap(),
            NonZeroU32::new(60).unwrap(),
            NonZeroU32::new(120).unwrap(),
            100,
            0, // 0 second TTL - everything expires immediately
        );
        let state = RateLimitState::new(config);

        // Create a user limiter
        let _ = state.get_user_limiter("test-user").await;

        // Wait a tiny bit
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Cleanup should remove the expired entry
        state.cleanup_expired_limiters().await;

        let limiters = state.user_limiters.read().await;
        assert!(limiters.is_empty(), "Expired limiters should be cleaned up");
    }
}
