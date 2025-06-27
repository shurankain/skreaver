use crate::agent::Agent;
use crate::tool::ToolRegistry;

pub struct Coordinator<A: Agent, R: ToolRegistry> {
    pub agent: A,
    pub registry: R,
}

impl<A: Agent, R: ToolRegistry> Coordinator<A, R> {
    pub fn new(agent: A, registry: R) -> Self {
        Self { agent, registry }
    }

    pub fn step(&mut self, observation: A::Observation) -> A::Action {
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
}
