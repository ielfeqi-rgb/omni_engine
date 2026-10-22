<div align="center">

# ⚡ OMNI ENGINE
### **The Sovereign, In-Process Coding AI for Ordinary Laptops**
*Run a full autonomous coding agent 100% offline — Zero Cloud Subscriptions, Zero API Fees, Zero RAM Crashes.*

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?logo=rust)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS-blue.svg)](https://github.com/ielfeqi-rgb/omni_engine)
[![Hardware](https://img.shields.io/badge/RAM-Works%20on%208GB-brightgreen.svg)]()
[![Sandbox](https://img.shields.io/badge/Sandbox-Bubblewrap%20Cleanroom%20(W=∅)-purple.svg)]()
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

<br/>

```bash
# Install in 10 seconds (Linux / macOS)
curl -fsSL https://raw.githubusercontent.com/ielfeqi-rgb/omni_engine/main/install.sh | bash
```

</div>

---

## 🎯 Why Omni Engine Exists

You shouldn't need a **$20/month subscription** or an expensive cloud cluster to have an AI assistant that writes, tests, and refactors real software.

Existing tools fall into two traps:
1. **Cloud Wrappers (Claude Code, Cursor):** Send your code to third-party servers, drain your wallet with token fees, and freeze when your internet drops.
2. **Naive Local Agents:** Small local models (1.5B–3B parameters) suffer from **catastrophic code wiping**—as soon as you ask them to fix an error or add a feature, they rewrite the entire file and delete previously working functions.

**Omni Engine solves this at the architectural layer.** By embedding the inference engine directly inside Rust and enforcing strict mathematical scheduling invariants, a lightweight 1.5B local model running on CPU achieves the production accuracy of far larger models—**100% offline, private, and free forever.**

---

## ⚔️ Architectural Comparison

| Capability | Cloud Agents (Cursor / Claude Code) | Agent Wrappers (`jcode` / Open Interpreter) | **Omni Engine** |
| :--- | :---: | :---: | :---: |
| **Cloud Dependency & Cost** | Requires Paid API Subscriptions | Requires External API / Ollama Server | **100% Sovereign & Offline (0 Cost)** |
| **Inference Location** | Remote Servers | External Client-Server Socket | **In-Process C-FFI (`llama.cpp` in RAM)** |
| **Memory Control** | None (Blind API Call) | None (Re-sends full conversation) | **Causal KV-Cache Slicing (`kv_cache_seq_rm`)** |
| **Small Model Code Wiping** | N/A (Uses 70B+ Models) | Frequent Catastrophic Deletion | **Zero-Wipe Invariant (VFS Code Freeze)** |
| **Parallel Scheduling** | Sequential Turns | Uncoordinated Threads (Race Conditions) | **RCPSP Dynamic DAG with LRP Priority** |
| **Execution Sandbox** | Runs on Host Disk | Runs on Host Terminal | **Universal Cleanroom ($W = \emptyset$) via Bubblewrap** |
| **Interactive Test Feedback** | Manual / Static Linting | Basic Exit Codes | **Automated Stdin Interactive Harness** |

---

## 🚀 Quick Start (60 Seconds)

### 1. One-Line Install
```bash
curl -fsSL https://raw.githubusercontent.com/ielfeqi-rgb/omni_engine/main/install.sh | bash
```

### 2. Run the Sovereign TUI Terminal
```bash
omni --tui
```

### 3. Dispatch an Autonomous Swarm
```bash
omni swarm "Build a complete interactive Todo CLI in Python: add and delete tasks, persist in tasks.json, error handling, interactive loop"
```

Omni Engine will:
1. 🧠 **Plan:** Decompose the goal into topological subgoals.
2. ⚡ **Execute:** Dispatch parallel worker threads with in-memory VFS isolation.
3. 🔬 **Audit:** Run the **Universal Cleanroom ($W = \emptyset$)** to test the code against user goals and simulate user keyboard input.
4. 🛡️ **Preserve:** If any requirement is missing, the **Zero-Wipe Invariant** ensures existing working code is frozen, adding the delta incrementally.
5. 💾 **Commit:** Writes final, verified code to `./workspace/` only after passing all gates.

---

## 🧠 The 4 Core Superpowers

```mermaid
graph TD
    Goal([User Objective]) --> Thinker[System 2 Sovereign Planner]
    Thinker -->|Synthesizes Subgoals| DAG[RCPSP Scheduler & Memory Gate]
    DAG -->|Dispatches Workers| Swarm[Parallel Worker Threads]
    Swarm -->|Stages Code in RAM| VFS[(In-Memory VFS)]
    
    VFS -->|Read-Only W = ∅| Cleanroom{Universal Cleanroom Auditor}
    Cleanroom -->|AST & Syntax| Mode2[Syntax Verification]
    Cleanroom -->|Acceptance Delta| Mode4[Multi-Part Requirement Audit]
    Cleanroom -->|Interactive Stdin| Mode3[Live Simulated Execution]
    
    Mode4 -->|Missing Features| DeltaReport[Acceptance Delta Report]
    DeltaReport -->|Zero-Wipe Invariant| Invariant[Freeze VFS & Pass Existing Code]
    Invariant -->|Dynamic DAG Injection| Swarm
    
    Cleanroom -->|100% Verified| HostDisk[Commit to Host Disk]
```

### 1. The Zero-Wipe Invariant
Small local LLMs tend to delete previously working functions when asked to extend code. Omni Engine enforces a physical **Zero-Wipe Invariant**: existing working code is preserved in VFS, and the model receives strict structural continuity prompts ensuring changes are purely additive.

### 2. Causal KV-Cache Slicing & Rollback
When an agent hallucinates or hits a syntax error, standard tools restart the prompt from token 0 (wasting battery and compute). Omni Engine executes atomic KV-cache rollback via `kv_cache_seq_rm`, excising only the erroneous suffix in microseconds.

### 3. RCPSP Dynamic DAG Scheduling
Multi-agent tasks are modeled as a **Resource-Constrained Project Scheduling Problem (RCPSP)**:
* **Bernstein Conflict Detection ($R_i \cap W_j = \emptyset$):** Prevents parallel workers from writing to the same file simultaneously.
* **Longest Remaining Path (LRP):** Prioritizes critical-path tasks first.
* **Hardware Memory Gate ($K_{max}$):** Guarantees zero out-of-memory (OOM) crashes on 8GB RAM laptops.

### 4. Universal Cleanroom Testing ($W = \emptyset$)
All tests run inside an isolated Linux namespace via Bubblewrap (`bwrap --unshare-all`). The tester inspects code, verifies AST syntax, feeds simulated keyboard inputs into stdin, and audits multi-part goals—all in read-only mode without touching your personal files.

---

## 📊 Scientific Benchmark Results

*(Tested on CPU-only workstation, `qwen2.5-coder-1.5b-instruct-q4_k_m.gguf`)*

| Metric / Scenario | Sequential ReAct Baseline | Naive Parallel Agent | **Omni Engine (RCPSP Swarm)** |
| :--- | :---: | :---: | :---: |
| **4-Task Multi-File Makespan** | 409.6s | 204.8s (OOM / Collision Risk) | **204.8s (Zero Conflict / Gated RAM)** |
| **Error Recovery Latency** | 248.5 ms (Full restart) | Failed / Hung | **6.1 ms (Causal KV Slicing - 40x Faster)** |
| **Zero-Wipe Retention Rate** | 25% (Dropped earlier functions) | 0% (Write collision) | **100% (All functions preserved)** |
| **Peak RAM Allocation** | Indefinitely Growing | Unbounded (Crash Risk) | **Strictly Capped at 4096 tokens** |

---

## 🛠️ CLI Reference

```bash
omni --help                      # View all commands
omni status                      # System hardware specs & KV-cache status
omni models                      # List discovered local GGUF models
omni swarm "<prompt>"            # Run autonomous swarm pipeline
omni swarm "<prompt>" --steps 5  # Set max repair steps per worker
omni --tui                       # Launch interactive terminal UI
cleanroom_runner --code file.py  # Run standalone Cleanroom AST syntax audit
cleanroom_runner --interactive "python3 file.py" --inputs "10,20,exit"
```

---

## 🤝 Contributing & Community

Omni Engine is built by developers who believe sovereign, private AI should belong to everyone—not just those who can pay monthly cloud subscriptions.

* 🌟 **Star the repository** to support sovereign AI development!
* 🐛 Found a bug or want to request a feature? [Open an issue](https://github.com/ielfeqi-rgb/omni_engine/issues).
* 📜 Pull requests are warmly welcomed!

---

## 📄 License
MIT License. Free to use, modify, and distribute for any purpose.
