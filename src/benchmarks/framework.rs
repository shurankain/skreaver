//! Centralized Benchmark Framework
//!
//! Provides a unified interface for running benchmarks with resource monitoring,
//! baseline comparison, and standardized reporting.

use super::metrics::{CpuTracker, MemoryTracker, MetricsError, ResourceMetrics};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::{Duration, Instant};

/// Custom serialization for Duration as milliseconds
mod duration_serde {
    use super::*;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

/// Configuration for benchmark execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Name of the benchmark
    pub name: String,
    /// Number of iterations to run
    pub iterations: u32,
    /// Warmup iterations before measurement
    pub warmup_iterations: u32,
    /// Maximum duration for the benchmark
    #[serde(with = "duration_serde")]
    pub max_duration: Duration,
    /// Enable memory tracking
    pub track_memory: bool,
    /// Enable CPU tracking
    pub track_cpu: bool,
    /// Target performance thresholds
    pub thresholds: PerformanceThresholds,
}

/// Performance thresholds from DEVELOPMENT_PLAN.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// p50 latency threshold (target: < 30ms)
    pub p50_ms: f64,
    /// p95 latency threshold (target: < 200ms)
    pub p95_ms: f64,
    /// p99 latency threshold (target: < 400ms)
    pub p99_ms: f64,
    /// Maximum RSS memory in MB (target: ≤ 128MB)
    pub max_rss_mb: f64,
    /// Maximum error rate percentage (target: ≤ 0.5%)
    pub max_error_rate_percent: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            p50_ms: 30.0,
            p95_ms: 200.0,
            p99_ms: 400.0,
            max_rss_mb: 128.0,
            max_error_rate_percent: 0.5,
        }
    }
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            name: "default_benchmark".to_string(),
            iterations: 1000,
            warmup_iterations: 100,
            max_duration: Duration::from_secs(60),
            track_memory: true,
            track_cpu: true,
            thresholds: PerformanceThresholds::default(),
        }
    }
}

/// Result of a benchmark execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Configuration used for this benchmark
    pub config: BenchmarkConfig,
    /// Execution statistics
    pub stats: ExecutionStats,
    /// Resource usage metrics
    pub resources: Option<ResourceMetrics>,
    /// Whether the benchmark passed all thresholds
    pub passed: bool,
    /// Threshold violations (if any)
    pub violations: Vec<ThresholdViolation>,
    /// Timestamp when benchmark was executed
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Total number of iterations executed
    pub iterations: u32,
    /// Total execution time
    #[serde(with = "duration_serde")]
    pub total_duration: Duration,
    /// Individual iteration durations
    #[serde(with = "duration_vec_serde")]
    pub iteration_durations: Vec<Duration>,
    /// Number of errors encountered
    pub error_count: u32,
    /// Calculated percentiles
    pub percentiles: Percentiles,
}

/// Custom serialization for `Vec<Duration>`
mod duration_vec_serde {
    use super::*;

    pub fn serialize<S>(durations: &[Duration], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let millis: Vec<u64> = durations.iter().map(|d| d.as_millis() as u64).collect();
        millis.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = Vec::<u64>::deserialize(deserializer)?;
        Ok(millis.into_iter().map(Duration::from_millis).collect())
    }
}

/// Calculated percentile values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Percentiles {
    #[serde(with = "duration_serde")]
    pub p50: Duration,
    #[serde(with = "duration_serde")]
    pub p95: Duration,
    #[serde(with = "duration_serde")]
    pub p99: Duration,
    #[serde(with = "duration_serde")]
    pub min: Duration,
    #[serde(with = "duration_serde")]
    pub max: Duration,
    #[serde(with = "duration_serde")]
    pub mean: Duration,
}

/// Threshold violation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdViolation {
    pub metric: String,
    pub expected: f64,
    pub actual: f64,
    pub severity: ViolationSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Warning,
    Error,
    Critical,
}

/// Main benchmark framework
pub struct BenchmarkFramework {
    memory_tracker: Option<MemoryTracker>,
    cpu_tracker: Option<CpuTracker>,
}

impl BenchmarkFramework {
    /// Create a new benchmark framework
    pub fn new() -> Self {
        Self {
            memory_tracker: None,
            cpu_tracker: None,
        }
    }

