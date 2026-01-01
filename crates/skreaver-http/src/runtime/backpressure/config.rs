//! Configuration types for backpressure management.

use std::str::FromStr;
use std::time::Duration;

/// Backpressure strategy mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureMode {
    /// No adaptive backpressure - only enforce queue/concurrency limits
    Static,
    /// Adaptive backpressure based on system load and processing times
    Adaptive,
}

impl Default for BackpressureMode {
    fn default() -> Self {
        Self::Adaptive
    }
}

impl FromStr for BackpressureMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "static" => Ok(Self::Static),
            "adaptive" => Ok(Self::Adaptive),
            _ => Err(format!(
                "Invalid backpressure mode '{}'. Valid values: 'static', 'adaptive'",
                s
            )),
        }
    }
}

impl std::fmt::Display for BackpressureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static => write!(f, "static"),
            Self::Adaptive => write!(f, "adaptive"),
        }
    }
}

/// Configuration for backpressure and queue management
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Maximum number of requests in queue per agent
    pub max_queue_size: usize,
    /// Maximum number of concurrent requests per agent
    pub max_concurrent_requests: usize,
    /// Global maximum concurrent requests across all agents
    pub global_max_concurrent: usize,
    /// Request timeout in the queue
    pub queue_timeout: Duration,
    /// Processing timeout for individual requests
    pub processing_timeout: Duration,
    /// Backpressure strategy mode
    pub mode: BackpressureMode,
    /// Target processing time for adaptive backpressure (milliseconds)
    pub target_processing_time_ms: u64,
    /// Load factor threshold for triggering backpressure (0.0-1.0)
    pub load_threshold: f64,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 100,
            max_concurrent_requests: 10,
            global_max_concurrent: 500,
            queue_timeout: Duration::from_secs(30),
            processing_timeout: Duration::from_secs(60),
            mode: BackpressureMode::default(),
            target_processing_time_ms: 1000,
            load_threshold: 0.8,
        }
    }
}

/// Priority levels for requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backpressure_mode_from_str() {
        // Test valid values (case-insensitive)
        assert_eq!("static".parse::<BackpressureMode>().unwrap(), BackpressureMode::Static);
        assert_eq!("Static".parse::<BackpressureMode>().unwrap(), BackpressureMode::Static);
        assert_eq!("STATIC".parse::<BackpressureMode>().unwrap(), BackpressureMode::Static);

        assert_eq!("adaptive".parse::<BackpressureMode>().unwrap(), BackpressureMode::Adaptive);
        assert_eq!("Adaptive".parse::<BackpressureMode>().unwrap(), BackpressureMode::Adaptive);
        assert_eq!("ADAPTIVE".parse::<BackpressureMode>().unwrap(), BackpressureMode::Adaptive);
    }

    #[test]
    fn test_backpressure_mode_from_str_invalid() {
        // Test invalid values
        let result = "invalid".parse::<BackpressureMode>();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Invalid backpressure mode"));
        assert!(err.contains("'invalid'"));

        let result = "true".parse::<BackpressureMode>();
        assert!(result.is_err());
    }

    #[test]
    fn test_backpressure_mode_display() {
        assert_eq!(BackpressureMode::Static.to_string(), "static");
        assert_eq!(BackpressureMode::Adaptive.to_string(), "adaptive");
    }

    #[test]
    fn test_backpressure_mode_default() {
        assert_eq!(BackpressureMode::default(), BackpressureMode::Adaptive);
    }
}
