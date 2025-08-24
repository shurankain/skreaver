use super::config::ReasoningProfile;
use super::states::*;
use super::typestate::TypedReasoningAgent;
use skreaver::agent::Agent;
use skreaver::memory::{Memory, MemoryUpdate};
use skreaver::tool::{ExecutionResult, ToolCall};

/// Agent trait wrapper for backward compatibility
pub struct ReasoningAgentWrapper {
    agent: ReasoningAgentEnum,
}

enum ReasoningAgentEnum {
    Initial(TypedReasoningAgent<Initial>),
    Analyzing(TypedReasoningAgent<Analyzing>),
    Deducing(TypedReasoningAgent<Deducing>),
    Concluding(TypedReasoningAgent<Concluding>),
    Reflecting(TypedReasoningAgent<Reflecting>),
    Complete(TypedReasoningAgent<Complete>),
}

impl ReasoningAgentWrapper {
    pub fn new(memory: Box<dyn Memory + Send>, profile: ReasoningProfile) -> Self {
        Self {
            agent: ReasoningAgentEnum::Initial(TypedReasoningAgent::new(memory, profile)),
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.agent, ReasoningAgentEnum::Complete(_))
    }

    pub fn final_result(&self) -> AgentFinal {
        match &self.agent {
            ReasoningAgentEnum::Complete(agent) => agent.final_result(),
            _ => AgentFinal::InProgress,
        }
    }

