//! Production Benchmark using the new Framework
//!
//! This benchmark demonstrates the new production benchmark framework
//! with resource monitoring and baseline comparison capabilities.

use criterion::{Criterion, criterion_group, criterion_main};
use skreaver::{
    Agent, ExecutionResult, InMemoryMemory, MemoryReader, MemoryUpdate, MemoryWriter, ToolCall,
};
use skreaver_http::runtime::Coordinator;
use skreaver_testing::{MockTool, MockToolRegistry};
use skreaver_workspace::benchmarks::{
    BenchmarkConfig, BenchmarkFramework, BenchmarkReport, ReportFormat, ReportRecommendation,
};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Production benchmark using the new framework
fn production_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("production_agent_loop", |b| {
        b.to_async(&rt).iter(|| async {
            // Create the benchmark framework
            let mut framework = BenchmarkFramework::new();

            // Configure benchmark according to development plan targets
            let config = BenchmarkConfig {
                name: "agent_tool_loop".to_string(),
                iterations: 100,
                warmup_iterations: 10,
                max_duration: Duration::from_secs(30),
                track_memory: true,
                track_cpu: true,
                thresholds: Default::default(), // Uses development plan targets
            };

            // Run the benchmark
            let result = framework
                .execute_benchmark(config, || async { execute_agent_tool_loop().await })
                .await
                .unwrap();

            // Log results for visibility
            println!("Benchmark completed:");
            println!("  p50: {:?}", result.stats.percentiles.p50);
            println!("  p95: {:?}", result.stats.percentiles.p95);
            println!("  p99: {:?}", result.stats.percentiles.p99);

            if let Some(ref resources) = result.resources {
                if let Some(ref memory) = resources.memory {
                    println!(
                        "  Peak RSS: {:.2}MB",
                        memory.peak_rss_bytes as f64 / 1_048_576.0
                    );
                }
                if let Some(ref cpu) = resources.cpu {
                    println!("  Avg CPU: {:.2}%", cpu.avg_cpu_percent);
                }
            }

            if !result.passed {
                println!("  ⚠️  Threshold violations:");
                for violation in &result.violations {
                    println!(
                        "    {}: expected ≤ {:.2}, got {:.2}",
                        violation.metric, violation.expected, violation.actual
                    );
                }
            }

            result
        })
    });
}

/// Enhanced production benchmark with full reporting
fn enhanced_production_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("enhanced_production_benchmark", |b| {
        b.to_async(&rt).iter(|| async {
            // Run multiple benchmark scenarios
            let mut all_results = Vec::new();

            // Scenario 1: Basic agent loop
            let mut framework = BenchmarkFramework::new();
            let basic_config = BenchmarkConfig {
                name: "basic_agent_loop".to_string(),
                iterations: 50,
                warmup_iterations: 5,
                max_duration: Duration::from_secs(15),
                track_memory: true,
                track_cpu: true,
                thresholds: Default::default(),
            };

            let basic_result = framework
                .execute_benchmark(basic_config, || async { execute_basic_agent_loop().await })
                .await
                .unwrap();
            all_results.push(basic_result);

            // Scenario 2: Agent with tools
            let tool_config = BenchmarkConfig {
                name: "agent_with_tools".to_string(),
                iterations: 30,
                warmup_iterations: 5,
                max_duration: Duration::from_secs(20),
                track_memory: true,
                track_cpu: true,
                thresholds: Default::default(),
            };

            let tool_result = framework
                .execute_benchmark(tool_config, || async { execute_agent_with_tools().await })
                .await
                .unwrap();
            all_results.push(tool_result);

            // Generate comprehensive report
            let report = BenchmarkReport::new(all_results, Vec::new());

            // Save reports in different formats (for demonstration)
            if let Err(e) = report.save_to_file("target/benchmark_report.json", ReportFormat::Json)
            {
                eprintln!("Failed to save JSON report: {}", e);
            }

            if let Err(e) =
                report.save_to_file("target/benchmark_report.md", ReportFormat::Markdown)
            {
                eprintln!("Failed to save Markdown report: {}", e);
            }

            println!("📊 Benchmark Report Summary:");
            println!("  Total benchmarks: {}", report.summary.total_benchmarks);
            println!("  Pass rate: {:.1}%", report.summary.pass_rate);
            println!("  Avg p95 latency: {:.2}ms", report.summary.avg_p95_ms);
            println!("  Peak memory: {:.1}MB", report.summary.peak_memory_mb);

            match report.summary.recommendation {
                ReportRecommendation::Pass => {
                    println!("  ✅ All benchmarks passed!");
                }
                ReportRecommendation::Warning { ref issues } => {
                    println!("  ⚠️  Warnings: {}", issues.join(", "));
                }
                ReportRecommendation::Fail {
                    ref critical_issues,
                } => {
                    println!("  ❌ Critical issues: {}", critical_issues.join(", "));
                }
            }

            report
        })
    });
}

