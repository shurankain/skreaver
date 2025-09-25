//! # Golden Test Macros
//!
//! This module provides convenient macros for creating golden tests with minimal boilerplate.

/// Create a golden test scenario for a standard tool
///
/// # Examples
///
/// ```rust
/// use skreaver_testing::golden_test;
/// use skreaver_core::StandardTool;
///
/// let scenario = golden_test!(
///     test_id: "http_get_api",
///     tool: StandardTool::HttpGet,
///     input: "https://api.example.com/data",
///     description: "Test HTTP GET to API endpoint"
/// );
/// ```
#[macro_export]
macro_rules! golden_test {
    (
        test_id: $test_id:expr,
        tool: $tool:expr,
        input: $input:expr
    ) => {
        $crate::golden_harness::GoldenTestScenario::for_standard_tool($test_id, $tool, $input)
    };

    (
        test_id: $test_id:expr,
        tool: $tool:expr,
        input: $input:expr,
        description: $description:expr
    ) => {
        $crate::golden_harness::GoldenTestScenario::for_standard_tool($test_id, $tool, $input)
            .with_description($description)
    };

    (
        test_id: $test_id:expr,
        tool: $tool:expr,
        input: $input:expr,
        expect_failure: true
    ) => {
        $crate::golden_harness::GoldenTestScenario::for_standard_tool($test_id, $tool, $input)
            .expect_failure()
    };

    (
        test_id: $test_id:expr,
        tool: $tool:expr,
        input: $input:expr,
        description: $description:expr,
        expect_failure: true
    ) => {
        $crate::golden_harness::GoldenTestScenario::for_standard_tool($test_id, $tool, $input)
            .with_description($description)
            .expect_failure()
    };
}

/// Create a golden test scenario for a custom tool
///
/// # Examples
///
/// ```rust
/// use skreaver_testing::golden_custom_test;
///
/// let scenario = golden_custom_test!(
///     test_id: "my_custom_tool_test",
///     tool_name: "my_custom_tool",
///     input: "test input data",
///     description: "Test custom tool functionality"
/// ).expect("Valid tool name");
/// ```
#[macro_export]
macro_rules! golden_custom_test {
    (
        test_id: $test_id:expr,
        tool_name: $tool_name:expr,
        input: $input:expr
    ) => {
        $crate::golden_harness::GoldenTestScenario::for_custom_tool($test_id, $tool_name, $input)
    };

    (
        test_id: $test_id:expr,
        tool_name: $tool_name:expr,
        input: $input:expr,
        description: $description:expr
    ) => {
        $crate::golden_harness::GoldenTestScenario::for_custom_tool($test_id, $tool_name, $input)
            .map(|s| s.with_description($description))
    };

    (
        test_id: $test_id:expr,
        tool_name: $tool_name:expr,
        input: $input:expr,
        expect_failure: true
    ) => {
        $crate::golden_harness::GoldenTestScenario::for_custom_tool($test_id, $tool_name, $input)
            .map(|s| s.expect_failure())
    };
}

/// Create multiple golden test scenarios for testing all standard tools
///
/// # Examples
///
/// ```rust
/// use skreaver_testing::golden_test_suite;
/// use skreaver_core::StandardTool;
///
/// let scenarios = golden_test_suite!(
///     http_tools: {
///         "http_get_basic" => (StandardTool::HttpGet, "https://httpbin.org/get"),
///         "http_post_json" => (StandardTool::HttpPost, r#"{"data": "test"}"#),
///     },
///     file_tools: {
///         "file_read_test" => (StandardTool::FileRead, "/tmp/test.txt"),
///         "file_write_test" => (StandardTool::FileWrite, "test content"),
///     }
/// );
/// ```
#[macro_export]
macro_rules! golden_test_suite {
    (
        $($suite_name:ident: {
            $($test_id:literal => ($tool:expr, $input:expr)),* $(,)?
        }),* $(,)?
    ) => {
        {
            let mut scenarios = Vec::new();
            $(
                $(
                    scenarios.push($crate::golden_harness::GoldenTestScenario::for_standard_tool(
                        format!("{}_{}", stringify!($suite_name), $test_id),
                        $tool,
                        $input
                    ).with_description(format!("{} suite: {}", stringify!($suite_name), $test_id)));
                )*
            )*
            scenarios
        }
    };
}

