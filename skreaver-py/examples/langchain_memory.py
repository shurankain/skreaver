"""
LangChain integration: Skreaver FileMemory as persistent chat history

Implements LangChain's BaseChatMessageHistory backed by Skreaver's FileMemory,
giving conversation threads durable JSON storage with atomic writes.

Usage:
    python langchain_memory.py

Requirements:
    pip install langchain-core skreaver
    # For the full chain example at the bottom:
    pip install langchain-openai
"""

import json
from typing import Sequence

from langchain_core.chat_history import BaseChatMessageHistory
from langchain_core.messages import (
    AIMessage,
    BaseMessage,
    HumanMessage,
    messages_from_dict,
    messages_to_dict,
)

from skreaver.memory import FileMemory


class SkreaveFileMessageHistory(BaseChatMessageHistory):
    """Persistent chat message history backed by Skreaver's FileMemory.

    Each session_id maps to one key in the JSON file, so a single file
    can hold multiple conversation threads.

    Args:
        session_id: Unique identifier for this conversation (e.g. user ID,
                    thread ID). Each session is stored under its own key.
        file_path:  Path to the JSON file used for persistence.
                    FileMemory uses atomic writes, so the file is always
                    in a consistent state.
    """

    def __init__(
        self,
        session_id: str,
        file_path: str = "/tmp/skreaver_chat_history.json",
    ) -> None:
        self.session_id = session_id
        self._memory = FileMemory(file_path)

    @property
    def messages(self) -> list[BaseMessage]:
        raw = self._memory.load(self.session_id)
        if not raw:
            return []
        return messages_from_dict(json.loads(raw))

    def add_message(self, message: BaseMessage) -> None:
        current = self.messages
        current.append(message)
        self._memory.store(self.session_id, json.dumps(messages_to_dict(current)))

    def add_messages(self, messages: Sequence[BaseMessage]) -> None:
        current = self.messages
        current.extend(messages)
        self._memory.store(self.session_id, json.dumps(messages_to_dict(current)))

    def clear(self) -> None:
        self._memory.store(self.session_id, "[]")


def get_session_history(
    session_id: str,
    file_path: str = "/tmp/skreaver_chat_history.json",
) -> SkreaveFileMessageHistory:
    """Factory for use with RunnableWithMessageHistory.

    Example:
        from langchain_core.runnables.history import RunnableWithMessageHistory

        chain_with_history = RunnableWithMessageHistory(
            chain,
            lambda sid: get_session_history(sid, "/var/data/chat.json"),
            input_messages_key="input",
            history_messages_key="history",
        )
    """
    return SkreaveFileMessageHistory(session_id, file_path)


# ---------------------------------------------------------------------------
# Demo
# ---------------------------------------------------------------------------

def demo() -> None:
    history_file = "/tmp/skreaver_demo_history.json"
    session_a = SkreaveFileMessageHistory("user-alice", history_file)
    session_b = SkreaveFileMessageHistory("user-bob", history_file)

    # Start fresh for the demo
    session_a.clear()
    session_b.clear()

    # Simulate two independent conversation threads in the same file
    session_a.add_messages([
        HumanMessage(content="What is the A2A protocol?"),
        AIMessage(content=(
            "A2A (Agent-to-Agent) is a protocol for inter-agent communication. "
            "It enables task delegation, capability discovery, and streaming "
            "updates between AI agents."
        )),
        HumanMessage(content="How does Skreaver implement it?"),
        AIMessage(content=(
            "Skreaver implements A2A in the skreaver-a2a crate, providing "
            "an HTTP+SSE transport, agent cards, full task lifecycle, and "
            "a Python client via PyO3 bindings."
        )),
    ])

    session_b.add_messages([
        HumanMessage(content="Explain MCP in one sentence."),
        AIMessage(content=(
            "MCP (Model Context Protocol) is a JSON-RPC 2.0 standard for "
            "agents to discover and invoke tools, access resources, and "
            "request LLM completions."
        )),
    ])

    print(f"Session 'user-alice': {len(session_a.messages)} messages")
    print(f"Session 'user-bob':   {len(session_b.messages)} messages")
    print()

    print("Alice's conversation:")
    for msg in session_a.messages:
        role = "Human" if isinstance(msg, HumanMessage) else "AI   "
        print(f"  [{role}] {msg.content[:80]}")

    print("\nBob's conversation:")
    for msg in session_b.messages:
        role = "Human" if isinstance(msg, HumanMessage) else "AI   "
        print(f"  [{role}] {msg.content[:80]}")

    # Verify persistence: create a new instance pointing at the same file
    reloaded = SkreaveFileMessageHistory("user-alice", history_file)
    assert len(reloaded.messages) == len(session_a.messages), "Persistence check failed"
    print("\nPersistence verified: history survives across SkreaveFileMessageHistory instances.")

    # --- RunnableWithMessageHistory integration ---
    #
    # from langchain_core.runnables.history import RunnableWithMessageHistory
    # from langchain_core.prompts import ChatPromptTemplate, MessagesPlaceholder
    # from langchain_openai import ChatOpenAI
    #
    # prompt = ChatPromptTemplate.from_messages([
    #     ("system", "You are a concise technical assistant."),
    #     MessagesPlaceholder("history"),
    #     ("human", "{input}"),
    # ])
    # llm = ChatOpenAI(model="gpt-4o")
    # chain = prompt | llm
    #
    # chain_with_history = RunnableWithMessageHistory(
    #     chain,
    #     lambda sid: get_session_history(sid, "/var/data/chat.json"),
    #     input_messages_key="input",
    #     history_messages_key="history",
    # )
    #
    # # First turn
    # r1 = chain_with_history.invoke(
    #     {"input": "What is MCP?"},
    #     config={"configurable": {"session_id": "user-alice"}},
    # )
    # print(r1.content)
    #
    # # Second turn — history is loaded automatically from FileMemory
    # r2 = chain_with_history.invoke(
    #     {"input": "And how does it relate to A2A?"},
    #     config={"configurable": {"session_id": "user-alice"}},
    # )
    # print(r2.content)


if __name__ == "__main__":
    demo()
