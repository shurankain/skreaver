# Skreaver CLI Guide

**Version**: 0.5.0
**Last Updated**: 2025-10-17

## Overview

The Skreaver CLI provides powerful scaffolding tools for creating AI agents and tools. It helps you quickly bootstrap new projects with best-practice templates, reducing boilerplate and setup time.

## Installation

```bash
# From the repository root
cargo build --package skreaver-cli --release

# The binary will be at target/release/skreaver-cli
# Optionally, add it to your PATH
```

## Quick Start

### Create a New Agent

```bash
# Create a simple agent
skreaver new --name my-agent --template simple

# Create a reasoning agent with tool support
skreaver new --name smart-agent --template reasoning

# Create a multi-tool agent with examples
skreaver new --name toolbox-agent --template multi-tool
```

### Generate Tools

```bash
# Generate an HTTP client tool
skreaver generate --type tool --template http-client --output src/tools/http.rs

# Generate a file system tool
skreaver generate --type tool --template filesystem --output src/tools/fs.rs

# Generate an API client with authentication
skreaver generate --type tool --template api-client --output src/tools/api.rs
```

### List Available Templates

```bash
# List all available tool templates
skreaver list --category tools

# List all available agent templates
skreaver list --category agents
```

## Commands Reference

### `new` - Create New Agent

Create a new agent project from a template.

**Usage:**
```bash
skreaver new --name <name> --template <template> [--output <dir>]
```

**Arguments:**
- `--name`: Agent name (required)
- `--template`: Template type - `simple`, `reasoning`, or `multi-tool` (default: `simple`)
- `--output`: Output directory (default: current directory)

**Templates:**

| Template | Description | Use Case |
|----------|-------------|----------|
| `simple` | Basic agent with minimal configuration | Quick prototypes, simple tasks |
| `reasoning` | Agent with tool support and reasoning capabilities | Complex problem-solving, multi-step tasks |
| `multi-tool` | Pre-configured with HTTP client and calculator | API interactions, data processing |

**Example:**
```bash
skreaver new --name weather-agent --template reasoning --output ./agents
```

**Generated Structure:**
```
weather-agent/
├── Cargo.toml          # Project dependencies
├── README.md           # Project documentation
├── .gitignore          # Git ignore rules
└── src/
    ├── main.rs         # Agent entry point
    └── tools/          # Custom tools directory
        └── mod.rs
```

### `generate` - Generate Tool Boilerplate

Generate tool implementations from templates.

**Usage:**
```bash
skreaver generate --type <type> --template <template> --output <path>
```

**Arguments:**
- `--type`: What to generate (currently only `tool` is supported)
- `--template`: Template type (see below)
- `--output`: Output file path (required)

**Tool Templates:**

| Template | Description | Features |
|----------|-------------|----------|
| `http-client` | Simple HTTP client | GET/POST requests, basic error handling |
| `api-client` | Advanced API client | Authentication, rate limiting, custom headers |
| `database` | Database query tool | SQL execution (skeleton only) |
| `filesystem` | File system operations | Read, write, list files with path traversal protection |
| `workflow` | Workflow executor | Multi-step pipelines, variable substitution |
| `custom` | Empty template | Starting point for custom tools |

**Examples:**

```bash
# Generate HTTP client tool
skreaver generate --type tool --template http-client --output src/tools/http.rs

# Generate filesystem tool
skreaver generate --type tool --template filesystem --output src/tools/fs.rs

# Generate workflow tool
skreaver generate --type tool --template workflow --output src/tools/pipeline.rs
```

### `list` - List Available Templates

Display available templates with descriptions.

**Usage:**
```bash
skreaver list --category <category>
```

**Arguments:**
- `--category`: Template category - `tools` or `agents` (default: `tools`)

**Example:**
```bash
# List tool templates
skreaver list --category tools

# List agent templates
skreaver list --category agents
```

### `agent` - Run Example Agents

Run built-in example agents.

**Usage:**
```bash
skreaver agent --name <agent-name>
```

**Available Agents:**
- `echo` - Simple echo agent
- `multi` - Multi-tool agent example
- `reasoning` - Reasoning agent example
- `tools` - Standard tools agent

**Example:**
```bash
skreaver agent --name reasoning
```

### `perf` - Performance Tools

Performance regression detection and benchmarking.

**Usage:**
```bash
skreaver perf <subcommand>
```

**Subcommands:**
- `run` - Run full analysis workflow
- `create-baseline` - Create new performance baselines
- `check` - Check for regressions
- `list` - List all baselines
- `ci` - CI-friendly check (exits with error if regressions found)

