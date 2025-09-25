//! # CLI Tools for Performance Regression Detection
//!
//! Command-line interface for managing performance baselines and running
//! regression detection analysis.

use crate::criterion_parser::{CriterionCli, CriterionParser};
use crate::regression::{BaselineManager, RegressionConfig, RegressionError};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// CLI tool for performance regression detection
pub struct RegressionCli {
    baseline_path: PathBuf,
    config: RegressionConfig,
}

impl RegressionCli {
    /// Create new CLI with default baseline path
    pub fn new() -> Self {
        Self {
            baseline_path: PathBuf::from("./baselines"),
            config: RegressionConfig::default(),
        }
    }

    /// Create CLI with custom baseline path
    pub fn with_baseline_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            baseline_path: path.as_ref().to_path_buf(),
            config: RegressionConfig::default(),
        }
    }

    /// Create CLI with custom configuration
    pub fn with_config(config: RegressionConfig) -> Self {
        Self {
            baseline_path: PathBuf::from("./baselines"),
            config,
        }
    }

    /// Run benchmarks and capture output for regression analysis
    pub fn run_benchmarks(&self, benchmark_name: Option<&str>) -> Result<String, RegressionError> {
        let mut cmd = Command::new("cargo");
        cmd.arg("bench");

        if let Some(name) = benchmark_name {
            cmd.arg("--bench").arg(name);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        println!("Running benchmarks...");
        let mut child = cmd.spawn().map_err(|e| {
            RegressionError::ConfigError(format!("Failed to run cargo bench: {}", e))
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RegressionError::ConfigError("Failed to capture stdout".to_string()))?;

        let reader = BufReader::new(stdout);
        let mut output = String::new();

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    println!("{}", line);
                    output.push_str(&line);
                    output.push('\n');
                }
                Err(e) => eprintln!("Error reading benchmark output: {}", e),
            }
        }

        let exit_status = child.wait().map_err(|e| {
            RegressionError::ConfigError(format!("Failed to wait for process: {}", e))
        })?;

        if !exit_status.success() {
            return Err(RegressionError::ConfigError(
                "Benchmark execution failed".to_string(),
            ));
        }

        Ok(output)
    }

    /// Create new baselines from benchmark output
    pub fn create_baselines(&self, benchmark_output: &str) -> Result<usize, RegressionError> {
        let parser =
            CriterionParser::with_auto_git_info().unwrap_or_else(|_| CriterionParser::new());

        let measurements = parser.parse_console_output(benchmark_output)?;
        let mut manager = BaselineManager::with_config(&self.baseline_path, self.config.clone())?;

        for measurement in &measurements {
            manager.update_baseline(measurement.clone())?;
        }

        println!(
            "Created {} baseline(s) in {}",
            measurements.len(),
            self.baseline_path.display()
        );

        Ok(measurements.len())
    }

    /// Update existing baselines with new measurements
    pub fn update_baselines(&self, benchmark_output: &str) -> Result<usize, RegressionError> {
        self.create_baselines(benchmark_output) // Same implementation as create
    }

    /// Run regression detection on benchmark output
    pub fn detect_regressions(&self, benchmark_output: &str) -> Result<bool, RegressionError> {
        self.detect_regressions_with_exit(benchmark_output, true)
    }

    /// Run regression detection on benchmark output with configurable exit behavior
    pub fn detect_regressions_with_exit(
        &self,
        benchmark_output: &str,
        exit_on_regression: bool,
    ) -> Result<bool, RegressionError> {
        let parser =
            CriterionParser::with_auto_git_info().unwrap_or_else(|_| CriterionParser::new());

        let measurements = parser.parse_console_output(benchmark_output)?;

        // Don't update baselines here - analyze against existing baselines
        let analyses = CriterionCli::analyze_regressions(&measurements, &self.baseline_path)?;

        CriterionCli::print_regression_results_with_exit(&analyses, exit_on_regression);

        // Return true if any regressions were found
        Ok(analyses.iter().any(|a| a.is_regression))
    }

    /// List all available baselines
    pub fn list_baselines(&self) -> Result<(), RegressionError> {
        let manager = BaselineManager::with_config(&self.baseline_path, self.config.clone())?;
        let baselines = manager.list_baselines();

        if baselines.is_empty() {
            println!("No baselines found in {}", self.baseline_path.display());
            return Ok(());
        }

        println!("Available baselines in {}:", self.baseline_path.display());
        for (i, name) in baselines.iter().enumerate() {
            if let Some(baseline) = manager.get_baseline(name) {
                println!(
                    "  {}. {} ({} measurements, last updated: {:?})",
                    i + 1,
                    name,
                    baseline.measurements.len(),
                    baseline.updated_at
                );
            }
        }

        Ok(())
    }

    /// Show details of a specific baseline
    pub fn show_baseline(&self, name: &str) -> Result<(), RegressionError> {
        let manager = BaselineManager::with_config(&self.baseline_path, self.config.clone())?;

        match manager.get_baseline(name) {
            Some(baseline) => {
                println!("Baseline: {}", baseline.benchmark_name);
                println!("Created: {:?}", baseline.created_at);
                println!("Updated: {:?}", baseline.updated_at);
                println!("Measurements: {}", baseline.measurements.len());

                if let Some(latest) = baseline.latest_measurement() {
                    println!("\nLatest measurement:");
                    println!("  Mean: {}μs", latest.mean_duration_nanos / 1000);
                    println!("  Median: {}μs", latest.median_duration_nanos / 1000);
                    println!("  Min: {}μs", latest.min_duration_nanos / 1000);
                    println!("  Max: {}μs", latest.max_duration_nanos / 1000);
                    println!("  Samples: {}", latest.sample_count);

                    if let Some(throughput) = latest.throughput_ops_per_sec {
                        println!("  Throughput: {:.0} ops/sec", throughput);
                    }

                    if let Some(commit) = &latest.commit_hash {
                        println!("  Commit: {}", commit);
                    }

                    if let Some(branch) = &latest.branch {
                        println!("  Branch: {}", branch);
                    }
                }

                // Show baseline statistics
                let stats = baseline.calculate_baseline_stats(self.config.min_samples);
                if stats.sample_count >= self.config.min_samples {
                    println!(
                        "\nBaseline statistics (last {} measurements):",
                        stats.sample_count
                    );
                    println!("  Mean: {}μs", stats.mean_duration_nanos / 1000);
                    println!("  Std Dev: {}μs", stats.std_dev_nanos / 1000);
                    println!(
                        "  Range: {}μs - {}μs",
                        stats.min_duration_nanos / 1000,
                        stats.max_duration_nanos / 1000
                    );
                }
            }
            None => {
                return Err(RegressionError::BaselineNotFound(name.to_string()));
            }
        }

        Ok(())
    }

    /// Remove a baseline
    pub fn remove_baseline(&self, name: &str) -> Result<(), RegressionError> {
        let mut manager = BaselineManager::with_config(&self.baseline_path, self.config.clone())?;

        if manager.remove_baseline(name)? {
            println!("Removed baseline: {}", name);
        } else {
            println!("Baseline not found: {}", name);
        }

        Ok(())
    }

    /// Export baseline to external file
    pub fn export_baseline(&self, name: &str, output_path: &Path) -> Result<(), RegressionError> {
        let manager = BaselineManager::with_config(&self.baseline_path, self.config.clone())?;
        manager.export_baseline(name, output_path)?;
        println!("Exported baseline '{}' to {}", name, output_path.display());
        Ok(())
    }

    /// Import baseline from external file
    pub fn import_baseline(&self, input_path: &Path) -> Result<(), RegressionError> {
        let mut manager = BaselineManager::with_config(&self.baseline_path, self.config.clone())?;
        let name = manager.import_baseline(input_path)?;
        println!("Imported baseline '{}' from {}", name, input_path.display());
        Ok(())
    }

    /// Run full workflow: benchmark -> update baselines -> detect regressions
    pub fn run_full_analysis(&self, benchmark_name: Option<&str>) -> Result<bool, RegressionError> {
        println!("Starting full performance analysis workflow...\n");

        // Step 1: Run benchmarks
        let output = self.run_benchmarks(benchmark_name)?;

        // Step 2: Update baselines
        println!("\nUpdating baselines...");
        let baseline_count = self.update_baselines(&output)?;
        println!("Updated {} baselines", baseline_count);

        // Step 3: Detect regressions
        println!("\nAnalyzing for regressions...");
        let regressions_found = self.detect_regressions(&output)?;

        if regressions_found {
            println!("\n❌ Performance regressions detected!");
        } else {
            println!("\n✅ No performance regressions detected!");
        }

        Ok(regressions_found)
    }

    /// Compare performance between two commits
    pub fn compare_commits(
        &self,
        commit1: &str,
        commit2: &str,
        _benchmark_name: Option<&str>,
    ) -> Result<(), RegressionError> {
        println!(
            "Comparing performance between commits {} and {}",
            commit1, commit2
        );

        // This is a simplified implementation - in practice, you'd want to:
        // 1. Checkout commit1, run benchmarks, store results
        // 2. Checkout commit2, run benchmarks, compare with step 1
        // 3. Restore original checkout

        println!("Note: Commit comparison requires more complex Git operations.");
        println!("For now, use the baseline workflow to track performance over time.");

        Ok(())
    }

    /// Run CI-friendly regression check (exit with error if regressions found)
    pub fn ci_check(&self, benchmark_name: Option<&str>) -> Result<(), RegressionError> {
        let regressions_found = self.run_full_analysis(benchmark_name)?;

        if regressions_found {
            std::process::exit(1);
        }

        Ok(())
    }
}

