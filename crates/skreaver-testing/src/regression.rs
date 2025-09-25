//! # Performance Regression Detection System
//!
//! This module provides comprehensive performance regression detection capabilities
//! including baseline storage, threshold-based detection, and historical analysis.
//!
//! ## Features
//!
//! - **Baseline Management**: Store and retrieve performance baselines
//! - **Regression Detection**: Statistical analysis with configurable thresholds
//! - **Historical Tracking**: Time-series storage for trend analysis
//! - **Criterion Integration**: Parse criterion benchmark output
//! - **CLI Tools**: Command-line interface for baseline operations

use crate::benchmarks::BenchmarkResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use thiserror::Error;

/// Errors that can occur during regression detection
#[derive(Error, Debug)]
pub enum RegressionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Baseline not found: {0}")]
    BaselineNotFound(String),
    #[error("Invalid threshold: {0}")]
    InvalidThreshold(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Configuration for regression detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionConfig {
    /// Maximum allowed percentage increase in mean duration
    pub mean_threshold_percent: f64,
    /// Maximum allowed percentage increase in P95 duration
    pub p95_threshold_percent: f64,
    /// Maximum allowed percentage increase in P99 duration
    pub p99_threshold_percent: f64,
    /// Minimum number of samples required for comparison
    pub min_samples: usize,
    /// Whether to use statistical significance testing
    pub use_statistical_test: bool,
    /// Significance level for statistical tests (e.g., 0.05 for 95% confidence)
    pub significance_level: f64,
}

impl Default for RegressionConfig {
    fn default() -> Self {
        Self {
            mean_threshold_percent: 10.0, // 10% increase
            p95_threshold_percent: 15.0,  // 15% increase
            p99_threshold_percent: 20.0,  // 20% increase
            min_samples: 10,
            use_statistical_test: true,
            significance_level: 0.05,
        }
    }
}

/// A single performance measurement with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMeasurement {
    pub benchmark_name: String,
    pub timestamp: SystemTime,
    pub commit_hash: Option<String>,
    pub branch: Option<String>,
    pub mean_duration_nanos: u64,
    pub median_duration_nanos: u64,
    pub min_duration_nanos: u64,
    pub max_duration_nanos: u64,
    pub std_dev_nanos: u64,
    pub sample_count: usize,
    pub throughput_ops_per_sec: Option<f64>,
    /// Additional custom metrics
    pub custom_metrics: HashMap<String, f64>,
}

impl From<BenchmarkResult> for PerformanceMeasurement {
    fn from(result: BenchmarkResult) -> Self {
        Self {
            benchmark_name: result.name,
            timestamp: SystemTime::now(),
            commit_hash: None,
            branch: None,
            mean_duration_nanos: result.mean.as_nanos() as u64,
            median_duration_nanos: result.median.as_nanos() as u64,
            min_duration_nanos: result.min.as_nanos() as u64,
            max_duration_nanos: result.max.as_nanos() as u64,
            std_dev_nanos: result.std_dev.as_nanos() as u64,
            sample_count: result.iterations,
            throughput_ops_per_sec: result.throughput,
            custom_metrics: HashMap::new(),
        }
    }
}

impl PerformanceMeasurement {
    /// Create a new performance measurement with Git metadata
    pub fn new_with_git(
        result: BenchmarkResult,
        commit_hash: Option<String>,
        branch: Option<String>,
    ) -> Self {
        let mut measurement = Self::from(result);
        measurement.commit_hash = commit_hash;
        measurement.branch = branch;
        measurement
    }

    /// Get mean duration as Duration
    pub fn mean_duration(&self) -> Duration {
        Duration::from_nanos(self.mean_duration_nanos)
    }

    /// Get P95 approximation (mean + 1.645 * std_dev for normal distribution)
    pub fn p95_duration(&self) -> Duration {
        let p95_nanos = self
            .mean_duration_nanos
            .saturating_add((1.645 * self.std_dev_nanos as f64) as u64);
        Duration::from_nanos(p95_nanos)
    }

    /// Get P99 approximation (mean + 2.326 * std_dev for normal distribution)
    pub fn p99_duration(&self) -> Duration {
        let p99_nanos = self
            .mean_duration_nanos
            .saturating_add((2.326 * self.std_dev_nanos as f64) as u64);
        Duration::from_nanos(p99_nanos)
    }
}

