---
language:
- en
license: mit
tags:
- rust
- agent
- code-generation
- autonomous-agent
- local-ai
- gguf
- llama-cpp
- offline
- cleanroom-sandbox
pipeline_tag: text-generation
---

<div align="center">

# ⚡ OMNI ENGINE (v2.1.1)
### **The Sovereign, On-Device Autonomous Software Engineering Runtime**
*100% Offline • Zero Cloud Subscriptions • Direct In-Process C-FFI • Zero-GPU Commodity Laptop Baseline*

[![GitHub Release](https://img.shields.io/badge/GitHub-v2.1.1%20Release-blue.svg?logo=github)](https://github.com/ielfeqi-rgb/omni_engine)
[![Language](https://img.shields.io/badge/Language-Pure%20Rust-orange.svg?logo=rust)](https://www.rust-lang.org)
[![Hardware](https://img.shields.io/badge/Hardware-Commodity%208GB%20RAM%20CPU-brightgreen.svg)]()
[![Sandbox](https://img.shields.io/badge/Sandbox-Bubblewrap%20Cleanroom%20(W=∅)-purple.svg)]()
[![License](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

</div>

---

## 💡 What is Omni Engine?

**Omni Engine** is an open-source, sovereign software development runtime built in pure Rust. Unlike typical web chatbots or cloud-tethered IDE extensions (which stream partial code snippets and leave the debugging to you), Omni Engine operates as an **autonomous multi-agent engineering swarm directly inside your machine**:

- **Runs 100% On-Device:** Zero telemetry, zero external API keys, zero network data leakage.
- **Runs on Everyday Laptops:** Engineered to run smoothly on standard commodity laptops (8GB RAM, CPU-only, zero GPU required).
- **Direct C-FFI In-Process Inference:** Integrates `llama.cpp` directly as a linked static C-FFI library—eliminating the latency and memory overhead of local HTTP daemons (like Ollama).
- **Fail-Closed Cleanroom Sandbox:** Executes, compiles, and smoke-tests code inside a hardened Linux Bubblewrap container (`bwrap`) with isolated network, process tree, and tmpfs caps.
- **Causal KV-Cache Tail Rollback:** Erases failed generation paths and epistemic reasoning loops with $O(1)$ pointer arithmetic, saving up to 3,800+ wasted tokens per worker run.

---

## 📊 Omni Engine vs. Traditional Setups

| Feature | Cloud Assistants (Cursor / Copilot) | Local Wrappers (Ollama / Open-WebUI) | **Omni Engine v2.1.1** |
| :--- | :--- | :--- | :--- |
| **Privacy & Sovereignty** | ❌ Sends code to remote servers | ✅ Local | 🔒 **100% Sovereign (Air-Gapped)** |
| **Hardware Floor** | Cloud GPU Clusters | High RAM / GPU Recommended | 💻 **8GB Laptop CPU (Zero GPU)** |
| **Runtime Architecture** | Web / Node.js daemon | HTTP Client-Server REST | 🦀 **Direct In-Process Rust C-FFI** |
| **Execution Sandbox** | Remote Docker or Host | None (Runs directly on host) | 🛡️ **Bubblewrap Fail-Closed Jail** |
| **File Integrity** | Overwrite risks | Manual copy-pasting | ⚡ **Atomic POSIX Two-Phase Commit** |
| **Memory Strategy** | Truncation / Rolling context | Stateless or Full Re-eval | 🧠 **Causal DAG KV-Cache Rollback** |

---

## 📦 Supported Open-Source GGUF Models

Omni Engine works with any standard quantized **GGUF** model from the Hugging Face Hub. Recommended community models:

| Model Architecture | Recommended Quant | Context Size | RAM Usage | Target Tasks |
| :--- | :--- | :--- | :--- | :--- |
| **[Qwen2.5-Coder-1.5B-Instruct](https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF)** | `Q4_K_M` | 4,096 | ~1.4 GB | Rapid scripting, utilities, algorithms |
| **[Qwen2.5-Coder-7B-Instruct](https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF)** | `Q4_K_M` | 8,192 | ~4.8 GB | Full-stack web apps, complex refactoring |
| **[Phi-4-mini-instruct (3.8B)](https://huggingface.co/microsoft/Phi-4-mini-instruct-GGUF)** | `Q4_K_M` | 4,096 | ~2.9 GB | High-reasoning math, backend logic |
| **[Llama-3.2-3B-Instruct](https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF)** | `Q4_K_M` | 4,096 | ~2.6 GB | General engineering & documentation |

---

## 🚀 Quickstart in 60 Seconds

### 1. Download Pre-Built Binary
Grab the latest release tarball for Linux x86_64:

```bash
# Download from GitHub Releases
wget https://github.com/ielfeqi-rgb/omni_engine/releases/download/v2.1.1/omni_engine_v2.1.1_linux_x86_64.tar.gz

# Extract
tar -xzvf omni_engine_v2.1.1_linux_x86_64.tar.gz
cd omni_engine
```

### 2. Add Any GGUF Model
Drop any GGUF model into the `./models` directory (Omni Engine automatically detects it):

```bash
mkdir -p models
# Example: Download Qwen2.5-Coder-1.5B
wget -P models/ https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF/resolve/main/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf
```

### 3. Launch the Autonomous Engine

```bash
# Autonomous Project Generation
./bin/omni_engine run "Build a responsive Markdown-to-HTML converter with live preview and dark mode"

# Interactive Plan Review Mode (Review before execution)
./bin/omni_engine run "Implement an encrypted SQLite password vault" --review

# Incremental Refactoring / Modification Mode
./bin/omni_engine modify --input-dir ./my_project --goal "Add JWT authentication and rate limiting"
```

---

## 🛡️ Sovereign Security & Zero-Wipe Invariant

Omni Engine implements enterprise-grade sandboxing for untrusted LLM code:

1. **Bubblewrap Isolation:** Every code execution occurs in an unshared namespace (`--unshare-all`) with no network access, isolated PIDs, detached controlling TTY (`--new-session`), and a 64MB memory cap.
2. **Two-Phase Atomic Commit:** Deliverables are validated in Phase 1 (dry-run checking intermediate symlinks, absolute path breakouts, and collisions). Phase 2 commits atomically using unguessable temporary files (`NamedTempFile`), `fsync` disk durability, mode preservation, and atomic `rename(2)`.
3. **Zero-Wipe Guarantee:** Pre-existing files in refactoring mode are protected against accidental deletion by the model.

---

## 🔗 Links & Resources

- **GitHub Repository:** [ielfeqi-rgb/omni_engine](https://github.com/ielfeqi-rgb/omni_engine)
- **Releases & Checksums:** [v2.1.1 Security Release](https://github.com/ielfeqi-rgb/omni_engine/releases/tag/v2.1.1)
- **License:** MIT Open Source License