impl Default for RegressionCli {
    fn default() -> Self {
        Self::new()
    }
}

/// Command-line argument parsing and execution
pub struct CliRunner;

impl CliRunner {
    /// Parse command line arguments and execute appropriate action
    pub fn run(args: Vec<String>) -> Result<(), RegressionError> {
        if args.len() < 2 {
            Self::print_help();
            return Ok(());
        }

        let cli = RegressionCli::new();

        match args[1].as_str() {
            "run" => {
                let benchmark_name = args.get(2).map(|s| s.as_str());
                cli.run_full_analysis(benchmark_name)?;
            }
            "create-baseline" => {
                let benchmark_name = args.get(2).map(|s| s.as_str());
                let output = cli.run_benchmarks(benchmark_name)?;
                cli.create_baselines(&output)?;
            }
            "update-baseline" => {
                let benchmark_name = args.get(2).map(|s| s.as_str());
                let output = cli.run_benchmarks(benchmark_name)?;
                cli.update_baselines(&output)?;
            }
            "check" => {
                let benchmark_name = args.get(2).map(|s| s.as_str());
                cli.detect_regressions(&cli.run_benchmarks(benchmark_name)?)?;
            }
            "list" => {
                cli.list_baselines()?;
            }
            "show" => {
                if let Some(name) = args.get(2) {
                    cli.show_baseline(name)?;
                } else {
                    println!("Error: Baseline name required for 'show' command");
                    return Err(RegressionError::ConfigError(
                        "Missing baseline name".to_string(),
                    ));
                }
            }
            "remove" => {
                if let Some(name) = args.get(2) {
                    cli.remove_baseline(name)?;
                } else {
                    println!("Error: Baseline name required for 'remove' command");
                    return Err(RegressionError::ConfigError(
                        "Missing baseline name".to_string(),
                    ));
                }
            }
            "export" => {
                if let (Some(name), Some(path)) = (args.get(2), args.get(3)) {
                    cli.export_baseline(name, Path::new(path))?;
                } else {
                    println!(
                        "Error: Both baseline name and output path required for 'export' command"
                    );
                    return Err(RegressionError::ConfigError(
                        "Missing arguments for export".to_string(),
                    ));
                }
            }
            "import" => {
                if let Some(path) = args.get(2) {
                    cli.import_baseline(Path::new(path))?;
                } else {
                    println!("Error: Input path required for 'import' command");
                    return Err(RegressionError::ConfigError(
                        "Missing path for import".to_string(),
                    ));
                }
            }
            "ci" => {
                let benchmark_name = args.get(2).map(|s| s.as_str());
                cli.ci_check(benchmark_name)?;
            }
            "help" | "--help" | "-h" => {
                Self::print_help();
            }
            _ => {
                println!("Unknown command: {}", args[1]);
                Self::print_help();
                return Err(RegressionError::ConfigError(format!(
                    "Unknown command: {}",
                    args[1]
                )));
            }
        }

        Ok(())
    }