/// Create a golden test harness with standard configuration
///
/// # Examples
///
/// ```rust
/// use skreaver_testing::golden_harness;
///
/// let harness = golden_harness!(
///     snapshot_dir: "tests/golden",
///     auto_update: false,
///     normalize_outputs: true
/// );
/// ```
#[macro_export]
macro_rules! golden_harness {
    () => {
        $crate::golden_harness::GoldenTestHarnessBuilder::new().build_with_mocks()
    };

    (
        snapshot_dir: $dir:expr
    ) => {
        $crate::golden_harness::GoldenTestHarnessBuilder::new()
            .snapshot_dir($dir)
            .build_with_mocks()
    };

    (
        snapshot_dir: $dir:expr,
        auto_update: $auto_update:expr
    ) => {
        $crate::golden_harness::GoldenTestHarnessBuilder::new()
            .snapshot_dir($dir)
            .auto_update($auto_update)
            .build_with_mocks()
    };

    (
        snapshot_dir: $dir:expr,
        auto_update: $auto_update:expr,
        normalize_outputs: $normalize:expr
    ) => {
        $crate::golden_harness::GoldenTestHarnessBuilder::new()
            .snapshot_dir($dir)
            .auto_update($auto_update)
            .normalize_outputs($normalize)
            .build_with_mocks()
    };

    (
        snapshot_dir: $dir:expr,
        auto_update: $auto_update:expr,
        normalize_outputs: $normalize:expr,
        validate_timing: $timing:expr
    ) => {
        $crate::golden_harness::GoldenTestHarnessBuilder::new()
            .snapshot_dir($dir)
            .auto_update($auto_update)
            .normalize_outputs($normalize)
            .validate_timing($timing)
            .build_with_mocks()
    };
}

/// Run a complete golden test with harness setup and execution
///
/// # Examples
///
/// ```rust,no_run
/// use skreaver_testing::{run_golden_test, golden_test};
/// use skreaver_core::StandardTool;
///
/// run_golden_test!(
///     harness_config: {
///         snapshot_dir: "tests/golden",
///         auto_update: false,
///     },
///     scenarios: [
///         golden_test!(
///             test_id: "http_get_test",
///             tool: StandardTool::HttpGet,
///             input: "https://httpbin.org/get"
///         ),
///         golden_test!(
///             test_id: "json_parse_test",
///             tool: StandardTool::JsonParse,
///             input: r#"{"key": "value"}"#
///         )
///     ]
/// );
/// ```
#[macro_export]
macro_rules! run_golden_test {
    (
        harness_config: {
            $($config_key:ident: $config_value:expr),* $(,)?
        },
        scenarios: [
            $($scenario:expr),* $(,)?
        ]
    ) => {
        {
            let mut harness = $crate::golden_harness::GoldenTestHarnessBuilder::new()
                $(.$config_key($config_value))*
                .build_with_mocks()
                .expect("Failed to create golden test harness");

            let scenarios = vec![$($scenario),*];
            let results = harness.run_golden_scenarios(scenarios)
                .expect("Failed to run golden test scenarios");

            harness.print_results();
            results
        }
    };
}

