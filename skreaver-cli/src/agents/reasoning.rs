use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use skreaver::ToolCall;
use skreaver::agent::Agent;
use skreaver::memory::{FileMemory, Memory, MemoryUpdate};
use skreaver::runtime::Coordinator;
use skreaver::tool::registry::{InMemoryToolRegistry, ToolRegistry};
use skreaver::tool::{ExecutionResult, Tool};

pub fn run_reasoning_agent() {
    let memory_path = PathBuf::from("reasoning_memory.json");

    let agent = ReasoningAgent {
        memory: Box::new(FileMemory::new(memory_path)),
        current_problem: None,
        reasoning_chain: Vec::new(),
        reasoning_state: ReasoningState::Initial,
    };

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
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input == "quit" {
            break;
        }

        if input.is_empty() {
            continue;
        }

        println!("\n🔍 Reasoning Process:");
        
        // Reset agent state for new problem
        coordinator.agent.observe(input.to_string());
        
        // Execute reasoning chain step by step
        while coordinator.agent.reasoning_state != ReasoningState::Complete {
            let tool_calls = coordinator.agent.call_tools();
            if tool_calls.is_empty() {
                break;
            }
            
            for tool_call in tool_calls {
                if let Some(result) = coordinator.registry.dispatch(tool_call) {
                    coordinator.agent.handle_result(result);
                } else {
                    eprintln!("Tool not found in registry");
                    break;
                }
            }
        }
        
        let output = coordinator.agent.act();
        println!("\n✅ Final Answer: {}", output);
        println!("{}", "─".repeat(50));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReasoningState {
    Initial,
    Analyzing,
    Deducing,
    Concluding,
    Reflecting,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub step_type: String,
    pub input: String,
    pub output: String,
    pub confidence: f32,
    pub timestamp: String,
}

impl ReasoningStep {
    pub fn new(step_type: &str, input: &str, output: &str, confidence: f32) -> Self {
        Self {
            step_type: step_type.to_string(),
            input: input.to_string(),
            output: output.to_string(),
            confidence,
            timestamp: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        }
    }
}

pub struct ReasoningAgent {
    memory: Box<dyn Memory>,
    current_problem: Option<String>,
    reasoning_chain: Vec<ReasoningStep>,
    reasoning_state: ReasoningState,
}

impl Agent for ReasoningAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        self.current_problem = Some(input.clone());
        self.reasoning_chain.clear();
        self.reasoning_state = ReasoningState::Initial;
        
        self.memory.store(MemoryUpdate {
            key: "current_problem".into(),
            value: input,
        });

        let chain_json = serde_json::to_string(&self.reasoning_chain).unwrap_or_default();
        self.memory.store(MemoryUpdate {
            key: "reasoning_chain".into(),
            value: chain_json,
        });
    }

    fn act(&mut self) -> Self::Action {
        match self.reasoning_state {
            ReasoningState::Complete => {
                if let Some(last_step) = self.reasoning_chain.last() {
                    format!(
                        "After {} reasoning steps, my conclusion is: {}",
                        self.reasoning_chain.len(),
                        last_step.output
                    )
                } else {
                    "Unable to reach a conclusion.".to_string()
                }
            }
            _ => "Reasoning in progress...".to_string(),
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let Some(problem) = &self.current_problem {
            match self.reasoning_state {
                ReasoningState::Initial => {
                    vec![ToolCall {
                        name: "analyze".into(),
                        input: problem.clone(),
                    }]
                }
                ReasoningState::Analyzing => {
                    if let Some(last_step) = self.reasoning_chain.last() {
                        vec![ToolCall {
                            name: "deduce".into(),
                            input: format!("Problem: '{}'\nPrevious analysis: '{}'", problem, last_step.output),
                        }]
                    } else {
                        vec![]
                    }
                }
                ReasoningState::Deducing => {
                    if let Some(last_step) = self.reasoning_chain.last() {
                        vec![ToolCall {
                            name: "conclude".into(),
                            input: format!("Problem: '{}'\nPrevious deduction: '{}'", problem, last_step.output),
                        }]
                    } else {
                        vec![]
                    }
                }
                ReasoningState::Concluding => {
                    let chain_summary = self.reasoning_chain
                        .iter()
                        .map(|step| format!("{}: {}", step.step_type, step.output))
                        .collect::<Vec<_>>()
                        .join("\n");
                    
                    vec![ToolCall {
                        name: "reflect".into(),
                        input: format!("Problem: '{}'\nReasoning chain:\n{}", problem, chain_summary),
                    }]
                }
                ReasoningState::Reflecting | ReasoningState::Complete => vec![],
            }
        } else {
            vec![]
        }
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if !result.success {
            return;
        }

        let (step_type, next_state) = match self.reasoning_state {
            ReasoningState::Initial => ("analyze", ReasoningState::Analyzing),
            ReasoningState::Analyzing => ("deduce", ReasoningState::Deducing),
            ReasoningState::Deducing => ("conclude", ReasoningState::Concluding),
            ReasoningState::Concluding => ("reflect", ReasoningState::Complete),
            _ => return,
        };

        let confidence = self.extract_confidence(&result.output);
        let step = ReasoningStep::new(
            step_type,
            &self.current_problem.clone().unwrap_or_default(),
            &result.output,
            confidence,
        );

        println!("  {} {}: {}", self.get_step_emoji(step_type), step_type.to_uppercase(), result.output);
        
        self.reasoning_chain.push(step);
        self.reasoning_state = next_state;

        let chain_json = serde_json::to_string(&self.reasoning_chain).unwrap_or_default();
        self.memory.store(MemoryUpdate {
            key: "reasoning_chain".into(),
            value: chain_json,
        });
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        self.memory.store(update);
    }

    fn memory(&mut self) -> &mut dyn Memory {
        &mut *self.memory
    }
}

