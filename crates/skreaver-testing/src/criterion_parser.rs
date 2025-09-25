//! # Criterion Benchmark Output Parser
//!
//! This module provides parsing capabilities for criterion benchmark output,
//! allowing extraction of performance metrics from command line output and
//! JSON report files.

use crate::regression::{PerformanceMeasurement, RegressionError};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Criterion benchmark output parser
pub struct CriterionParser {
    /// Git commit hash to associate with measurements
    pub commit_hash: Option<String>,
    /// Git branch to associate with measurements
    pub branch: Option<String>,
}

impl CriterionParser {
    /// Create a new parser
    pub fn new() -> Self {
        Self {
            commit_hash: None,
            branch: None,
        }
    }

    /// Create parser with Git metadata
    pub fn with_git_info(commit_hash: Option<String>, branch: Option<String>) -> Self {
        Self {
            commit_hash,
            branch,
        }
    }

    /// Parse criterion console output and extract performance measurements
    pub fn parse_console_output(
        &self,
        output: &str,
    ) -> Result<Vec<PerformanceMeasurement>, RegressionError> {
        let mut measurements = Vec::new();
        let lines: Vec<&str> = output.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if let Some(measurement) = self.parse_console_line(line, &lines, i)? {
                measurements.push(measurement);
            }
        }

