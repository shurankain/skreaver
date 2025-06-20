# Skreaver

**Skreaver** is a Rust-native coordination runtime for building modular AI agents and agentic infrastructures.

Skreaver aims to be the *Tokio* of agent systems: lightweight, pluggable, and ready for real-world orchestration.

---

## 🧠 Why Skreaver?

Modern AI agents suffer from:

- Complex stacks (Python + LangChain + glue code)
- Implicit architectures and fragile wrappers
- Poor performance in constrained or embedded environments

**Skreaver** solves this with a strict, high-performance, type-safe platform built in Rust, designed for real-world agent deployment.

---

## ⚙️ Core Principles

- **Rust 2024-first**: zero-cost abstractions, full control
- **Agent-centric**: traits and modules for memory, tools, goals
- **Composable runtime**: run agents locally or integrate with infra
- **Open by design**: build your own memory/tool systems, no lock-in

---

## 📐 Architecture Preview

```text
[Agent] → [ToolCall] → [ExecutionResult]
   ↓             ↑
[Memory] ← [ContextUpdate]
   ↓
[Coordinator Runtime]
````

Skreaver gives you the scaffolding. You build the logic.

---

## 📦 Status

> 🚧 Skreaver is in early development.
> First `Agent` trait, memory module, and coordinator runtime coming soon.
> Follow to stay updated.

---

## 🤝 Contribute / Follow

* ⭐ Star the repo
* 👀 Watch for progress
* 💬 Feedback via GitHub Discussions
* 💸 Support via [GitHub Sponsors](https://github.com/sponsors/shurankain)

---

## 🔗 Links

* [ohusiev.com](https://ohusiev.com)
* [Medium](https://medium.com/@ohusiev_6834)
* [Skreaver.com](https://skreaver.com)

---

## 📄 License

MIT — see [LICENSE](./LICENSE)
