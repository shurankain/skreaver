use crate::Agent;

pub struct Coordinator<A: Agent> {
    pub agent: A,
}

impl<A: Agent> Coordinator<A> {
    pub fn new(agent: A) -> Self {
        Self { agent }
    }

    pub fn step(&mut self, observation: A::Observation) -> A::Action {
        self.agent.observe(observation);
        self.agent.act()
    }
}