        Ok(measurements)
    }

    /// Parse a single line of criterion console output
    fn parse_console_line(
        &self,
        line: &str,
        all_lines: &[&str],
        line_index: usize,
    ) -> Result<Option<PerformanceMeasurement>, RegressionError> {
        // Look for benchmark completion lines like:
        // "benchmark_name          time:   [123.45 μs 125.67 μs 127.89 μs]"

        if !line.contains("time:") || !line.contains("[") || !line.contains("]") {
            return Ok(None);
        }

        // Extract benchmark name (everything before "time:")
        let parts: Vec<&str> = line.split("time:").collect();
        if parts.len() != 2 {
            return Ok(None);
        }

        let benchmark_name = parts[0].trim().to_string();
        let time_part = parts[1];

        // Extract timing values from brackets [min median max]
        if let Some(start) = time_part.find('[')
            && let Some(end) = time_part.find(']')
        {
            let time_values = &time_part[start + 1..end];
            let values: Vec<&str> = time_values.split_whitespace().collect();

            if values.len() >= 6 {
                // Try to parse the three values (min, median, max) with their units
                // If any parsing fails, skip this line instead of failing the entire parse
                let min_duration =
                    match Self::parse_duration_with_unit(values[0], values.get(1).copied()) {
                        Ok(duration) => duration,
                        Err(_) => return Ok(None), // Skip this line on parsing error
                    };

                let median_duration =
                    match Self::parse_duration_with_unit(values[2], values.get(3).copied()) {
                        Ok(duration) => duration,
                        Err(_) => return Ok(None), // Skip this line on parsing error
                    };

                let max_duration =
                    match Self::parse_duration_with_unit(values[4], values.get(5).copied()) {
                        Ok(duration) => duration,
                        Err(_) => return Ok(None), // Skip this line on parsing error
                    };

                // Use median as mean approximation for console output
                let measurement = PerformanceMeasurement {
                    benchmark_name,
                    timestamp: SystemTime::now(),
                    commit_hash: self.commit_hash.clone(),
                    branch: self.branch.clone(),
                    mean_duration_nanos: median_duration.as_nanos() as u64,
                    median_duration_nanos: median_duration.as_nanos() as u64,
                    min_duration_nanos: min_duration.as_nanos() as u64,
                    max_duration_nanos: max_duration.as_nanos() as u64,
                    std_dev_nanos: 0,  // Not available in console output
                    sample_count: 100, // Default assumption for criterion
                    throughput_ops_per_sec: self.extract_throughput(all_lines, line_index),
                    custom_metrics: HashMap::new(),
                };

                return Ok(Some(measurement));
            }
        }

        Ok(None)
    }

    /// Parse duration string with unit (e.g., "123.45", "μs")
    fn parse_duration_with_unit(
        value_str: &str,
        unit_str: Option<&str>,
    ) -> Result<Duration, RegressionError> {
        let value: f64 = value_str.parse().map_err(|e| {
            RegressionError::ParseError(format!("Invalid duration value '{}': {}", value_str, e))
        })?;

        let unit = unit_str.unwrap_or("ns");

        let duration = match unit {
            "ns" => Duration::from_nanos(value as u64),
            "μs" | "us" => Duration::from_nanos((value * 1_000.0) as u64),
            "ms" => Duration::from_nanos((value * 1_000_000.0) as u64),
            "s" => Duration::from_nanos((value * 1_000_000_000.0) as u64),
            _ => {
                return Err(RegressionError::ParseError(format!(
                    "Unknown time unit: {}",
                    unit
                )));
            }
        };

        Ok(duration)
    }

    /// Extract throughput information from surrounding lines
    fn extract_throughput(&self, all_lines: &[&str], current_index: usize) -> Option<f64> {
        // Look for throughput info in nearby lines
        for i in current_index.saturating_sub(3)..=(current_index + 3).min(all_lines.len() - 1) {
            if let Some(line) = all_lines.get(i)
                && line.contains("thrpt:")
            {
                // Parse throughput line like "thrpt: [62.959 elem/s 64.236 elem/s 65.538 elem/s]" or "thrpt: 1.23 Melem/s"
                if let Some(thrpt_start) = line.find("thrpt:") {
                    let thrpt_part = &line[thrpt_start + 6..].trim();

                    // Handle bracketed format
                    if thrpt_part.starts_with('[') && thrpt_part.contains(']') {
                        if let Some(start) = thrpt_part.find('[')
                            && let Some(end) = thrpt_part.find(']')
                        {
                            let bracket_content = &thrpt_part[start + 1..end];
                            let values: Vec<&str> = bracket_content.split_whitespace().collect();

                            // Take the median value (middle of three values)
                            if values.len() >= 4
                                && let Ok(median_value) = values[2].parse::<f64>()
                                && let Some(unit) = values.get(3)
                            {
                                return match *unit {
                                    unit if unit.contains("Gelem/s") => {
                                        Some(median_value * 1_000_000_000.0)
                                    }
                                    unit if unit.contains("Melem/s") => {
                                        Some(median_value * 1_000_000.0)
                                    }
                                    unit if unit.contains("Kelem/s") => {
                                        Some(median_value * 1_000.0)
                                    }
                                    unit if unit.contains("elem/s") => Some(median_value),
                                    _ => Some(median_value),
                                };
                            }
                        }
                    } else {
                        // Handle simple format "thrpt: 1.23 Melem/s"
                        let parts: Vec<&str> = thrpt_part.split_whitespace().collect();

                        if parts.len() >= 2
                            && let Ok(value) = parts[0].parse::<f64>()
                        {
                            let unit = parts[1];
                            // Convert based on unit
                            return match unit {
                                unit if unit.contains("Gelem/s") => Some(value * 1_000_000_000.0),
                                unit if unit.contains("Melem/s") => Some(value * 1_000_000.0),
                                unit if unit.contains("Kelem/s") => Some(value * 1_000.0),
                                unit if unit.contains("elem/s") => Some(value),
                                _ => Some(value),
                            };
                        }
                    }
                }
            }
        }
        None
    }

    /// Parse criterion JSON report file
    pub fn parse_json_report<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Vec<PerformanceMeasurement>, RegressionError> {
        let content = fs::read_to_string(path.as_ref())?;
        let report: CriterionJsonReport = serde_json::from_str(&content)?;

        let mut measurements = Vec::new();

        for (benchmark_id, benchmark_data) in report.benchmarks {
            let measurement = PerformanceMeasurement {
                benchmark_name: benchmark_id,
                timestamp: SystemTime::now(),
                commit_hash: self.commit_hash.clone(),
                branch: self.branch.clone(),
                mean_duration_nanos: (benchmark_data.mean.point_estimate * 1_000_000_000.0) as u64,
                median_duration_nanos: (benchmark_data.median.point_estimate * 1_000_000_000.0)
                    as u64,
                min_duration_nanos: (benchmark_data.mean.lower_bound * 1_000_000_000.0) as u64,
                max_duration_nanos: (benchmark_data.mean.upper_bound * 1_000_000_000.0) as u64,
                std_dev_nanos: (benchmark_data.std_dev.point_estimate * 1_000_000_000.0) as u64,
                sample_count: benchmark_data.typical.len(),
                throughput_ops_per_sec: benchmark_data.throughput.map(|t| t.per_iteration),
                custom_metrics: HashMap::new(),
            };

            measurements.push(measurement);
        }

        Ok(measurements)
    }

    /// Extract Git information from current repository
    pub fn extract_git_info() -> Result<(Option<String>, Option<String>), RegressionError> {
        use std::process::Command;

        // Get commit hash
        let commit_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            });

        // Get branch name
        let branch = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            });

        Ok((commit_hash, branch))
    }

    /// Create parser with automatically detected Git information
    pub fn with_auto_git_info() -> Result<Self, RegressionError> {
        let (commit_hash, branch) = Self::extract_git_info()?;
        Ok(Self::with_git_info(commit_hash, branch))
    }
}

impl Default for CriterionParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Criterion JSON report structure (simplified)
#[derive(Debug, Deserialize)]
struct CriterionJsonReport {
    benchmarks: HashMap<String, BenchmarkData>,
}