/// Assert that a golden test passes
///
/// # Examples
///
/// ```rust
/// use skreaver_testing::{assert_golden_passes, golden_test};
/// use skreaver_core::StandardTool;
///
/// assert_golden_passes!(
///     golden_test!(
///         test_id: "json_parse_valid",
///         tool: StandardTool::JsonParse,
///         input: r#"{"valid": "json"}"#
///     ),
///     snapshot_dir: "tests/golden"
/// );
/// ```
#[macro_export]
macro_rules! assert_golden_passes {
    ($scenario:expr, snapshot_dir: $dir:expr) => {{
        let mut harness = $crate::golden_harness::GoldenTestHarnessBuilder::new()
            .snapshot_dir($dir)
            .build_with_mocks()
            .expect("Failed to create golden test harness");

        let result = harness
            .run_golden_test(&$scenario.test_id, $scenario.tool_call)
            .expect("Failed to run golden test");

        assert!(
            result.passed,
            "Golden test failed: {}",
            result.scenario_name
        );
    }};
}

/// Assert that a golden test fails
///
/// # Examples
///
/// ```rust,no_run
/// use skreaver_testing::{assert_golden_fails, golden_test};
/// use skreaver_core::StandardTool;
///
/// assert_golden_fails!(
///     golden_test!(
///         test_id: "json_parse_invalid",
///         tool: StandardTool::JsonParse,
///         input: "invalid json",
///         expect_failure: true
///     ),
///     snapshot_dir: "tests/golden"
/// );
/// ```
#[macro_export]
macro_rules! assert_golden_fails {
    ($scenario:expr, snapshot_dir: $dir:expr) => {{
        let mut harness = $crate::golden_harness::GoldenTestHarnessBuilder::new()
            .snapshot_dir($dir)
            .build_with_mocks()
            .expect("Failed to create golden test harness");

        let result = harness
            .run_golden_test(&$scenario.test_id, $scenario.tool_call)
            .expect("Failed to run golden test");

        assert!(
            !result.passed,
            "Golden test unexpectedly passed: {}",
            result.scenario_name
        );
    }};
}

/// Create standard tool test inputs for comprehensive testing
///
/// # Examples
///
/// ```rust
/// use skreaver_testing::standard_tool_inputs;
/// use skreaver_core::StandardTool;
/// use std::collections::HashMap;
///
/// let inputs = standard_tool_inputs!();
/// // Creates HashMap<StandardTool, Vec<String>> with default test inputs for all tools
///
/// let custom_inputs = standard_tool_inputs!(
///     HttpGet => ["https://httpbin.org/get", "https://api.example.com/test"],
///     JsonParse => [r#"{"key": "value"}"#, r#"[1, 2, 3]"#],
/// );
/// ```
#[macro_export]
macro_rules! standard_tool_inputs {
    () => {
        {
            use std::collections::HashMap;
            use $crate::StandardTool;

            let mut inputs = HashMap::new();

            // HTTP tools
            inputs.insert(StandardTool::HttpGet, vec![
                "https://httpbin.org/get".to_string(),
                "https://api.github.com".to_string(),
            ]);

            inputs.insert(StandardTool::HttpPost, vec![
                r#"{"test": "data"}"#.to_string(),
                "form data".to_string(),
            ]);

            inputs.insert(StandardTool::HttpPut, vec![
                r#"{"update": "data"}"#.to_string(),
            ]);

            inputs.insert(StandardTool::HttpDelete, vec![
                "resource_id_123".to_string(),
            ]);

            // File tools
            inputs.insert(StandardTool::FileRead, vec![
                "/tmp/test.txt".to_string(),
                "test_file.json".to_string(),
            ]);

            inputs.insert(StandardTool::FileWrite, vec![
                "test content".to_string(),
                r#"{"json": "content"}"#.to_string(),
            ]);

            inputs.insert(StandardTool::DirectoryList, vec![
                "/tmp".to_string(),
                ".".to_string(),
            ]);

            inputs.insert(StandardTool::DirectoryCreate, vec![
                "/tmp/test_dir".to_string(),
                "new_directory".to_string(),
            ]);

            // Data processing tools
            inputs.insert(StandardTool::JsonParse, vec![
                r#"{"key": "value"}"#.to_string(),
                r#"[1, 2, 3, 4, 5]"#.to_string(),
                r#"{"nested": {"object": true}}"#.to_string(),
            ]);

            inputs.insert(StandardTool::JsonTransform, vec![
                r#"{"transform": "this"}"#.to_string(),
            ]);

            inputs.insert(StandardTool::XmlParse, vec![
                r#"<root><item>value</item></root>"#.to_string(),
            ]);

            // Text tools
            inputs.insert(StandardTool::TextAnalyze, vec![
                "This is a sample text for analysis.".to_string(),
                "Short".to_string(),
                "Very long text that should be analyzed for various metrics like word count, character count, and other statistical information.".to_string(),
            ]);

            inputs.insert(StandardTool::TextReverse, vec![
                "hello world".to_string(),
                "racecar".to_string(),
            ]);

            inputs.insert(StandardTool::TextSearch, vec![
                "search pattern".to_string(),
                "find this text".to_string(),
            ]);

            inputs.insert(StandardTool::TextSplit, vec![
                "split,this,text".to_string(),
                "word1 word2 word3".to_string(),
            ]);

            inputs.insert(StandardTool::TextUppercase, vec![
                "convert to uppercase".to_string(),
                "MiXeD CaSe TeXt".to_string(),
            ]);

            inputs
        }
    };

    (
        $($tool:ident => [$($input:expr),* $(,)?]),* $(,)?
    ) => {
        {
            use std::collections::HashMap;
            use $crate::StandardTool;

            let mut inputs = HashMap::new();

            $(
                inputs.insert(StandardTool::$tool, vec![
                    $($input.to_string()),*
                ]);
            )*

            inputs
        }
    };
}

