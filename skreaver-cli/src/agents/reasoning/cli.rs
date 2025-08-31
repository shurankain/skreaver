use crate::agents::reasoning::config::ReasoningProfile;
use crate::agents::reasoning::coordinator::ReasoningCoordinatorExt;
use crate::agents::reasoning::tools::{AnalyzeTool, ConcludeTool, DeduceTool, ReflectTool};
use crate::agents::reasoning::wrapper::ReasoningAgentWrapper;
use std::sync::Arc;

use skreaver::InMemoryToolRegistry;
use skreaver::runtime::Coordinator;

pub fn run_reasoning_agent() {
    let agent =
        ReasoningAgentWrapper::new_with_file("reasoning_memory.json", ReasoningProfile::default());

    let registry = InMemoryToolRegistry::new()
        .with_tool("analyze", Arc::new(AnalyzeTool))
        .with_tool("deduce", Arc::new(DeduceTool))
        .with_tool("conclude", Arc::new(ConcludeTool))
        .with_tool("reflect", Arc::new(ReflectTool));

    let mut coordinator = Coordinator::new(agent, registry);

    println!("🧠 Reasoning Agent Started");
    println!("Enter problems to solve (type 'quit' to exit):");

    loop {
        print!("\n🤔 Problem: ");
        if let Err(e) = std::io::Write::flush(&mut std::io::stdout()) {
            tracing::error!(error = %e, "Failed to flush stdout");
            continue;
        }

        let mut input = String::new();
        if let Err(e) = std::io::stdin().read_line(&mut input) {
            tracing::error!(error = %e, "Failed to read user input");
            continue;
        }
        let input = input.trim();

        if input == "quit" {
            break;
        }

        if input.is_empty() {
            continue;
        }

        println!("\n🔍 Reasoning Process:");

        coordinator.observe(input.to_string());
        coordinator.drive_until_complete(coordinator.agent.profile().max_loop_iters);

        println!("\n✅ {}", coordinator.agent.final_result());
        println!("{}", "─".repeat(50));
    }
}