/// Performance baseline containing historical measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    pub benchmark_name: String,
    pub measurements: Vec<PerformanceMeasurement>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl PerformanceBaseline {
    /// Create a new baseline with initial measurement
    pub fn new(measurement: PerformanceMeasurement) -> Self {
        let now = SystemTime::now();
        Self {
            benchmark_name: measurement.benchmark_name.clone(),
            measurements: vec![measurement],
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a new measurement to the baseline
    pub fn add_measurement(&mut self, measurement: PerformanceMeasurement) {
        self.measurements.push(measurement);
        self.updated_at = SystemTime::now();

        // Keep only the latest 1000 measurements to prevent unbounded growth
        if self.measurements.len() > 1000 {
            self.measurements.drain(0..self.measurements.len() - 1000);
        }
    }

    /// Get the most recent measurement
    pub fn latest_measurement(&self) -> Option<&PerformanceMeasurement> {
        self.measurements.last()
    }

    /// Get measurements from a specific time period
    pub fn measurements_since(&self, since: SystemTime) -> Vec<&PerformanceMeasurement> {
        self.measurements
            .iter()
            .filter(|m| m.timestamp >= since)
            .collect()
    }

    /// Calculate statistical baseline from recent measurements
    pub fn calculate_baseline_stats(&self, sample_count: usize) -> BaselineStats {
        let recent_measurements: Vec<_> =
            self.measurements.iter().rev().take(sample_count).collect();

        if recent_measurements.is_empty() {
            return BaselineStats::default();
        }

        let mean_durations: Vec<u64> = recent_measurements
            .iter()
            .map(|m| m.mean_duration_nanos)
            .collect();

        let mean_of_means = mean_durations.iter().sum::<u64>() as f64 / mean_durations.len() as f64;

        let variance = mean_durations
            .iter()
            .map(|&x| {
                let diff = x as f64 - mean_of_means;
                diff * diff
            })
            .sum::<f64>()
            / mean_durations.len() as f64;

        let std_dev = variance.sqrt();

        BaselineStats {
            sample_count: recent_measurements.len(),
            mean_duration_nanos: mean_of_means as u64,
            std_dev_nanos: std_dev as u64,
            min_duration_nanos: mean_durations.iter().min().copied().unwrap_or(0),
            max_duration_nanos: mean_durations.iter().max().copied().unwrap_or(0),
        }
    }
}

/// Statistical summary of baseline performance
#[derive(Debug, Clone, Default)]
pub struct BaselineStats {
    pub sample_count: usize,
    pub mean_duration_nanos: u64,
    pub std_dev_nanos: u64,
    pub min_duration_nanos: u64,
    pub max_duration_nanos: u64,
}

/// Result of a regression analysis
#[derive(Debug, Clone)]
pub struct RegressionAnalysis {
    pub benchmark_name: String,
    pub is_regression: bool,
    pub mean_change_percent: f64,
    pub p95_change_percent: f64,
    pub p99_change_percent: f64,
    pub baseline_stats: BaselineStats,
    pub current_measurement: PerformanceMeasurement,
    pub confidence_level: Option<f64>,
    pub details: String,
}

impl RegressionAnalysis {
    /// Generate a human-readable summary of the analysis
    pub fn summary(&self) -> String {
        let status = if self.is_regression {
            "REGRESSION"
        } else {
            "OK"
        };

        format!(
            "{}: {} - Mean: {:.1}%, P95: {:.1}%, P99: {:.1}%",
            self.benchmark_name,
            status,
            self.mean_change_percent,
            self.p95_change_percent,
            self.p99_change_percent
        )
    }
}

/// Manager for performance baselines and regression detection
pub struct BaselineManager {
    storage_path: PathBuf,
    config: RegressionConfig,
    baselines: HashMap<String, PerformanceBaseline>,
}

impl BaselineManager {
    /// Create a new baseline manager with specified storage path
    pub fn new<P: AsRef<Path>>(storage_path: P) -> Result<Self, RegressionError> {
        let path = storage_path.as_ref().to_path_buf();

        // Create storage directory if it doesn't exist
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }

        let mut manager = Self {
            storage_path: path,
            config: RegressionConfig::default(),
            baselines: HashMap::new(),
        };

        // Load existing baselines
        manager.load_all_baselines()?;

        Ok(manager)
    }

    /// Create manager with custom configuration
    pub fn with_config<P: AsRef<Path>>(
        storage_path: P,
        config: RegressionConfig,
    ) -> Result<Self, RegressionError> {
        let mut manager = Self::new(storage_path)?;
        manager.config = config;
        Ok(manager)
    }

    /// Load all baseline files from storage
    fn load_all_baselines(&mut self) -> Result<(), RegressionError> {
        if !self.storage_path.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.storage_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_baseline_file(&path) {
                    Ok(baseline) => {
                        // Use the original benchmark name from the baseline data, not the filename
                        self.baselines
                            .insert(baseline.benchmark_name.clone(), baseline);
                    }
                    Err(e) => {
                        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                            eprintln!("Warning: Failed to load baseline {}: {}", filename, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a single baseline file
    fn load_baseline_file(&self, path: &Path) -> Result<PerformanceBaseline, RegressionError> {
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Save a baseline to disk
    fn save_baseline(&self, baseline: &PerformanceBaseline) -> Result<(), RegressionError> {
        // Replace slashes and other problematic characters in filename
        let safe_name = baseline.benchmark_name.replace(['/', '\\'], "_");
        let filename = format!("{}.json", safe_name);
        let path = self.storage_path.join(filename);

        let content = serde_json::to_string_pretty(baseline)?;
        fs::write(path, content)?;

        Ok(())
    }

    /// Create or update a baseline with a new measurement
    pub fn update_baseline(
        &mut self,
        measurement: PerformanceMeasurement,
    ) -> Result<(), RegressionError> {
        let benchmark_name = measurement.benchmark_name.clone();

        match self.baselines.get_mut(&benchmark_name) {
            Some(baseline) => {
                baseline.add_measurement(measurement);
            }
            None => {
                let baseline = PerformanceBaseline::new(measurement);
                self.baselines.insert(benchmark_name.clone(), baseline);
            }
        }

        // Save to disk
        if let Some(baseline) = self.baselines.get(&benchmark_name) {
            self.save_baseline(baseline)?;
        }

        Ok(())
    }

    /// Detect regression by comparing current measurement with baseline
    pub fn detect_regression(
        &self,
        measurement: &PerformanceMeasurement,
    ) -> Result<RegressionAnalysis, RegressionError> {
        let baseline = self
            .baselines
            .get(&measurement.benchmark_name)
            .ok_or_else(|| RegressionError::BaselineNotFound(measurement.benchmark_name.clone()))?;

        let baseline_stats = baseline.calculate_baseline_stats(self.config.min_samples);

        if baseline_stats.sample_count < self.config.min_samples {
            return Ok(RegressionAnalysis {
                benchmark_name: measurement.benchmark_name.clone(),
                is_regression: false,
                mean_change_percent: 0.0,
                p95_change_percent: 0.0,
                p99_change_percent: 0.0,
                baseline_stats: baseline_stats.clone(),
                current_measurement: measurement.clone(),
                confidence_level: None,
                details: format!(
                    "Insufficient baseline data: {} samples (need {})",
                    baseline_stats.sample_count, self.config.min_samples
                ),
            });
        }

        // Calculate percentage changes
        let mean_change_percent = self.calculate_percentage_change(
            baseline_stats.mean_duration_nanos,
            measurement.mean_duration_nanos,
        );

        let baseline_p95 = baseline_stats
            .mean_duration_nanos
            .saturating_add((1.645 * baseline_stats.std_dev_nanos as f64) as u64);
        let p95_change_percent = self.calculate_percentage_change(
            baseline_p95,
            measurement.p95_duration().as_nanos() as u64,
        );

        let baseline_p99 = baseline_stats
            .mean_duration_nanos
            .saturating_add((2.326 * baseline_stats.std_dev_nanos as f64) as u64);
        let p99_change_percent = self.calculate_percentage_change(
            baseline_p99,
            measurement.p99_duration().as_nanos() as u64,
        );

        // Check thresholds
        let mean_regression = mean_change_percent > self.config.mean_threshold_percent;
        let p95_regression = p95_change_percent > self.config.p95_threshold_percent;
        let p99_regression = p99_change_percent > self.config.p99_threshold_percent;

        let is_regression = mean_regression || p95_regression || p99_regression;

        let details = if is_regression {
            let mut issues = Vec::new();
            if mean_regression {
                issues.push(format!(
                    "Mean exceeded threshold: {:.1}% > {:.1}%",
                    mean_change_percent, self.config.mean_threshold_percent
                ));
            }
            if p95_regression {
                issues.push(format!(
                    "P95 exceeded threshold: {:.1}% > {:.1}%",
                    p95_change_percent, self.config.p95_threshold_percent
                ));
            }
            if p99_regression {
                issues.push(format!(
                    "P99 exceeded threshold: {:.1}% > {:.1}%",
                    p99_change_percent, self.config.p99_threshold_percent
                ));
            }
            issues.join("; ")
        } else {
            "Performance within acceptable thresholds".to_string()
        };

        Ok(RegressionAnalysis {
            benchmark_name: measurement.benchmark_name.clone(),
            is_regression,
            mean_change_percent,
            p95_change_percent,
            p99_change_percent,
            baseline_stats,
            current_measurement: measurement.clone(),
            confidence_level: None, // TODO: Implement statistical testing
            details,
        })
    }

    /// Calculate percentage change between baseline and current values
    fn calculate_percentage_change(&self, baseline: u64, current: u64) -> f64 {
        if baseline == 0 {
            return 0.0;
        }
        ((current as f64 - baseline as f64) / baseline as f64) * 100.0
    }

    /// Get all baseline names
    pub fn list_baselines(&self) -> Vec<String> {
        self.baselines.keys().cloned().collect()
    }

    /// Get a specific baseline
    pub fn get_baseline(&self, name: &str) -> Option<&PerformanceBaseline> {
        self.baselines.get(name)
    }

    /// Remove a baseline
    pub fn remove_baseline(&mut self, name: &str) -> Result<bool, RegressionError> {
        if self.baselines.remove(name).is_some() {
            // Remove file using safe naming
            let safe_name = name.replace(['/', '\\'], "_");
            let filename = format!("{}.json", safe_name);
            let path = self.storage_path.join(filename);
            if path.exists() {
                fs::remove_file(path)?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Export baseline data for external analysis
    pub fn export_baseline(&self, name: &str, path: &Path) -> Result<(), RegressionError> {
        let baseline = self
            .baselines
            .get(name)
            .ok_or_else(|| RegressionError::BaselineNotFound(name.to_string()))?;

        let content = serde_json::to_string_pretty(baseline)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Import baseline data from external source
    pub fn import_baseline(&mut self, path: &Path) -> Result<String, RegressionError> {
        let baseline = self.load_baseline_file(path)?;
        let name = baseline.benchmark_name.clone();

        self.baselines.insert(name.clone(), baseline);

        // Save to our storage location
        if let Some(baseline) = self.baselines.get(&name) {
            self.save_baseline(baseline)?;
        }

        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmarks::BenchmarkResult;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_benchmark_result(name: &str, mean_micros: u64) -> BenchmarkResult {
        BenchmarkResult {
            name: name.to_string(),
            iterations: 100,
            mean: Duration::from_micros(mean_micros),
            median: Duration::from_micros(mean_micros),
            min: Duration::from_micros(mean_micros - 10),
            max: Duration::from_micros(mean_micros + 10),
            std_dev: Duration::from_micros(5),
            throughput: None,
            total_operations: None,
        }
    }

    #[test]
    fn test_performance_measurement_creation() {
        let result = create_test_benchmark_result("test_benchmark", 1000);
        let measurement = PerformanceMeasurement::from(result);

        assert_eq!(measurement.benchmark_name, "test_benchmark");
        assert_eq!(measurement.mean_duration_nanos, 1_000_000); // 1000 microseconds
        assert_eq!(measurement.sample_count, 100);
    }

    #[test]
    fn test_baseline_creation_and_updates() {
        let result = create_test_benchmark_result("test", 1000);
        let measurement = PerformanceMeasurement::from(result);

        let mut baseline = PerformanceBaseline::new(measurement.clone());
        assert_eq!(baseline.measurements.len(), 1);

        baseline.add_measurement(measurement);
        assert_eq!(baseline.measurements.len(), 2);
    }

    #[test]
    fn test_baseline_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BaselineManager::new(temp_dir.path()).unwrap();

        assert_eq!(manager.baselines.len(), 0);
        assert!(temp_dir.path().exists());
    }

    #[test]
    fn test_baseline_update_and_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = BaselineManager::new(temp_dir.path()).unwrap();

        let result = create_test_benchmark_result("persist_test", 1000);
        let measurement = PerformanceMeasurement::from(result);

        manager.update_baseline(measurement).unwrap();
        assert_eq!(manager.baselines.len(), 1);

        // Check file was created
        let baseline_file = temp_dir.path().join("persist_test.json");
        assert!(baseline_file.exists());
    }

    #[test]
    fn test_regression_detection_no_regression() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = BaselineManager::new(temp_dir.path()).unwrap();

        // Add baseline measurements
        for i in 0..15 {
            let result = create_test_benchmark_result("stable_test", 1000 + i);
            let measurement = PerformanceMeasurement::from(result);
            manager.update_baseline(measurement).unwrap();
        }

        // Test with similar performance
        let test_result = create_test_benchmark_result("stable_test", 1005);
        let test_measurement = PerformanceMeasurement::from(test_result);

        let analysis = manager.detect_regression(&test_measurement).unwrap();
        assert!(!analysis.is_regression);
        assert!(analysis.mean_change_percent < 10.0);
    }

    #[test]
    fn test_regression_detection_with_regression() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = BaselineManager::new(temp_dir.path()).unwrap();

        // Add baseline measurements with consistent performance
        for _ in 0..15 {
            let result = create_test_benchmark_result("regression_test", 1000);
            let measurement = PerformanceMeasurement::from(result);
            manager.update_baseline(measurement).unwrap();
        }

        // Test with significantly worse performance (20% slower)
        let test_result = create_test_benchmark_result("regression_test", 1200);
        let test_measurement = PerformanceMeasurement::from(test_result);

        let analysis = manager.detect_regression(&test_measurement).unwrap();
        assert!(analysis.is_regression);
        assert!(analysis.mean_change_percent > 10.0);
    }

    #[test]
    fn test_insufficient_baseline_data() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = BaselineManager::new(temp_dir.path()).unwrap();

        // Add only a few measurements (less than min_samples)
        for i in 0..5 {
            let result = create_test_benchmark_result("sparse_test", 1000 + i);
            let measurement = PerformanceMeasurement::from(result);
            manager.update_baseline(measurement).unwrap();
        }

        let test_result = create_test_benchmark_result("sparse_test", 1500);
        let test_measurement = PerformanceMeasurement::from(test_result);

        let analysis = manager.detect_regression(&test_measurement).unwrap();
        assert!(!analysis.is_regression); // Should not flag regression with insufficient data
        assert!(analysis.details.contains("Insufficient baseline data"));
    }

    #[test]
    fn test_baseline_export_import() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = BaselineManager::new(temp_dir.path()).unwrap();

        // Create and save baseline
        let result = create_test_benchmark_result("export_test", 1000);
        let measurement = PerformanceMeasurement::from(result);
        manager.update_baseline(measurement).unwrap();

        // Export baseline
        let export_path = temp_dir.path().join("exported.json");
        manager
            .export_baseline("export_test", &export_path)
            .unwrap();
        assert!(export_path.exists());

        // Clear manager and import
        manager.baselines.clear();
        assert_eq!(manager.baselines.len(), 0);

        let imported_name = manager.import_baseline(&export_path).unwrap();
        assert_eq!(imported_name, "export_test");
        assert_eq!(manager.baselines.len(), 1);
    }

    #[test]
    fn test_custom_regression_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = RegressionConfig {
            mean_threshold_percent: 5.0, // Very strict threshold
            p95_threshold_percent: 8.0,
            p99_threshold_percent: 10.0,
            min_samples: 5,
            use_statistical_test: false,
            significance_level: 0.01,
        };

        let mut manager = BaselineManager::with_config(temp_dir.path(), config).unwrap();

        // Add baseline
        for _ in 0..10 {
            let result = create_test_benchmark_result("strict_test", 1000);
            let measurement = PerformanceMeasurement::from(result);
            manager.update_baseline(measurement).unwrap();
        }

        // Test with 6% increase (should trigger with strict config)
        let test_result = create_test_benchmark_result("strict_test", 1060);
        let test_measurement = PerformanceMeasurement::from(test_result);

        let analysis = manager.detect_regression(&test_measurement).unwrap();
        assert!(analysis.is_regression);
    }
}
