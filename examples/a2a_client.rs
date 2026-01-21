//! # A2A Client Example
//!
//! This example demonstrates how to use the A2A client to connect to and
//! interact with A2A-compatible agents. It shows agent discovery, message
//! sending, multi-turn conversations, and streaming support.
//!
//! ## Prerequisites
//!
//! Start an A2A server first (in another terminal):
//! ```bash
//! cargo run --example a2a_server calculator
//! ```
//!
//! ## Running
//!
//! ```bash
//! # Basic usage (connects to localhost:3001)
//! cargo run --example a2a_client
//!
//! # Connect to a custom URL
//! cargo run --example a2a_client -- --url http://my-agent.example.com
//!
//! # Use streaming mode
//! cargo run --example a2a_client -- --streaming
//! ```
//!
//! ## Features Demonstrated
//!
//! - Agent discovery via `/.well-known/agent.json`
//! - Sending messages and receiving responses
//! - Multi-turn conversations (continuing tasks)
//! - Streaming responses via Server-Sent Events
//! - Task status polling

use futures::StreamExt;
use skreaver_a2a::{A2aClient, Role, StreamingEvent, TaskStatus};
use std::io::{self, Write};
use std::time::Duration;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry().with(fmt::layer()).init();

    println!("==============================================");
    println!("       A2A Client Example - Skreaver");
    println!("==============================================");
    println!();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let url = args
        .iter()
        .position(|a| a == "--url")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("http://localhost:3001");

    let use_streaming = args.iter().any(|a| a == "--streaming");

    // Create the A2A client
    let mut client = A2aClient::new(url)?;
    println!("Connecting to: {}", url);
    println!();

    // Discover the agent
    println!("Discovering agent...");
    match client.discover().await {
        Ok(card) => {
            println!("Connected to: {} ({})", card.name, card.agent_id);
            if let Some(desc) = &card.description {
                println!("Description: {}", desc);
            }
            println!("Skills:");
            for skill in &card.skills {
                println!("  - {} ({})", skill.name, skill.id);
                if let Some(desc) = &skill.description {
                    println!("    {}", desc);
                }
            }
            println!();
        }
        Err(e) => {
            println!("Failed to discover agent: {}", e);
            println!();
            println!("Make sure an A2A server is running:");
            println!("  cargo run --example a2a_server");
            return Ok(());
        }
    }

    if use_streaming {
        run_streaming_demo(&client).await?;
    } else {
        run_interactive_demo(&client).await?;
    }

    Ok(())
}

/// Run an interactive demo where the user can chat with the agent
async fn run_interactive_demo(client: &A2aClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("Interactive mode - type your messages (or 'quit' to exit)");
    println!("Commands: help, quit");
    println!();

    let mut current_task_id: Option<String> = None;

    loop {
        // Prompt
        print!("> ");
        io::stdout().flush()?;

        // Read input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input.eq_ignore_ascii_case("quit") || input.eq_ignore_ascii_case("exit") {
            println!("Goodbye!");
            break;
        }

        if input.eq_ignore_ascii_case("new") {
            current_task_id = None;
            println!("Starting new conversation");
            continue;
        }

        // Send the message
        let task = if let Some(task_id) = &current_task_id {
            // Continue existing task
            client.continue_task(task_id, input).await
        } else {
            // New task
            client.send_message(input).await
        };

        match task {
            Ok(task) => {
                // Update current task ID
                current_task_id = Some(task.id.clone());

                // Print response
                println!();
                println!("Task: {} (Status: {})", task.id, task.status);

                // Print agent messages
                for msg in task.messages.iter().filter(|m| m.role == Role::Agent) {
                    for part in &msg.parts {
                        if let Some(text) = part.as_text() {
                            println!("Agent: {}", text);
                        }
                    }
                }

                // Print artifacts if any
                for artifact in &task.artifacts {
                    println!("Artifact [{}]: {:?}", artifact.id, artifact.label);
                    for part in &artifact.parts {
                        if let Some(text) = part.as_text() {
                            println!("  {}", text);
                        }
                    }
                }

                // Check if task requires more input
                if task.status == TaskStatus::InputRequired {
                    println!("(Agent needs more input)");
                } else if task.is_terminal() {
                    // Start fresh for next message
                    current_task_id = None;
                }
                println!();
            }
            Err(e) => {
                println!("Error: {}", e);
                println!();
            }
        }
    }

    Ok(())
}

/// Run a streaming demo that shows real-time updates
async fn run_streaming_demo(client: &A2aClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("Streaming mode - sending a request and showing real-time updates");
    println!();

    // Send a message that will trigger streaming (if the server supports it)
    let message = "Count to 5";
    println!("Sending: {}", message);
    println!();

    let mut stream = client.send_message_streaming(message).await?;

    println!("Receiving streaming updates:");
    println!("----------------------------");

    while let Some(result) = stream.next().await {
        match result {
            Ok(event) => match event {
                StreamingEvent::TaskStatusUpdate(update) => {
                    println!(
                        "[Status] Task {}: {:?}",
                        &update.task_id[..8.min(update.task_id.len())],
                        update.status
                    );
                    if let Some(msg) = &update.message {
                        for part in &msg.parts {
                            if let Some(text) = part.as_text() {
                                println!("         Message: {}", text);
                            }
                        }
                    }
                }
                StreamingEvent::TaskArtifactUpdate(update) => {
                    println!(
                        "[Artifact] {}: {}",
                        update.artifact.id,
                        update.artifact.label.as_deref().unwrap_or("unnamed")
                    );
                    for part in &update.artifact.parts {
                        if let Some(text) = part.as_text() {
                            println!("           {}", text);
                        }
                    }
                    if update.is_final {
                        println!("           (final artifact)");
                    }
                }
            },
            Err(e) => {
                println!("[Error] {}", e);
                break;
            }
        }
    }

    println!();
    println!("Stream ended");

    Ok(())
}

/// Demonstrate polling for task completion
#[allow(dead_code)]
async fn demonstrate_polling(client: &A2aClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("Demonstrating task polling...");
    println!();

    // Send a message
    let task = client.send_message("2 + 2").await?;
    println!("Created task: {}", task.id);

    if !task.is_terminal() {
        // Poll until complete
        println!("Polling for completion...");
        let completed_task = client
            .wait_for_task(
                &task.id,
                Duration::from_millis(500),
                Duration::from_secs(30),
            )
            .await?;

        println!("Task completed with status: {}", completed_task.status);
    } else {
        println!("Task already completed: {}", task.status);
    }

    Ok(())
}

/// Demonstrate multi-turn conversation
#[allow(dead_code)]
async fn demonstrate_multi_turn(client: &A2aClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("Demonstrating multi-turn conversation...");
    println!();

    // First message
    let task = client.send_message("What can you help me with?").await?;
    println!("Task created: {}", task.id);
    print_task_response(&task);

    // Continue the conversation
    if task.status == TaskStatus::InputRequired || task.status == TaskStatus::Working {
        let task = client.continue_task(&task.id, "2 + 2").await?;
        print_task_response(&task);
    }

    Ok(())
}

fn print_task_response(task: &skreaver_a2a::Task) {
    for msg in task.messages.iter().filter(|m| m.role == Role::Agent) {
        for part in &msg.parts {
            if let Some(text) = part.as_text() {
                println!("Agent: {}", text);
            }
        }
    }
    println!();
}
