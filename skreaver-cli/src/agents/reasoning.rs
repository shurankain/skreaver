use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

// Constants for limits (no magic numbers)
const MAX_LOOP_ITERS: usize = 16;
const MAX_PREV_OUTPUT: usize = 1024;
const MAX_CHAIN_LINE: usize = 512;
const MAX_CHAIN_SUMMARY: usize = 2048;

use skreaver::ToolCall;
use skreaver::agent::Agent;
use skreaver::memory::{FileMemory, Memory, MemoryUpdate};
use skreaver::runtime::Coordinator;
use skreaver::tool::registry::{InMemoryToolRegistry, ToolRegistry};
use skreaver::tool::{ExecutionResult, Tool};

// Structured tool output format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichResult {
    pub summary: String,
    pub confidence: f32,     // 0.0..1.0
    pub evidence: Vec<String>
}

// Agent result types
pub enum AgentFinal {
    Complete { steps: usize, answer: String },
    InProgress,
}

// Extension trait for reasoning-specific coordinator methods
trait ReasoningCoordinatorExt {
    fn is_complete(&self) -> bool;
}

impl ReasoningCoordinatorExt for Coordinator<ReasoningAgent, InMemoryToolRegistry> {
    fn is_complete(&self) -> bool {
        matches!(self.agent.reasoning_state, ReasoningState::Complete)
    }
}

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
        coordinator.observe(input.to_string());
        
        // Execute reasoning chain step by step
        let mut guard = 0usize;
        while !coordinator.is_complete() && guard < MAX_LOOP_ITERS {
            guard += 1;
            let tool_calls = coordinator.get_tool_calls();
            if tool_calls.is_empty() {
                break;
            }
            
            for tool_call in tool_calls {
                if let Some(result) = coordinator.dispatch_tool(tool_call) {
                    coordinator.handle_tool_result(result);
                } else {
                    eprintln!("Tool not found in registry");
                    break;
                }
            }
        }
        
        if guard >= MAX_LOOP_ITERS {
            tracing::warn!("Reasoning loop guard triggered - stopped after {} iterations", guard);
        }
        
        match coordinator.agent.final_result() {
            AgentFinal::Complete { steps, answer } => {
                println!("\n✅ Final Answer ({} steps): {}", steps, answer);
            }
            AgentFinal::InProgress => {
                println!("\n⚠️ Incomplete.");
            }
        }
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
    pub evidence: Vec<String>,
    pub timestamp: String,
}

