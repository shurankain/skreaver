"""
LangChain integration: A2A Agent as a Tool

Wraps a Skreaver A2A agent as a LangChain BaseTool, so any LangChain
agent or chain can delegate tasks to an A2A-compatible agent.

Usage:
    python langchain_a2a_tool.py

Requirements:
    pip install langchain-core skreaver
    # For the full agent example at the bottom:
    pip install langchain-openai
"""

import asyncio
from typing import Optional, Type

from langchain_core.callbacks import (
    AsyncCallbackManagerForToolRun,
    CallbackManagerForToolRun,
)
from langchain_core.tools import BaseTool
from pydantic import BaseModel, Field

from skreaver import A2aClient, TaskStatus


class A2aToolInput(BaseModel):
    message: str = Field(description="The message or task to send to the A2A agent")


class A2aAgentTool(BaseTool):
    """LangChain tool that delegates tasks to a Skreaver A2A agent.

    Wraps A2aClient so any LangChain agent can call an A2A-compatible
    endpoint as a native tool call.
    """

    name: str = "a2a_agent"
    description: str = "Delegates a task to a specialized A2A agent and returns the result"
    args_schema: Type[BaseModel] = A2aToolInput

    agent_url: str
    bearer_token: Optional[str] = None
    poll_interval_ms: int = 1000
    timeout_ms: int = 30000

    def _get_client(self) -> A2aClient:
        client = A2aClient(self.agent_url)
        if self.bearer_token:
            client = client.with_bearer_token(self.bearer_token)
        return client

    def _run(
        self,
        message: str,
        run_manager: Optional[CallbackManagerForToolRun] = None,
    ) -> str:
        # _arun is preferred; this sync wrapper is a fallback.
        return asyncio.run(self._arun(message))

    async def _arun(
        self,
        message: str,
        run_manager: Optional[AsyncCallbackManagerForToolRun] = None,
    ) -> str:
        client = self._get_client()
        task = await client.send_message(message)

        if not task.is_terminal():
            task = await client.wait_for_task(
                task.id,
                poll_interval_ms=self.poll_interval_ms,
                timeout_ms=self.timeout_ms,
            )

        if task.status == TaskStatus.Failed:
            task_dict = task.to_dict()
            return f"Task failed: {task_dict.get('error', 'unknown error')}"

        if task.status != TaskStatus.Completed:
            return f"Task ended with status: {task.status}"

        # Extract text from the last agent message in the task
        task_dict = task.to_dict()
        for msg in reversed(task_dict.get("messages", [])):
            if msg.get("role") == "agent":
                for part in msg.get("parts", []):
                    if part.get("type") == "text":
                        return part["text"]

        return "Task completed (no text output)"


# --- Factory helpers ---

def make_a2a_tool(
    name: str,
    description: str,
    agent_url: str,
    bearer_token: Optional[str] = None,
    timeout_ms: int = 30000,
) -> A2aAgentTool:
    """Create a named A2A tool for a specific agent endpoint."""
    return A2aAgentTool(
        name=name,
        description=description,
        agent_url=agent_url,
        bearer_token=bearer_token,
        timeout_ms=timeout_ms,
    )


# --- Demo ---

async def demo() -> None:
    tool = A2aAgentTool(
        name="data_analyzer",
        description="Analyzes data and returns structured insights",
        agent_url="http://localhost:8080",
    )

    print(f"Tool: {tool.name}")
    print(f"Description: {tool.description}")
    print(f"Input schema: {list(tool.args_schema.model_fields.keys())}")

    # Uncomment when a real A2A agent is running:
    # result = await tool.arun("Summarize the Q1 2026 sales data")
    # print(f"Result: {result}")

    # --- LangChain agent integration ---
    #
    # from langchain_openai import ChatOpenAI
    # from langchain.agents import create_tool_calling_agent, AgentExecutor
    # from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
    #
    # tools = [
    #     make_a2a_tool(
    #         "code_reviewer",
    #         "Reviews code for bugs, security issues, and best practices",
    #         "http://code-reviewer-agent:8080",
    #     ),
    #     make_a2a_tool(
    #         "data_analyst",
    #         "Analyzes datasets and produces statistical summaries",
    #         "http://data-analyst-agent:8081",
    #         bearer_token="my-token",
    #     ),
    # ]
    #
    # llm = ChatOpenAI(model="gpt-4o")
    # prompt = ChatPromptTemplate.from_messages([
    #     ("system", "You are a helpful assistant with access to specialized AI agents."),
    #     MessagesPlaceholder("chat_history", optional=True),
    #     ("human", "{input}"),
    #     MessagesPlaceholder("agent_scratchpad"),
    # ])
    # agent = create_tool_calling_agent(llm, tools, prompt)
    # executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
    #
    # result = await executor.ainvoke({"input": "Review this Python function: def add(a, b): return a + b"})
    # print(result["output"])


if __name__ == "__main__":
    asyncio.run(demo())
