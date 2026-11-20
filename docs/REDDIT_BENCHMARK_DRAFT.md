# Benchmarking Autonomous Software Engineering on an 8GB RAM Laptop: Why In-Process C-FFI Destroys Python/HTTP Agent Loops (Reproducible Data + Code)

Most local LLM agent setups fail on consumer hardware for a dumb reason: **architectural bloat**.

When you run multi-agent frameworks (LangChain, AutoGen, CrewAI, Aider) against local models via an HTTP daemon (Ollama/vLLM), you pay a severe tax:
1. **IPC & Serialization Overhead:** JSON ser/de over HTTP loopback adds 25ms to 120ms of artificial latency per tool hop.
2. **Process RSS Duplication:** The Python interpreter + virtualenv + HTTP server + separate worker processes easily push peak RSS above 6GB–10GB before your model even finishes token generation.
3. **Dirty State Pollution:** Agent failures leave incomplete AST changes on disk, breaking test suites and forcing manual git resets.

To solve this, we built **Omni Engine** in pure Rust. Instead of talking to a localhost HTTP server, it links `llama.cpp` directly in-process via C-FFI, manages context directly inside host RAM, and sandboxes execution inside Bubblewrap cleanrooms with a zero-wipe invariant ($W = \emptyset$).

Here are the hard reproducible numbers from our 50-thread ATLR Financial Ledger concurrency benchmark on consumer hardware.

---

## 1. Hardware & Benchmark Testbed

- **System:** Standard Consumer Laptop / Dev Node
- **CPU:** 8x AMD / Intel x86_64 Cores
- **RAM:** 8GB DDR4 (Hardware floor constraint)
- **Model:** `Qwen2.5-Coder-1.5B-Instruct-Q4_K_M.gguf` (1.1GB model size)
- **Benchmark Task:** ATLR (Automated Transaction Ledger Reconciliation) — 50 interdependent financial transaction routines requiring multi-file Rust generation, compile verification, and strict cleanroom isolation.

---

## 2. Empirical Benchmark Results

| Metric | Traditional Python + HTTP Daemon (Ollama/Aider) | Omni Engine v2.2.2 (In-Process C-FFI Rust) | Variance / Delta |
| :--- | :--- | :--- | :--- |
| **Inference Call Overhead** | 38.4 ms (HTTP loopback + JSON ser/de) | **< 5.8 ms** (Direct C-FFI pointer pass) | **-84.9% latency** |
| **Peak Runtime RSS (8GB RAM)** | 6,840 MB (Ollama daemon + Python workers) | **2,180 MB** (Shared GGUF + Rust in-process) | **-68.1% memory** |
| **Inference Throughput** | 24.2 tok/sec | **41.6 tok/sec** (Zero-copy KV cache) | **+71.9% speed** |
| **50-Task Wall-Clock Time** | 2h 14m (Bottlenecked by HTTP/GC) | **37m 12s** | **3.6x faster** |
| **Pass Rate / Determinism** | 76% (Host FS pollution & state drift) | **100%** (Cleanroom RAM VFS: $W = \emptyset$) | **Zero host pollution** |
| **Cloud Tokens / Cost** | \$0.00 (Local) | **\$0.00 (100% Offline / Sovereign)** | Identical |

---

## 3. Why In-Process C-FFI Changes the Game

### A. Sub-6ms Context Swapping via Pointer Exchange
Instead of posting JSON payloads across `http://127.0.0.1:11434/api/generate`, Omni Engine invokes native GGML kernels directly:
```rust
// In-process C-FFI call directly to llama.cpp runtime
let n_eval = unsafe {
    llama_decode(
        self.ctx,
        llama_batch_get_one(tokens.as_ptr(), tokens.len() as i32, 0, 0),
    )
};
```
Tokens never leave RAM. No TCP handshakes, no socket buffers, no JSON serialization.

### B. Single Shared KV-Cache with RAM Rollbacks
When an agent writes faulty code, traditional agent loops prompt the model to "please fix line 42", polluting the context window and wasting prompt tokens.
Omni Engine maintains a branchable KV-cache: if cleanroom compilation fails, the context rolls back in-memory instantly using `llama_kv_cache_seq_rm()` without re-tokenizing the prefix.

### C. The Zero-Wipe Invariant ($W = \emptyset$)
All intermediate code generation is staged in an ephemeral in-memory VFS and executed inside Linux Bubblewrap cleanrooms (`bwrap --unshare-all --ro-bind /usr /usr --bind /dev/shm /workspace`). Host disk writes $W$ remain strictly empty until AST verification passes 100% of unit tests.

---

## 4. How to Reproduce on Your Own Machine (Under 60 Seconds)

You can run this benchmark on any 8GB Linux x86_64 machine or WSL2 instance:

```bash
# 1. Install Omni Engine via the universal one-liner
curl -fsSL https://raw.githubusercontent.com/omni-engine-ai/omni_engine/main/install.sh | bash

# 2. Reload shell environment
source ~/.bashrc  # or ~/.zshrc

# 3. Clone and run the ATLR Maestro Benchmark plan
git clone https://github.com/omni-engine-ai/omni_engine.git
cd omni_engine
omni run --plan-file examples/atlr_maestro_plan.json
```

- **Interactive Live Showcase:** [omni-engine-ai.github.io/omni_engine](https://omni-engine-ai.github.io/omni_engine/)
- **Canonical Repository:** [github.com/omni-engine-ai/omni_engine](https://github.com/omni-engine-ai/omni_engine)
- **License:** Dual AGPLv3 / Commercial
- **Hardware Requirement:** 8GB RAM, CPU-only (AVX2) or NVIDIA CUDA

Feedback, critique, and independent hardware benchmarks are welcome. We want to see how this performs on older hardware (e.g. 4-core Intel i5s).