    /// Print CLI help information
    pub fn print_help() {
        println!("Skreaver Performance Regression Detection Tool");
        println!("==============================================");
        println!();
        println!("USAGE:");
        println!("    skreaver-perf <COMMAND> [OPTIONS]");
        println!();
        println!("COMMANDS:");
        println!(
            "    run [BENCHMARK]         Run full analysis workflow (benchmark -> baseline -> detect)"
        );
        println!("    create-baseline [BENCH] Create new performance baselines");
        println!("    update-baseline [BENCH] Update existing baselines with new measurements");
        println!("    check [BENCHMARK]       Run regression detection against existing baselines");
        println!("    list                    List all available baselines");
        println!("    show <BASELINE>         Show details of a specific baseline");
        println!("    remove <BASELINE>       Remove a baseline");
        println!("    export <BASELINE> <FILE> Export baseline to external file");
        println!("    import <FILE>           Import baseline from external file");
        println!(
            "    ci [BENCHMARK]          CI-friendly check (exits with error if regressions found)"
        );
        println!("    help                    Show this help message");
        println!();
        println!("EXAMPLES:");
        println!(
            "    skreaver-perf run                    # Run all benchmarks and check for regressions"
        );
        println!("    skreaver-perf run quick_benchmark    # Run specific benchmark");
        println!("    skreaver-perf create-baseline        # Create baseline from all benchmarks");
        println!("    skreaver-perf list                   # List all baselines");
        println!("    skreaver-perf show memory_quick/store # Show baseline details");
        println!(
            "    skreaver-perf ci                     # Run in CI mode (exit 1 on regression)"
        );
        println!();
        println!("CONFIGURATION:");
        println!("    Baselines are stored in ./baselines/ by default");
        println!("    Configuration can be customized via RegressionConfig");
        println!("    Default thresholds: Mean +10%, P95 +15%, P99 +20%");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regression::{BaselineManager, PerformanceMeasurement};
    use std::time::Duration;
    use tempfile::TempDir;

    const SAMPLE_BENCHMARK_OUTPUT: &str = r#"
Running benches/quick_benchmark.rs (target/release/deps/quick_benchmark-abc123)
Benchmarking memory_quick/store
memory_quick/store      time:   [394.06 ns 397.42 ns 401.14 ns]
Benchmarking memory_quick/load
memory_quick/load       time:   [476.19 ns 480.95 ns 486.43 ns]
"#;

    #[test]
    fn test_cli_creation() {
        let cli = RegressionCli::new();
        assert_eq!(cli.baseline_path, PathBuf::from("./baselines"));
    }

    #[test]
    fn test_cli_with_custom_path() {
        let temp_dir = TempDir::new().unwrap();
        let cli = RegressionCli::with_baseline_path(temp_dir.path());
        assert_eq!(cli.baseline_path, temp_dir.path());
    }

    #[test]
    fn test_create_baselines() {
        let temp_dir = TempDir::new().unwrap();
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        let count = cli.create_baselines(SAMPLE_BENCHMARK_OUTPUT).unwrap();
        assert_eq!(count, 2);

        // Check that baseline files were created
        let baselines = std::fs::read_dir(temp_dir.path()).unwrap().count();
        assert!(baselines > 0);
    }

    #[test]
    fn test_list_baselines() {
        let temp_dir = TempDir::new().unwrap();
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Initially no baselines
        let result = cli.list_baselines();
        assert!(result.is_ok());

        // Create some baselines
        cli.create_baselines(SAMPLE_BENCHMARK_OUTPUT).unwrap();

        // Now should show baselines
        let result = cli.list_baselines();
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_nonexistent_baseline() {
        let temp_dir = TempDir::new().unwrap();
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        let result = cli.show_baseline("nonexistent");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RegressionError::BaselineNotFound(_)
        ));
    }

