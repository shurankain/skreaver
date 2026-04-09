"""
CrewAI integration: A2A Agent as a CrewAI Tool

Wraps a Skreaver A2A agent as a CrewAI BaseTool, so any CrewAI agent
can delegate tasks to a remote A2A-compatible agent.

Usage:
    python crewai_a2a_tool.py

Requirements:
    pip install crewai skreaver
"""

import asyncio
from typing import Optional, Type

from crewai.tools import BaseTool
from pydantic import BaseModel, Field

from skreaver import A2aClient, TaskStatus


class A2aToolInput(BaseModel):
    message: str = Field(description="The task or question to send to the A2A agent")


class A2aAgentTool(BaseTool):
    """CrewAI tool that delegates work to a Skreaver A2A agent.

    Sends a text message to an A2A endpoint, waits for completion,
    and returns the agent's response.
    """

    name: str = "a2a_agent"
    description: str = "Delegates a task to a remote A2A agent and returns the result"
    args_schema: Type[BaseModel] = A2aToolInput

    agent_url: str = ""
    bearer_token: Optional[str] = None
    poll_interval_ms: int = 1000
    timeout_ms: int = 30000

    def _get_client(self) -> A2aClient:
        client = A2aClient(self.agent_url)
        if self.bearer_token:
            client = client.with_bearer_token(self.bearer_token)
        return client

    def _run(self, message: str) -> str:
        return asyncio.run(self._call_agent(message))

    async def _call_agent(self, message: str) -> str:
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

        # Extract the last agent message
        task_dict = task.to_dict()
        for msg in reversed(task_dict.get("messages", [])):
            if msg.get("role") == "agent":
                for part in msg.get("parts", []):
                    if part.get("type") == "text":
                        return part["text"]

        return "Task completed (no text output)"


# --- Factory helper ---

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

def demo() -> None:
    tool = A2aAgentTool(
        name="data_analyzer",
        description="Analyzes data and returns structured insights",
        agent_url="http://localhost:8080",
    )

    print(f"Tool: {tool.name}")
    print(f"Description: {tool.description}")
    print(f"Input schema: {list(tool.args_schema.model_fields.keys())}")

    # --- Full CrewAI integration ---
    #
    # from crewai import Agent, Task, Crew, Process
    #
    # code_review_tool = make_a2a_tool(
    #     "code_reviewer",
    #     "Reviews code for bugs, security issues, and best practices",
    #     "http://code-reviewer-agent:8080",
    # )
    # analysis_tool = make_a2a_tool(
    #     "data_analyst",
    #     "Analyzes datasets and produces statistical summaries",
    #     "http://data-analyst-agent:8081",
    #     bearer_token="my-token",
    # )
    #
    # reviewer = Agent(
    #     role="Senior Code Reviewer",
    #     goal="Ensure code quality and security across the project",
    #     backstory="You are a staff engineer who has reviewed thousands of PRs.",
    #     tools=[code_review_tool],
    #     llm="gpt-4o",
    #     verbose=True,
    # )
    #
    # analyst = Agent(
    #     role="Data Analyst",
    #     goal="Extract actionable insights from data",
    #     backstory="Expert data scientist with a focus on concise, accurate analysis.",
    #     tools=[analysis_tool],
    #     llm="gpt-4o",
    #     verbose=True,
    # )
    #
    # review_task = Task(
    #     description="Review the authentication module for security issues.",
    #     expected_output="A list of findings with severity and fix suggestions.",
    #     agent=reviewer,
    # )
    #
    # analysis_task = Task(
    #     description="Analyze the Q1 2026 sales data and identify trends.",
    #     expected_output="A summary with 3-5 key data points and a recommendation.",
    #     agent=analyst,
    # )
    #
    # crew = Crew(
    #     agents=[reviewer, analyst],
    #     tasks=[review_task, analysis_task],
    #     process=Process.sequential,
    #     verbose=True,
    # )
    #
    # result = crew.kickoff()
    # print(result.raw)


if __name__ == "__main__":
    demo()
