//! HTTP Connection Limits
//!
//! This module provides connection tracking and limiting for the HTTP runtime
//! to prevent resource exhaustion and ensure stability under high load.
//!
//! Similar to WebSocket connection limits, but for regular HTTP connections.

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Behavior when ConnectInfo is missing from the request
///
/// This determines how the connection limit middleware handles requests
/// that don't have client IP information (e.g., behind certain proxies).
#[derive(Debug, Clone, PartialEq)]
pub enum MissingConnectInfoBehavior {
    /// Reject requests without ConnectInfo with 400 Bad Request
    ///
    /// Safest option for production. Requires proxy to forward client IP.
    /// Use X-Forwarded-For header or PROXY protocol with your load balancer.
    Reject,

    /// Use a fallback IP address for tracking
    ///
    /// Useful for development/testing, but in production this means all
    /// requests without ConnectInfo share the same connection limit.
    /// This can cause legitimate users to be rate-limited together.
    UseFallback(IpAddr),

    /// Disable per-IP limits when ConnectInfo is missing
    ///
    /// Only global connection limit is enforced. Use when you trust the
    /// infrastructure and want to allow requests through.
    DisablePerIpLimits,
}

impl Default for MissingConnectInfoBehavior {
    fn default() -> Self {
        // Default to fallback for ease of development and testing
        // Production deployments should explicitly configure this to Reject for maximum security
        // via environment variable: SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR=reject

        // Safety: "127.0.0.1" is a valid IPv4 address constant
        Self::UseFallback("127.0.0.1".parse().expect("127.0.0.1 is valid IP"))
    }
}

/// Connection limit enforcement mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionLimitMode {
    /// Connection limiting is disabled - no enforcement
    Disabled,
    /// Connection limiting is enabled and enforced
    Enabled,
}

impl Default for ConnectionLimitMode {
    fn default() -> Self {
        Self::Enabled
    }
}

/// Configuration for HTTP connection limits
#[derive(Debug, Clone)]
pub struct ConnectionLimitConfig {
    /// Maximum total concurrent HTTP connections (default: 10,000)
    pub max_connections: usize,
    /// Maximum concurrent connections per IP address (default: 100)
    pub max_connections_per_ip: usize,
    /// Connection limiting mode
    pub mode: ConnectionLimitMode,
    /// Behavior when ConnectInfo is missing from request
    pub missing_connect_info_behavior: MissingConnectInfoBehavior,
}

impl Default for ConnectionLimitConfig {
    fn default() -> Self {
        Self {
            max_connections: 10_000,
            max_connections_per_ip: 100,
            mode: ConnectionLimitMode::default(),
            missing_connect_info_behavior: MissingConnectInfoBehavior::default(),
        }
    }
}

/// Connection tracker for HTTP requests
#[derive(Debug, Clone)]
pub struct ConnectionTracker {
    config: ConnectionLimitConfig,
    /// Total active connections
    active_connections: Arc<AtomicUsize>,
    /// Connections per IP address
    connections_per_ip: Arc<RwLock<HashMap<IpAddr, usize>>>,
}

