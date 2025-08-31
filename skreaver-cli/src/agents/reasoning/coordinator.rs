use crate::agents::reasoning::wrapper::ReasoningAgent;
use skreaver::InMemoryToolRegistry;
use skreaver::runtime::Coordinator;

/// Extension trait for reasoning-specific coordinator methods
pub trait ReasoningCoordinatorExt {
    fn is_complete(&self) -> bool;
    fn drive_until_complete(&mut self, max_iters: usize);
}

impl ReasoningCoordinatorExt for Coordinator<ReasoningAgent, InMemoryToolRegistry> {
    fn is_complete(&self) -> bool {
        self.agent.is_complete()
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
