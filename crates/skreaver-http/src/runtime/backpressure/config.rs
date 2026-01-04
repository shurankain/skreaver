//! Configuration types for backpressure management.

use std::num::NonZeroUsize;
use std::str::FromStr;
use std::time::Duration;

use crate::runtime::config::ConfigError;

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

/// Validated queue size (1-10,000)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueueSize(NonZeroUsize);

impl QueueSize {
    pub const MAX: usize = 10_000;

    pub fn new(size: usize) -> Result<Self, ConfigError> {
        let non_zero = NonZeroUsize::new(size).ok_or_else(|| {
            ConfigError::ValidationError("queue size must be at least 1".to_string())
        })?;
        if size > Self::MAX {
            return Err(ConfigError::ValidationError(format!(
                "queue size must be at most {}",
                Self::MAX
            )));
        }
        Ok(Self(non_zero))
    }

    pub fn get(&self) -> usize {
        self.0.get()
    }
}

impl std::fmt::Display for QueueSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Validated concurrency limit (1-1,000)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConcurrencyLimit(NonZeroUsize);

impl ConcurrencyLimit {
    pub const MAX: usize = 1_000;

    pub fn new(limit: usize) -> Result<Self, ConfigError> {
        let non_zero = NonZeroUsize::new(limit).ok_or_else(|| {
            ConfigError::ValidationError("concurrency limit must be at least 1".to_string())
        })?;
        if limit > Self::MAX {
            return Err(ConfigError::ValidationError(format!(
                "concurrency limit must be at most {}",
                Self::MAX
            )));
        }
        Ok(Self(non_zero))
    }

    pub fn get(&self) -> usize {
        self.0.get()
    }
}

impl std::fmt::Display for ConcurrencyLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Validated load threshold (0.0-1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct LoadThreshold(f64);

impl LoadThreshold {
    pub fn new(threshold: f64) -> Result<Self, ConfigError> {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(ConfigError::ValidationError(
                "load threshold must be between 0.0 and 1.0".to_string(),
            ));
        }
        Ok(Self(threshold))
    }

    pub fn get(&self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for LoadThreshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for backpressure and queue management
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Maximum number of requests in queue per agent
    pub max_queue_size: QueueSize,
    /// Maximum number of concurrent requests per agent
    pub max_concurrent_requests: ConcurrencyLimit,
    /// Global maximum concurrent requests across all agents
    pub global_max_concurrent: ConcurrencyLimit,
    /// Request timeout in the queue
    pub queue_timeout: Duration,
    /// Processing timeout for individual requests
    pub processing_timeout: Duration,
    /// Backpressure strategy mode
    pub mode: BackpressureMode,
    /// Target processing time for adaptive backpressure (milliseconds)
    pub target_processing_time_ms: u64,
    /// Load factor threshold for triggering backpressure (0.0-1.0)
    pub load_threshold: LoadThreshold,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            // SAFETY: These values are within valid ranges
            max_queue_size: QueueSize::new(100).expect("default queue size is valid"),
            max_concurrent_requests: ConcurrencyLimit::new(10)
                .expect("default concurrency limit is valid"),
            global_max_concurrent: ConcurrencyLimit::new(500)
                .expect("default global concurrency limit is valid"),
            queue_timeout: Duration::from_secs(30),
            processing_timeout: Duration::from_secs(60),
            mode: BackpressureMode::default(),
            target_processing_time_ms: 1000,
            load_threshold: LoadThreshold::new(0.8).expect("default load threshold is valid"),
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
        assert_eq!(
            "static".parse::<BackpressureMode>().unwrap(),
            BackpressureMode::Static
        );
        assert_eq!(
            "Static".parse::<BackpressureMode>().unwrap(),
            BackpressureMode::Static
        );
        assert_eq!(
            "STATIC".parse::<BackpressureMode>().unwrap(),
            BackpressureMode::Static
        );

        assert_eq!(
            "adaptive".parse::<BackpressureMode>().unwrap(),
            BackpressureMode::Adaptive
        );
        assert_eq!(
            "Adaptive".parse::<BackpressureMode>().unwrap(),
            BackpressureMode::Adaptive
        );
        assert_eq!(
            "ADAPTIVE".parse::<BackpressureMode>().unwrap(),
            BackpressureMode::Adaptive
        );
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

    #[test]
    fn test_queue_size_validation() {
        // Valid queue sizes
        assert!(QueueSize::new(1).is_ok());
        assert!(QueueSize::new(100).is_ok());
        assert!(QueueSize::new(QueueSize::MAX).is_ok());

        // Invalid: zero
        assert!(QueueSize::new(0).is_err());

        // Invalid: exceeds max
        assert!(QueueSize::new(QueueSize::MAX + 1).is_err());
    }

    #[test]
    fn test_concurrency_limit_validation() {
        // Valid concurrency limits
        assert!(ConcurrencyLimit::new(1).is_ok());
        assert!(ConcurrencyLimit::new(100).is_ok());
        assert!(ConcurrencyLimit::new(ConcurrencyLimit::MAX).is_ok());

        // Invalid: zero
        assert!(ConcurrencyLimit::new(0).is_err());

        // Invalid: exceeds max
        assert!(ConcurrencyLimit::new(ConcurrencyLimit::MAX + 1).is_err());
    }

    #[test]
    fn test_load_threshold_validation() {
        // Valid load thresholds
        assert!(LoadThreshold::new(0.0).is_ok());
        assert!(LoadThreshold::new(0.5).is_ok());
        assert!(LoadThreshold::new(1.0).is_ok());

        // Invalid: negative
        assert!(LoadThreshold::new(-0.1).is_err());

        // Invalid: exceeds 1.0
        assert!(LoadThreshold::new(1.1).is_err());
        assert!(LoadThreshold::new(2.0).is_err());
    }

    #[test]
    fn test_newtype_get_methods() {
        let queue_size = QueueSize::new(100).unwrap();
        assert_eq!(queue_size.get(), 100);

        let concurrency = ConcurrencyLimit::new(50).unwrap();
        assert_eq!(concurrency.get(), 50);

        let threshold = LoadThreshold::new(0.8).unwrap();
        assert_eq!(threshold.get(), 0.8);
    }
}
