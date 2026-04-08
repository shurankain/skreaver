"""
LangChain integration: MCP Tool Definitions → LangChain StructuredTools

Two patterns:
  1. Run a Skreaver McpServer so Claude Desktop (or any MCP client) can
     call Python functions exposed as MCP tools.
  2. Convert Skreaver McpToolDefinition objects into LangChain StructuredTools
     so the same tool descriptions drive both MCP clients and LangChain agents.

Usage:
    # Start MCP server for Claude Desktop:
    python langchain_mcp_server.py serve

    # Show LangChain tools and run demo calls:
    python langchain_mcp_server.py langchain

Requirements:
    pip install langchain-core skreaver
    # For the full agent example at the bottom:
    pip install langchain-openai
"""

import asyncio
import json
import sys
from typing import Any, Callable, Optional, Type

from langchain_core.tools import StructuredTool
from pydantic import BaseModel, Field, create_model

from skreaver.mcp import McpServer, McpToolAnnotations, McpToolDefinition


# ---------------------------------------------------------------------------
# Tool implementations (plain Python functions)
# ---------------------------------------------------------------------------

def web_search(params: dict) -> dict:
    """Search the web for information."""
    query = params.get("query", "")
    # Replace with a real search API in production.
    return {
        "results": [
            {
                "title": f"Result for '{query}'",
                "url": "https://example.com",
                "snippet": "Example snippet...",
            }
        ],
        "total": 1,
    }


def calculate(params: dict) -> dict:
    """Evaluate a mathematical expression."""
    expression = params.get("expression", "")
    allowed_chars = set("0123456789+-*/()., ")
    if not all(c in allowed_chars for c in expression):
        return {"error": "Expression contains invalid characters"}
    try:
        result = eval(expression, {"__builtins__": {}})  # noqa: S307
        return {"result": result, "expression": expression}
    except Exception as exc:
        return {"error": str(exc)}


def get_weather(params: dict) -> dict:
    """Get current weather for a location (mock)."""
    location = params.get("location", "Unknown")
    return {
        "location": location,
        "temperature_c": 22,
        "condition": "Partly cloudy",
        "humidity_pct": 65,
    }


# ---------------------------------------------------------------------------
# Tool registry: definitions paired with handlers
# ---------------------------------------------------------------------------

TOOL_REGISTRY: list[tuple[McpToolDefinition, Callable[[dict], Any]]] = [
    (
        McpToolDefinition(
            "web_search",
            "Search the web for up-to-date information on any topic",
            {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"},
                },
                "required": ["query"],
            },
        ).with_annotations(McpToolAnnotations().with_read_only(True)),
        web_search,
    ),
    (
        McpToolDefinition(
            "calculate",
            "Evaluate a mathematical expression (e.g. '2 ** 10 + 3 * 5')",
            {
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "Arithmetic expression using +, -, *, /, **, ()",
                    },
                },
                "required": ["expression"],
            },
        ).with_annotations(
            McpToolAnnotations().with_read_only(True).with_idempotent(True)
        ),
        calculate,
    ),
    (
        McpToolDefinition(
            "get_weather",
            "Get the current weather conditions for a city or location",
            {
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name or coordinates",
                    },
                },
                "required": ["location"],
            },
        ).with_annotations(McpToolAnnotations().with_read_only(True)),
        get_weather,
    ),
]


# ---------------------------------------------------------------------------
# Pattern 1: MCP Server (Claude Desktop / any MCP client)
# ---------------------------------------------------------------------------

def build_mcp_server() -> McpServer:
    """Build a Skreaver McpServer from the shared tool registry."""
    server = McpServer("langchain-tools-server", "1.0.0")
    for definition, handler in TOOL_REGISTRY:
        server.add_tool(
            definition.name,
            definition.description,
            handler,
            definition.input_schema,
        )
    return server


