use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

// Configuration profile for reasoning behavior
#[derive(Clone, Debug)]
pub struct ReasoningProfile {
    pub max_loop_iters: usize,
    pub max_prev_output: usize,
    pub max_chain_line: usize,
    pub max_chain_summary: usize,
}

impl Default for ReasoningProfile {
    fn default() -> Self {
        Self {
            max_loop_iters: 16,
            max_prev_output: 1024,
            max_chain_line: 512,
            max_chain_summary: 2048,
        }
    }
}

impl ReasoningProfile {
    /// Create a new builder for configuring reasoning behavior.
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_cli::agents::reasoning::ReasoningProfile;
    ///
    /// let profile = ReasoningProfile::builder()
    ///     .max_loop_iters(32)
    ///     .max_prev_output(2048)
    ///     .max_chain_line(1024)
    ///     .build();
    /// ```
    #[allow(dead_code)]
    pub fn builder() -> ReasoningProfileBuilder {
        ReasoningProfileBuilder::default()
    }

    /// Create a high-performance profile for fast reasoning.
    #[allow(dead_code)]
    pub fn fast() -> Self {
        Self::builder()
            .max_loop_iters(8)
            .max_prev_output(512)
            .max_chain_line(256)
            .max_chain_summary(1024)
            .build()
    }

    /// Create a comprehensive profile for thorough reasoning.
    #[allow(dead_code)]
    pub fn comprehensive() -> Self {
        Self::builder()
            .max_loop_iters(32)
            .max_prev_output(4096)
            .max_chain_line(1024)
            .max_chain_summary(8192)
            .build()
    }
}

/// Builder for configuring ReasoningProfile instances.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ReasoningProfileBuilder {
    max_loop_iters: usize,
    max_prev_output: usize,
    max_chain_line: usize,
    max_chain_summary: usize,
}

impl Default for ReasoningProfileBuilder {
    fn default() -> Self {
        Self {
            max_loop_iters: 16,
            max_prev_output: 1024,
            max_chain_line: 512,
            max_chain_summary: 2048,
        }
    }
}

#[allow(dead_code)]
impl ReasoningProfileBuilder {
    /// Set the maximum number of reasoning loop iterations.
    ///
    /// This prevents infinite loops in complex reasoning scenarios.
    ///
    /// # Parameters
    ///
    /// * `iters` - Maximum loop iterations (default: 16)
    pub fn max_loop_iters(mut self, iters: usize) -> Self {
        self.max_loop_iters = iters;
        self
    }

    /// Set the maximum length of previous tool output to include.
    ///
    /// Controls how much context from previous tools is passed forward.
    ///
    /// # Parameters
    ///
    /// * `chars` - Maximum character count (default: 1024)
    pub fn max_prev_output(mut self, chars: usize) -> Self {
        self.max_prev_output = chars;
        self
    }

    /// Set the maximum length for individual chain step summaries.
    ///
    /// Keeps reasoning chains concise while preserving key information.
    ///
    /// # Parameters
    ///
    /// * `chars` - Maximum character count per step (default: 512)
    pub fn max_chain_line(mut self, chars: usize) -> Self {
        self.max_chain_line = chars;
        self
    }

    /// Set the maximum length for the complete reasoning chain summary.
    ///
    /// Controls the total context size when reflecting on reasoning.
    ///
    /// # Parameters
    ///
    /// * `chars` - Maximum total summary length (default: 2048)
    pub fn max_chain_summary(mut self, chars: usize) -> Self {
        self.max_chain_summary = chars;
        self
    }

    /// Build the configured ReasoningProfile.
    ///
    /// # Returns
    ///
    /// A new `ReasoningProfile` with the specified configuration
    pub fn build(self) -> ReasoningProfile {
        ReasoningProfile {
            max_loop_iters: self.max_loop_iters,
            max_prev_output: self.max_prev_output,
            max_chain_line: self.max_chain_line,
            max_chain_summary: self.max_chain_summary,
        }
    }
}

use skreaver::ToolCall;
use skreaver::agent::Agent;
use skreaver::memory::{FileMemory, Memory, MemoryUpdate};
use skreaver::runtime::Coordinator;
use skreaver::tool::registry::InMemoryToolRegistry;
use skreaver::tool::{ExecutionResult, Tool};

