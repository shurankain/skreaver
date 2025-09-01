//! # Performance Benchmarks
//!
//! This module provides performance testing and benchmarking capabilities
//! for agents, tools, and the overall Skreaver runtime.

use crate::{MockTool, MockToolRegistry};
use skreaver_core::{Agent, InMemoryMemory, MemoryReader, MemoryUpdate, MemoryWriter, Tool};
use skreaver_http::runtime::Coordinator;
use skreaver_tools::ToolRegistry;
use std::time::{Duration, Instant};

/// Performance benchmark runner
pub struct BenchmarkRunner {
    results: Vec<BenchmarkResult>,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Benchmark agent execution performance
    pub fn benchmark_agent<A, R>(
        &mut self,
        name: impl Into<String>,
        coordinator: &mut Coordinator<A, R>,
        observation: A::Observation,
        iterations: usize,
    ) -> &BenchmarkResult
    where
        A: Agent,
        A::Observation: Clone + std::fmt::Display,
        A::Action: ToString,
        R: ToolRegistry + Clone,
    {
        let mut durations = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            let start = Instant::now();
            let _action = coordinator.step(observation.clone());
            durations.push(start.elapsed());
        }

        let result = BenchmarkResult::from_durations(name.into(), durations);
        self.results.push(result);
        self.results.last().unwrap()
    }

    /// Benchmark tool execution performance
    pub fn benchmark_tool(
        &mut self,
        name: impl Into<String>,
        tool: &dyn Tool,
        input: impl Into<String>,
        iterations: usize,
    ) -> &BenchmarkResult {
        let input_str = input.into();
        let mut durations = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            let start = Instant::now();
            let _result = tool.call(input_str.clone());
            durations.push(start.elapsed());
        }

        let result = BenchmarkResult::from_durations(name.into(), durations);
        self.results.push(result);
        self.results.last().unwrap()
    }

    /// Benchmark memory operations
    pub fn benchmark_memory(
        &mut self,
        name: impl Into<String>,
        memory: &mut dyn MemoryWriter,
        iterations: usize,
    ) -> &BenchmarkResult {
        let mut durations = Vec::with_capacity(iterations);

        for i in 0..iterations {
            let key_str = format!("bench_key_{}", i);
            let value = format!("bench_value_{}", i);

            let start = Instant::now();
            if let Ok(update) = MemoryUpdate::new(&key_str, &value) {
                let _ = memory.store(update);
            }
            durations.push(start.elapsed());
        }

        let result = BenchmarkResult::from_durations(name.into(), durations);
        self.results.push(result);
        self.results.last().unwrap()
    }

    /// Run throughput benchmark (operations per second)
    pub fn benchmark_throughput<F>(
        &mut self,
        name: impl Into<String>,
        mut operation: F,
        duration: Duration,
    ) -> &BenchmarkResult
    where
        F: FnMut(),
    {
        let start = Instant::now();
        let mut count = 0;
        let mut operation_times = Vec::new();

        while start.elapsed() < duration {
            let op_start = Instant::now();
            operation();
            operation_times.push(op_start.elapsed());
            count += 1;
        }

        let total_time = start.elapsed();
        let ops_per_sec = count as f64 / total_time.as_secs_f64();

        let mut result = BenchmarkResult::from_durations(name.into(), operation_times);
        result.throughput = Some(ops_per_sec);
        result.total_operations = Some(count);

        self.results.push(result);
        self.results.last().unwrap()
    }

    /// Get all benchmark results
    pub fn results(&self) -> &[BenchmarkResult] {
        &self.results
    }

    /// Print a summary of all benchmark results
    pub fn print_summary(&self) {
        println!("Benchmark Results");
        println!("================");

        for result in &self.results {
            println!("{}", result.summary());
        }

        if !self.results.is_empty() {
            println!("\nOverall Statistics:");
            let total_benchmarks = self.results.len();
            let avg_mean = self.results.iter().map(|r| r.mean.as_nanos()).sum::<u128>()
                / total_benchmarks as u128;

            println!("  Total benchmarks: {}", total_benchmarks);
            println!("  Average mean time: {}μs", avg_mean / 1000);
        }
    }

    /// Clear all results
    pub fn clear(&mut self) {
        self.results.clear();
    }
}

