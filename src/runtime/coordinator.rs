use crate::{Agent, Tool};

pub struct Coordinator<A: Agent, T: Tool> {
    pub agent: A,
    pub tool: T,
}

impl<A: Agent, T: Tool> Coordinator<A, T> {
    pub fn new(agent: A, tool: T) -> Self {
        Self { agent, tool }
    }
    pub fn step(&mut self, observation: A::Observation) -> A::Action {
        self.agent.observe(observation);

        if let Some(tool_call) = self.agent.call_tool() {
            if tool_call.name == self.tool.name() {
                let result = self.tool.call(tool_call.input);
                self.agent.handle_result(result);
            } else {
                eprintln!("Unknown tool: {}", tool_call.name);
            }
        }

        self.agent.act()
    }
}
