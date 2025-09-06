//! Benchmark Reporting
//!
//! Standardized reporting formats for CI integration and analysis.

use super::baseline::BaselineComparison;
use super::framework::BenchmarkResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Complete benchmark report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    /// Report metadata
    pub metadata: ReportMetadata,
    /// Individual benchmark results
    pub results: Vec<BenchmarkResult>,
    /// Baseline comparisons (if available)
    pub baseline_comparisons: Vec<BaselineComparison>,
    /// Overall summary
    pub summary: ReportSummary,
}

/// Report metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    /// When the report was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// Git commit hash (if available)
    pub commit_hash: Option<String>,
    /// Branch name (if available)
    pub branch: Option<String>,
    /// CI job ID or build number
    pub build_id: Option<String>,
    /// Environment information
    pub environment: HashMap<String, String>,
    /// Skreaver version
    pub skreaver_version: String,
}

/// Report summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    /// Total number of benchmarks run
    pub total_benchmarks: u32,
    /// Number of benchmarks that passed all thresholds
    pub passed_benchmarks: u32,
    /// Number of benchmarks that failed thresholds
    pub failed_benchmarks: u32,
    /// Number of performance regressions detected
    pub regressions_detected: u32,
    /// Overall pass rate
    pub pass_rate: f64,
    /// Average p50 latency across all benchmarks
    pub avg_p50_ms: f64,
    /// Average p95 latency across all benchmarks
    pub avg_p95_ms: f64,
    /// Peak memory usage across all benchmarks
    pub peak_memory_mb: f64,
    /// Overall recommendation
    pub recommendation: ReportRecommendation,
}

/// Report recommendation based on results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportRecommendation {
    /// All benchmarks passed, ready to merge/deploy
    Pass,
    /// Some performance issues but within acceptable limits
    Warning { issues: Vec<String> },
    /// Significant performance regressions or failures
    Fail { critical_issues: Vec<String> },
}

/// Supported report formats
#[derive(Debug, Clone, Copy)]
pub enum ReportFormat {
    /// Human-readable JSON format
    Json,
    /// GitHub Actions format for CI integration
    GitHubActions,
    /// JUnit XML format for CI systems
    JunitXml,
    /// Markdown format for reports and PRs
    Markdown,
}

impl BenchmarkReport {
    /// Create a new benchmark report
    pub fn new(
        results: Vec<BenchmarkResult>,
        baseline_comparisons: Vec<BaselineComparison>,
    ) -> Self {
        let summary = Self::calculate_summary(&results, &baseline_comparisons);

        Self {
            metadata: ReportMetadata {
                generated_at: chrono::Utc::now(),
                commit_hash: get_git_commit_hash(),
                branch: get_git_branch(),
                build_id: std::env::var("BUILD_ID").ok(),
                environment: collect_environment_info(),
                skreaver_version: env!("CARGO_PKG_VERSION").to_string(),
            },
            results,
            baseline_comparisons,
            summary,
        }
    }

    /// Save report to file in the specified format
    pub fn save_to_file<P: AsRef<Path>>(
        &self,
        path: P,
        format: ReportFormat,
    ) -> Result<(), ReportError> {
        let content = match format {
            ReportFormat::Json => self.to_json()?,
            ReportFormat::GitHubActions => self.to_github_actions()?,
            ReportFormat::JunitXml => self.to_junit_xml()?,
            ReportFormat::Markdown => self.to_markdown()?,
        };

        fs::write(path, content)
            .map_err(|e| ReportError::Io(format!("Failed to write report file: {}", e)))?;

        Ok(())
    }