impl Default for BenchmarkRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a performance benchmark
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub mean: Duration,
    pub median: Duration,
    pub min: Duration,
    pub max: Duration,
    pub std_dev: Duration,
    pub throughput: Option<f64>, // operations per second
    pub total_operations: Option<usize>,
}

impl BenchmarkResult {
    /// Create benchmark result from a collection of durations
    pub fn from_durations(name: String, mut durations: Vec<Duration>) -> Self {
        durations.sort();

        let iterations = durations.len();
        let min = *durations.first().unwrap_or(&Duration::ZERO);
        let max = *durations.last().unwrap_or(&Duration::ZERO);

        // Calculate mean
        let total_nanos: u128 = durations.iter().map(|d| d.as_nanos()).sum();
        let mean = Duration::from_nanos((total_nanos / iterations as u128) as u64);

        // Calculate median
        let median = if iterations % 2 == 0 {
            let mid1 = durations[iterations / 2 - 1];
            let mid2 = durations[iterations / 2];
            Duration::from_nanos(((mid1.as_nanos() + mid2.as_nanos()) / 2) as u64)
        } else {
            durations[iterations / 2]
        };

        // Calculate standard deviation
        let variance_sum: f64 = durations
            .iter()
            .map(|d| {
                let diff = d.as_nanos() as f64 - mean.as_nanos() as f64;
                diff * diff
            })
            .sum();

        let variance = variance_sum / iterations as f64;
        let std_dev = Duration::from_nanos(variance.sqrt() as u64);

        Self {
            name,
            iterations,
            mean,
            median,
            min,
            max,
            std_dev,
            throughput: None,
            total_operations: None,
        }
    }

    /// Get a formatted summary of the benchmark result
    pub fn summary(&self) -> String {
        let mut summary = format!(
            "{}: {} iterations, mean: {}μs, median: {}μs, min: {}μs, max: {}μs, std: {}μs",
            self.name,
            self.iterations,
            self.mean.as_micros(),
            self.median.as_micros(),
            self.min.as_micros(),
            self.max.as_micros(),
            self.std_dev.as_micros()
        );

        if let Some(throughput) = self.throughput {
            summary.push_str(&format!(", throughput: {:.0} ops/sec", throughput));
        }

        summary
    }

    /// Check if performance meets a target threshold
    pub fn meets_target(&self, target_mean: Duration) -> bool {
        self.mean <= target_mean
    }

    /// Get performance grade based on common thresholds
    pub fn performance_grade(&self) -> PerformanceGrade {
        match self.mean.as_micros() {
            0..=100 => PerformanceGrade::Excellent,
            101..=1000 => PerformanceGrade::Good,
            1001..=10000 => PerformanceGrade::Fair,
            _ => PerformanceGrade::Poor,
        }
    }
}

/// Performance grade for benchmark results
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerformanceGrade {
    Excellent,
    Good,
    Fair,
    Poor,
}

impl std::fmt::Display for PerformanceGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PerformanceGrade::Excellent => write!(f, "Excellent (≤100μs)"),
            PerformanceGrade::Good => write!(f, "Good (≤1ms)"),
            PerformanceGrade::Fair => write!(f, "Fair (≤10ms)"),
            PerformanceGrade::Poor => write!(f, "Poor (>10ms)"),
        }
    }
}

/// Collection of predefined performance tests
pub struct PerformanceTest;

impl PerformanceTest {
    /// Run standard agent performance tests
    pub fn run_agent_benchmarks<A, R>(
        coordinator: &mut Coordinator<A, R>,
        observation: A::Observation,
    ) -> Vec<BenchmarkResult>
    where
        A: Agent,
        A::Observation: Clone + std::fmt::Display,
        A::Action: ToString,
        R: ToolRegistry + Clone,
    {
        let mut runner = BenchmarkRunner::new();

        // Different iteration counts for different aspects
        runner.benchmark_agent("agent_single_step", coordinator, observation.clone(), 1);
        runner.benchmark_agent("agent_10_steps", coordinator, observation.clone(), 10);
        runner.benchmark_agent("agent_100_steps", coordinator, observation.clone(), 100);
        runner.benchmark_agent("agent_1000_steps", coordinator, observation, 1000);

        runner.results().to_vec()
    }