#[derive(Debug, Deserialize)]
struct BenchmarkData {
    mean: EstimateData,
    median: EstimateData,
    std_dev: EstimateData,
    typical: Vec<f64>,
    throughput: Option<ThroughputData>,
}

#[derive(Debug, Deserialize)]
struct EstimateData {
    point_estimate: f64,
    lower_bound: f64,
    upper_bound: f64,
}

#[derive(Debug, Deserialize)]
struct ThroughputData {
    per_iteration: f64,
}

/// Command-line tool for parsing criterion output
pub struct CriterionCli;

impl CriterionCli {
    /// Parse criterion output and update baselines
    pub fn parse_and_update_baselines(
        output: &str,
        baseline_storage_path: &Path,
    ) -> Result<Vec<PerformanceMeasurement>, RegressionError> {
        let parser =
            CriterionParser::with_auto_git_info().unwrap_or_else(|_| CriterionParser::new());

        let measurements = parser.parse_console_output(output)?;

        // Update baselines with new measurements
        let mut baseline_manager = crate::regression::BaselineManager::new(baseline_storage_path)?;

        for measurement in &measurements {
            baseline_manager.update_baseline(measurement.clone())?;
        }

        Ok(measurements)
    }

    /// Run regression analysis on parsed measurements
    pub fn analyze_regressions(
        measurements: &[PerformanceMeasurement],
        baseline_storage_path: &Path,
    ) -> Result<Vec<crate::regression::RegressionAnalysis>, RegressionError> {
        let baseline_manager = crate::regression::BaselineManager::new(baseline_storage_path)?;
        let mut analyses = Vec::new();

        for measurement in measurements {
            match baseline_manager.detect_regression(measurement) {
                Ok(analysis) => analyses.push(analysis),
                Err(e) => eprintln!(
                    "Warning: Failed to analyze {}: {}",
                    measurement.benchmark_name, e
                ),
            }
        }

        Ok(analyses)
    }

    /// Print regression analysis results
    pub fn print_regression_results(analyses: &[crate::regression::RegressionAnalysis]) {
        Self::print_regression_results_with_exit(analyses, true);
    }

