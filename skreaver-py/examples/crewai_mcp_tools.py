"""
CrewAI integration: Skreaver MCP Tool Definitions as CrewAI Tools

Defines tools once as Skreaver McpToolDefinitions and converts them
into CrewAI BaseTool instances, so the same definitions can serve
both MCP clients (Claude Desktop) and CrewAI agents.

Usage:
    python crewai_mcp_tools.py

Requirements:
    pip install crewai skreaver
"""

import json
from typing import Any, Callable, Optional, Type

from crewai.tools import BaseTool
from pydantic import BaseModel, Field, create_model

from skreaver.mcp import McpToolAnnotations, McpToolDefinition


# ---------------------------------------------------------------------------
# Tool implementations
# ---------------------------------------------------------------------------

def web_search(params: dict) -> dict:
    """Search the web for information."""
    query = params.get("query", "")
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
# Shared tool registry: (McpToolDefinition, handler)
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
# McpToolDefinition -> CrewAI BaseTool converter
# ---------------------------------------------------------------------------

_JSON_TO_PYTHON: dict[str, type] = {
    "string": str,
    "number": float,
    "integer": int,
    "boolean": bool,
}


def mcp_definition_to_crewai(
    definition: McpToolDefinition,
    handler: Callable[[dict], Any],
) -> BaseTool:
    """Convert a Skreaver McpToolDefinition into a CrewAI BaseTool.

    Builds a Pydantic input model from the JSON Schema so CrewAI
    validates and parses tool arguments automatically.
    """
    schema = definition.input_schema
    properties: dict = schema.get("properties", {})
    required: set[str] = set(schema.get("required", []))

    field_defs: dict[str, Any] = {}
    for prop_name, prop_schema in properties.items():
        desc = prop_schema.get("description", "")
        python_type = _JSON_TO_PYTHON.get(prop_schema.get("type", "string"), str)
        if prop_name in required:
            field_defs[prop_name] = (python_type, Field(description=desc))
        else:
            field_defs[prop_name] = (
                Optional[python_type],
                Field(default=None, description=desc),
            )

    input_model: Type[BaseModel] = create_model(
        f"{definition.name.title().replace('_', '')}Input",
        **field_defs,
    )

    # Build the subclass dynamically
    tool_name = definition.name
    tool_desc = definition.description

    class McpCrewTool(BaseTool):
        name: str = tool_name
        description: str = tool_desc
        args_schema: Type[BaseModel] = input_model

        def _run(self, **kwargs: Any) -> str:
            result = handler(kwargs)
            return json.dumps(result) if isinstance(result, dict) else str(result)

    # Give the class a readable name for debugging
    McpCrewTool.__name__ = f"McpCrewTool_{tool_name}"
    McpCrewTool.__qualname__ = McpCrewTool.__name__
    return McpCrewTool()


def build_crewai_tools() -> list[BaseTool]:
    """Build CrewAI tools from the shared MCP tool registry."""
    return [
        mcp_definition_to_crewai(definition, handler)
        for definition, handler in TOOL_REGISTRY
    ]


# ---------------------------------------------------------------------------
# Demo
# ---------------------------------------------------------------------------

def demo() -> None:
    tools = build_crewai_tools()

    print(f"Built {len(tools)} CrewAI tools from MCP definitions:\n")
    for tool in tools:
        schema_fields = list(tool.args_schema.model_fields.keys())
        print(f"  {tool.name}: {tool.description}")
        print(f"    inputs: {schema_fields}")

    print("\nDirect invocations:")
    calc = next(t for t in tools if t.name == "calculate")
    print(f"  calculate('2 ** 10') = {calc.run(expression='2 ** 10')}")

    weather = next(t for t in tools if t.name == "get_weather")
    print(f"  get_weather('Berlin') = {weather.run(location='Berlin')}")

    search = next(t for t in tools if t.name == "web_search")
    print(f"  web_search('Skreaver') = {search.run(query='Skreaver')}")

    # --- CrewAI agent + crew integration ---
    #
    # from crewai import Agent, Task, Crew, Process
    #
    # researcher = Agent(
    #     role="Research Analyst",
    #     goal="Find accurate, up-to-date information",
    #     backstory="Expert researcher with strong analytical skills.",
    #     tools=tools,
    #     llm="gpt-4o",
    #     verbose=True,
    # )
    #
    # task = Task(
    #     description="Research {topic} and provide a summary with key statistics.",
    #     expected_output="A concise summary with at least 3 key data points.",
    #     agent=researcher,
    # )
    #
    # crew = Crew(
    #     agents=[researcher],
    #     tasks=[task],
    #     process=Process.sequential,
    #     verbose=True,
    # )
    #
    # result = crew.kickoff(inputs={"topic": "renewable energy trends 2026"})
    # print(result.raw)


if __name__ == "__main__":
    demo()