    #[test]
    fn test_remove_baseline() {
        let temp_dir = TempDir::new().unwrap();
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Create baseline
        cli.create_baselines(SAMPLE_BENCHMARK_OUTPUT).unwrap();

        // Remove it
        let result = cli.remove_baseline("memory_quick/store");
        assert!(result.is_ok());
    }

    #[test]
    fn test_export_import_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Create baseline
        cli.create_baselines(SAMPLE_BENCHMARK_OUTPUT).unwrap();

        // Export
        let export_path = temp_dir.path().join("exported.json");
        let result = cli.export_baseline("memory_quick/store", &export_path);
        assert!(result.is_ok());
        assert!(export_path.exists());

        // Remove original
        cli.remove_baseline("memory_quick/store").unwrap();

        // Import back
        let result = cli.import_baseline(&export_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_runner_help() {
        let args = vec!["skreaver-perf".to_string(), "help".to_string()];
        let result = CliRunner::run(args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_runner_unknown_command() {
        let args = vec!["skreaver-perf".to_string(), "unknown".to_string()];
        let result = CliRunner::run(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_regression_detection_workflow() {
        let temp_dir = TempDir::new().unwrap();

        // Create baseline manager directly for more control
        let mut manager = BaselineManager::new(temp_dir.path()).unwrap();

        // Create baseline measurements manually
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
            let measurement = PerformanceMeasurement::from(result);
            manager.update_baseline(measurement).unwrap();
        }

        // Create slower measurement for regression detection
        let slow_result = crate::benchmarks::BenchmarkResult {
            name: "memory_quick/store".to_string(),
            iterations: 100,
            mean: Duration::from_nanos(600), // 50% slower
            median: Duration::from_nanos(600),
            min: Duration::from_nanos(590),
            max: Duration::from_nanos(610),
            std_dev: Duration::from_nanos(5),
            throughput: None,
            total_operations: None,
        };
        let slow_measurement = PerformanceMeasurement::from(slow_result);

        // Detect regression
        let analysis = manager.detect_regression(&slow_measurement).unwrap();
        assert!(analysis.is_regression);
        assert!(analysis.mean_change_percent > 10.0);
    }
}
