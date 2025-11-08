//! Tool Execution Performance Benchmarks
//!
//! Benchmarks individual tool execution performance and tool registry operations.
//! Focuses on the overhead of tool dispatch and execution.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use skreaver_core::ToolId as ToolName;
use skreaver_core::{Tool, ToolCall};
use skreaver_testing::{MockTool, MockToolRegistry};
use skreaver_tools::ToolRegistry;
use std::time::Duration;

/// Benchmark individual tool execution
fn bench_tool_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_execution");

    group.throughput(Throughput::Elements(1));
    group.sample_size(2000);
    group.measurement_time(Duration::from_secs(10));

    // Benchmark different tool types
    let tools = vec![
        (
            "fast_tool",
            MockTool::new("fast_tool").with_default_response("fast"),
        ),
        (
            "json_tool",
            MockTool::new("json_tool").with_default_response(r#"{"result": "parsed"}"#),
        ),
        (
            "text_tool",
            MockTool::new("text_tool").with_default_response("TRANSFORMED"),
        ),
        (
            "large_response_tool",
            MockTool::new("large_tool").with_default_response("x".repeat(10000)),
        ),
    ];

    for (name, tool) in tools {
        group.bench_with_input(BenchmarkId::new("direct_call", name), &tool, |b, tool| {
            b.iter(|| {
                std::hint::black_box(tool.call(std::hint::black_box("benchmark input".to_string())))
            })
        });
    }

    group.finish();
}

/// Benchmark tool registry lookup and dispatch
fn bench_tool_registry(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_registry");

    group.throughput(Throughput::Elements(1));
    group.sample_size(1500);
    group.measurement_time(Duration::from_secs(10));

    // Create registries with different sizes
    let small_registry = MockToolRegistry::new()
        .with_tool(MockTool::new("tool1").with_default_response("response1"))
        .with_tool(MockTool::new("tool2").with_default_response("response2"))
        .with_tool(MockTool::new("tool3").with_default_response("response3"));

    let medium_registry = (0..20).fold(MockToolRegistry::new(), |registry, i| {
        registry.with_tool(
            MockTool::new(format!("tool{}", i)).with_default_response(format!("response{}", i)),
        )
    });

    let large_registry = (0..100).fold(MockToolRegistry::new(), |registry, i| {
        registry.with_tool(
            MockTool::new(format!("tool{}", i)).with_default_response(format!("response{}", i)),
        )
    });

    let registries = vec![
        ("small_3_tools", small_registry),
        ("medium_20_tools", medium_registry),
        ("large_100_tools", large_registry),
    ];

    for (name, registry) in registries {
        group.bench_with_input(
            BenchmarkId::new("registry_execute", name),
            &registry,
            |b, registry| {
                b.iter(|| {
                    let tool_call = ToolCall::new("tool1", "benchmark input").unwrap();
                    std::hint::black_box(
                        registry.dispatch(std::hint::black_box(tool_call)).unwrap(),
                    )
                })
            },
        );
    }

    group.finish();
}

/// Benchmark multiple tool calls (batch execution)
fn bench_batch_tool_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_tool_execution");

    group.sample_size(500);
    group.measurement_time(Duration::from_secs(15));

    let registry = MockToolRegistry::new()
        .with_tool(MockTool::new("http_get").with_default_response(r#"{"data": "response"}"#))
        .with_tool(MockTool::new("json_parse").with_default_response(r#"{"parsed": true}"#))
        .with_tool(MockTool::new("text_transform").with_default_response("TRANSFORMED"))
        .with_tool(MockTool::new("file_write").with_default_response("written"));

    for batch_size in [1, 2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_execute", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    for i in 0..size {
                        let tool_name = match i % 4 {
                            0 => "http_get",
                            1 => "json_parse",
                            2 => "text_transform",
                            _ => "file_write",
                        };

                        let tool_call =
                            ToolCall::new(tool_name, &format!("batch input {}", i)).unwrap();

                        std::hint::black_box(
                            registry.dispatch(std::hint::black_box(tool_call)).unwrap(),
                        );
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark tool error handling overhead
fn bench_tool_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_error_handling");

    group.throughput(Throughput::Elements(1));
    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(10));

    let registry = MockToolRegistry::new()
        .with_tool(MockTool::new("success_tool").with_default_response("success"))
        .with_tool(MockTool::new("failure_tool").with_failure("input", "intentional failure"));

    group.bench_function("successful_execution", |b| {
        b.iter(|| {
            let tool_call = ToolCall::new("success_tool", "test input").unwrap();
            std::hint::black_box(registry.dispatch(std::hint::black_box(tool_call)).unwrap())
        })
    });

    group.bench_function("failed_execution", |b| {
        b.iter(|| {
            let tool_call = ToolCall::new("failure_tool", "input").unwrap();
            std::hint::black_box(registry.dispatch(std::hint::black_box(tool_call)).unwrap())
        })
    });

    group.bench_function("nonexistent_tool", |b| {
        b.iter(|| {
            let tool_call = ToolCall::new("nonexistent", "test input").unwrap();
            std::hint::black_box(registry.dispatch(std::hint::black_box(tool_call)).unwrap())
        })
    });

    group.finish();
}

/// Benchmark tool name parsing and validation
fn bench_tool_name_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_name_operations");

    group.throughput(Throughput::Elements(1));
    group.sample_size(3000);
    group.measurement_time(Duration::from_secs(5));

    let valid_names = vec![
        "tool1",
        "http_get",
        "json_parse",
        "text_transform",
        "file_write",
    ];
    let invalid_names = vec![
        "",
        "tool-with-hyphens",
        "tool with spaces",
        "TOOL",
        "123tool",
    ];

    group.bench_function("valid_tool_names", |b| {
        b.iter(|| {
            for name in &valid_names {
                let _ = std::hint::black_box(ToolName::parse(std::hint::black_box(name)));
            }
        })
    });

    group.bench_function("invalid_tool_names", |b| {
        b.iter(|| {
            for name in &invalid_names {
                let _ = std::hint::black_box(ToolName::parse(std::hint::black_box(name)));
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tool_execution,
    bench_tool_registry,
    bench_batch_tool_execution,
    bench_tool_error_handling,
    bench_tool_name_operations
);
criterion_main!(benches);