#[cfg(test)]
mod tests {
    use skreaver_core::StandardTool;

    #[test]
    fn test_golden_test_macro() {
        let scenario = golden_test!(
            test_id: "test_macro",
            tool: StandardTool::JsonParse,
            input: r#"{"test": "value"}"#
        );

        assert_eq!(scenario.test_id, "test_macro");
        assert_eq!(scenario.tool_call.name(), "json_parse");
    }

    #[test]
    fn test_golden_test_with_description() {
        let scenario = golden_test!(
            test_id: "test_with_desc",
            tool: StandardTool::HttpGet,
            input: "https://example.com",
            description: "Test HTTP GET functionality"
        );

        assert!(scenario.description.is_some());
        assert_eq!(scenario.description.unwrap(), "Test HTTP GET functionality");
    }

    #[test]
    fn test_golden_test_suite_macro() {
        let scenarios = golden_test_suite!(
            http_tools: {
                "get_test" => (StandardTool::HttpGet, "https://example.com"),
                "post_test" => (StandardTool::HttpPost, "data"),
            },
            json_tools: {
                "parse_test" => (StandardTool::JsonParse, r#"{"key": "value"}"#),
            }
        );

        assert_eq!(scenarios.len(), 3);
        assert!(scenarios.iter().any(|s| s.test_id == "http_tools_get_test"));
        assert!(
            scenarios
                .iter()
                .any(|s| s.test_id == "http_tools_post_test")
        );
        assert!(
            scenarios
                .iter()
                .any(|s| s.test_id == "json_tools_parse_test")
        );
    }

    #[test]
    fn test_standard_tool_inputs_macro() {
        let inputs = standard_tool_inputs!();
        assert!(!inputs.is_empty());
        assert!(inputs.contains_key(&StandardTool::HttpGet));
        assert!(inputs.contains_key(&StandardTool::JsonParse));
    }

    #[test]
    fn test_custom_standard_tool_inputs() {
        let inputs = standard_tool_inputs!(
            HttpGet => ["https://test.com"],
            JsonParse => [r#"{"test": true}"#, r#"[1,2,3]"#]
        );

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[&StandardTool::HttpGet].len(), 1);
        assert_eq!(inputs[&StandardTool::JsonParse].len(), 2);
    }
}
