//! # Testing Framework Example
//!
//! This example demonstrates how to use Skreaver's comprehensive testing framework
//! for testing agents, tools, and runtime performance.

use skreaver::{
    Agent, ExecutionResult, InMemoryMemory, MemoryReader, MemoryUpdate, MemoryWriter, Tool,
    ToolCall,
    testing::{
        AgentTestHarness, BenchmarkRunner, IntegrationTest, MockTool, MockToolRegistry,
        PerformanceTest, TestHarnessBuilder, TestRunner, TestScenario,
    },
};
use std::time::Duration;

/// Example agent for testing demonstrations
struct TestDemoAgent {
    memory: InMemoryMemory,
    last_input: Option<String>,
    tool_responses: Vec<String>,
}

impl TestDemoAgent {
    fn new() -> Self {
        Self {
            memory: InMemoryMemory::new(),
            last_input: None,
            tool_responses: Vec::new(),
        }
    }
}

impl Agent for TestDemoAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        println!("🔍 Agent observed: {}", input);
        self.last_input = Some(input.clone());
        if let Ok(update) = MemoryUpdate::new("last_input", &input) {
            let _ = self.memory.store(update);
        }
    }

    fn act(&mut self) -> Self::Action {
        let response = match self.last_input.as_deref() {
            Some(input) if input.starts_with("greet:") => {
                let name = input.strip_prefix("greet:").unwrap_or("World");
                format!("Hello, {}!", name)
            }
            Some(input) if input.starts_with("calculate:") => {
                let expr = input.strip_prefix("calculate:").unwrap_or("0");
                format!("Result: {}", expr) // Simplified calculation
            }
            Some(input) if input.starts_with("error") => {
                "This is an intentional error response".to_string()
            }
            Some(input) => format!("Processed: {}", input),
            None => "No input received".to_string(),
        };

        println!("💭 Agent responding: {}", response);
        response
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let Some(input) = &self.last_input {
            if input.starts_with("tool:") {
                let tool_input = input.strip_prefix("tool:").unwrap_or("");
                return vec![ToolCall::new("demo_tool", tool_input).unwrap()];
            }

            if input == "multi_tools" {
                return vec![
                    ToolCall::new("tool1", "first").unwrap(),
                    ToolCall::new("tool2", "second").unwrap(),
                ];
            }
        }
        Vec::new()
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        let message = if result.is_success() {
            format!("✅ Tool succeeded: {}", result.output())
        } else {
            format!("❌ Tool failed: {}", result.output())
        };

        println!("{}", message);
        self.tool_responses.push(message.clone());

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Skreaver Testing Framework Demo");
    println!("==================================");

    // 1. Demonstrate Mock Tools
    println!("\n1️⃣ Mock Tools Demo");
    demo_mock_tools();

    // 2. Demonstrate Agent Test Harness
    println!("\n2️⃣ Agent Test Harness Demo");
    demo_agent_testing();

    // 3. Demonstrate Performance Benchmarks
    println!("\n3️⃣ Performance Benchmarks Demo");
    demo_performance_testing();

    // 4. Demonstrate Integration Testing
    println!("\n4️⃣ Integration Testing Demo");
    demo_integration_testing();

    // 5. Demonstrate Test Runner
    println!("\n5️⃣ Test Runner Demo");
    demo_test_runner();

    Ok(())
}

/// Demonstrate mock tools functionality
fn demo_mock_tools() {
    println!("Creating mock tools with different behaviors...");

    // Create a mock tool with specific responses
    let mock_tool = MockTool::new("demo_tool")
        .with_response("hello", "Hello response")
        .with_response("error", "Error occurred")
        .with_failure("fail", "Mock failure")
        .with_default_response("Default response");

    // Test the mock tool
    let result1 = mock_tool.call("hello".to_string());
    println!(
        "  Input 'hello': {} ({})",
        result1.output(),
        if result1.is_success() { "✅" } else { "❌" }
    );

    let result2 = mock_tool.call("fail".to_string());
    println!(
        "  Input 'fail': {} ({})",
        result2.output(),
        if result2.is_success() { "✅" } else { "❌" }
    );

    let result3 = mock_tool.call("unknown".to_string());
    println!(
        "  Input 'unknown': {} ({})",
        result3.output(),
        if result3.is_success() { "✅" } else { "❌" }
    );

    println!("  Call count: {}", mock_tool.call_count());
    println!(
        "  Was called with 'hello': {}",
        mock_tool.was_called_with("hello")
    );
}

