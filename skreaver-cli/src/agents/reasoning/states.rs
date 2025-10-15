use serde::{Deserialize, Serialize};

/// Represents the different states of the reasoning process.
/// Note: This enum is kept for backward compatibility and serialization.
/// The typestate pattern (Initial, Analyzing, etc. structs) is used for compile-time safety.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReasoningState {
    Initial,
    Analyzing,
    Deducing,
    Concluding,
    Reflecting,
    Complete,
}

/// A single step in the reasoning process.
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

// Typestate pattern - Zero-sized state types for compile-time safety
pub struct Initial;

pub struct Analyzing {
    pub problem: String,
    pub reasoning_chain: Vec<ReasoningStep>,
}

pub struct Deducing {
    pub problem: String,
    pub reasoning_chain: Vec<ReasoningStep>,
}

pub struct Concluding {
    pub problem: String,
    pub reasoning_chain: Vec<ReasoningStep>,
}

pub struct Reflecting {
    pub problem: String,
    pub reasoning_chain: Vec<ReasoningStep>,
}

pub struct Complete {
    #[allow(dead_code)] // Used for future extensibility
    pub problem: String,
    pub reasoning_chain: Vec<ReasoningStep>,
}

/// Final result of reasoning process.
#[derive(Debug, Clone, PartialEq)]
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