impl ReasoningStep {
    pub fn new(step_type: &str, input: &str, output: &str, confidence: f32, evidence: Vec<String>) -> Self {
        Self {
            step_type: step_type.to_string(),
            input: input.to_string(),
            output: output.to_string(),
            confidence,
            evidence,
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
                            input: format!("Problem: '{}'\nPrevious analysis: '{}'", 
                                problem, self.clip_utf8(&last_step.output, MAX_PREV_OUTPUT)),
                        }]
                    } else {
                        vec![]
                    }
                }
                ReasoningState::Deducing => {
                    if let Some(last_step) = self.reasoning_chain.last() {
                        vec![ToolCall {
                            name: "conclude".into(),
                            input: format!("Problem: '{}'\nPrevious deduction: '{}'", 
                                problem, self.clip_utf8(&last_step.output, MAX_PREV_OUTPUT)),
                        }]
                    } else {
                        vec![]
                    }
                }
                ReasoningState::Concluding => {
                    let chain_summary = self.reasoning_chain
                        .iter()
                        .rev()
                        .take(5)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev() // keep chronological order
                        .map(|step| format!("{}: {}", step.step_type, self.clip_utf8(&step.output, MAX_CHAIN_LINE)))
                        .collect::<Vec<_>>()
                        .join("\n");
                    
                    vec![ToolCall {
                        name: "reflect".into(),
                        input: format!("Problem: '{}'\nReasoning chain:\n{}", 
                            problem, self.clip_utf8(&chain_summary, MAX_CHAIN_SUMMARY)),
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

        // Parse structured output or fallback to plain text
        let parsed: Option<RichResult> = serde_json::from_str(&result.output).ok();
        let (out_text, conf, evidence) = match parsed {
            Some(rr) => (rr.summary, rr.confidence, rr.evidence),
            None => (result.output.clone(), self.extract_confidence(&result.output), vec![]),
        };

        let step = ReasoningStep::new(
            step_type,
            &self.current_problem.clone().unwrap_or_default(),
            &out_text,   // Store summary, not raw JSON
            conf,
            evidence,
        );

        println!("  {} {} (conf {:.2}): {}", self.get_step_emoji(step_type), step_type.to_uppercase(), conf, out_text);
        tracing::info!(step=%step_type, state=?self.reasoning_state, next=?next_state, "step complete");
        
        self.reasoning_chain.push(step);
        self.reasoning_state = next_state;

        // Save reasoning state
        self.memory.store(MemoryUpdate {
            key: "reasoning_state".into(),
            value: format!("{:?}", self.reasoning_state),
        });

        // Save chain length for atomic operations
        self.memory.store(MemoryUpdate {
            key: "reasoning_chain_len".into(),
            value: self.reasoning_chain.len().to_string(),
        });

        // Save last step atomically
        if let Some(last_step) = self.reasoning_chain.last() {
            let step_json = serde_json::to_string(last_step).unwrap_or_default();
            self.memory.store(MemoryUpdate {
                key: "last_reasoning_step".into(),
                value: step_json,
            });
        }

        // Periodically save full chain (every 4 steps or at completion)
        if self.reasoning_chain.len() % 4 == 0 || self.reasoning_state == ReasoningState::Complete {
            let chain_json = serde_json::to_string(&self.reasoning_chain).unwrap_or_default();
            self.memory.store(MemoryUpdate {
                key: "reasoning_chain".into(),
                value: chain_json,
            });
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        self.memory.store(update);
    }

    fn memory(&mut self) -> &mut dyn Memory {
        &mut *self.memory
    }
}

impl ReasoningAgent {
    fn parse_confidence(&self, output: &str) -> Option<f32> {
        serde_json::from_str::<RichResult>(output)
            .map(|r| r.confidence)
            .ok()
    }

    pub fn final_result(&self) -> AgentFinal {
        match self.reasoning_state {
            ReasoningState::Complete => {
                let answer = self.reasoning_chain
                    .last()
                    .map(|s| s.output.clone())
                    .unwrap_or_default();
                AgentFinal::Complete { 
                    steps: self.reasoning_chain.len(), 
                    answer 
                }
            }
            _ => AgentFinal::InProgress
        }
    }

    #[cfg(test)]
    pub fn new_for_test(
        memory: Box<dyn Memory>,
        current_problem: Option<String>,
        reasoning_chain: Vec<ReasoningStep>,
        reasoning_state: ReasoningState,
    ) -> Self {
        Self {
            memory,
            current_problem,
            reasoning_chain,
            reasoning_state,
        }
    }

    fn extract_confidence(&self, output: &str) -> f32 {
        let low = output.to_lowercase();
        if low.contains("very confident") || low.contains("certain") {
            0.9
        } else if low.contains("confident") {
            0.8
        } else if low.contains("likely") {
            0.7
        } else if low.contains("uncertain") || low.contains("maybe") {
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

    // Safe UTF-8 clip that won't panic on multi-byte chars.
    fn clip_utf8(&self, s: &str, max: usize) -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            let clipped: String = s.chars().take(max).collect();
            format!("{}... [truncated]", clipped)
        }
    }
}

struct AnalyzeTool;

impl Tool for AnalyzeTool {
    fn name(&self) -> &str {
        "analyze"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let payload = RichResult {
            summary: format!(
                "Problem Analysis: Breaking down '{}' into core components. Identifying key elements, constraints, and required approach for systematic resolution.",
                input.trim()
            ),
            confidence: 0.75,
            evidence: vec![],
        };
        
        ExecutionResult {
            // Serialize to string for current Coordinator API
            output: serde_json::to_string(&payload).unwrap_or_else(|_| payload.summary),
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
        let payload = RichResult {
            summary: format!(
                "Logical Deduction: From the analysis of '{}', applying reasoning principles and domain knowledge to derive intermediate conclusions and logical steps toward solution.",
                input.trim()
            ),
            confidence: 0.8,
            evidence: vec!["Previous analysis context".into()],
        };

        ExecutionResult {
            output: serde_json::to_string(&payload).unwrap_or_else(|_| payload.summary),
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
        let payload = RichResult {
            summary: format!(
                "Final Conclusion: Synthesizing analysis and deductions for '{}'. Based on the reasoning chain, arriving at the most supported and logical resolution to the problem.",
                input.trim()
            ),
            confidence: 0.85,
            evidence: vec!["Analysis".into(), "Deduction".into()],
        };

        ExecutionResult {
            output: serde_json::to_string(&payload).unwrap_or_else(|_| payload.summary),
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
        
        let payload = RichResult {
            summary: format!(
                "Meta-Reflection: Evaluating reasoning quality for '{}'. The chain of thought was {} and {}, maintaining logical coherence throughout the process.",
                input.trim(), 
                complexity,
                quality
            ),
            confidence: 0.9,
            evidence: vec!["Complete reasoning chain".into(), "Step coherence".into()],
        };

        ExecutionResult {
            output: serde_json::to_string(&payload).unwrap_or_else(|_| payload.summary),
            success: true,
        }
    }
}