/// Demonstrate agent test harness
fn demo_agent_testing() {
    println!("Setting up agent test harness...");

    let agent = TestDemoAgent::new();
    let registry = MockToolRegistry::new()
        .with_tool(MockTool::new("demo_tool").with_default_response("Mock tool response"))
        .with_tool(MockTool::new("tool1").with_default_response("Tool 1 response"))
        .with_tool(MockTool::new("tool2").with_default_response("Tool 2 response"));

    let mut harness = AgentTestHarness::new(agent, registry);

    // Create test scenarios
    let scenarios = vec![
        TestScenario::simple_observation("greet:Alice"),
        TestScenario::named("calculation_test", "calculate:2+2"),
        TestScenario::named("tool_test", "tool:test_input")
            .expect_actions(vec!["Mock tool response".to_string()]),
        TestScenario::named("error_test", "error_input").should_fail(),
    ];

    println!("Running test scenarios...");
    let results = harness.run_scenarios(scenarios);

    for result in results {
        println!("  {}", result);
    }
}

/// Demonstrate performance benchmarks
fn demo_performance_testing() {
    println!("Running performance benchmarks...");

    let mut runner = BenchmarkRunner::new();

    // Benchmark mock tools
    let fast_tool = MockTool::new("fast_tool").with_default_response("fast");
    runner.benchmark_tool("fast_tool_performance", &fast_tool, "test", 1000);

    let slow_tool = MockTool::new("slow_tool").with_default_response("slow"); // In reality, this might have artificial delays
    runner.benchmark_tool("slow_tool_performance", &slow_tool, "test", 100);

    // Benchmark throughput
    let mut counter = 0;
    runner.benchmark_throughput(
        "simple_operation",
        || {
            counter += 1;
            // Simulate some work
            for _ in 0..10 {
                std::hint::black_box(counter * 2);
            }
        },
        Duration::from_millis(100),
    );

    println!("Benchmark results:");
    for result in runner.results() {
        println!("  {}", result.summary());
        println!("    Performance grade: {}", result.performance_grade());
    }
}

/// Demonstrate integration testing capabilities
fn demo_integration_testing() {
    println!("Demonstrating integration testing concepts...");

    // This would normally test the HTTP runtime, but for the demo
    // we'll show the test structure
    println!("  Standard HTTP tests would include:");
    for test_name in IntegrationTest::standard_http_tests() {
        println!("    - {}", test_name);
    }

    // Simulate running a load test
    println!("  Simulating load test results...");
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let load_result = IntegrationTest::run_load_test(
            "http://localhost:3000",
            "/health",
            10,  // concurrent requests
            100, // total requests
        )
        .await;

        load_result.print_summary();
    });
}

/// Demonstrate test runner capabilities
fn demo_test_runner() {
    println!("Using test runner for comprehensive testing...");

    let agent = TestDemoAgent::new();
    let registry = MockToolRegistry::new()
        .with_echo_tool()
        .with_success_tool("success_tool")
        .with_failure_tool("failure_tool");

    let mut harness = AgentTestHarness::new(agent, registry);
    let mut runner = TestRunner::new();

    // Create comprehensive test suite
    let test_suite = vec![
        TestScenario::simple_observation("Hello, World!"),
        TestScenario::named("greeting_test", "greet:Bob"),
        TestScenario::named("calculation_test", "calculate:5*3"),
        TestScenario::named("empty_input", ""),
        TestScenario::named("long_input", "x".repeat(1000)),
    ];

    runner.run_all_tests(&mut harness, test_suite);
    runner.print_results();

    let summary = runner.summary();
    println!("\nTest execution completed:");
    println!(
        "  Success rate: {:.1}%",
        (summary.passed as f64 / summary.total as f64) * 100.0
    );
}

/// Demonstrate the builder pattern for test harnesses
#[allow(dead_code)]
fn demo_test_builder() {
    println!("Demonstrating test harness builder pattern...");

    let agent = TestDemoAgent::new();

    let _harness = TestHarnessBuilder::new()
        .with_mock_tools()
        .build_with_agent(agent);

    println!("  Test harness created with builder pattern");
}

/// Demonstrate comprehensive performance testing
#[allow(dead_code)]
fn demo_comprehensive_benchmarks() {
    println!("Running comprehensive performance test suite...");
    PerformanceTest::run_full_benchmark_suite();
}
