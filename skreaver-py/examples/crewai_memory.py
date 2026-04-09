"""
CrewAI integration: Skreaver FileMemory as CrewAI memory storage

Implements a CrewAI-compatible memory storage backend using Skreaver's
FileMemory for durable, atomic JSON persistence. This replaces the
default ChromaDB-based RAGStorage with a lightweight file backend.

Usage:
    python crewai_memory.py

Requirements:
    pip install crewai skreaver
"""

import json
import time
from typing import Any, Optional

from crewai.memory import ShortTermMemory

from skreaver.memory import FileMemory


class SkreaveMemoryStorage:
    """CrewAI memory storage backed by Skreaver's FileMemory.

    Stores memory entries as JSON in a single file with atomic writes.
    Each entry is keyed by a timestamp + agent combo for uniqueness.
    Search does simple substring matching; for production use with
    semantic search, pair this with an embedding model.

    Implements the CrewAI storage interface:
        save(value, metadata, agent) -> None
        search(query, limit, score_threshold) -> list[dict]
        reset() -> None
    """

    def __init__(self, file_path: str = "/tmp/skreaver_crewai_memory.json") -> None:
        self._memory = FileMemory(file_path)

    def save(
        self,
        value: Any,
        metadata: dict,
        agent: Optional[str] = None,
    ) -> None:
        """Save a memory entry."""
        # Load existing index
        entries = self._load_entries()

        entry = {
            "value": str(value),
            "metadata": metadata,
            "agent": agent,
            "timestamp": time.time(),
        }
        entries.append(entry)

        self._memory.store("_entries", json.dumps(entries))

    def search(
        self,
        query: str,
        limit: int = 3,
        score_threshold: float = 0.0,
    ) -> list[dict]:
        """Search memory entries by substring matching.

        Returns entries in the format CrewAI expects:
            [{"context": str, "score": float, "metadata": dict}, ...]
        """
        entries = self._load_entries()
        query_lower = query.lower()

        scored: list[tuple[float, dict]] = []
        for entry in entries:
            value = entry["value"].lower()
            # Simple relevance: ratio of query terms found in value
            terms = query_lower.split()
            if not terms:
                continue
            matched = sum(1 for t in terms if t in value)
            score = matched / len(terms)
            if score > score_threshold:
                scored.append((score, entry))

        scored.sort(key=lambda x: x[0], reverse=True)

        return [
            {
                "context": entry["value"],
                "score": score,
                "metadata": entry.get("metadata", {}),
            }
            for score, entry in scored[:limit]
        ]

    def reset(self) -> None:
        """Clear all stored memories."""
        self._memory.store("_entries", "[]")

    def _load_entries(self) -> list[dict]:
        raw = self._memory.load("_entries")
        if not raw:
            return []
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return []


# ---------------------------------------------------------------------------
# Demo
# ---------------------------------------------------------------------------

def demo() -> None:
    storage = SkreaveMemoryStorage("/tmp/skreaver_crewai_demo_memory.json")
    storage.reset()

    # Simulate storing memories from crew execution
    storage.save(
        "The A2A protocol uses HTTP+SSE for real-time agent communication.",
        metadata={"task": "research", "topic": "protocols"},
        agent="researcher",
    )
    storage.save(
        "MCP provides JSON-RPC 2.0 based tool discovery and invocation.",
        metadata={"task": "research", "topic": "protocols"},
        agent="researcher",
    )
    storage.save(
        "Skreaver's gateway translates between MCP and A2A bidirectionally.",
        metadata={"task": "research", "topic": "skreaver"},
        agent="researcher",
    )
    storage.save(
        "Python bindings use PyO3 0.24 for zero-copy Rust-Python interop.",
        metadata={"task": "analysis", "topic": "implementation"},
        agent="analyst",
    )

    # Search
    print("Search: 'MCP protocol tool'")
    results = storage.search("MCP protocol tool", limit=2)
    for r in results:
        print(f"  [{r['score']:.2f}] {r['context'][:80]}")

    print("\nSearch: 'A2A agent communication'")
    results = storage.search("A2A agent communication", limit=2)
    for r in results:
        print(f"  [{r['score']:.2f}] {r['context'][:80]}")

    print("\nSearch: 'Python Rust bindings'")
    results = storage.search("Python Rust bindings", limit=2)
    for r in results:
        print(f"  [{r['score']:.2f}] {r['context'][:80]}")

    # Show integration with CrewAI's ShortTermMemory
    print("\n--- CrewAI ShortTermMemory integration ---")
    print("Pass SkreaveMemoryStorage to ShortTermMemory:")
    print("  short_term_memory = ShortTermMemory(storage=SkreaveMemoryStorage('/path/to/mem.json'))")

    # --- Full CrewAI integration ---
    #
    # from crewai import Agent, Task, Crew, Process
    # from crewai.memory import ShortTermMemory, LongTermMemory, EntityMemory
    #
    # mem_path = "/var/data/crew_memory"
    #
    # crew = Crew(
    #     agents=[...],
    #     tasks=[...],
    #     memory=True,
    #     short_term_memory=ShortTermMemory(
    #         storage=SkreaveMemoryStorage(f"{mem_path}/short_term.json"),
    #     ),
    #     long_term_memory=LongTermMemory(
    #         storage=SkreaveMemoryStorage(f"{mem_path}/long_term.json"),
    #     ),
    #     entity_memory=EntityMemory(
    #         storage=SkreaveMemoryStorage(f"{mem_path}/entities.json"),
    #     ),
    #     process=Process.sequential,
    #     verbose=True,
    # )
    #
    # result = crew.kickoff(inputs={"topic": "AI agent protocols"})
    # print(result.raw)
    #
    # # Memories persist across runs via FileMemory:
    # result2 = crew.kickoff(inputs={"topic": "AI agent security"})
    # # ^ This run benefits from memories stored during the first run


if __name__ == "__main__":
    demo()