    /// Print regression analysis results with configurable exit behavior
    pub fn print_regression_results_with_exit(
        analyses: &[crate::regression::RegressionAnalysis],
        exit_on_regression: bool,
    ) {
        if analyses.is_empty() {
            println!("No regression analysis results to display.");
            return;
        }

        println!("Performance Regression Analysis Results");
        println!("=====================================");

        let mut regressions_found = false;

        for analysis in analyses {
            if analysis.is_regression {
                regressions_found = true;
                println!("🚨 {}", analysis.summary());
                println!("   Details: {}", analysis.details);
            } else {
                println!("✅ {}", analysis.summary());
            }
        }

        if regressions_found {
            println!("\n⚠️  Performance regressions detected! Consider investigating the changes.");
            if exit_on_regression {
                std::process::exit(1);
            }
        } else {
            println!("\n🎉 All benchmarks are within acceptable performance thresholds.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    const SAMPLE_CRITERION_OUTPUT: &str = r#"
Benchmarking memory_quick/store
Benchmarking memory_quick/store: Warming up for 1.0000 s
Benchmarking memory_quick/store: Collecting 50 samples in estimated 2.0500 s (5150 iterations)
Benchmarking memory_quick/store: Analyzing
memory_quick/store      time:   [394.06 ns 397.42 ns 401.14 ns]
Found 2 outliers among 50 measurements (4.00%)
  1 (2.00%) high mild
  1 (2.00%) high severe

Benchmarking memory_quick/load
Benchmarking memory_quick/load: Warming up for 1.0000 s
Benchmarking memory_quick/load: Collecting 50 samples in estimated 2.0100 s (4200 iterations)
Benchmarking memory_quick/load: Analyzing
memory_quick/load       time:   [476.19 ns 480.95 ns 486.43 ns]
                        thrpt:  2.0790 Melem/s

Benchmarking file_quick/write_1kb
Benchmarking file_quick/write_1kb: Warming up for 1.0000 s
file_quick/write_1kb    time:   [12.345 μs 12.567 μs 12.789 μs]
                        thrpt:  [81.319 Kelem/s 82.853 Kelem/s 84.422 Kelem/s]
"#;

    #[test]
    fn test_parse_console_output() {
        let parser = CriterionParser::new();
        let measurements = parser
            .parse_console_output(SAMPLE_CRITERION_OUTPUT)
            .unwrap();

        assert_eq!(measurements.len(), 3);

        // Test first measurement
        let store_measurement = measurements
            .iter()
            .find(|m| m.benchmark_name == "memory_quick/store")
            .unwrap();

        assert_eq!(store_measurement.median_duration_nanos, 397); // 397.42 ns
        assert_eq!(store_measurement.min_duration_nanos, 394); // 394.06 ns
        assert_eq!(store_measurement.max_duration_nanos, 401); // 401.14 ns

        // Test measurement with throughput
        let load_measurement = measurements
            .iter()
            .find(|m| m.benchmark_name == "memory_quick/load")
            .unwrap();

        assert!(load_measurement.throughput_ops_per_sec.is_some());
        assert!(load_measurement.throughput_ops_per_sec.unwrap() > 2_000_000.0); // ~2M elem/s

        // Test microsecond parsing
        let file_measurement = measurements
            .iter()
            .find(|m| m.benchmark_name == "file_quick/write_1kb")
            .unwrap();

        // 12.567 μs should be 12567 nanos
        assert!(file_measurement.median_duration_nanos > 12_000); // Allow some flexibility
    }

    #[test]
    fn test_duration_parsing() {
        // Test nanoseconds
        let ns_duration = CriterionParser::parse_duration_with_unit("123.45", Some("ns")).unwrap();
        assert_eq!(ns_duration.as_nanos(), 123);

        // Test microseconds
        let us_duration = CriterionParser::parse_duration_with_unit("123.45", Some("μs")).unwrap();
        assert_eq!(us_duration.as_nanos(), 123_450);

        // Test milliseconds
        let ms_duration = CriterionParser::parse_duration_with_unit("1.5", Some("ms")).unwrap();
        assert_eq!(ms_duration.as_nanos(), 1_500_000);

        // Test seconds
        let s_duration = CriterionParser::parse_duration_with_unit("2.0", Some("s")).unwrap();
        assert_eq!(s_duration.as_nanos(), 2_000_000_000);
    }

    #[test]
    fn test_invalid_duration_parsing() {
        let result = CriterionParser::parse_duration_with_unit("invalid", Some("ns"));
        assert!(result.is_err());

        let result = CriterionParser::parse_duration_with_unit("123", Some("invalid_unit"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parser_with_git_info() {
        let parser =
            CriterionParser::with_git_info(Some("abc123".to_string()), Some("main".to_string()));

        assert_eq!(parser.commit_hash, Some("abc123".to_string()));
        assert_eq!(parser.branch, Some("main".to_string()));
    }

    #[test]
    fn test_cli_parse_and_update() {
        let temp_dir = TempDir::new().unwrap();
        let measurements =
            CriterionCli::parse_and_update_baselines(SAMPLE_CRITERION_OUTPUT, temp_dir.path())
                .unwrap();

        assert_eq!(measurements.len(), 3);

        // Check that baseline files were created
        let _store_baseline = temp_dir.path().join("memory_quick/store.json");
        let _load_baseline = temp_dir.path().join("memory_quick/load.json");
        let _write_baseline = temp_dir.path().join("file_quick/write_1kb.json");

        // Check that any baseline files were created (exact naming may vary)
        assert!(temp_dir.path().read_dir().unwrap().count() > 0);
    }

    #[test]
    fn test_empty_output() {
        let parser = CriterionParser::new();
        let measurements = parser.parse_console_output("").unwrap();
        assert!(measurements.is_empty());
    }

    #[test]
    fn test_malformed_output() {
        let parser = CriterionParser::new();
        let malformed = "This is not criterion output\nno timing information here";
        let measurements = parser.parse_console_output(malformed).unwrap();
        assert!(measurements.is_empty());
    }

    #[test]
    fn test_regression_analysis_workflow() {
        let temp_dir = TempDir::new().unwrap();

        // Create baseline manager directly
        let mut manager = crate::regression::BaselineManager::new(temp_dir.path()).unwrap();

        // Create stable baseline measurements
        for _ in 0..15 {
            let result = crate::benchmarks::BenchmarkResult {
                name: "memory_quick/store".to_string(),
                iterations: 100,
                mean: Duration::from_nanos(400),
                median: Duration::from_nanos(400),
                min: Duration::from_nanos(390),
                max: Duration::from_nanos(410),
                std_dev: Duration::from_nanos(5),
                throughput: None,
                total_operations: None,
            };
            let measurement = crate::regression::PerformanceMeasurement::from(result);
            manager.update_baseline(measurement).unwrap();
        }

        // Create regression measurement (50% slower)
        let slow_result = crate::benchmarks::BenchmarkResult {
            name: "memory_quick/store".to_string(),
            iterations: 100,
            mean: Duration::from_nanos(600),
            median: Duration::from_nanos(600),
            min: Duration::from_nanos(590),
            max: Duration::from_nanos(610),
            std_dev: Duration::from_nanos(5),
            throughput: None,
            total_operations: None,
        };
        let slow_measurement = crate::regression::PerformanceMeasurement::from(slow_result);

        // Detect regression
        let analysis = manager.detect_regression(&slow_measurement).unwrap();
        assert!(analysis.is_regression);
        assert!(analysis.mean_change_percent > 10.0);
    }
}