    /// Convert to JSON format
    pub fn to_json(&self) -> Result<String, ReportError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ReportError::Serialization(format!("JSON serialization failed: {}", e)))
    }

    /// Convert to GitHub Actions format
    pub fn to_github_actions(&self) -> Result<String, ReportError> {
        let mut output = String::new();

        // Set outputs for GitHub Actions
        output.push_str(&format!(
            "::set-output name=total_benchmarks::{}\n",
            self.summary.total_benchmarks
        ));
        output.push_str(&format!(
            "::set-output name=passed_benchmarks::{}\n",
            self.summary.passed_benchmarks
        ));
        output.push_str(&format!(
            "::set-output name=failed_benchmarks::{}\n",
            self.summary.failed_benchmarks
        ));
        output.push_str(&format!(
            "::set-output name=pass_rate::{:.2}\n",
            self.summary.pass_rate
        ));
        output.push_str(&format!(
            "::set-output name=avg_p95_ms::{:.2}\n",
            self.summary.avg_p95_ms
        ));
        output.push_str(&format!(
            "::set-output name=peak_memory_mb::{:.2}\n",
            self.summary.peak_memory_mb
        ));

        // Add annotations for failures
        for result in &self.results {
            if !result.passed {
                for violation in &result.violations {
                    let message = format!(
                        "Performance threshold violation in '{}': {} expected ≤ {:.2}, got {:.2}",
                        result.config.name, violation.metric, violation.expected, violation.actual
                    );
                    match violation.severity {
                        crate::benchmarks::framework::ViolationSeverity::Critical => {
                            output.push_str(&format!("::error::{}\n", message));
                        }
                        crate::benchmarks::framework::ViolationSeverity::Error => {
                            output.push_str(&format!("::error::{}\n", message));
                        }
                        crate::benchmarks::framework::ViolationSeverity::Warning => {
                            output.push_str(&format!("::warning::{}\n", message));
                        }
                    }
                }
            }
        }

        // Add regression warnings
        for comparison in &self.baseline_comparisons {
            for comp in &comparison.comparisons {
                if comp.regression_detected {
                    let message = format!(
                        "Performance regression detected in '{}': p95 latency increased from {:.2}ms to {:.2}ms",
                        comp.benchmark_name, comp.baseline_p95_ms, comp.current_p95_ms
                    );
                    output.push_str(&format!("::warning::{}\n", message));
                }
            }
        }

        Ok(output)
    }

    /// Convert to JUnit XML format
    pub fn to_junit_xml(&self) -> Result<String, ReportError> {
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str(&format!(
            "<testsuite name=\"Skreaver Benchmarks\" tests=\"{}\" failures=\"{}\" time=\"{:.3}\">\n",
            self.summary.total_benchmarks,
            self.summary.failed_benchmarks,
            self.results.iter().map(|r| r.stats.total_duration.as_secs_f64()).sum::<f64>()
        ));

        for result in &self.results {
            xml.push_str(&format!(
                "  <testcase name=\"{}\" time=\"{:.3}\"",
                result.config.name,
                result.stats.total_duration.as_secs_f64()
            ));

            if result.passed {
                xml.push_str(" />\n");
            } else {
                xml.push_str(">\n");
                xml.push_str("    <failure message=\"Performance threshold violations\">\n");
                for violation in &result.violations {
                    xml.push_str(&format!(
                        "      {}: expected ≤ {:.2}, got {:.2}\n",
                        violation.metric, violation.expected, violation.actual
                    ));
                }
                xml.push_str("    </failure>\n");
                xml.push_str("  </testcase>\n");
            }
        }

        xml.push_str("</testsuite>\n");
        Ok(xml)
    }

    /// Convert to Markdown format
    pub fn to_markdown(&self) -> Result<String, ReportError> {
        let mut md = String::new();

        // Header
        md.push_str("# Skreaver Benchmark Report\n\n");
        md.push_str(&format!(
            "**Generated:** {}\n",
            self.metadata.generated_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        if let Some(ref commit) = self.metadata.commit_hash {
            md.push_str(&format!("**Commit:** `{}`\n", commit));
        }
        if let Some(ref branch) = self.metadata.branch {
            md.push_str(&format!("**Branch:** `{}`\n", branch));
        }
        md.push_str(&format!(
            "**Skreaver Version:** {}\n\n",
            self.metadata.skreaver_version
        ));

        // Summary
        md.push_str("## Summary\n\n");
        md.push_str(&format!(
            "- **Total Benchmarks:** {}\n",
            self.summary.total_benchmarks
        ));
        md.push_str(&format!(
            "- **Passed:** {} ✅\n",
            self.summary.passed_benchmarks
        ));
        md.push_str(&format!(
            "- **Failed:** {} ❌\n",
            self.summary.failed_benchmarks
        ));
        md.push_str(&format!(
            "- **Pass Rate:** {:.1}%\n",
            self.summary.pass_rate
        ));
        md.push_str(&format!(
            "- **Average p95 Latency:** {:.2}ms\n",
            self.summary.avg_p95_ms
        ));
        md.push_str(&format!(
            "- **Peak Memory Usage:** {:.1}MB\n",
            self.summary.peak_memory_mb
        ));

        match &self.summary.recommendation {
            ReportRecommendation::Pass => {
                md.push_str("\n✅ **All benchmarks passed!** Ready to merge.\n\n");
            }
            ReportRecommendation::Warning { issues } => {
                md.push_str("\n⚠️ **Performance warnings detected:**\n");
                for issue in issues {
                    md.push_str(&format!("- {}\n", issue));
                }
                md.push('\n');
            }
            ReportRecommendation::Fail { critical_issues } => {
                md.push_str("\n❌ **Critical performance issues detected:**\n");
                for issue in critical_issues {
                    md.push_str(&format!("- {}\n", issue));
                }
                md.push('\n');
            }
        }

        // Detailed results table
        md.push_str("## Detailed Results\n\n");
        md.push_str("| Benchmark | Status | p50 | p95 | p99 | Memory | Errors |\n");
        md.push_str("|-----------|--------|-----|-----|-----|--------|--------|\n");

        for result in &self.results {
            let status = if result.passed { "✅" } else { "❌" };
            let p50 = format!("{:.1}ms", result.stats.percentiles.p50.as_millis());
            let p95 = format!("{:.1}ms", result.stats.percentiles.p95.as_millis());
            let p99 = format!("{:.1}ms", result.stats.percentiles.p99.as_millis());
            let memory = result
                .resources
                .as_ref()
                .and_then(|r| r.memory.as_ref())
                .map(|m| format!("{:.1}MB", m.peak_rss_bytes as f64 / 1_048_576.0))
                .unwrap_or_else(|| "N/A".to_string());
            let errors = if result.stats.error_count > 0 {
                format!("{}", result.stats.error_count)
            } else {
                "-".to_string()
            };

            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                result.config.name, status, p50, p95, p99, memory, errors
            ));
        }

        Ok(md)
    }

    /// Calculate report summary from results
    fn calculate_summary(
        results: &[BenchmarkResult],
        baseline_comparisons: &[BaselineComparison],
    ) -> ReportSummary {
        let total_benchmarks = results.len() as u32;
        let passed_benchmarks = results.iter().filter(|r| r.passed).count() as u32;
        let failed_benchmarks = total_benchmarks - passed_benchmarks;

        let regressions_detected = baseline_comparisons
            .iter()
            .map(|bc| {
                bc.comparisons
                    .iter()
                    .filter(|c| c.regression_detected)
                    .count()
            })
            .sum::<usize>() as u32;

        let pass_rate = if total_benchmarks > 0 {
            (passed_benchmarks as f64 / total_benchmarks as f64) * 100.0
        } else {
            0.0
        };

        let avg_p50_ms = if !results.is_empty() {
            results
                .iter()
                .map(|r| r.stats.percentiles.p50.as_millis() as f64)
                .sum::<f64>()
                / results.len() as f64
        } else {
            0.0
        };

        let avg_p95_ms = if !results.is_empty() {
            results
                .iter()
                .map(|r| r.stats.percentiles.p95.as_millis() as f64)
                .sum::<f64>()
                / results.len() as f64
        } else {
            0.0
        };

        let peak_memory_mb = results
            .iter()
            .filter_map(|r| r.resources.as_ref().and_then(|res| res.memory.as_ref()))
            .map(|m| m.peak_rss_bytes as f64 / 1_048_576.0)
            .fold(0.0, f64::max);

        // Determine recommendation
        let recommendation = if failed_benchmarks == 0 && regressions_detected == 0 {
            ReportRecommendation::Pass
        } else if failed_benchmarks == 0 && regressions_detected > 0 {
            ReportRecommendation::Warning {
                issues: vec![format!(
                    "{} performance regressions detected",
                    regressions_detected
                )],
            }
        } else {
            let mut critical_issues = Vec::new();
            if failed_benchmarks > 0 {
                critical_issues.push(format!(
                    "{} benchmarks failed threshold checks",
                    failed_benchmarks
                ));
            }
            if regressions_detected > 0 {
                critical_issues.push(format!(
                    "{} performance regressions detected",
                    regressions_detected
                ));
            }
            ReportRecommendation::Fail { critical_issues }
        };

        ReportSummary {
            total_benchmarks,
            passed_benchmarks,
            failed_benchmarks,
            regressions_detected,
            pass_rate,
            avg_p50_ms,
            avg_p95_ms,
            peak_memory_mb,
            recommendation,
        }
    }
}

/// Get git commit hash if available
fn get_git_commit_hash() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Get git branch name if available
fn get_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
}

/// Collect relevant environment information
fn collect_environment_info() -> HashMap<String, String> {
    let mut env = HashMap::new();

    env.insert("os".to_string(), std::env::consts::OS.to_string());
    env.insert("arch".to_string(), std::env::consts::ARCH.to_string());
    env.insert(
        "rustc_version".to_string(),
        std::env::var("RUSTC_VERSION").unwrap_or_else(|_| "unknown".to_string()),
    );

    // Add common CI environment variables
    for var in &["CI", "GITHUB_ACTIONS", "GITHUB_WORKFLOW", "RUNNER_OS"] {
        if let Ok(value) = std::env::var(var) {
            env.insert(var.to_lowercase(), value);
        }
    }

    env
}

/// Report generation errors
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}