    pub fn profile(&self) -> &ReasoningProfile {
        match &self.agent {
            ReasoningAgentEnum::Initial(agent) => &agent.profile,
            ReasoningAgentEnum::Analyzing(agent) => &agent.profile,
            ReasoningAgentEnum::Deducing(agent) => &agent.profile,
            ReasoningAgentEnum::Concluding(agent) => &agent.profile,
            ReasoningAgentEnum::Reflecting(agent) => &agent.profile,
            ReasoningAgentEnum::Complete(agent) => &agent.profile,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(
        memory: Box<dyn Memory + Send>,
        current_problem: Option<String>,
        reasoning_chain: Vec<ReasoningStep>,
        reasoning_state: ReasoningState,
    ) -> Self {
        let profile = ReasoningProfile::default();
        let agent = match reasoning_state {
            ReasoningState::Initial => {
                ReasoningAgentEnum::Initial(TypedReasoningAgent::new(memory, profile))
            }
            ReasoningState::Analyzing => ReasoningAgentEnum::Analyzing(TypedReasoningAgent {
                memory,
                profile,
                state: Analyzing {
                    problem: current_problem.unwrap_or_default(),
                    reasoning_chain,
                },
            }),
            ReasoningState::Deducing => ReasoningAgentEnum::Deducing(TypedReasoningAgent {
                memory,
                profile,
                state: Deducing {
                    problem: current_problem.unwrap_or_default(),
                    reasoning_chain,
                },
            }),
            ReasoningState::Concluding => ReasoningAgentEnum::Concluding(TypedReasoningAgent {
                memory,
                profile,
                state: Concluding {
                    problem: current_problem.unwrap_or_default(),
                    reasoning_chain,
                },
            }),
            ReasoningState::Reflecting => ReasoningAgentEnum::Reflecting(TypedReasoningAgent {
                memory,
                profile,
                state: Reflecting {
                    problem: current_problem.unwrap_or_default(),
                    reasoning_chain,
                },
            }),
            ReasoningState::Complete => ReasoningAgentEnum::Complete(TypedReasoningAgent {
                memory,
                profile,
                state: Complete {
                    problem: current_problem.unwrap_or_default(),
                    reasoning_chain,
                },
            }),
        };
        Self { agent }
    }
}

impl Agent for ReasoningAgentWrapper {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        match std::mem::replace(
            &mut self.agent,
            ReasoningAgentEnum::Initial(TypedReasoningAgent::new(
                Box::new(skreaver::memory::InMemoryMemory::new()),
                ReasoningProfile::default(),
            )),
        ) {
            ReasoningAgentEnum::Initial(agent) => {
                self.agent = ReasoningAgentEnum::Analyzing(agent.observe(input));
            }
            _ => {
                // Reset to initial state with the same memory and profile
                // This is a simplified implementation - in practice you might want to preserve memory
                self.agent = ReasoningAgentEnum::Initial(TypedReasoningAgent::new(
                    Box::new(skreaver::memory::InMemoryMemory::new()),
                    ReasoningProfile::default(),
                ));
                if let ReasoningAgentEnum::Initial(agent) = std::mem::replace(
                    &mut self.agent,
                    ReasoningAgentEnum::Initial(TypedReasoningAgent::new(
                        Box::new(skreaver::memory::InMemoryMemory::new()),
                        ReasoningProfile::default(),
                    )),
                ) {
                    self.agent = ReasoningAgentEnum::Analyzing(agent.observe(input));
                }
            }
        }
    }

    fn act(&mut self) -> Self::Action {
        match &self.agent {
            ReasoningAgentEnum::Complete(agent) => {
                if let Some(last_step) = agent.state.reasoning_chain.last() {
                    let steps_str = agent.state.reasoning_chain.len().to_string();
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
        match &self.agent {
            ReasoningAgentEnum::Initial(_) => vec![], // Should be moved to Analyzing first
            ReasoningAgentEnum::Analyzing(agent) => agent.get_tool_calls(),
            ReasoningAgentEnum::Deducing(agent) => agent.get_tool_calls(),
            ReasoningAgentEnum::Concluding(agent) => agent.get_tool_calls(),
            ReasoningAgentEnum::Reflecting(agent) => agent.get_tool_calls(),
            ReasoningAgentEnum::Complete(agent) => agent.get_tool_calls(),
        }
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if !result.is_success() {
            return;
        }

        let new_agent = match std::mem::replace(
            &mut self.agent,
            ReasoningAgentEnum::Initial(TypedReasoningAgent::new(
                Box::new(skreaver::memory::InMemoryMemory::new()),
                ReasoningProfile::default(),
            )),
        ) {
            ReasoningAgentEnum::Analyzing(agent) => match agent.analyze(result) {
                Ok(deducing_agent) => ReasoningAgentEnum::Deducing(deducing_agent),
                Err(analyzing_agent) => ReasoningAgentEnum::Analyzing(analyzing_agent),
            },
            ReasoningAgentEnum::Deducing(agent) => match agent.deduce(result) {
                Ok(concluding_agent) => ReasoningAgentEnum::Concluding(concluding_agent),
                Err(deducing_agent) => ReasoningAgentEnum::Deducing(deducing_agent),
            },
            ReasoningAgentEnum::Concluding(agent) => match agent.conclude(result) {
                Ok(reflecting_agent) => ReasoningAgentEnum::Reflecting(reflecting_agent),
                Err(concluding_agent) => ReasoningAgentEnum::Concluding(concluding_agent),
            },
            ReasoningAgentEnum::Reflecting(agent) => match agent.reflect(result) {
                Ok(complete_agent) => ReasoningAgentEnum::Complete(complete_agent),
                Err(reflecting_agent) => ReasoningAgentEnum::Reflecting(reflecting_agent),
            },
            other => other, // Initial or Complete - no change
        };

        self.agent = new_agent;
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        match &mut self.agent {
            ReasoningAgentEnum::Initial(agent) => {
                let _ = agent.memory.store(update);
            }
            ReasoningAgentEnum::Analyzing(agent) => {
                let _ = agent.memory.store(update);
            }
            ReasoningAgentEnum::Deducing(agent) => {
                let _ = agent.memory.store(update);
            }
            ReasoningAgentEnum::Concluding(agent) => {
                let _ = agent.memory.store(update);
            }
            ReasoningAgentEnum::Reflecting(agent) => {
                let _ = agent.memory.store(update);
            }
            ReasoningAgentEnum::Complete(agent) => {
                let _ = agent.memory.store(update);
            }
        }
    }

    fn memory(&mut self) -> &mut dyn Memory {
        match &mut self.agent {
            ReasoningAgentEnum::Initial(agent) => &mut *agent.memory,
            ReasoningAgentEnum::Analyzing(agent) => &mut *agent.memory,
            ReasoningAgentEnum::Deducing(agent) => &mut *agent.memory,
            ReasoningAgentEnum::Concluding(agent) => &mut *agent.memory,
            ReasoningAgentEnum::Reflecting(agent) => &mut *agent.memory,
            ReasoningAgentEnum::Complete(agent) => &mut *agent.memory,
        }
    }
}

// Type alias for backward compatibility
pub type ReasoningAgent = ReasoningAgentWrapper;
