//! # A2A Server Example
//!
//! This example demonstrates how to create an A2A protocol server that exposes
//! AI agents via HTTP endpoints. Other systems can discover and interact with
//! these agents using the standard A2A protocol.
//!
//! ## A2A Protocol Overview
//!
//! The Agent2Agent (A2A) protocol is Google's standard for agent interoperability:
//! - **Agent Card**: JSON at `/.well-known/agent.json` describing capabilities
//! - **Tasks**: Units of work with lifecycle states (Working, InputRequired, Completed, etc.)
//! - **Messages**: Communications between agents with support for text, data, and files
//! - **Streaming**: Real-time updates via Server-Sent Events (SSE)
//!
//! ## Running
//!
//! ```bash
//! cargo run --example a2a_server
//! ```
//!
//! ## Testing
//!
//! ```bash
//! # Discover the agent
//! curl http://localhost:3001/.well-known/agent.json | jq
//!
//! # Send a message
//! curl -X POST http://localhost:3001/tasks/send \
//!   -H "Content-Type: application/json" \
//!   -d '{"message": {"role": "user", "parts": [{"type": "text", "text": "Hello!"}]}}'
//!
//! # Send with streaming
//! curl -N -X POST http://localhost:3001/tasks/sendSubscribe \
//!   -H "Content-Type: application/json" \
//!   -d '{"message": {"role": "user", "parts": [{"type": "text", "text": "Count to 5"}]}}'
//! ```

use async_trait::async_trait;
use skreaver_a2a::{
    A2aServer, AgentCard, AgentHandler, AgentSkill, Artifact, Message, Task, TaskStatus,
    send_artifact_update, send_status_update,
};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

// =============================================================================
// Example Agent: Calculator
// =============================================================================

/// A simple calculator agent that can perform basic math operations.
struct CalculatorAgent;

#[async_trait]
impl AgentHandler for CalculatorAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new(
            "calculator-agent",
            "Calculator Agent",
            "http://localhost:3001",
        )
        .with_description("A simple calculator that performs basic math operations")
        .with_skill(
            AgentSkill::new("calculate", "Calculate")
                .with_description("Evaluate mathematical expressions like '2 + 2' or '10 * 5'"),
        )
        .with_skill(
            AgentSkill::new("help", "Help")
                .with_description("Get help on how to use the calculator"),
        )
    }

    async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String> {
        let input = message
            .parts
            .first()
            .and_then(|p| p.as_text())
            .unwrap_or("")
            .trim()
            .to_lowercase();

        // Handle help requests
        if input.contains("help") {
            task.add_message(Message::agent(
                "I'm a calculator agent! I can perform basic math:\n\
                 - Addition: '2 + 2'\n\
                 - Subtraction: '10 - 3'\n\
                 - Multiplication: '5 * 4'\n\
                 - Division: '20 / 4'\n\n\
                 Just send me an expression and I'll calculate it!",
            ));
            task.set_status(TaskStatus::Completed);
            return Ok(());
        }

        // Try to evaluate the expression
        match evaluate_expression(&input) {
            Some(result) => {
                task.add_message(Message::agent(format!("Result: {} = {}", input, result)));
                task.set_status(TaskStatus::Completed);
            }
            None => {
                task.add_message(Message::agent(format!(
                    "I couldn't understand '{}'. Please use format like '2 + 2' or '10 * 5'.\n\
                     Send 'help' for more information.",
                    input
                )));
                task.set_status(TaskStatus::InputRequired);
            }
        }

        Ok(())
    }
}

/// Simple expression evaluator for basic math
fn evaluate_expression(expr: &str) -> Option<f64> {
    let expr = expr.replace(' ', "");

    // Try addition
    if let Some(pos) = expr.find('+') {
        let (a, b) = expr.split_at(pos);
        let a: f64 = a.parse().ok()?;
        let b: f64 = b[1..].parse().ok()?;
        return Some(a + b);
    }

    // Try subtraction (handle negative numbers)
    if let Some(pos) = expr[1..].find('-') {
        let (a, b) = expr.split_at(pos + 1);
        let a: f64 = a.parse().ok()?;
        let b: f64 = b[1..].parse().ok()?;
        return Some(a - b);
    }

    // Try multiplication
    if let Some(pos) = expr.find('*') {
        let (a, b) = expr.split_at(pos);
        let a: f64 = a.parse().ok()?;
        let b: f64 = b[1..].parse().ok()?;
        return Some(a * b);
    }

    // Try division
    if let Some(pos) = expr.find('/') {
        let (a, b) = expr.split_at(pos);
        let a: f64 = a.parse().ok()?;
        let b: f64 = b[1..].parse().ok()?;
        if b == 0.0 {
            return None;
        }
        return Some(a / b);
    }

    None
}

// =============================================================================
// Example Agent: Streaming Counter
// =============================================================================

/// An agent that demonstrates streaming responses by counting.
struct StreamingCounterAgent;

