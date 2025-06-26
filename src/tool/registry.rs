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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tool;
    use std::sync::Arc;

    struct UppercaseTool;

    impl Tool for UppercaseTool {
        fn name(&self) -> &str {
            "uppercase"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult {
                output: input.to_uppercase(),
                success: true,
            }
        }
    }

    struct ReverseTool;

    impl Tool for ReverseTool {
        fn name(&self) -> &str {
            "reverse"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult {
                output: input.chars().rev().collect(),
                success: true,
            }
        }
    }

    #[test]
    fn registry_dispatches_to_correct_tool() {
        let registry = InMemoryToolRegistry::new()
            .with_tool("uppercase", Arc::new(UppercaseTool))
            .with_tool("reverse", Arc::new(ReverseTool));

        let upper = registry.dispatch(ToolCall {
            name: "uppercase".into(),
            input: "skreaver".into(),
        });

        let reversed = registry.dispatch(ToolCall {
            name: "reverse".into(),
            input: "skreaver".into(),
        });

        let missing = registry.dispatch(ToolCall {
            name: "nonexistent".into(),
            input: "skreaver".into(),
        });

        assert_eq!(upper.unwrap().output, "SKREAVER");
        assert_eq!(reversed.unwrap().output, "revaerks");
        assert!(missing.is_none());
    }
}
