//! Resource limits and monitoring

use super::errors::SecurityError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Resource limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory usage in MB
    pub max_memory_mb: u64,
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
    /// Maximum execution time for single operation
    #[serde(with = "duration_serde", alias = "max_execution_time_seconds")]
    pub max_execution_time: Duration,
    /// Maximum number of concurrent operations
    pub max_concurrent_operations: u32,
    /// Maximum number of open file descriptors
    pub max_open_files: u32,
    /// Maximum disk space usage in MB
    pub max_disk_usage_mb: u64,
}

/// Custom serialization for Duration
mod duration_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 128,
            max_cpu_percent: 50.0,
            max_execution_time: Duration::from_secs(300),
            max_concurrent_operations: 10,
            max_open_files: 100,
            max_disk_usage_mb: 512,
        }
    }
}

/// Current resource usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_mb: u64,
    pub cpu_percent: f64,
    pub open_files: u32,
    pub disk_usage_mb: u64,
    pub active_operations: u32,
    #[serde(skip, default = "std::time::Instant::now")]
    pub start_time: Instant,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            memory_mb: 0,
            cpu_percent: 0.0,
            open_files: 0,
            disk_usage_mb: 0,
            active_operations: 0,
            start_time: Instant::now(),
        }
    }
}

/// Resource tracker for monitoring and enforcement
pub struct ResourceTracker {
    limits: ResourceLimits,
    usage: Arc<Mutex<HashMap<String, ResourceUsage>>>, // Keyed by agent_id
    process_monitor: Option<ProcessMonitor>,
}

impl ResourceTracker {
    pub fn new(limits: &ResourceLimits) -> Self {
        Self {
            limits: limits.clone(),
            usage: Arc::new(Mutex::new(HashMap::new())),
            process_monitor: ProcessMonitor::new().ok(),
        }
    }

    pub fn check_limits(&self, context: &super::SecurityContext) -> Result<(), SecurityError> {
        let mut usage_map = self.usage.lock().unwrap();
        let usage = usage_map.entry(context.agent_id.clone()).or_default();

        // Check concurrent operations limit
        if usage.active_operations >= self.limits.max_concurrent_operations {
            return Err(SecurityError::ConcurrencyLimitExceeded {
                count: usage.active_operations,
                limit: self.limits.max_concurrent_operations,
            });
        }

        // Update current resource usage if monitor is available
        if let Some(ref monitor) = self.process_monitor
            && let Ok(current_usage) = monitor.get_current_usage()
        {
            usage.memory_mb = current_usage.memory_mb;
            usage.cpu_percent = current_usage.cpu_percent;
        }

        // Check memory limit
        if usage.memory_mb > self.limits.max_memory_mb {
            return Err(SecurityError::MemoryLimitExceeded {
                requested: usage.memory_mb,
                limit: self.limits.max_memory_mb,
            });
        }

        // Check CPU limit
        if usage.cpu_percent > self.limits.max_cpu_percent {
            return Err(SecurityError::CpuLimitExceeded {
                usage: usage.cpu_percent,
                limit: self.limits.max_cpu_percent,
            });
        }

        Ok(())
    }

    pub fn start_operation(&self, agent_id: &str) -> OperationGuard {
        let mut usage_map = self.usage.lock().unwrap();
        let usage = usage_map.entry(agent_id.to_string()).or_default();

        usage.active_operations += 1;

        OperationGuard {
            agent_id: agent_id.to_string(),
            usage: Arc::clone(&self.usage),
            start_time: Instant::now(),
        }
    }

    pub fn get_usage(&self, agent_id: &str) -> Option<ResourceUsage> {
        let usage_map = self.usage.lock().unwrap();
        usage_map.get(agent_id).cloned()
    }

    pub fn cleanup_stale_agents(&self, max_age: Duration) {
        let mut usage_map = self.usage.lock().unwrap();
        let now = Instant::now();

        usage_map.retain(|_, usage| now.duration_since(usage.start_time) < max_age);
    }
}

/// RAII guard for tracking operation lifecycle
pub struct OperationGuard {
    agent_id: String,
    usage: Arc<Mutex<HashMap<String, ResourceUsage>>>,
    start_time: Instant,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        let mut usage_map = self.usage.lock().unwrap();
        if let Some(usage) = usage_map.get_mut(&self.agent_id) {
            usage.active_operations = usage.active_operations.saturating_sub(1);
        }

        // Log operation duration for monitoring
        let duration = self.start_time.elapsed();
        tracing::debug!(
            agent_id = %self.agent_id,
            duration_ms = duration.as_millis(),
            "Operation completed"
        );
    }
}