#[async_trait]
impl AgentHandler for StreamingCounterAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new(
            "counter-agent",
            "Streaming Counter Agent",
            "http://localhost:3001",
        )
        .with_description("An agent that counts with streaming updates")
        .with_streaming()
        .with_skill(
            AgentSkill::new("count", "Count")
                .with_description("Count from 1 to N with streaming updates"),
        )
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String> {
        let input = message
            .parts
            .first()
            .and_then(|p| p.as_text())
            .unwrap_or("5");

        // Parse the count target
        let target: i32 = input
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(5)
            .min(20); // Cap at 20 to prevent abuse

        task.add_message(Message::agent(format!("Counting to {}...", target)));

        // In non-streaming mode, just return the final result
        let numbers: Vec<String> = (1..=target).map(|i| i.to_string()).collect();
        task.add_message(Message::agent(format!("Count: {}", numbers.join(", "))));
        task.set_status(TaskStatus::Completed);

        Ok(())
    }

    async fn handle_message_streaming(
        &self,
        task: &mut Task,
        message: Message,
        event_tx: broadcast::Sender<skreaver_a2a::StreamingEvent>,
    ) -> Result<(), String> {
        let input = message
            .parts
            .first()
            .and_then(|p| p.as_text())
            .unwrap_or("5");

        // Parse the count target
        let target: i32 = input
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(5)
            .min(20);

        task.add_message(Message::agent(format!("Counting to {}...", target)));
        send_status_update(
            &event_tx,
            &task.id,
            TaskStatus::Working,
            Some(Message::agent(format!("Starting count to {}", target))),
        );

        // Stream each number as an artifact
        for i in 1..=target {
            tokio::time::sleep(Duration::from_millis(300)).await;

            let artifact = Artifact::new(format!("count-{}", i))
                .with_label("Count Progress")
                .with_part(skreaver_a2a::Part::text(format!("{}", i)));

            send_artifact_update(&event_tx, &task.id, artifact.clone(), i == target);
            task.artifacts.push(artifact);
        }

        task.add_message(Message::agent(format!("Finished counting to {}!", target)));
        task.set_status(TaskStatus::Completed);

        Ok(())
    }
}

// =============================================================================
// Example Agent: Echo with Metadata
// =============================================================================

/// An agent that echoes messages and demonstrates metadata handling.
struct EchoAgent;

#[async_trait]
impl AgentHandler for EchoAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new("echo-agent", "Echo Agent", "http://localhost:3001")
            .with_description("An agent that echoes messages back with metadata")
            .with_skill(AgentSkill::new("echo", "Echo").with_description("Echo the input message"))
    }

    async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String> {
        let text = message
            .parts
            .first()
            .and_then(|p| p.as_text())
            .unwrap_or("(empty)");

        // Echo with metadata
        let mut response = Message::agent(format!("Echo: {}", text));
        response
            .metadata
            .insert("original_length".to_string(), serde_json::json!(text.len()));
        response.metadata.insert(
            "echo_timestamp".to_string(),
            serde_json::json!(chrono::Utc::now().to_rfc3339()),
        );

        task.add_message(response);
        task.set_status(TaskStatus::Completed);

        Ok(())
    }
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry().with(fmt::layer()).init();

    println!("==============================================");
    println!("       A2A Server Example - Skreaver");
    println!("==============================================");
    println!();

    // You can choose which agent to expose
    let agent_type = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "calculator".to_string());

    match agent_type.as_str() {
        "calculator" => {
            println!("Starting Calculator Agent...");
            let server = A2aServer::new(CalculatorAgent);
            print_endpoints("calculator-agent", "Calculator Agent");
            server.serve("0.0.0.0:3001").await?;
        }
        "counter" => {
            println!("Starting Streaming Counter Agent...");
            let server = A2aServer::new(StreamingCounterAgent);
            print_endpoints("counter-agent", "Streaming Counter Agent");
            server.serve("0.0.0.0:3001").await?;
        }
        "echo" => {
            println!("Starting Echo Agent...");
            let server = A2aServer::new(EchoAgent);
            print_endpoints("echo-agent", "Echo Agent");
            server.serve("0.0.0.0:3001").await?;
        }
        _ => {
            println!("Unknown agent type: {}", agent_type);
            println!("Available: calculator, counter, echo");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_endpoints(agent_id: &str, name: &str) {
    println!();
    println!("Agent: {} ({})", name, agent_id);
    println!();
    println!("Server listening on http://0.0.0.0:3001");
    println!();
    println!("A2A Endpoints:");
    println!("  GET  /.well-known/agent.json     - Agent discovery");
    println!("  POST /tasks/send                 - Send message");
    println!("  POST /tasks/sendSubscribe        - Send with streaming");
    println!("  GET  /tasks/{{task_id}}            - Get task status");
    println!("  POST /tasks/{{task_id}}/cancel     - Cancel task");
    println!("  GET  /tasks/{{task_id}}/subscribe  - Subscribe to updates");
    println!();
    println!("Example requests:");
    println!();
    println!("  # Discover the agent");
    println!("  curl http://localhost:3001/.well-known/agent.json | jq");
    println!();
    println!("  # Send a message");
    println!(r#"  curl -X POST http://localhost:3001/tasks/send \"#);
    println!(r#"    -H "Content-Type: application/json" \"#);
    println!(
        r#"    -d '{{"message": {{"role": "user", "parts": [{{"type": "text", "text": "2 + 2"}}]}}}}'"#
    );
    println!();
    println!("Press Ctrl+C to stop the server");
    println!();
}