// Structured tool output format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichResult {
    pub summary: String,
    pub confidence: f32, // 0.0..1.0
    pub evidence: Vec<String>,
}

impl RichResult {
    /// Create a new builder for configuring RichResult instances.
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_cli::agents::reasoning::RichResult;
    ///
    /// let result = RichResult::builder()
    ///     .summary("Analysis complete".to_string())
    ///     .confidence(0.85)
    ///     .evidence(vec!["fact 1".to_string(), "fact 2".to_string()])
    ///     .build();
    /// ```
    #[allow(dead_code)]
    pub fn builder() -> RichResultBuilder {
        RichResultBuilder::default()
    }

    /// Create a high-confidence result.
    #[allow(dead_code)]
    pub fn confident(summary: String) -> Self {
        Self::builder().summary(summary).confidence(0.9).build()
    }

    /// Create a low-confidence result with explanation.
    #[allow(dead_code)]
    pub fn uncertain(summary: String, reason: String) -> Self {
        Self::builder()
            .summary(summary)
            .confidence(0.3)
            .add_evidence(reason)
            .build()
    }
}

/// Builder for configuring RichResult instances.
#[derive(Debug)]
#[allow(dead_code)]
pub struct RichResultBuilder {
    summary: String,
    confidence: f32,
    evidence: Vec<String>,
}

impl Default for RichResultBuilder {
    fn default() -> Self {
        Self {
            summary: String::new(),
            confidence: 0.0,
            evidence: Vec::new(),
        }
    }
}

#[allow(dead_code)]
impl RichResultBuilder {
    /// Set the summary text for the result.
    ///
    /// This should be a concise description of the tool's findings.
    ///
    /// # Parameters
    ///
    /// * `summary` - The summary description
    pub fn summary(mut self, summary: String) -> Self {
        self.summary = summary;
        self
    }

    /// Set the confidence level for the result.
    ///
    /// Confidence should be between 0.0 and 1.0, where 1.0 represents
    /// complete certainty and 0.0 represents complete uncertainty.
    ///
    /// # Parameters
    ///
    /// * `confidence` - Confidence level (0.0..1.0)
    pub fn confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the evidence supporting this result.
    ///
    /// Evidence provides supporting information or facts that back up
    /// the result's conclusions.
    ///
    /// # Parameters
    ///
    /// * `evidence` - Vector of evidence strings
    pub fn evidence(mut self, evidence: Vec<String>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Add a single piece of evidence to the result.
    ///
    /// This is a convenience method for adding evidence one item at a time.
    ///
    /// # Parameters
    ///
    /// * `evidence` - Single evidence string to add
    pub fn add_evidence(mut self, evidence: String) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Build the configured RichResult.
    ///
    /// # Returns
    ///
    /// A new `RichResult` with the specified configuration
    pub fn build(self) -> RichResult {
        RichResult {
            summary: self.summary,
            confidence: self.confidence,
            evidence: self.evidence,
        }
    }
}

// Agent result types
pub enum AgentFinal {
    Complete { steps: usize, answer: String },
    InProgress,
}

impl std::fmt::Display for AgentFinal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentFinal::Complete { steps, answer } => {
                write!(f, "Final Answer ({} steps): {}", steps, answer)
            }
            AgentFinal::InProgress => write!(f, "In progress"),
        }
    }
}

// Extension trait for reasoning-specific coordinator methods
pub(crate) trait ReasoningCoordinatorExt {
    fn is_complete(&self) -> bool;
    fn drive_until_complete(&mut self, max_iters: usize);
}

impl ReasoningCoordinatorExt for Coordinator<ReasoningAgent, InMemoryToolRegistry> {
    fn is_complete(&self) -> bool {
        matches!(self.agent.reasoning_state, ReasoningState::Complete)
    }

    fn drive_until_complete(&mut self, max_iters: usize) {
        let mut iters = 0;
        while !self.is_complete() && iters < max_iters {
            iters += 1;
            let calls = self.get_tool_calls();
            if calls.is_empty() {
                break;
            }
            for call in calls {
                if let Some(res) = self.dispatch_tool(call) {
                    self.handle_tool_result(res);
                } else {
                    tracing::error!("Tool not found in registry");
                    return;
                }
            }
        }
        if iters >= max_iters {
            tracing::warn!("Reasoning loop guard triggered at {}", iters);
        }
    }
}

