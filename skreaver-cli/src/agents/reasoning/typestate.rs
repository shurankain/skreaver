use super::config::ReasoningProfile;
use super::rich_result::RichResult;
use super::states::*;
use skreaver::memory::{MemoryReader, MemoryUpdate, MemoryWriter};
use skreaver::{ExecutionResult, ToolCall};

/// Typestate pattern for compile-time state safety
pub struct TypedReasoningAgent<M, S = Initial>
where
    M: MemoryReader + MemoryWriter,
{
    pub memory: M,
    pub profile: ReasoningProfile,
    pub state: S,
}

// Implementation for Initial state
impl<M> TypedReasoningAgent<M, Initial>
where
    M: MemoryReader + MemoryWriter,
{
    pub fn new(memory: M, profile: ReasoningProfile) -> Self {
        Self {
            memory,
            profile,
            state: Initial,
        }
    }

    pub fn observe(mut self, problem: String) -> TypedReasoningAgent<M, Analyzing> {
        self.memory
            .store(MemoryUpdate::new("current_problem", &problem).expect("Valid memory key"))
            .ok();

        let empty_chain: Vec<ReasoningStep> = Vec::new();
        let chain_json = serde_json::to_string(&empty_chain).unwrap_or_default();
        self.memory
            .store(MemoryUpdate::new("reasoning_chain", &chain_json).expect("Valid memory key"))
            .ok();

        TypedReasoningAgent {
            memory: self.memory,
            profile: self.profile,
            state: Analyzing {
                problem,
                reasoning_chain: Vec::new(),
            },
        }
    }
}

// Implementation for Analyzing state
impl<M> TypedReasoningAgent<M, Analyzing>
where
    M: MemoryReader + MemoryWriter,
{
    pub fn analyze(
        mut self,
        result: ExecutionResult,
    ) -> Result<TypedReasoningAgent<M, Deducing>, Self> {
        if !result.is_success() {
            return Err(self);
        }

        let (out_text, conf, evidence) = self.parse_result(&result);
        let step = ReasoningStep::new("analyze", &self.state.problem, &out_text, conf, evidence);

        self.log_step(&step);
        self.save_step(&step);

        let mut reasoning_chain = self.state.reasoning_chain;
        reasoning_chain.push(step);

        Ok(TypedReasoningAgent {
            memory: self.memory,
            profile: self.profile,
            state: Deducing {
                problem: self.state.problem,
                reasoning_chain,
            },
        })
    }

    pub fn get_tool_calls(&self) -> Vec<ToolCall> {
        vec![ToolCall::new("analyze", &self.state.problem).expect("Valid tool name")]
    }
}

// Implementation for Deducing state
impl<M> TypedReasoningAgent<M, Deducing>
where
    M: MemoryReader + MemoryWriter,
{
    pub fn deduce(
        mut self,
        result: ExecutionResult,
    ) -> Result<TypedReasoningAgent<M, Concluding>, Self> {
        if !result.is_success() {
            return Err(self);
        }

        let (out_text, conf, evidence) = self.parse_result(&result);
        let step = ReasoningStep::new("deduce", &self.state.problem, &out_text, conf, evidence);

        self.log_step(&step);
        self.save_step(&step);

        let mut reasoning_chain = self.state.reasoning_chain;
        reasoning_chain.push(step);

        Ok(TypedReasoningAgent {
            memory: self.memory,
            profile: self.profile,
            state: Concluding {
                problem: self.state.problem,
                reasoning_chain,
            },
        })
    }

    pub fn get_tool_calls(&self) -> Vec<ToolCall> {
        if let Some(last_step) = self.state.reasoning_chain.last() {
            let clipped_output = self.clip_utf8(&last_step.output, self.profile.max_prev_output);
            let mut input =
                String::with_capacity(self.state.problem.len() + clipped_output.len() + 32);
            input.push_str("Problem: '");
            input.push_str(&self.state.problem);
            input.push_str("'\nPrevious analysis: '");
            input.push_str(&clipped_output);
            input.push('\'');

            vec![ToolCall::new("deduce", &input).expect("Valid tool name")]
        } else {
            vec![]
        }
    }
}