    /// Execute a benchmark with the given configuration
    pub async fn execute_benchmark<F, Fut, R>(
        &mut self,
        config: BenchmarkConfig,
        benchmark_fn: F,
    ) -> Result<BenchmarkResult, BenchmarkError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<R, Box<dyn std::error::Error>>>,
    {
        // Initialize resource tracking
        if config.track_memory {
            self.memory_tracker = Some(MemoryTracker::new()?);
        }
        if config.track_cpu {
            self.cpu_tracker = Some(CpuTracker::new()?);
        }

        // Start resource monitoring
        if let Some(ref mut tracker) = self.memory_tracker {
            tracker.start_monitoring()?;
        }
        if let Some(ref mut tracker) = self.cpu_tracker {
            tracker.start_monitoring()?;
        }

        // Warmup iterations
        for _ in 0..config.warmup_iterations {
            let _ = benchmark_fn().await;
        }

        // Main benchmark execution
        let start_time = Instant::now();
        let mut iteration_durations = Vec::with_capacity(config.iterations as usize);
        let mut error_count = 0u32;

        for i in 0..config.iterations {
            let iter_start = Instant::now();

            match benchmark_fn().await {
                Ok(_) => {
                    let iter_duration = iter_start.elapsed();
                    iteration_durations.push(iter_duration);
                }
                Err(_) => {
                    error_count += 1;
                    // Still record the duration for failed iterations
                    let iter_duration = iter_start.elapsed();
                    iteration_durations.push(iter_duration);
                }
            }

            // Check if we've exceeded max duration
            if start_time.elapsed() > config.max_duration {
                tracing::warn!(
                    "Benchmark '{}' exceeded max duration, stopping at iteration {}/{}",
                    config.name,
                    i + 1,
                    config.iterations
                );
                break;
            }
        }

        let total_duration = start_time.elapsed();

        // Stop resource monitoring and collect metrics
        let resources = if config.track_memory || config.track_cpu {
            let mut metrics = ResourceMetrics::default();

            if let Some(ref mut tracker) = self.memory_tracker {
                tracker.stop_monitoring()?;
                metrics.memory = Some(tracker.get_metrics()?);
            }
            if let Some(ref mut tracker) = self.cpu_tracker {
                tracker.stop_monitoring()?;
                metrics.cpu = Some(tracker.get_metrics()?);
            }

            Some(metrics)
        } else {
            None
        };

        // Calculate statistics
        let stats = self.calculate_stats(iteration_durations, total_duration, error_count);

        // Check thresholds and create result
        let (passed, violations) = self.check_thresholds(&config.thresholds, &stats, &resources);

        Ok(BenchmarkResult {
            config,
            stats,
            resources,
            passed,
            violations,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Calculate execution statistics from raw data
    fn calculate_stats(
        &self,
        mut iteration_durations: Vec<Duration>,
        total_duration: Duration,
        error_count: u32,
    ) -> ExecutionStats {
        iteration_durations.sort();

        let len = iteration_durations.len();
        let percentiles = if len > 0 {
            Percentiles {
                p50: iteration_durations[len * 50 / 100],
                p95: iteration_durations[len * 95 / 100],
                p99: iteration_durations[len * 99 / 100],
                min: iteration_durations[0],
                max: iteration_durations[len - 1],
                mean: Duration::from_nanos(
                    (iteration_durations
                        .iter()
                        .map(|d| d.as_nanos())
                        .sum::<u128>()
                        / len as u128)
                        .try_into()
                        .unwrap_or(u64::MAX),
                ),
            }
        } else {
            Percentiles {
                p50: Duration::ZERO,
                p95: Duration::ZERO,
                p99: Duration::ZERO,
                min: Duration::ZERO,
                max: Duration::ZERO,
                mean: Duration::ZERO,
            }
        };

        ExecutionStats {
            iterations: len as u32,
            total_duration,
            iteration_durations,
            error_count,
            percentiles,
        }
    }

    /// Check if benchmark results meet the configured thresholds
    fn check_thresholds(
        &self,
        thresholds: &PerformanceThresholds,
        stats: &ExecutionStats,
        resources: &Option<ResourceMetrics>,
    ) -> (bool, Vec<ThresholdViolation>) {
        let mut violations = Vec::new();

        // Check latency thresholds
        let p50_ms = stats.percentiles.p50.as_millis() as f64;
        if p50_ms > thresholds.p50_ms {
            violations.push(ThresholdViolation {
                metric: "p50_latency_ms".to_string(),
                expected: thresholds.p50_ms,
                actual: p50_ms,
                severity: ViolationSeverity::Error,
            });
        }

        let p95_ms = stats.percentiles.p95.as_millis() as f64;
        if p95_ms > thresholds.p95_ms {
            violations.push(ThresholdViolation {
                metric: "p95_latency_ms".to_string(),
                expected: thresholds.p95_ms,
                actual: p95_ms,
                severity: ViolationSeverity::Error,
            });
        }

        let p99_ms = stats.percentiles.p99.as_millis() as f64;
        if p99_ms > thresholds.p99_ms {
            violations.push(ThresholdViolation {
                metric: "p99_latency_ms".to_string(),
                expected: thresholds.p99_ms,
                actual: p99_ms,
                severity: ViolationSeverity::Critical,
            });
        }

        // Check error rate threshold
        let error_rate = (stats.error_count as f64 / stats.iterations as f64) * 100.0;
        if error_rate > thresholds.max_error_rate_percent {
            violations.push(ThresholdViolation {
                metric: "error_rate_percent".to_string(),
                expected: thresholds.max_error_rate_percent,
                actual: error_rate,
                severity: ViolationSeverity::Critical,
            });
        }

        // Check memory threshold if available
        if let Some(metrics) = resources
            && let Some(ref memory) = metrics.memory
        {
            let rss_mb = memory.peak_rss_bytes as f64 / 1_048_576.0; // Convert bytes to MB
            if rss_mb > thresholds.max_rss_mb {
                violations.push(ThresholdViolation {
                    metric: "rss_memory_mb".to_string(),
                    expected: thresholds.max_rss_mb,
                    actual: rss_mb,
                    severity: ViolationSeverity::Error,
                });
            }
        }

        let passed = violations.is_empty();
        (passed, violations)
    }
}

impl Default for BenchmarkFramework {
    fn default() -> Self {
        Self::new()
    }
}

/// Benchmark execution errors
#[derive(Debug, thiserror::Error)]
pub enum BenchmarkError {
    #[error("Resource monitoring failed: {0}")]
    ResourceMonitoring(String),
    #[error("Benchmark execution failed: {0}")]
    Execution(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Metrics error: {0}")]
    Metrics(#[from] MetricsError),
}
