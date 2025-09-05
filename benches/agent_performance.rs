//! Agent Performance Benchmarks
//!
//! Benchmarks the core agent execution loop according to the development plan:
//! - Target: p50 < 30ms, p95 < 200ms, p99 < 400ms
//! - Memory: RSS ≤ 128MB with N=32 concurrent sessions
//! - Tool loop scenario: HTTP GET → JSON parse → Text transform → File write

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use skreaver::{
    Agent, ExecutionResult, InMemoryMemory, MemoryReader, MemoryUpdate, MemoryWriter, ToolCall,
};
use skreaver_http::runtime::Coordinator;
use skreaver_testing::{MockTool, MockToolRegistry};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark agent for performance testing
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
        match &self.last_input {
            Some(input) => format!("Processed: {}", input),
            None => "No input".to_string(),
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let (true, Some(input)) = (self.call_tools, &self.last_input) {
            return vec![
                ToolCall::new("http_get", "http://localhost/test").unwrap(),
                ToolCall::new("json_parse", input).unwrap(),
                ToolCall::new("text_transform", input).unwrap(),
                ToolCall::new("file_write", input).unwrap(),
            ];
        }
        Vec::new()
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        let message = format!("Tool result: {}", result.output());
        if let Ok(update) = MemoryUpdate::new("last_tool_result", &message) {
            let _ = self.memory.store(update);
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let _ = self.memory.store(update);
    }

    fn memory_reader(&self) -> &dyn MemoryReader {
        &self.memory
    }

    fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
        &mut self.memory
    }
}

/// Create a mock tool registry for benchmarks
fn create_benchmark_registry() -> MockToolRegistry {
    MockToolRegistry::new()
        .with_tool(
            MockTool::new("http_get")
                .with_default_response(r#"{"status": "ok", "data": "mock response"}"#),
        )
        .with_tool(
            MockTool::new("json_parse").with_default_response(r#"{"parsed": true, "valid": true}"#),
        )
        .with_tool(MockTool::new("text_transform").with_default_response("TRANSFORMED_TEXT"))
        .with_tool(MockTool::new("file_write").with_default_response("file_written"))
}

/// Benchmark single agent step (observe -> act)
fn bench_single_agent_step(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("agent_single_step");

    group.throughput(Throughput::Elements(1));
    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("simple_agent", |b| {
        b.to_async(&rt).iter(|| async {
            let agent = BenchmarkAgent::new();
            let registry = MockToolRegistry::new();
            let mut coordinator = Coordinator::new(agent, registry);

            std::hint::black_box(
                coordinator.step(std::hint::black_box("benchmark input".to_string())),
            )
        })
    });

    group.finish();
}

/// Benchmark agent with tool execution (full loop)
fn bench_agent_tool_loop(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("agent_tool_loop");

    group.throughput(Throughput::Elements(1));
    group.sample_size(500);
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("with_4_tools", |b| {
        b.to_async(&rt).iter(|| async {
            let agent = BenchmarkAgent::new_with_tools();
            let registry = create_benchmark_registry();
            let mut coordinator = Coordinator::new(agent, registry);

            std::hint::black_box(coordinator.step(std::hint::black_box(
                "benchmark input with tools".to_string(),
            )))
        })
    });

    group.finish();
}

/// Benchmark concurrent agent sessions
fn bench_concurrent_sessions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_sessions");

    group.sample_size(100);
    group.measurement_time(Duration::from_secs(20));

    for concurrent_count in [1, 4, 8, 16, 32].iter() {
        group.throughput(Throughput::Elements(*concurrent_count as u64));

        group.bench_with_input(
            BenchmarkId::new("concurrent_agents", concurrent_count),
            concurrent_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async move {
                    let mut handles = Vec::with_capacity(count);

                    for i in 0..count {
                        let handle = tokio::spawn(async move {
                            let agent = BenchmarkAgent::new_with_tools();
                            let registry = create_benchmark_registry();
                            let mut coordinator = Coordinator::new(agent, registry);

                            coordinator.step(format!("concurrent input {}", i))
                        });
                        handles.push(handle);
                    }

                    // Wait for all tasks to complete
                    for handle in handles {
                        std::hint::black_box(handle.await.unwrap());
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark memory operations under load
fn bench_memory_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_operations");

    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("memory_store_load", |b| {
        b.to_async(&rt).iter(|| async {
            let mut agent = BenchmarkAgent::new();

            // Simulate memory operations during agent execution
            for i in 0..10 {
                let key = format!("bench_key_{}", i);
                let value = format!("bench_value_{}", i);

                if let Ok(update) = MemoryUpdate::new(&key, &value) {
                    let _ = std::hint::black_box(agent.memory.store(update));
                }
            }

            // Read operations
            for i in 0..10 {
                let key = format!("bench_key_{}", i);
                if let Ok(key_obj) = skreaver_core::MemoryKey::new(&key) {
                    let _ = std::hint::black_box(agent.memory.load(&key_obj));
                }
            }
        })
    });

    group.finish();
}

/// Benchmark throughput over time (sustained load)
fn bench_sustained_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("sustained_throughput");

    group.sample_size(50);
    group.measurement_time(Duration::from_secs(30));

    group.bench_function("60_second_load", |b| {
        b.to_async(&rt).iter(|| async {
            let agent = BenchmarkAgent::new_with_tools();
            let registry = create_benchmark_registry();
            let mut coordinator = Coordinator::new(agent, registry);

            let start = std::time::Instant::now();
            let mut operations = 0;

            // Run for approximately 1 second in benchmark context
            while start.elapsed() < Duration::from_millis(100) {
                std::hint::black_box(coordinator.step(format!("sustained input {}", operations)));
                operations += 1;
            }

            operations
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_agent_step,
    bench_agent_tool_loop,
    bench_concurrent_sessions,
    bench_memory_operations,
    bench_sustained_throughput
);
criterion_main!(benches);
