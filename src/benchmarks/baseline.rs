//! Performance Baseline Management
//!
//! Manages historical performance baselines for regression detection.

use super::framework::{BenchmarkResult, PerformanceThresholds};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Performance baseline data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    /// Baseline name/identifier
    pub name: String,
    /// Git commit hash when baseline was created
    pub commit_hash: Option<String>,
    /// Timestamp when baseline was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Benchmark results that form this baseline
    pub results: Vec<BenchmarkResult>,
    /// Calculated performance thresholds from this baseline
    pub thresholds: PerformanceThresholds,
    /// Metadata about the environment when baseline was created
    pub environment: EnvironmentInfo,
}

/// Environment information for baseline context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    /// Operating system
    pub os: String,
    /// CPU information
    pub cpu_info: String,
    /// Total memory in MB
    pub total_memory_mb: u64,
    /// Rust version used
    pub rust_version: String,
    /// Additional environment variables or flags
    pub metadata: HashMap<String, String>,
}

impl EnvironmentInfo {
    /// Capture current environment information
    pub fn capture() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            cpu_info: get_cpu_info(),
            total_memory_mb: get_total_memory_mb(),
            rust_version: get_rust_version(),
            metadata: HashMap::new(),
        }
    }
}

/// Manages performance baselines
pub struct BaselineManager {
    /// Directory to store baseline files
    baseline_dir: PathBuf,
    /// Currently loaded baselines
    baselines: HashMap<String, PerformanceBaseline>,
}

impl BaselineManager {
    /// Create a new baseline manager
    pub fn new<P: AsRef<Path>>(baseline_dir: P) -> Result<Self, BaselineError> {
        let baseline_dir = baseline_dir.as_ref().to_path_buf();

        // Create baseline directory if it doesn't exist
        if !baseline_dir.exists() {
            fs::create_dir_all(&baseline_dir).map_err(|e| {
                BaselineError::Io(format!("Failed to create baseline directory: {}", e))
            })?;
        }

        let mut manager = Self {
            baseline_dir,
            baselines: HashMap::new(),
        };

        // Load existing baselines
        manager.load_all_baselines()?;

        Ok(manager)
    }

    /// Create a new baseline from benchmark results
    pub fn create_baseline(
        &mut self,
        name: String,
        results: Vec<BenchmarkResult>,
        commit_hash: Option<String>,
    ) -> Result<PerformanceBaseline, BaselineError> {
        if results.is_empty() {
            return Err(BaselineError::InvalidData(
                "Cannot create baseline from empty results".to_string(),
            ));
        }

        // Calculate thresholds from results with some safety margin
        let thresholds = self.calculate_thresholds_from_results(&results)?;

        let baseline = PerformanceBaseline {
            name: name.clone(),
            commit_hash,
            created_at: chrono::Utc::now(),
            results,
            thresholds,
            environment: EnvironmentInfo::capture(),
        };

        // Save baseline to file
        self.save_baseline(&baseline)?;

        // Add to loaded baselines
        self.baselines.insert(name, baseline.clone());

        Ok(baseline)
    }

    /// Get a baseline by name
    pub fn get_baseline(&self, name: &str) -> Option<&PerformanceBaseline> {
        self.baselines.get(name)
    }

    /// List all available baselines
    pub fn list_baselines(&self) -> Vec<&str> {
        self.baselines.keys().map(|s| s.as_str()).collect()
    }

    /// Compare results against a baseline
    pub fn compare_with_baseline(
        &self,
        baseline_name: &str,
        current_results: &[BenchmarkResult],
    ) -> Result<BaselineComparison, BaselineError> {
        let baseline = self.get_baseline(baseline_name).ok_or_else(|| {
            BaselineError::NotFound(format!("Baseline '{}' not found", baseline_name))
        })?;

        let mut comparisons = Vec::new();

        // Compare each current result with corresponding baseline result
        for current in current_results {
            if let Some(baseline_result) = baseline
                .results
                .iter()
                .find(|r| r.config.name == current.config.name)
            {
                let comparison = ResultComparison {
                    benchmark_name: current.config.name.clone(),
                    current_p50_ms: current.stats.percentiles.p50.as_millis() as f64,
                    baseline_p50_ms: baseline_result.stats.percentiles.p50.as_millis() as f64,
                    current_p95_ms: current.stats.percentiles.p95.as_millis() as f64,
                    baseline_p95_ms: baseline_result.stats.percentiles.p95.as_millis() as f64,
                    current_memory_mb: current
                        .resources
                        .as_ref()
                        .and_then(|r| r.memory.as_ref())
                        .map(|m| m.peak_rss_bytes as f64 / 1_048_576.0),
                    baseline_memory_mb: baseline_result
                        .resources
                        .as_ref()
                        .and_then(|r| r.memory.as_ref())
                        .map(|m| m.peak_rss_bytes as f64 / 1_048_576.0),
                    regression_detected: self.detect_regression(current, baseline_result),
                };

                comparisons.push(comparison);
            }
        }

        Ok(BaselineComparison {
            baseline_name: baseline_name.to_string(),
            baseline_created_at: baseline.created_at,
            comparison_timestamp: chrono::Utc::now(),
            comparisons,
        })
    }