/// Process-level resource monitoring
struct ProcessMonitor {
    #[allow(dead_code)]
    pid: u32,
}

impl ProcessMonitor {
    fn new() -> Result<Self, SecurityError> {
        Ok(Self {
            pid: std::process::id(),
        })
    }

    fn get_current_usage(&self) -> Result<ResourceUsage, SecurityError> {
        // This is a simplified implementation
        // In a real system, you'd use platform-specific APIs or libraries like `sysinfo`

        #[cfg(target_os = "linux")]
        {
            self.get_linux_usage()
        }

        #[cfg(target_os = "macos")]
        {
            self.get_macos_usage()
        }

        #[cfg(target_os = "windows")]
        {
            self.get_windows_usage()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Fallback for unsupported platforms
            Ok(ResourceUsage::default())
        }
    }

    #[cfg(target_os = "linux")]
    fn get_linux_usage(&self) -> Result<ResourceUsage, SecurityError> {
        use std::fs;

        // Read memory usage from /proc/self/status
        let status_content =
            fs::read_to_string("/proc/self/status").map_err(|_| SecurityError::ConfigError {
                message: "Failed to read process status".to_string(),
            })?;

        let mut memory_mb = 0;
        for line in status_content.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        memory_mb = kb / 1024; // Convert KB to MB
                    }
                }
                break;
            }
        }

        // For CPU usage, you'd typically need to sample over time
        // This is a simplified version
        let cpu_percent = 0.0; // TODO: Implement CPU monitoring

        Ok(ResourceUsage {
            memory_mb,
            cpu_percent,
            open_files: 0,    // TODO: Count open file descriptors
            disk_usage_mb: 0, // TODO: Calculate disk usage
            active_operations: 0,
            start_time: Instant::now(),
        })
    }

    #[cfg(target_os = "macos")]
    fn get_macos_usage(&self) -> Result<ResourceUsage, SecurityError> {
        // macOS-specific implementation using system APIs
        // This would require linking to system libraries
        Ok(ResourceUsage::default())
    }

    #[cfg(target_os = "windows")]
    fn get_windows_usage(&self) -> Result<ResourceUsage, SecurityError> {
        // Windows-specific implementation using WinAPI
        // This would require windows crate
        Ok(ResourceUsage::default())
    }
}

/// Rate limiter for controlling operation frequency
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    limit: u32,
    window: Duration,
}

impl RateLimiter {
    pub fn new(requests_per_window: u32, window: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            limit: requests_per_window,
            window,
        }
    }

    pub fn check_rate_limit(&self, key: &str) -> Result<(), SecurityError> {
        let mut requests_map = self.requests.lock().unwrap();
        let now = Instant::now();

        // Get or create request history for this key
        let requests = requests_map.entry(key.to_string()).or_default();

        // Remove old requests outside the window
        requests.retain(|&timestamp| now.duration_since(timestamp) < self.window);

        // Check if we've exceeded the limit
        if requests.len() >= self.limit as usize {
            return Err(SecurityError::RateLimitExceeded {
                requests: requests.len() as u32,
                window_seconds: self.window.as_secs() as u32,
            });
        }

        // Add current request
        requests.push(now);

        Ok(())
    }

    pub fn cleanup_stale_entries(&self) {
        let mut requests_map = self.requests.lock().unwrap();
        let now = Instant::now();

        requests_map.retain(|_, requests| {
            requests.retain(|&timestamp| now.duration_since(timestamp) < self.window);
            !requests.is_empty()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_mb, 128);
        assert_eq!(limits.max_cpu_percent, 50.0);
        assert_eq!(limits.max_execution_time, Duration::from_secs(300));
    }

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));

        // First two requests should succeed
        assert!(limiter.check_rate_limit("test_key").is_ok());
        assert!(limiter.check_rate_limit("test_key").is_ok());

        // Third request should fail
        assert!(limiter.check_rate_limit("test_key").is_err());
    }

    #[test]
    fn test_operation_guard() {
        let limits = ResourceLimits::default();
        let tracker = ResourceTracker::new(&limits);

        // Start an operation
        let _guard = tracker.start_operation("test_agent");

        // Check that active operations increased
        let usage = tracker.get_usage("test_agent").unwrap();
        assert_eq!(usage.active_operations, 1);

        // Guard drops here, should decrease active operations
        drop(_guard);

        // Check that active operations decreased
        let usage = tracker.get_usage("test_agent").unwrap();
        assert_eq!(usage.active_operations, 0);
    }
}
