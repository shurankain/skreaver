#[cfg(test)]
mod reasoning_tests {
    use crate::agents::reasoning::*;
    use skreaver::memory::InMemoryMemory;
    use skreaver::tool::{ExecutionResult, Tool};

    struct TestTool {
        json_output: bool,
    }

    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test_tool"
        }

        fn call(&self, input: String) -> ExecutionResult {
            if self.json_output {
                let payload = RichResult {
                    summary: format!("Test summary for: {}", input.trim()),
                    confidence: 0.95,
                    evidence: vec!["test evidence".into()],
                };
                ExecutionResult {
                    output: serde_json::to_string(&payload).unwrap(),
                    success: true,
                }
            } else {
                ExecutionResult {
                    output: format!("Plain text output for: {}", input.trim()),
                    success: true,
                }
            }
        }
    }

    #[test]
    fn test_json_to_summary_parsing() {
        let tool = TestTool { json_output: true };
        let result = tool.call("test input".into());

        // Should be valid JSON
        let parsed: RichResult = serde_json::from_str(&result.output).unwrap();
        assert_eq!(parsed.confidence, 0.95);
        assert_eq!(parsed.evidence, vec!["test evidence"]);
        assert!(parsed.summary.contains("Test summary for: test input"));
    }

    #[test]
    fn test_fallback_to_heuristic() {
        let tool = TestTool { json_output: false };
        let result = tool.call("test input".into());

        // Should fail to parse as JSON
        let parsed: Result<RichResult, _> = serde_json::from_str(&result.output);
        assert!(parsed.is_err());

        // Should contain plain text
        assert!(result.output.contains("Plain text output"));
    }

    #[test]
    fn test_agent_final_result() {
        let agent = ReasoningAgent::new_for_test(
            Box::new(InMemoryMemory::new()),
            Some("test problem".into()),
            vec![
                ReasoningStep::new("analyze", "test", "analysis", 0.8, vec![]),
                ReasoningStep::new("deduce", "test", "deduction", 0.9, vec![]),
            ],
            ReasoningState::Complete,
        );

        match agent.final_result() {
            AgentFinal::Complete { steps, answer } => {
                assert_eq!(steps, 2);
                assert_eq!(answer, "deduction");
            }
            _ => panic!("Should be complete"),
        }
    }

    #[test]
    fn test_agent_incomplete_result() {
        let agent = ReasoningAgent::new_for_test(
            Box::new(InMemoryMemory::new()),
            Some("test problem".into()),
            vec![],
            ReasoningState::Analyzing,
        );

        match agent.final_result() {
            AgentFinal::InProgress => {} // Expected
            _ => panic!("Should be in progress"),
        }
    }

    #[test]
    fn test_fsm_transitions_order() {
        use crate::agents::reasoning::ReasoningCoordinatorExt;
        use skreaver::runtime::Coordinator;
        use skreaver::tool::registry::InMemoryToolRegistry;
        use std::sync::Arc;

        let agent = ReasoningAgent::new_for_test(
            Box::new(InMemoryMemory::new()),
            None,
            vec![],
            ReasoningState::Initial,
        );

        let registry = InMemoryToolRegistry::new()
            .with_tool("analyze", Arc::new(TestTool { json_output: true }))
            .with_tool("deduce", Arc::new(TestTool { json_output: true }))
            .with_tool("conclude", Arc::new(TestTool { json_output: true }))
            .with_tool("reflect", Arc::new(TestTool { json_output: true }));

        let mut coordinator = Coordinator::new(agent, registry);
        coordinator.observe("test problem".to_string());

        // Step 1: Initial -> Analyzing
        assert!(!coordinator.is_complete());
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "analyze");

        if let Some(result) = coordinator.dispatch_tool(tools[0].clone()) {
            coordinator.handle_tool_result(result);
        }

        // Step 2: Analyzing -> Deducing
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "deduce");

        if let Some(result) = coordinator.dispatch_tool(tools[0].clone()) {
            coordinator.handle_tool_result(result);
        }

        // Step 3: Deducing -> Concluding
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "conclude");

        if let Some(result) = coordinator.dispatch_tool(tools[0].clone()) {
            coordinator.handle_tool_result(result);
        }

        // Step 4: Concluding -> Complete
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "reflect");

        if let Some(result) = coordinator.dispatch_tool(tools[0].clone()) {
            coordinator.handle_tool_result(result);
        }

        // Should be complete now
        assert!(coordinator.is_complete());
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 0); // No more tools to call

        // Final result should be Complete with 4 steps
        match coordinator.agent.final_result() {
            AgentFinal::Complete { steps, .. } => assert_eq!(steps, 4),
            _ => panic!("Should be complete with 4 steps"),
        }
    }
}
