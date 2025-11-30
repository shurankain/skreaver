#[cfg(test)]
mod tests {
    use crate::agents::reasoning::rich_result::RichResult;
    use crate::agents::reasoning::states::{AgentFinal, ReasoningState, ReasoningStep};
    use crate::agents::reasoning::tools::{AnalyzeTool, ConcludeTool, DeduceTool, ReflectTool};
    use crate::agents::reasoning::wrapper::ReasoningAgentWrapper;
    use skreaver::InMemoryMemory;
    use skreaver::InMemoryToolRegistry;
    use skreaver::runtime::Coordinator;
    use skreaver::{ExecutionResult, Tool};
    use std::sync::Arc;

    struct TestTool {
        json_output: bool,
    }

    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test"
        }

        fn call(&self, input: String) -> ExecutionResult {
            if self.json_output {
                let payload = RichResult {
                    summary: format!("Test summary for: {}", input.trim()),
                    confidence: 0.95,
                    evidence: vec!["test evidence".into()],
                };
                ExecutionResult::success(serde_json::to_string(&payload).unwrap())
            } else {
                ExecutionResult::success(format!("Plain text output for: {}", input.trim()))
            }
        }
    }

    #[test]
    fn test_json_to_summary_parsing() {
        let tool = TestTool { json_output: true };
        let result = tool.call("test input".into());

        // Should be valid JSON
        let parsed: RichResult = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(parsed.confidence, 0.95);
        assert_eq!(parsed.evidence, vec!["test evidence"]);
        assert!(parsed.summary.contains("Test summary for: test input"));
    }

    #[test]
    fn test_fallback_to_heuristic() {
        let tool = TestTool { json_output: false };
        let result = tool.call("test input".into());

        // Should fail to parse as JSON
        let parsed: Result<RichResult, _> = serde_json::from_str(&result.output());
        assert!(parsed.is_err());

        // Should contain plain text
        assert!(result.output().contains("Plain text output"));
    }

    #[test]
    fn test_agent_final_result() {
        let memory = Box::new(InMemoryMemory::new());
        let agent = ReasoningAgentWrapper::new_for_test(
            *memory,
            Some("test problem".into()),
            vec![ReasoningStep::new(
                "reflect",
                "test",
                "final answer",
                0.9,
                vec![],
            )],
            ReasoningState::Complete,
        );

        match agent.final_result() {
            AgentFinal::Complete { steps, answer } => {
                assert_eq!(steps, 1);
                assert_eq!(answer, "final answer");
            }
            _ => panic!("Should be complete"),
        }
    }

    #[test]
    fn test_agent_incomplete_result() {
        let memory = Box::new(InMemoryMemory::new());
        let agent = ReasoningAgentWrapper::new_for_test(
            *memory,
            Some("test problem".into()),
            vec![],
            ReasoningState::Initial,
        );

        assert_eq!(agent.final_result(), AgentFinal::InProgress);
    }

    #[test]
    fn test_fsm_transitions_order() {
        let agent = ReasoningAgentWrapper::new_for_test(
            InMemoryMemory::new(),
            None,
            vec![],
            ReasoningState::Initial,
        );

        let registry = InMemoryToolRegistry::new()
            .with_tool("analyze", Arc::new(AnalyzeTool))
            .with_tool("deduce", Arc::new(DeduceTool))
            .with_tool("conclude", Arc::new(ConcludeTool))
            .with_tool("reflect", Arc::new(ReflectTool));

        let mut coordinator = Coordinator::new(agent, registry);

        // Start: Initial -> Analyzing
        coordinator.observe("test problem".to_string());
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "analyze");

        if let Some(result) = coordinator.dispatch_tool(tools[0].clone()) {
            coordinator.handle_tool_result(result);
        }

        // Step 2: Analyzing -> Deducing
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "deduce");

        if let Some(result) = coordinator.dispatch_tool(tools[0].clone()) {
            coordinator.handle_tool_result(result);
        }

        // Step 3: Deducing -> Concluding
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "conclude");

        if let Some(result) = coordinator.dispatch_tool(tools[0].clone()) {
            coordinator.handle_tool_result(result);
        }

        // Step 4: Concluding -> Complete
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "reflect");

        if let Some(result) = coordinator.dispatch_tool(tools[0].clone()) {
            coordinator.handle_tool_result(result);
        }

        // Should be complete now
        assert!(coordinator.agent.is_complete());
        let tools = coordinator.get_tool_calls();
        assert_eq!(tools.len(), 0); // No more tools to call

        // Final result should be Complete with 4 steps
        match coordinator.agent.final_result() {
            AgentFinal::Complete { steps, .. } => assert_eq!(steps, 4),
            _ => panic!("Should be complete with 4 steps"),
        }
    }
}

#[cfg(test)]
mod builder_tests {
    use crate::agents::reasoning::config::ReasoningProfile;
    use crate::agents::reasoning::rich_result::RichResult;

    #[test]
    fn test_reasoning_profile_default() {
        let profile = ReasoningProfile::default();

        assert_eq!(profile.max_loop_iters, 16);
        assert_eq!(profile.max_prev_output, 1024);
        assert_eq!(profile.max_chain_line, 512);
        assert_eq!(profile.max_chain_summary, 2048);
    }

    #[test]
    fn test_reasoning_profile_presets() {
        let fast = ReasoningProfile::fast();
        assert_eq!(fast.max_loop_iters, 8);
        assert_eq!(fast.max_prev_output, 512);

        let comprehensive = ReasoningProfile::comprehensive();
        assert_eq!(comprehensive.max_loop_iters, 32);
        assert_eq!(comprehensive.max_prev_output, 4096);
    }

    #[test]
    fn test_rich_result_creation() {
        let result = RichResult {
            summary: "Test summary".to_string(),
            confidence: 0.85,
            evidence: vec!["Evidence 1".to_string(), "Evidence 2".to_string()],
        };

        assert_eq!(result.summary, "Test summary");
        assert_eq!(result.confidence, 0.85);
        assert_eq!(result.evidence.len(), 2);
        assert_eq!(result.evidence[0], "Evidence 1");
        assert_eq!(result.evidence[1], "Evidence 2");
    }

    #[test]
    fn test_rich_result_presets() {
        let high =
            RichResult::high_confidence("High conf".to_string(), vec!["Evidence".to_string()]);
        assert_eq!(high.confidence, 0.9);

        let medium =
            RichResult::medium_confidence("Med conf".to_string(), vec!["Evidence".to_string()]);
        assert_eq!(medium.confidence, 0.7);

        let low = RichResult::low_confidence("Low conf".to_string(), vec!["Evidence".to_string()]);
        assert_eq!(low.confidence, 0.4);
    }
}
