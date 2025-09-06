//! Production Benchmark Framework
//!
//! This module provides a centralized framework for performance measurement
//! aligned with the DEVELOPMENT_PLAN.md Phase 0.2 requirements.
//!
//! ## Key Components
//! - **Framework**: Centralized benchmark execution with standardized metrics
//! - **Metrics**: Memory, CPU, and resource monitoring during benchmarks
//! - **Baseline**: Historical performance tracking and regression detection
//! - **Reporting**: Standardized JSON output for CI integration
//!
//! ## Performance Targets (from DEVELOPMENT_PLAN.md)
//! - **Latency**: p50 < 30ms, p95 < 200ms, p99 < 400ms
//! - **Memory**: RSS ≤ 128MB with N=32 concurrent sessions
//! - **Error Rate**: ≤ 0.5% on integration tests
//! - **Resource**: CPU usage tracking and limits

pub mod baseline;
pub mod framework;
pub mod metrics;
pub mod reporting;

// Re-export key types for easy access
pub use baseline::{BaselineComparison, BaselineManager, PerformanceBaseline};
pub use framework::{BenchmarkConfig, BenchmarkFramework, BenchmarkResult, PerformanceThresholds};
pub use metrics::{CpuTracker, MemoryTracker, ResourceMetrics};
pub use reporting::{BenchmarkReport, ReportFormat, ReportRecommendation};
