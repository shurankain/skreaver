use crate::{Agent, tool::registry::ToolRegistry};

pub struct Coordinator<A: Agent, T: ToolRegistry> {
    pub agent: A,
    pub tools: T,
}

impl<A: Agent, R: ToolRegistry> Coordinator<A, R> {
    pub fn new(agent: A, tools: R) -> Self {
        Self { agent, tools }
    }

    pub fn step(&mut self, observation: A::Observation) -> A::Action {
        self.agent.observe(observation);

        if let Some(tool_call) = self.agent.call_tool() {
            if let Some(result) = self.tools.dispatch(tool_call) {
                self.agent.handle_result(result);
            }
        }

        self.agent.act()
    }
}