    /// Load all baseline files from the baseline directory
    fn load_all_baselines(&mut self) -> Result<(), BaselineError> {
        let entries = fs::read_dir(&self.baseline_dir)
            .map_err(|e| BaselineError::Io(format!("Failed to read baseline directory: {}", e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| BaselineError::Io(format!("Failed to read directory entry: {}", e)))?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_baseline_from_file(&path) {
                    Ok(baseline) => {
                        self.baselines.insert(baseline.name.clone(), baseline);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load baseline from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single baseline from file
    fn load_baseline_from_file(&self, path: &Path) -> Result<PerformanceBaseline, BaselineError> {
        let content = fs::read_to_string(path)
            .map_err(|e| BaselineError::Io(format!("Failed to read baseline file: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| BaselineError::Serialization(format!("Failed to parse baseline: {}", e)))
    }

    /// Save a baseline to file
    fn save_baseline(&self, baseline: &PerformanceBaseline) -> Result<(), BaselineError> {
        let filename = format!("{}.json", sanitize_filename(&baseline.name));
        let path = self.baseline_dir.join(filename);

        let content = serde_json::to_string_pretty(baseline).map_err(|e| {
            BaselineError::Serialization(format!("Failed to serialize baseline: {}", e))
        })?;

        fs::write(path, content)
            .map_err(|e| BaselineError::Io(format!("Failed to write baseline file: {}", e)))?;

        Ok(())
    }

    /// Calculate performance thresholds from benchmark results
    fn calculate_thresholds_from_results(
        &self,
        results: &[BenchmarkResult],
    ) -> Result<PerformanceThresholds, BaselineError> {
        if results.is_empty() {
            return Err(BaselineError::InvalidData(
                "No results provided".to_string(),
            ));
        }

        // Calculate percentiles across all results with safety margins
        let mut p50_values = Vec::new();
        let mut p95_values = Vec::new();
        let mut p99_values = Vec::new();
        let mut memory_values = Vec::new();

        for result in results {
            p50_values.push(result.stats.percentiles.p50.as_millis() as f64);
            p95_values.push(result.stats.percentiles.p95.as_millis() as f64);
            p99_values.push(result.stats.percentiles.p99.as_millis() as f64);

            if let Some(ref resources) = result.resources {
                if let Some(ref memory) = resources.memory {
                    memory_values.push(memory.peak_rss_bytes as f64 / 1_048_576.0);
                }
            }
        }

        // Calculate thresholds with 20% safety margin
        let safety_margin = 1.2;

        let p50_threshold = calculate_percentile(&mut p50_values, 0.95) * safety_margin; // 95th percentile of p50s
        let p95_threshold = calculate_percentile(&mut p95_values, 0.95) * safety_margin;
        let p99_threshold = calculate_percentile(&mut p99_values, 0.95) * safety_margin;

        let memory_threshold = if memory_values.is_empty() {
            128.0 // Default from development plan
        } else {
            calculate_percentile(&mut memory_values, 0.95) * safety_margin
        };

        Ok(PerformanceThresholds {
            p50_ms: p50_threshold.max(30.0), // At least development plan targets
            p95_ms: p95_threshold.max(200.0),
            p99_ms: p99_threshold.max(400.0),
            max_rss_mb: memory_threshold.max(128.0),
            max_error_rate_percent: 0.5, // From development plan
        })
    }

    /// Detect if there's a performance regression
    fn detect_regression(&self, current: &BenchmarkResult, baseline: &BenchmarkResult) -> bool {
        let regression_threshold = 1.1; // 10% regression threshold

        // Check latency regression
        let current_p95 = current.stats.percentiles.p95.as_millis() as f64;
        let baseline_p95 = baseline.stats.percentiles.p95.as_millis() as f64;

        if current_p95 > baseline_p95 * regression_threshold {
            return true;
        }

        // Check memory regression
        if let (Some(current_mem), Some(baseline_mem)) = (
            current.resources.as_ref().and_then(|r| r.memory.as_ref()),
            baseline.resources.as_ref().and_then(|r| r.memory.as_ref()),
        ) {
            let current_rss = current_mem.peak_rss_bytes as f64;
            let baseline_rss = baseline_mem.peak_rss_bytes as f64;

            if current_rss > baseline_rss * regression_threshold {
                return true;
            }
        }

        false
    }
}

/// Baseline comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    pub baseline_name: String,
    pub baseline_created_at: chrono::DateTime<chrono::Utc>,
    pub comparison_timestamp: chrono::DateTime<chrono::Utc>,
    pub comparisons: Vec<ResultComparison>,
}

/// Comparison between current result and baseline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultComparison {
    pub benchmark_name: String,
    pub current_p50_ms: f64,
    pub baseline_p50_ms: f64,
    pub current_p95_ms: f64,
    pub baseline_p95_ms: f64,
    pub current_memory_mb: Option<f64>,
    pub baseline_memory_mb: Option<f64>,
    pub regression_detected: bool,
}

/// Calculate percentile from a sorted vector
fn calculate_percentile(values: &mut [f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((values.len() as f64) * percentile) as usize;
    values[index.min(values.len() - 1)]
}

/// Sanitize filename for filesystem safety
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

/// Get CPU information
fn get_cpu_info() -> String {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| line.starts_with("model name"))
                    .and_then(|line| line.split(':').nth(1))
                    .map(|s| s.trim().to_string())
            })
            .unwrap_or_else(|| "Unknown CPU".to_string())
    }

    #[cfg(not(target_os = "linux"))]
    {
        "Unknown CPU".to_string()
    }
}

/// Get total system memory in MB
fn get_total_memory_mb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| line.starts_with("MemTotal:"))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(|kb| kb / 1024) // Convert KB to MB
            })
            .unwrap_or(0)
    }

    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// Get Rust version
fn get_rust_version() -> String {
    std::env::var("RUSTC_VERSION").unwrap_or_else(|_| "unknown".to_string())
}

/// Baseline management errors
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Baseline not found: {0}")]
    NotFound(String),
    #[error("Invalid data: {0}")]
    InvalidData(String),
}