async def run_mcp_server() -> None:
    """Start the MCP server and serve over stdio (blocks until disconnected)."""
    server = build_mcp_server()
    print(
        f"Starting MCP server '{server.name}' with tools: {server.list_tools()}",
        flush=True,
    )
    await server.serve_stdio()


# ---------------------------------------------------------------------------
# Pattern 2: McpToolDefinition → LangChain StructuredTool
# ---------------------------------------------------------------------------

_JSON_TO_PYTHON: dict[str, type] = {
    "string": str,
    "number": float,
    "integer": int,
    "boolean": bool,
}


def mcp_definition_to_langchain(
    definition: McpToolDefinition,
    handler: Callable[[dict], Any],
) -> StructuredTool:
    """Convert a Skreaver McpToolDefinition into a LangChain StructuredTool.

    Builds a Pydantic input model from the JSON Schema so LangChain can
    validate and parse tool arguments automatically.
    """
    schema = definition.input_schema
    properties: dict = schema.get("properties", {})
    required: set[str] = set(schema.get("required", []))

    field_defs: dict[str, Any] = {}
    for prop_name, prop_schema in properties.items():
        description = prop_schema.get("description", "")
        python_type = _JSON_TO_PYTHON.get(prop_schema.get("type", "string"), str)
        if prop_name in required:
            field_defs[prop_name] = (python_type, Field(description=description))
        else:
            field_defs[prop_name] = (
                Optional[python_type],
                Field(default=None, description=description),
            )

    input_model: Type[BaseModel] = create_model(
        f"{definition.name.title().replace('_', '')}Input",
        **field_defs,
    )

    def run(**kwargs: Any) -> str:
        result = handler(kwargs)
        return json.dumps(result) if isinstance(result, dict) else str(result)

    return StructuredTool(
        name=definition.name,
        description=definition.description,
        args_schema=input_model,
        func=run,
    )


def build_langchain_tools() -> list[StructuredTool]:
    """Build LangChain StructuredTools from the shared tool registry."""
    return [
        mcp_definition_to_langchain(definition, handler)
        for definition, handler in TOOL_REGISTRY
    ]


# ---------------------------------------------------------------------------
# Demo
# ---------------------------------------------------------------------------

def run_langchain_demo() -> None:
    tools = build_langchain_tools()

    print(f"Built {len(tools)} LangChain tools from MCP definitions:\n")
    for tool in tools:
        schema_fields = list(tool.args_schema.model_fields.keys())
        print(f"  {tool.name}: {tool.description}")
        print(f"    inputs: {schema_fields}")

    print("\nDirect invocations:")
    calc = next(t for t in tools if t.name == "calculate")
    print(f"  calculate('2 ** 10') = {calc.run({'expression': '2 ** 10'})}")

    weather = next(t for t in tools if t.name == "get_weather")
    print(f"  get_weather('Berlin') = {weather.run({'location': 'Berlin'})}")

    search = next(t for t in tools if t.name == "web_search")
    print(f"  web_search('Skreaver') = {search.run({'query': 'Skreaver'})}")

    # --- LangChain agent integration ---
    #
    # from langchain_openai import ChatOpenAI
    # from langchain.agents import create_tool_calling_agent, AgentExecutor
    # from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
    #
    # llm = ChatOpenAI(model="gpt-4o")
    # prompt = ChatPromptTemplate.from_messages([
    #     ("system", "You are a helpful assistant. Use tools when needed."),
    #     ("human", "{input}"),
    #     MessagesPlaceholder("agent_scratchpad"),
    # ])
    # agent = create_tool_calling_agent(llm, tools, prompt)
    # executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
    # result = executor.invoke({"input": "What is 2 to the power of 16, and what's the weather in Tokyo?"})
    # print(result["output"])


if __name__ == "__main__":
    command = sys.argv[1] if len(sys.argv) > 1 else "langchain"
    if command == "serve":
        asyncio.run(run_mcp_server())
    else:
        run_langchain_demo()
