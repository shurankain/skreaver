use crate::agent::Agent;
use crate::memory::MemoryUpdate;
use crate::tool::ToolRegistry;
use std::fmt::Display;

pub struct Coordinator<A: Agent, R: ToolRegistry>
where
    A::Observation: Display,
{
    pub agent: A,
    pub registry: R,
}

impl<A: Agent, R: ToolRegistry> Coordinator<A, R>
where
    A::Observation: Display,
{
    pub fn new(agent: A, registry: R) -> Self {
        Self { agent, registry }
    }

    pub fn step(&mut self, observation: A::Observation) -> A::Action {
        self.agent.memory().store(MemoryUpdate {
            key: "input".to_string(),
            value: format!("{observation}"),
        });

        self.agent.observe(observation);

        for tool_call in self.agent.call_tools() {
            if let Some(result) = self.registry.dispatch(tool_call) {
                self.agent.handle_result(result);
            } else {
                eprintln!("Tool not found in registry");
            }
        }

        self.agent.act()
    }

    pub fn update_context(&mut self, update: MemoryUpdate) {
        self.agent.memory().store(update.clone());
        self.agent.update_context(update);
    }

    // Wrapper methods for encapsulation
    pub fn observe(&mut self, observation: A::Observation) {
        self.agent.observe(observation);
    }

    pub fn get_tool_calls(&self) -> Vec<crate::tool::ToolCall> {
        self.agent.call_tools()
    }

    pub fn dispatch_tool(
        &self,
        tool_call: crate::tool::ToolCall,
    ) -> Option<crate::tool::ExecutionResult> {
        self.registry.dispatch(tool_call)
    }

    pub fn handle_tool_result(&mut self, result: crate::tool::ExecutionResult) {
        self.agent.handle_result(result);
    }

    pub fn get_action(&mut self) -> A::Action {
        self.agent.act()
    }
}
