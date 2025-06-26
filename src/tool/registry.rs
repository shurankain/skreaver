use super::{ExecutionResult, ToolCall};

pub trait ToolRegistry {
    fn dispatch(&self, call: ToolCall) -> Option<ExecutionResult>;
}

use std::collections::HashMap;
use std::sync::Arc;

pub struct InMemoryToolRegistry {
    tools: HashMap<String, Arc<dyn super::Tool>>,
}

impl Default for InMemoryToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn with_tool(mut self, name: &str, tool: Arc<dyn super::Tool>) -> Self {
        self.tools.insert(name.to_string(), tool);
        self
    }
}

impl super::registry::ToolRegistry for InMemoryToolRegistry {
    fn dispatch(&self, call: ToolCall) -> Option<ExecutionResult> {
        self.tools.get(&call.name).map(|tool| tool.call(call.input))
    }
}
