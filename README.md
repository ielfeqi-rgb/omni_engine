<div align="center">
  <h1>🚀 Omni Engine</h1>
  <p><b>The Causal-Memory Autonomous Agent Framework for Local LLMs</b></p>
</div>

Omni Engine is a high-performance, local-first **Autonomous AI Agent Framework** written in **Rust**. It is not just an inference server—it is a complete, self-healing agentic runtime that embeds `llama.cpp` directly via zero-allocation C FFI bindings. 

By tracking agent execution steps through a **Causal DAG** and performing **O(1) Memory Rollbacks**, Omni Engine allows your local AI agents to seamlessly recover from reasoning or coding errors without ever suffering the catastrophic latency of a cold prompt restart.

---

## 🔥 Why Omni Engine? (The Benchmark)

If you've tried running complex agentic loops locally using tools like **Open Interpreter** paired with **Ollama**, you know the pain: processes hang, contexts exhaust, and the model forgets its previous states, requiring human intervention. 

Omni Engine was built to solve this. **It operates entirely autonomously in the background.**

### 🏆 Benchmark: Omni Engine vs. Open Interpreter
*(Test Environment: CPU-only workstation, `qwen2.5-1.5b.gguf`, 400s timeout per task)*

| Task | Omni Engine (Autonomous Finish) | Open Interpreter (Autonomous Finish) |
|---|---|---|
| **Task 1: Coding Agent** | 🟢 **195.30s (Success)** | ❌ 400.0s (Hung in Interactive Mode) |
| **Task 2: Debug Agent** | 🟢 **324.61s (Success)** | ❌ 400.0s (Hung in Interactive Mode) |
| **Task 3: Multi-step File System** | 🟢 **394.89s (Success)** | ❌ 400.0s (Hung in Interactive Mode) |

**The Verdict:** Omni Engine plans, executes in its secure workspace, creates/modifies files, and gracefully exits when the task is complete—all without dropping into blocking REPL shells or requiring human prompting.

---

## ✨ Core Features

* 🧠 **Causal DAG Memory Tracking:** Omni Engine tracks Def-Use dataflow. If a coding step fails, it instantly excises the suffix from the KV cache.
* ⚡ **1.81x TTFT Acceleration:** Because it uses surgical KV-cache pruning instead of full restarts, it evaluates corrections nearly twice as fast as standard inference engines.
* 🛡️ **Secure Lua Sandbox:** Agents execute code inside a heavily restricted, memory-capped (32 MiB) Lua environment with instruction hooks to prevent infinite loops.
* 💻 **Terminal Session Bridge:** Direct, native access to bash/shell for advanced multi-step execution.
* 🦀 **Safe Rust & Zero-Dependency:** Written in Rust for fearless concurrency and memory safety. Binds directly to `libllama.so` with no Python middleware overhead.
* 🔌 **OpenAI-Compatible API:** Exposes a drop-in `/v1` REST API for your existing frontends.

---

## 🛠️ Getting Started

### 1. Requirements
* Linux (x86_64)
* Rust toolchain (`cargo`)
* C++ Build tools (`make`, `gcc`)

### 2. Build the Engine
Clone the repository and build the project (this will also compile the embedded `llama.cpp` shared libraries):
```bash
git clone https://github.com/yourusername/omni_engine.git
cd omni_engine
cargo build --release
```

### 3. Download a Local Model (GGUF)
Omni Engine works beautifully with small-parameter models like `Gemma-2B` and `Qwen2.5-1.5B` optimized for CPU. Place your `.gguf` files in the `models/` directory.

### 4. Run the Agent
Run the engine directly from the CLI:
```bash
# Ensure the embedded llama.cpp library is in the path
export LD_LIBRARY_PATH=$(pwd)/bin

# Start an autonomous agent task
cargo run --release -- start models/qwen2.5-1.5b.gguf
```

Or start the OpenAI-compatible Web Server:
```bash
cargo run --release -- serve
```

---

## 🏗️ Architecture

```text
                    ┌─────────────────────────────────────┐
                    │           omni_engine                │
                    │         (Rust Binary)                │
                    └──────────────┬──────────────────────┘
                                   │
           ┌───────────────────────┼───────────────────────┐
           ▼                       ▼                       ▼
   ┌───────────────┐     ┌─────────────────┐    ┌──────────────────┐
   │  native_llama │     │  causal_memory  │    │    sandbox/      │
   │  (llama.cpp)  │     │  ┌──────────┐   │    │  ┌────────────┐  │
   │               │     │  │CausalDAG │   │    │  │LuaRunner   │  │
   │  Load/Eval    │     │  │Tail-Roll │   │    │  │(sandboxed) │  │
   │  KV Cache     │     │  └──────────┘   │    │  ├────────────┤  │
   └───────────────┘     └─────────────────┘    │  │MemoryVfs   │  │
                                                │  ├────────────┤  │
                                                │  │Terminal    │  │
                                                └──────────────────┘
```

---

## 📖 Whitepaper & Research
For a deep dive into the mathematical constraints of Rotary Position Embeddings (RoPE), hardware alignment penalties, and how Omni Engine achieves its caching speedups, read our official Whitepaper:  
👉 **[In-Process KV-Cache Rollback and Dataflow Dependency Tracking](docs/RESEARCH_PAPER.md)**

---

## 📄 License
MIT License. Feel free to use, modify, and distribute.
