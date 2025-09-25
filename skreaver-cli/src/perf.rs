//! Performance regression detection CLI commands

use crate::PerfCommands;
use serde::{Deserialize, Serialize};
use skreaver_testing::{RegressionCli, RegressionConfig, RegressionError};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Performance configuration from JSON file
#[derive(Debug, Serialize, Deserialize)]
struct PerformanceConfig {
    regression_thresholds: RegressionThresholds,
}

#[derive(Debug, Serialize, Deserialize)]
struct RegressionThresholds {
    mean_percent: f64,
    p95_percent: f64,
    p99_percent: f64,
}

/// Load configuration from JSON file, with fallbacks
fn load_config_from_file() -> RegressionConfig {
    let config_path = ".github/perf-config.json";

    let mut config = RegressionConfig::default();

    if let Ok(content) = fs::read_to_string(config_path) {
        if let Ok(perf_config) = serde_json::from_str::<PerformanceConfig>(&content) {
            config.mean_threshold_percent = perf_config.regression_thresholds.mean_percent;
            config.p95_threshold_percent = perf_config.regression_thresholds.p95_percent;
            config.p99_threshold_percent = perf_config.regression_thresholds.p99_percent;
        }
    }

    config
}

/// Create CLI instance with environment-aware configuration
fn create_regression_cli() -> Result<RegressionCli, RegressionError> {
    // Read baseline directory from environment or use default
    let baseline_path = env::var("SKREAVER_BASELINE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./target/performance-baselines"));

    // Load config from file, then apply environment overrides
    let mut config = load_config_from_file();

    // Allow environment variables to override thresholds
    if let Ok(mean_threshold) = env::var("SKREAVER_MEAN_THRESHOLD_PERCENT")
        && let Ok(threshold) = mean_threshold.parse::<f64>()
    {
        config.mean_threshold_percent = threshold;
    }

    if let Ok(p95_threshold) = env::var("SKREAVER_P95_THRESHOLD_PERCENT")
        && let Ok(threshold) = p95_threshold.parse::<f64>()
    {
        config.p95_threshold_percent = threshold;
    }

    if let Ok(p99_threshold) = env::var("SKREAVER_P99_THRESHOLD_PERCENT")
        && let Ok(threshold) = p99_threshold.parse::<f64>()
    {
        config.p99_threshold_percent = threshold;
    }

    if let Ok(min_samples) = env::var("SKREAVER_MIN_SAMPLES")
        && let Ok(samples) = min_samples.parse::<usize>()
    {
        config.min_samples = samples;
    }

    // For now, use the baseline path. The regression detection config
    // can be extended later if needed for more granular control
    Ok(RegressionCli::with_baseline_path(&baseline_path))
}

pub fn run_perf_command(command: PerfCommands) -> Result<(), RegressionError> {
    let cli = create_regression_cli()?;

    match command {
        PerfCommands::Run { benchmark } => {
            println!("🚀 Running full performance analysis...");
            let regressions_found = cli.run_full_analysis(benchmark.as_deref())?;

            if regressions_found {
                println!("❌ Performance regressions detected!");
                std::process::exit(1);
            } else {
                println!("✅ No performance regressions detected!");
            }
        }

        PerfCommands::CreateBaseline { benchmark } => {
            println!("📊 Creating performance baselines...");
            let output = cli.run_benchmarks(benchmark.as_deref())?;
            let count = cli.create_baselines(&output)?;
            println!("✅ Created {} baseline(s)", count);
        }

        PerfCommands::UpdateBaseline { benchmark } => {
            println!("🔄 Updating performance baselines...");
            let output = cli.run_benchmarks(benchmark.as_deref())?;
            let count = cli.update_baselines(&output)?;
            println!("✅ Updated {} baseline(s)", count);
        }

        PerfCommands::Check { benchmark } => {
            println!("🔍 Checking for performance regressions...");
            let output = cli.run_benchmarks(benchmark.as_deref())?;
            let regressions_found = cli.detect_regressions(&output)?;

            if regressions_found {
                std::process::exit(1);
            }
        }

        PerfCommands::List => {
            println!("📋 Available performance baselines:");
            cli.list_baselines()?;
        }

        PerfCommands::Show { name } => {
            println!("📊 Baseline details for '{}':", name);
            cli.show_baseline(&name)?;
        }

        PerfCommands::Remove { name } => {
            println!("🗑️  Removing baseline '{}'...", name);
            cli.remove_baseline(&name)?;
        }

        PerfCommands::Export { name, path } => {
            println!("📤 Exporting baseline '{}' to {}...", name, path);
            cli.export_baseline(&name, Path::new(&path))?;
        }

        PerfCommands::Import { path } => {
            println!("📥 Importing baseline from {}...", path);
            cli.import_baseline(Path::new(&path))?;
        }

        PerfCommands::Ci { benchmark } => {
            println!("🤖 Running CI performance check...");
            cli.ci_check(benchmark.as_deref())?;
        }
    }

    Ok(())
}