impl ConnectionTracker {
    /// Create a new connection tracker with the given configuration
    pub fn new(config: ConnectionLimitConfig) -> Self {
        Self {
            config,
            active_connections: Arc::new(AtomicUsize::new(0)),
            connections_per_ip: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get current number of active connections
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Get connection count for a specific IP
    pub async fn connections_for_ip(&self, ip: &IpAddr) -> usize {
        let connections = self.connections_per_ip.read().await;
        connections.get(ip).copied().unwrap_or(0)
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> &ConnectionLimitConfig {
        &self.config
    }

    /// Check if we can accept a new connection from the given IP
    ///
    /// Returns Ok(()) if connection can be accepted, Err with status code otherwise
    pub async fn check_limits(&self, ip: IpAddr) -> Result<(), StatusCode> {
        if self.config.mode == ConnectionLimitMode::Disabled {
            return Ok(());
        }

        // Check global connection limit
        let total = self.active_connections.load(Ordering::Relaxed);
        if total >= self.config.max_connections {
            warn!(
                "Global connection limit exceeded: {} >= {}",
                total, self.config.max_connections
            );
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }

        // Check per-IP connection limit
        let connections = self.connections_per_ip.read().await;
        let ip_count = connections.get(&ip).copied().unwrap_or(0);
        if ip_count >= self.config.max_connections_per_ip {
            warn!(
                "Per-IP connection limit exceeded for {}: {} >= {}",
                ip, ip_count, self.config.max_connections_per_ip
            );
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }

        Ok(())
    }

    /// Increment connection counters for the given IP
    pub async fn increment(&self, ip: IpAddr) {
        if self.config.mode == ConnectionLimitMode::Disabled {
            return;
        }

        // Increment global counter
        self.active_connections.fetch_add(1, Ordering::Relaxed);

        // Increment per-IP counter
        let mut connections = self.connections_per_ip.write().await;
        let count = connections.entry(ip).or_insert(0);
        *count += 1;
        debug!(
            "Connection from {} incremented (total: {})",
            ip,
            count
        );
    }

    /// Decrement connection counters for the given IP
    pub async fn decrement(&self, ip: IpAddr) {
        if self.config.mode == ConnectionLimitMode::Disabled {
            return;
        }

        // Decrement global counter
        self.active_connections.fetch_sub(1, Ordering::Relaxed);

        // Decrement per-IP counter
        let mut connections = self.connections_per_ip.write().await;
        if let Some(count) = connections.get_mut(&ip) {
            *count = count.saturating_sub(1);
            let remaining = *count;
            if remaining == 0 {
                connections.remove(&ip);
            }
            debug!(
                "Connection from {} decremented (remaining: {})",
                ip, remaining
            );
        }
    }

    /// Get current statistics
    pub async fn stats(&self) -> ConnectionStats {
        let connections = self.connections_per_ip.read().await;
        ConnectionStats {
            total_connections: self.active_connections.load(Ordering::Relaxed),
            unique_ips: connections.len(),
            max_connections: self.config.max_connections,
            max_connections_per_ip: self.config.max_connections_per_ip,
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// Total active connections
    pub total_connections: usize,
    /// Number of unique IP addresses
    pub unique_ips: usize,
    /// Maximum allowed connections
    pub max_connections: usize,
    /// Maximum connections per IP
    pub max_connections_per_ip: usize,
}

/// Connection guard that automatically decrements counters when dropped
pub struct ConnectionGuard {
    tracker: ConnectionTracker,
    ip: IpAddr,
}

impl ConnectionGuard {
    /// Create a new connection guard
    pub fn new(tracker: ConnectionTracker, ip: IpAddr) -> Self {
        Self { tracker, ip }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let tracker = self.tracker.clone();
        let ip = self.ip;
        tokio::spawn(async move {
            tracker.decrement(ip).await;
        });
    }
}

/// Middleware for connection tracking and limiting
///
/// This middleware tracks and limits HTTP connections both globally and per-IP.
/// It handles missing ConnectInfo based on the configured behavior.
pub async fn connection_limit_middleware(
    axum::extract::State(tracker): axum::extract::State<Arc<ConnectionTracker>>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // Try to extract ConnectInfo
    let (mut parts, body) = request.into_parts();

    let ip_option = match ConnectInfo::<SocketAddr>::from_request_parts(&mut parts, &()).await {
        Ok(ConnectInfo(addr)) => Some(addr.ip()),
        Err(_) => {
            // No ConnectInfo available - handle based on configuration
            match &tracker.config.missing_connect_info_behavior {
                MissingConnectInfoBehavior::Reject => {
                    return (
                        StatusCode::BAD_REQUEST,
                        "Missing connection info. Configure your proxy to forward client IP addresses."
                    ).into_response();
                }
                MissingConnectInfoBehavior::UseFallback(fallback_ip) => {
                    debug!("ConnectInfo missing, using fallback IP: {}", fallback_ip);
                    Some(*fallback_ip)
                }
                MissingConnectInfoBehavior::DisablePerIpLimits => {
                    debug!("ConnectInfo missing, per-IP limits disabled for this request");
                    None
                }
            }
        }
    };

    // Reconstruct request
    request = Request::from_parts(parts, body);

    // Check global limit first (always enforced)
    let total = tracker.active_connections.load(Ordering::Relaxed);
    if total >= tracker.config.max_connections {
        warn!(
            "Global connection limit exceeded: {} >= {}",
            total, tracker.config.max_connections
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "Global connection limit exceeded: {}/{}",
                total, tracker.config.max_connections
            ),
        )
            .into_response();
    }

    // If we have an IP, check per-IP limits and track
    if let Some(ip) = ip_option {
        // Check per-IP limit
        if let Err(status) = tracker.check_limits(ip).await {
            return (
                status,
                format!(
                    "Connection limit exceeded. Try again later. (Global: {}/{}, Per-IP: {}/{})",
                    tracker.active_connections(),
                    tracker.config.max_connections,
                    tracker.connections_for_ip(&ip).await,
                    tracker.config.max_connections_per_ip
                ),
            )
                .into_response();
        }

        // Increment counters
        tracker.increment(ip).await;

        // Create guard to ensure decrement on drop
        let _guard = ConnectionGuard::new(tracker.as_ref().clone(), ip);

        // Process request
        next.run(request).await
    } else {
        // No IP tracking - only global limit enforced
        tracker.active_connections.fetch_add(1, Ordering::Relaxed);
        let response = next.run(request).await;
        tracker.active_connections.fetch_sub(1, Ordering::Relaxed);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_tracker_basic() {
        let config = ConnectionLimitConfig {
            max_connections: 100,
            max_connections_per_ip: 10,
            mode: ConnectionLimitMode::Enabled,
            missing_connect_info_behavior: MissingConnectInfoBehavior::UseFallback(
                "127.0.0.1".parse().unwrap(),
            ),
        };
        let tracker = ConnectionTracker::new(config);

        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        assert_eq!(tracker.active_connections(), 0);
        assert_eq!(tracker.connections_for_ip(&ip).await, 0);

        tracker.increment(ip).await;
        assert_eq!(tracker.active_connections(), 1);
        assert_eq!(tracker.connections_for_ip(&ip).await, 1);

        tracker.decrement(ip).await;
        assert_eq!(tracker.active_connections(), 0);
        assert_eq!(tracker.connections_for_ip(&ip).await, 0);
    }

    #[tokio::test]
    async fn test_global_limit() {
        let config = ConnectionLimitConfig {
            max_connections: 2,
            max_connections_per_ip: 10,
            mode: ConnectionLimitMode::Enabled,
            missing_connect_info_behavior: MissingConnectInfoBehavior::UseFallback(
                "127.0.0.1".parse().unwrap(),
            ),
        };
        let tracker = ConnectionTracker::new(config);

        let ip1: IpAddr = "127.0.0.1".parse().unwrap();
        let ip2: IpAddr = "127.0.0.2".parse().unwrap();
        let ip3: IpAddr = "127.0.0.3".parse().unwrap();

        // First two connections should succeed
        assert!(tracker.check_limits(ip1).await.is_ok());
        tracker.increment(ip1).await;

        assert!(tracker.check_limits(ip2).await.is_ok());
        tracker.increment(ip2).await;

        // Third connection should be rejected (global limit)
        assert_eq!(
            tracker.check_limits(ip3).await,
            Err(StatusCode::SERVICE_UNAVAILABLE)
        );
    }

    #[tokio::test]
    async fn test_per_ip_limit() {
        let config = ConnectionLimitConfig {
            max_connections: 100,
            max_connections_per_ip: 2,
            mode: ConnectionLimitMode::Enabled,
            missing_connect_info_behavior: MissingConnectInfoBehavior::UseFallback(
                "127.0.0.1".parse().unwrap(),
            ),
        };
        let tracker = ConnectionTracker::new(config);

        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        // First two connections should succeed
        assert!(tracker.check_limits(ip).await.is_ok());
        tracker.increment(ip).await;

        assert!(tracker.check_limits(ip).await.is_ok());
        tracker.increment(ip).await;

        // Third connection from same IP should be rejected
        assert_eq!(
            tracker.check_limits(ip).await,
            Err(StatusCode::TOO_MANY_REQUESTS)
        );
    }

    #[tokio::test]
    async fn test_disabled_limits() {
        let config = ConnectionLimitConfig {
            max_connections: 1,
            max_connections_per_ip: 1,
            mode: ConnectionLimitMode::Disabled,
            missing_connect_info_behavior: MissingConnectInfoBehavior::UseFallback(
                "127.0.0.1".parse().unwrap(),
            ),
        };
        let tracker = ConnectionTracker::new(config);

        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        // Should always succeed when disabled
        for _ in 0..10 {
            assert!(tracker.check_limits(ip).await.is_ok());
            tracker.increment(ip).await;
        }

        // Counters should not be incremented when disabled
        assert_eq!(tracker.active_connections(), 0);
    }

    #[tokio::test]
    async fn test_stats() {
        let config = ConnectionLimitConfig::default();
        let tracker = ConnectionTracker::new(config.clone());

        let ip1: IpAddr = "127.0.0.1".parse().unwrap();
        let ip2: IpAddr = "127.0.0.2".parse().unwrap();

        tracker.increment(ip1).await;
        tracker.increment(ip1).await;
        tracker.increment(ip2).await;

        let stats = tracker.stats().await;
        assert_eq!(stats.total_connections, 3);
        assert_eq!(stats.unique_ips, 2);
        assert_eq!(stats.max_connections, config.max_connections);
        assert_eq!(stats.max_connections_per_ip, config.max_connections_per_ip);
    }
}