// Implementation for Concluding state
impl<M> TypedReasoningAgent<M, Concluding>
where
    M: MemoryReader + MemoryWriter,
{
    pub fn conclude(
        mut self,
        result: ExecutionResult,
    ) -> Result<TypedReasoningAgent<M, Reflecting>, Self> {
        if !result.is_success() {
            return Err(self);
        }

        let (out_text, conf, evidence) = self.parse_result(&result);
        let step = ReasoningStep::new("conclude", &self.state.problem, &out_text, conf, evidence);

        self.log_step(&step);
        self.save_step(&step);

        let mut reasoning_chain = self.state.reasoning_chain;
        reasoning_chain.push(step);

        Ok(TypedReasoningAgent {
            memory: self.memory,
            profile: self.profile,
            state: Reflecting {
                problem: self.state.problem,
                reasoning_chain,
            },
        })
    }

    pub fn get_tool_calls(&self) -> Vec<ToolCall> {
        if let Some(last_step) = self.state.reasoning_chain.last() {
            let clipped_output = self.clip_utf8(&last_step.output, self.profile.max_prev_output);
            let mut input =
                String::with_capacity(self.state.problem.len() + clipped_output.len() + 32);
            input.push_str("Problem: '");
            input.push_str(&self.state.problem);
            input.push_str("'\nPrevious deduction: '");
            input.push_str(&clipped_output);
            input.push('\'');

            vec![ToolCall::new("conclude", &input).expect("Valid tool name")]
        } else {
            vec![]
        }
    }
}

// Implementation for Reflecting state
impl<M> TypedReasoningAgent<M, Reflecting>
where
    M: MemoryReader + MemoryWriter,
{
    pub fn reflect(
        mut self,
        result: ExecutionResult,
    ) -> Result<TypedReasoningAgent<M, Complete>, Self> {
        if !result.is_success() {
            return Err(self);
        }

        let (out_text, conf, evidence) = self.parse_result(&result);
        let step = ReasoningStep::new("reflect", &self.state.problem, &out_text, conf, evidence);

        self.log_step(&step);
        self.save_step(&step);

        let mut reasoning_chain = self.state.reasoning_chain;
        reasoning_chain.push(step);

        Ok(TypedReasoningAgent {
            memory: self.memory,
            profile: self.profile,
            state: Complete {
                problem: self.state.problem,
                reasoning_chain,
            },
        })
    }

    pub fn get_tool_calls(&self) -> Vec<ToolCall> {
        let mut chain_summary = String::new();
        let recent_steps: Vec<_> = self.state.reasoning_chain.iter().rev().take(5).collect();

        for (i, step) in recent_steps.iter().rev().enumerate() {
            if i > 0 {
                chain_summary.push('\n');
            }
            chain_summary.push_str(&step.step_type);
            chain_summary.push_str(": ");
            chain_summary.push_str(&self.clip_utf8(&step.output, self.profile.max_chain_line));
        }

        let clipped_summary = self.clip_utf8(&chain_summary, self.profile.max_chain_summary);
        let mut input =
            String::with_capacity(self.state.problem.len() + clipped_summary.len() + 32);
        input.push_str("Problem: '");
        input.push_str(&self.state.problem);
        input.push_str("'\nReasoning chain:\n");
        input.push_str(&clipped_summary);

        vec![ToolCall::new("reflect", &input).expect("Valid tool name")]
    }
}

// Implementation for Complete state
impl<M> TypedReasoningAgent<M, Complete>
where
    M: MemoryReader + MemoryWriter,
{
    pub fn final_result(&self) -> AgentFinal {
        let answer = self
            .state
            .reasoning_chain
            .last()
            .map(|s| s.output.clone())
            .unwrap_or_default();
        AgentFinal::Complete {
            steps: self.state.reasoning_chain.len(),
            answer,
        }
    }

    pub fn get_tool_calls(&self) -> Vec<ToolCall> {
        vec![] // Complete state has no more tools to call
    }
}

// Shared implementations across all states
impl<M, S> TypedReasoningAgent<M, S>
where
    M: MemoryReader + MemoryWriter,
{
    fn parse_result(&self, result: &ExecutionResult) -> (String, f32, Vec<String>) {
        let parsed: Option<RichResult> = serde_json::from_str(result.output()).ok();
        match parsed {
            Some(rr) => (rr.summary, rr.confidence, rr.evidence),
            None => (
                result.output().to_string(),
                self.extract_confidence(result.output()),
                vec![],
            ),
        }
    }

    fn log_step(&self, step: &ReasoningStep) {
        println!(
            "  {} {} (conf {:.2}, evidence {}): {}",
            self.get_step_emoji(&step.step_type),
            step.step_type.to_uppercase(),
            step.confidence,
            step.evidence.len(),
            step.output
        );
        tracing::info!(step=%step.step_type, "step complete");
    }

    fn save_step(&mut self, step: &ReasoningStep) {
        let step_json = serde_json::to_string(step).unwrap_or_default();
        let _ = self.memory.store(MemoryUpdate {
            key: skreaver::memory::MemoryKey::new("last_reasoning_step").expect("Valid memory key"),
            value: step_json,
        });
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