/// Execute the basic agent tool loop (from development plan)
async fn execute_agent_tool_loop() -> Result<(), Box<dyn std::error::Error>> {
    let agent = BenchmarkAgent::new();
    let registry = create_benchmark_registry();
    let mut coordinator = Coordinator::new(agent, registry);

    // Simulate the tool loop: HTTP GET → JSON parse → Text transform → File write
    std::hint::black_box(coordinator.step("benchmark input with tools".to_string()));
    Ok(())
}

/// Execute basic agent loop without tools
async fn execute_basic_agent_loop() -> Result<(), Box<dyn std::error::Error>> {
    let agent = BenchmarkAgent::new();
    let registry = MockToolRegistry::new();
    let mut coordinator = Coordinator::new(agent, registry);

    std::hint::black_box(coordinator.step("simple benchmark input".to_string()));
    Ok(())
}

/// Execute agent with tools scenario
async fn execute_agent_with_tools() -> Result<(), Box<dyn std::error::Error>> {
    let agent = BenchmarkAgent::new_with_tools();
    let registry = create_complex_registry();
    let mut coordinator = Coordinator::new(agent, registry);

    std::hint::black_box(coordinator.step("complex tool scenario".to_string()));
    Ok(())
}

/// Benchmark agent for testing
struct BenchmarkAgent {
    memory: InMemoryMemory,
    last_input: Option<String>,
    call_tools: bool,
}

impl BenchmarkAgent {
    fn new() -> Self {
        Self {
            memory: InMemoryMemory::new(),
            last_input: None,
            call_tools: false,
        }
    }

    fn new_with_tools() -> Self {
        Self {
            memory: InMemoryMemory::new(),
            last_input: None,
            call_tools: true,
        }
    }
}

impl Agent for BenchmarkAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        self.last_input = Some(input.clone());
        if let Ok(update) = MemoryUpdate::new("last_input", &input) {
            let _ = self.memory.store(update);
        }
    }

    fn act(&mut self) -> Self::Action {
        if let Some(ref input) = self.last_input {
            format!("Processed: {}", input)
        } else {
            "No input received".to_string()
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if self.call_tools {
            vec![
                ToolCall::new("benchmark_tool", "test data").unwrap(),
                ToolCall::new("json_tool", r#"{"test": "data"}"#).unwrap(),
            ]
        } else {
            Vec::new()
        }
    }

    fn handle_result(&mut self, _result: ExecutionResult) {
        // Handle tool execution result
    }

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

/// Create a benchmark tool registry
fn create_benchmark_registry() -> MockToolRegistry {
    MockToolRegistry::new()
        .with_tool(MockTool::new("http_tool").with_default_response("HTTP response"))
        .with_tool(MockTool::new("json_tool").with_default_response(r#"{"result": "success"}"#))
        .with_tool(MockTool::new("text_tool").with_default_response("Text result"))
        .with_tool(MockTool::new("file_tool").with_default_response("File content"))
}

/// Create a complex tool registry for testing
fn create_complex_registry() -> MockToolRegistry {
    create_benchmark_registry()
        .with_tool(MockTool::new("slow_tool").with_default_response("Slow response"))
}

fn production_config() -> Criterion {
    let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();

    if is_ci {
        Criterion::default()
            .sample_size(10)
            .measurement_time(Duration::from_secs(5))
            .warm_up_time(Duration::from_secs(1))
    } else {
        Criterion::default()
            .sample_size(50)
            .measurement_time(Duration::from_secs(30))
            .warm_up_time(Duration::from_secs(5))
    }
}

criterion_group! {
    name = production_benchmarks;
    config = production_config();
    targets = production_benchmark, enhanced_production_benchmark
}

criterion_main!(production_benchmarks);