pub fn run_reasoning_agent() {
    let memory_path = PathBuf::from("reasoning_memory.json");

    let agent = ReasoningAgent {
        memory: Box::new(FileMemory::new(memory_path)),
        current_problem: None,
        reasoning_chain: Vec::new(),
        reasoning_state: ReasoningState::Initial,
        profile: ReasoningProfile::default(),
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
        coordinator.drive_until_complete(coordinator.agent.profile.max_loop_iters);

        println!("\n✅ {}", coordinator.agent.final_result());
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
    pub fn new(
        step_type: &str,
        input: &str,
        output: &str,
        confidence: f32,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            step_type: step_type.to_string(),
            input: input.to_string(),
            output: output.to_string(),
            confidence,
            evidence,
            timestamp: chrono::Utc::now()
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
        }
    }
}

pub struct ReasoningAgent {
    memory: Box<dyn Memory + Send>,
    current_problem: Option<String>,
    reasoning_chain: Vec<ReasoningStep>,
    reasoning_state: ReasoningState,
    profile: ReasoningProfile,
}

impl Agent for ReasoningAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        self.current_problem = Some(input.clone());
        self.reasoning_chain.clear();
        self.reasoning_state = ReasoningState::Initial;

        self.memory
            .store(MemoryUpdate::new("current_problem", &input).expect("Valid memory key"))
            .ok();

        let chain_json = serde_json::to_string(&self.reasoning_chain).unwrap_or_default();
        self.memory
            .store(MemoryUpdate::new("reasoning_chain", &chain_json).expect("Valid memory key"))
            .ok();
    }

    fn act(&mut self) -> Self::Action {
        match self.reasoning_state {
            ReasoningState::Complete => {
                if let Some(last_step) = self.reasoning_chain.last() {
                    // Use efficient string building instead of format!
                    let steps_str = self.reasoning_chain.len().to_string();
                    let mut result =
                        String::with_capacity(last_step.output.len() + steps_str.len() + 48);
                    result.push_str("After ");
                    result.push_str(&steps_str);
                    result.push_str(" reasoning steps, my conclusion is: ");
                    result.push_str(&last_step.output);
                    result
                } else {
                    String::from("Unable to reach a conclusion.")
                }
            }
            _ => String::from("Reasoning in progress..."),
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let Some(problem) = &self.current_problem {
            match self.reasoning_state {
                ReasoningState::Initial => {
                    vec![ToolCall::new("analyze", problem).expect("Valid tool name")]
                }
                ReasoningState::Analyzing => {
                    if let Some(last_step) = self.reasoning_chain.last() {
                        // Pre-allocate capacity to avoid reallocations
                        let clipped_output =
                            self.clip_utf8(&last_step.output, self.profile.max_prev_output);
                        let mut input =
                            String::with_capacity(problem.len() + clipped_output.len() + 32);
                        input.push_str("Problem: '");
                        input.push_str(problem);
                        input.push_str("'\nPrevious analysis: '");
                        input.push_str(&clipped_output);
                        input.push('\'');

                        vec![ToolCall::new("deduce", &input).expect("Valid tool name")]
                    } else {
                        vec![]
                    }
                }
                ReasoningState::Deducing => {
                    if let Some(last_step) = self.reasoning_chain.last() {
                        // Pre-allocate capacity to avoid reallocations
                        let clipped_output =
                            self.clip_utf8(&last_step.output, self.profile.max_prev_output);
                        let mut input =
                            String::with_capacity(problem.len() + clipped_output.len() + 32);
                        input.push_str("Problem: '");
                        input.push_str(problem);
                        input.push_str("'\nPrevious deduction: '");
                        input.push_str(&clipped_output);
                        input.push('\'');

                        vec![ToolCall::new("conclude", &input).expect("Valid tool name")]
                    } else {
                        vec![]
                    }
                }
                ReasoningState::Concluding => {
                    // Build chain summary efficiently without intermediate format! calls
                    let mut chain_summary = String::new();
                    let recent_steps: Vec<_> = self.reasoning_chain.iter().rev().take(5).collect();

                    for (i, step) in recent_steps.iter().rev().enumerate() {
                        if i > 0 {
                            chain_summary.push('\n');
                        }
                        chain_summary.push_str(&step.step_type);
                        chain_summary.push_str(": ");
                        chain_summary
                            .push_str(&self.clip_utf8(&step.output, self.profile.max_chain_line));
                    }

                    // Build final input efficiently
                    let clipped_summary =
                        self.clip_utf8(&chain_summary, self.profile.max_chain_summary);
                    let mut input =
                        String::with_capacity(problem.len() + clipped_summary.len() + 32);
                    input.push_str("Problem: '");
                    input.push_str(problem);
                    input.push_str("'\nReasoning chain:\n");
                    input.push_str(&clipped_summary);

                    vec![ToolCall::new("reflect", &input).expect("Valid tool name")]
                }
                ReasoningState::Reflecting | ReasoningState::Complete => vec![],
            }
        } else {
            vec![]
        }
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if !result.is_success() {
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
        let parsed: Option<RichResult> = serde_json::from_str(result.output()).ok();
        let (out_text, conf, evidence) = match parsed {
            Some(rr) => (rr.summary, rr.confidence, rr.evidence),
            None => (
                result.output().to_string(),
                self.extract_confidence(result.output()),
                vec![],
            ),
        };

        let evidence_count = evidence.len();
        let step = ReasoningStep::new(
            step_type,
            &self.current_problem.clone().unwrap_or_default(),
            &out_text, // Store summary, not raw JSON
            conf,
            evidence,
        );

        println!(
            "  {} {} (conf {:.2}, evidence {}): {}",
            self.get_step_emoji(step_type),
            step_type.to_uppercase(),
            conf,
            evidence_count,
            out_text
        );
        tracing::info!(step=%step_type, state=?self.reasoning_state, next=?next_state, "step complete");

        self.reasoning_chain.push(step);
        self.reasoning_state = next_state;

        // Save reasoning state
        let _ = self.memory.store(MemoryUpdate {
            key: skreaver::memory::MemoryKey::new("reasoning_state").expect("Valid memory key"),
            value: format!("{:?}", self.reasoning_state),
        });

        // Save chain length for atomic operations
        let _ = self.memory.store(MemoryUpdate {
            key: skreaver::memory::MemoryKey::new("reasoning_chain_len").expect("Valid memory key"),
            value: self.reasoning_chain.len().to_string(),
        });

        // Save last step atomically
        if let Some(last_step) = self.reasoning_chain.last() {
            let step_json = serde_json::to_string(last_step).unwrap_or_default();
            let _ = self.memory.store(MemoryUpdate {
                key: skreaver::memory::MemoryKey::new("last_reasoning_step")
                    .expect("Valid memory key"),
                value: step_json,
            });
        }

        // Periodically save full chain (every 4 steps or at completion)
        if self.reasoning_chain.len() % 4 == 0 || self.reasoning_state == ReasoningState::Complete {
            let chain_json = serde_json::to_string(&self.reasoning_chain).unwrap_or_default();
            let _ = self.memory.store(MemoryUpdate {
                key: skreaver::memory::MemoryKey::new("reasoning_chain").expect("Valid memory key"),
                value: chain_json,
            });
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let _ = self.memory.store(update);
    }

    fn memory(&mut self) -> &mut dyn Memory {
        &mut *self.memory
    }
}

impl ReasoningAgent {
    pub fn final_result(&self) -> AgentFinal {
        match self.reasoning_state {
            ReasoningState::Complete => {
                let answer = self
                    .reasoning_chain
                    .last()
                    .map(|s| s.output.clone())
                    .unwrap_or_default();
                AgentFinal::Complete {
                    steps: self.reasoning_chain.len(),
                    answer,
                }
            }
            _ => AgentFinal::InProgress,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(
        memory: Box<dyn Memory + Send>,
        current_problem: Option<String>,
        reasoning_chain: Vec<ReasoningStep>,
        reasoning_state: ReasoningState,
    ) -> Self {
        Self {
            memory,
            current_problem,
            reasoning_chain,
            reasoning_state,
            profile: ReasoningProfile::default(),
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

        ExecutionResult::success(serde_json::to_string(&payload).unwrap_or(payload.summary))
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

        ExecutionResult::success(serde_json::to_string(&payload).unwrap_or(payload.summary))
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

        ExecutionResult::success(serde_json::to_string(&payload).unwrap_or(payload.summary))
    }
}

struct ReflectTool;

impl Tool for ReflectTool {
    fn name(&self) -> &str {
        "reflect"
    }

    fn call(&self, input: String) -> ExecutionResult {
        let word_count = input.split_whitespace().count();
        let complexity = if word_count > 20 {
            "comprehensive"
        } else {
            "focused"
        };
        let quality = if input.len() > 50 {
            "thorough"
        } else {
            "concise"
        };

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

        ExecutionResult::success(serde_json::to_string(&payload).unwrap_or(payload.summary))
    }
}

#[cfg(test)]
mod builder_tests {
    use super::*;

    #[test]
    fn test_reasoning_profile_builder() {
        let profile = ReasoningProfile::builder()
            .max_loop_iters(32)
            .max_prev_output(2048)
            .max_chain_line(1024)
            .max_chain_summary(4096)
            .build();

        assert_eq!(profile.max_loop_iters, 32);
        assert_eq!(profile.max_prev_output, 2048);
        assert_eq!(profile.max_chain_line, 1024);
        assert_eq!(profile.max_chain_summary, 4096);
    }

    #[test]
    fn test_reasoning_profile_builder_defaults() {
        let profile = ReasoningProfile::builder().build();

        assert_eq!(profile.max_loop_iters, 16);
        assert_eq!(profile.max_prev_output, 1024);
        assert_eq!(profile.max_chain_line, 512);
        assert_eq!(profile.max_chain_summary, 2048);
    }

    #[test]
    fn test_rich_result_builder() {
        let result = RichResult::builder()
            .summary("Analysis complete".to_string())
            .confidence(0.85)
            .evidence(vec!["fact 1".to_string(), "fact 2".to_string()])
            .build();

        assert_eq!(result.summary, "Analysis complete");
        assert_eq!(result.confidence, 0.85);
        assert_eq!(result.evidence, vec!["fact 1", "fact 2"]);
    }

    #[test]
    fn test_rich_result_builder_with_add_evidence() {
        let result = RichResult::builder()
            .summary("Test summary".to_string())
            .confidence(0.9)
            .add_evidence("evidence 1".to_string())
            .add_evidence("evidence 2".to_string())
            .build();

        assert_eq!(result.summary, "Test summary");
        assert_eq!(result.confidence, 0.9);
        assert_eq!(result.evidence, vec!["evidence 1", "evidence 2"]);
    }

    #[test]
    fn test_rich_result_builder_confidence_clamping() {
        let result_high = RichResult::builder()
            .summary("Test".to_string())
            .confidence(1.5) // Should be clamped to 1.0
            .build();

        let result_low = RichResult::builder()
            .summary("Test".to_string())
            .confidence(-0.5) // Should be clamped to 0.0
            .build();

        assert_eq!(result_high.confidence, 1.0);
        assert_eq!(result_low.confidence, 0.0);
    }

    #[test]
    fn test_rich_result_builder_defaults() {
        let result = RichResult::builder().build();

        assert_eq!(result.summary, "");
        assert_eq!(result.confidence, 0.0);
        assert!(result.evidence.is_empty());
    }

    #[test]
    fn test_reasoning_profile_presets() {
        let fast = ReasoningProfile::fast();
        assert_eq!(fast.max_loop_iters, 8);
        assert_eq!(fast.max_prev_output, 512);

        let comprehensive = ReasoningProfile::comprehensive();
        assert_eq!(comprehensive.max_loop_iters, 32);
        assert_eq!(comprehensive.max_prev_output, 4096);
    }

    #[test]
    fn test_rich_result_presets() {
        let confident = RichResult::confident("High confidence result".to_string());
        assert_eq!(confident.summary, "High confidence result");
        assert_eq!(confident.confidence, 0.9);

        let uncertain = RichResult::uncertain(
            "Low confidence".to_string(),
            "Insufficient data".to_string(),
        );
        assert_eq!(uncertain.summary, "Low confidence");
        assert_eq!(uncertain.confidence, 0.3);
        assert_eq!(uncertain.evidence, vec!["Insufficient data"]);
    }
}