impl ReasoningAgent {
    fn extract_confidence(&self, output: &str) -> f32 {
        if output.contains("very confident") || output.contains("certain") {
            0.9
        } else if output.contains("confident") {
            0.8
        } else if output.contains("likely") {
            0.7
        } else if output.contains("uncertain") || output.contains("maybe") {
            0.5
        } else {
            0.6
        }
    }

    fn get_step_emoji(&self, step_type: &str) -> &str {
        match step_type {
            "analyze" => "🔍",
            "deduce" => "🧠",
            "conclude" => "💡",
            "reflect" => "🤔",
            _ => "⚡",
        }
    }
}

struct AnalyzeTool;

impl Tool for AnalyzeTool {
    fn name(&self) -> &str {
        "analyze"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let analysis = format!(
            "Problem Analysis: Breaking down '{}' into core components. Identifying key elements, constraints, and required approach for systematic resolution.",
            input.trim()
        );

        ExecutionResult {
            output: analysis,
            success: true,
        }
    }
}

struct DeduceTool;

impl Tool for DeduceTool {
    fn name(&self) -> &str {
        "deduce"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let deduction = format!(
            "Logical Deduction: From the analysis of '{}', applying reasoning principles and domain knowledge to derive intermediate conclusions and logical steps toward solution.",
            input.trim()
        );

        ExecutionResult {
            output: deduction,
            success: true,
        }
    }
}

struct ConcludeTool;

impl Tool for ConcludeTool {
    fn name(&self) -> &str {
        "conclude"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let conclusion = format!(
            "Final Conclusion: Synthesizing analysis and deductions for '{}'. Based on the reasoning chain, arriving at the most supported and logical resolution to the problem.",
            input.trim()
        );

        ExecutionResult {
            output: conclusion,
            success: true,
        }
    }
}

struct ReflectTool;

impl Tool for ReflectTool {
    fn name(&self) -> &str {
        "reflect"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let word_count = input.split_whitespace().count();
        let complexity = if word_count > 20 { "comprehensive" } else { "focused" };
        let quality = if input.len() > 50 { "thorough" } else { "concise" };
        
        let reflection = format!(
            "Meta-Reflection: Evaluating reasoning quality for '{}'. The chain of thought was {} and {}, maintaining logical coherence throughout the process.",
            input.trim(), 
            complexity,
            quality
        );

        ExecutionResult {
            output: reflection,
            success: true,
        }
    }
}