See the performance documentation for detailed usage.

## Tool Template Details

### HTTP Client Tool

**Features:**
- Simple GET/POST requests
- JSON response parsing
- Basic error handling

**Usage in Agent:**
```rust
use tools::http_client::HttpClientTool;

let mut tools = ToolRegistry::new();
tools.register(Box::new(HttpClientTool::new()));
```

### API Client Tool

**Features:**
- Bearer token authentication
- Rate limiting (configurable)
- Custom headers support
- Supports GET, POST, PUT, DELETE
- Automatic JSON/text response handling

**Configuration:**
```rust
let tool = ApiClientTool::with_config(
    Some("your-api-key".to_string()),
    10  // max concurrent requests
);
```

### File System Tool

**Features:**
- Read files
- Write files (with directory creation)
- List directory contents
- **Path traversal protection**

**Security:**
All paths are validated to prevent directory traversal attacks.

**Usage:**
```rust
let tool = FileSystemTool::new("./data");  // Base path
tools.register(Box::new(tool));
```

### Workflow Tool

**Features:**
- Multi-step execution
- Variable substitution (`$variable` syntax)
- Context passing between steps
- Supported actions: `log`, `transform`

**Example Workflow:**
```json
{
  "steps": [
    {
      "name": "step1",
      "action": "transform",
      "inputs": {
        "input": "hello",
        "type": "uppercase"
      }
    },
    {
      "name": "step2",
      "action": "log",
      "inputs": {
        "message": "$step1"
      }
    }
  ]
}
```

## Best Practices

### Project Organization

```
my-agent/
├── src/
│   ├── main.rs           # Agent configuration
│   ├── tools/            # Tool implementations
│   │   ├── mod.rs
│   │   ├── http.rs
│   │   └── database.rs
│   └── config/           # Configuration files (optional)
├── tests/                # Integration tests
├── Cargo.toml
└── README.md
```

### Tool Development

1. **Start with a template**: Use `skreaver generate` to create boilerplate
2. **Implement TODOs**: Fill in the marked TODO sections
3. **Add error handling**: Use `ToolError` for consistent error reporting
4. **Write tests**: Test each tool independently
5. **Document parameters**: Use JSON Schema in `parameters()` method

### Agent Configuration

```rust
let config = AgentConfig::default()
    .with_name("my-agent")
    .with_system_prompt(
        "You are a helpful assistant with access to tools. \
         Think step-by-step and use tools when appropriate."
    )
    .with_max_iterations(10)
    .with_timeout(Duration::from_secs(300));
```

## Examples

### Complete Example: Weather Agent

```bash
# 1. Create agent
skreaver new --name weather-agent --template reasoning

# 2. Navigate to project
cd weather-agent

# 3. Generate HTTP client tool
skreaver generate --type tool --template api-client --output src/tools/weather_api.rs

# 4. Edit main.rs to register the tool
# 5. Build and run
cargo build
cargo run
```

### Complete Example: File Processor Agent

```bash
# 1. Create multi-tool agent
skreaver new --name file-processor --template multi-tool

cd file-processor

# 2. Add filesystem tool
skreaver generate --type tool --template filesystem --output src/tools/fs.rs

# 3. Add workflow tool for processing pipeline
skreaver generate --type tool --template workflow --output src/tools/pipeline.rs

# 4. Configure tools in main.rs
# 5. Run
cargo run
```

## Troubleshooting

### Common Issues

**Issue**: "Unknown template" error
**Solution**: Run `skreaver list` to see available templates

**Issue**: Generated tool doesn't compile
**Solution**: Make sure you have all required dependencies in Cargo.toml

**Issue**: Tool not found by agent
**Solution**: Ensure you've registered the tool with `tools.register()`

### Getting Help

```bash
# Get general help
skreaver --help

# Get command-specific help
skreaver new --help
skreaver generate --help
skreaver list --help
```

## Changelog

### v0.5.0 (Current)
- ✨ Added 3 new tool templates: `filesystem`, `api-client`, `workflow`
- ✨ Added `list` command to show available templates
- ✨ Auto-generate README.md and .gitignore for new projects
- 🎨 Improved help messages and examples
- 📝 Added comprehensive CLI documentation

### v0.4.0
- Initial CLI scaffolding
- Basic agent templates (simple, reasoning, multi-tool)
- Tool templates (http-client, database, custom)
- Performance benchmarking tools

## Contributing

To add a new template:

1. Add template variant to `ToolTemplate` or `AgentTemplate` enum
2. Implement template function in `templates.rs`
3. Update the `all()` method to include description
4. Update this documentation

## License

MIT
