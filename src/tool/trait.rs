#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub input: String,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub output: String,
    pub success: bool,
}

pub trait Tool {
    fn name(&self) -> &str;
    fn call(&self, input: String) -> ExecutionResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult {
                output: format!("Echo: {input}"),
                success: true,
            }
        }
    }

    #[test]
    fn tool_can_echo_input() {
        let tool = EchoTool;
        let result = tool.call("Skreaver".into());
        assert_eq!(result.output, "Echo: Skreaver");
        assert!(result.success);
    }

    #[test]
    fn tool_reports_name() {
        let tool = EchoTool;
        assert_eq!(tool.name(), "echo");
    }
}