    /// Run standard tool performance tests
    pub fn run_tool_benchmarks() -> Vec<BenchmarkResult> {
        let mut runner = BenchmarkRunner::new();

        // Test different mock tool scenarios
        let fast_tool = MockTool::new("fast_tool").with_default_response("fast");

        let slow_tool = MockTool::new("slow_tool").with_default_response("slow");

        runner.benchmark_tool("fast_tool_100", &fast_tool, "test", 100);
        runner.benchmark_tool("fast_tool_1000", &fast_tool, "test", 1000);
        runner.benchmark_tool("slow_tool_100", &slow_tool, "test", 100);

        runner.results().to_vec()
    }

    /// Run memory performance benchmarks
    pub fn run_memory_benchmarks() -> Vec<BenchmarkResult> {
        let mut runner = BenchmarkRunner::new();
        let mut memory = InMemoryMemory::new();

        runner.benchmark_memory("memory_store_100", &mut memory, 100);
        runner.benchmark_memory("memory_store_1000", &mut memory, 1000);

        runner.results().to_vec()
    }

    /// Run comprehensive performance test suite
    pub fn run_full_benchmark_suite() {
        println!("Running Skreaver Performance Benchmark Suite");
        println!("============================================");

        // Mock agent for testing
        use skreaver_core::{Agent, InMemoryMemory, MemoryUpdate};
        use skreaver_core::{ExecutionResult, ToolCall};

        struct BenchAgent {
            memory: InMemoryMemory,
        }

        impl Agent for BenchAgent {
            type Observation = String;
            type Action = String;

            fn observe(&mut self, _input: String) {}
            fn act(&mut self) -> String {
                "bench response".to_string()
            }
            fn call_tools(&self) -> Vec<ToolCall> {
                Vec::new()
            }
            fn handle_result(&mut self, _result: ExecutionResult) {}
            fn update_context(&mut self, update: MemoryUpdate) {
                let _ = self.memory_writer().store(update);
            }
            fn memory_reader(&self) -> &dyn MemoryReader {
                &self.memory
            }
            fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
                &mut self.memory
            }
        }

        let agent = BenchAgent {
            memory: InMemoryMemory::new(),
        };
        let registry = MockToolRegistry::new().with_echo_tool();
        let mut coordinator = Coordinator::new(agent, registry);

        println!("\nAgent Performance:");
        let agent_results = Self::run_agent_benchmarks(&mut coordinator, "test".to_string());
        for result in agent_results {
            println!("  {}", result.summary());
        }

        println!("\nTool Performance:");
        let tool_results = Self::run_tool_benchmarks();
        for result in tool_results {
            println!("  {}", result.summary());
        }

        println!("\nMemory Performance:");
        let memory_results = Self::run_memory_benchmarks();
        for result in memory_results {
            println!("  {}", result.summary());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_result_from_durations() {
        let durations = vec![
            Duration::from_micros(100),
            Duration::from_micros(200),
            Duration::from_micros(150),
        ];

        let result = BenchmarkResult::from_durations("test".to_string(), durations);

        assert_eq!(result.name, "test");
        assert_eq!(result.iterations, 3);
        assert_eq!(result.min, Duration::from_micros(100));
        assert_eq!(result.max, Duration::from_micros(200));
        assert_eq!(result.median, Duration::from_micros(150));
    }

    #[test]
    fn performance_grade_classification() {
        let excellent =
            BenchmarkResult::from_durations("fast".to_string(), vec![Duration::from_micros(50)]);
        assert_eq!(excellent.performance_grade(), PerformanceGrade::Excellent);

        let poor =
            BenchmarkResult::from_durations("slow".to_string(), vec![Duration::from_millis(20)]);
        assert_eq!(poor.performance_grade(), PerformanceGrade::Poor);
    }

    #[test]
    fn benchmark_runner_works() {
        let mut runner = BenchmarkRunner::new();
        let tool = MockTool::new("test").with_default_response("response");

        let result = runner.benchmark_tool("test_benchmark", &tool, "input", 10);

        assert_eq!(result.name, "test_benchmark");
        assert_eq!(result.iterations, 10);
        assert!(result.mean > Duration::ZERO);
    }
}
