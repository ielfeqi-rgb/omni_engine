# OmniEngine v2.1.2 Release Notes: "LLM's Compatibility Update"

**Release Date:** October 27, 2026  
**Target Architecture:** Linux x86_64 (AVX2 / Haswell / Modern CPU)  
**Verification:** In-Process Sovereign Swarm on Consumer Hardware (2 cores / 4 threads, 8GB RAM)  
**Cleanroom Verdict:** 100% Pass Rate across Bubblewrap (`bwrap`) Sandboxes

---

## 🚀 Executive Summary

OmniEngine **v2.1.2 ("LLM's Compatibility Update")** introduces the **Decoupled Maestro Architectural Protocol**, fundamentally transforming how sovereign local AI engines collaborate with frontier cloud models (Claude 3.5 Sonnet, GPT-4o, Gemini 1.5 Pro). 

Prior to v2.1.2, small local models (1.5B–4B parameters) struggled with two distinct failure modes:
1. **Planning Hallucination**: Small System 2 models attempting to architect complex 5–15 file codebases often suffered from context collapse, cyclic dependencies, or missing contracts.
2. **Execution Drift**: Workers lacked structural boundaries, frequently duplicating classes already declared in upstream modules or exceeding hardcoded context windows.

With v2.1.2, the **planning and execution concerns are decoupled**:
- **Frontier LLMs (Maestro)** author a deterministic, mathematically grounded JSON blueprint (`omni_plan.json`).
- **OmniEngine (Sovereign Swarm)** parses the blueprint into a directed acyclic graph (DAG), dynamically sizes KV-caches, enforces strict symbol contracts, isolates workers in Bubblewrap cleanrooms, and commits production-ready code directly to disk.

---

## 🛠️ Major Architectural Additions

### 1. Sovereign Maestro Plan Protocol (`--plan-file`)
Users and external LLMs can now provide rich architectural blueprints via the `--plan-file <path>` CLI flag. The blueprint defines:
- **`nodes`**: Discrete deliverables with explicit responsibilities, imports, invariants, and acceptance criteria.
- **`symbols`**: Concrete canonical method and class signatures.
- **`depends_on`**: Directed acyclic dependencies defining execution order.
- **`concurrency_limit`**: Thread safety ceiling respecting host hardware.

```bash
omni_engine run --plan-file ./omni_plan.json --output-dir ./MyProject --model ./models/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf
```

### 2. Pre-Sandbox Symbol Contract Gate
An instant, zero-latency AST scanner verifies that mandatory classes, functions, and symbols exist in RAM VFS before executing code in the sandbox. If an implementation is incomplete, the gate rejects the deliverable immediately (`ERR_MISSING_CONTRACT_SYMBOL`), preventing downstream workers from crashing with import errors.

### 3. Anti-Redeclaration Guard
Prevents smaller models from repeating or duplicating classes that already exist in upstream dependencies. The guard cross-checks newly generated code against all deliverables in `read_set`. If a duplicate is detected, it raises `ERR_FORBIDDEN_REDECLARATION` and instructs the worker to import the existing symbol instead.

### 4. Dynamic Dependency-Aware KV-Cache Scaling
Replaced static context allocations with dynamic context scaling:
- Context windows scale smoothly from **4,096 tokens** up to **16,384 tokens** based on the depth of the dependency graph:
  $$\text{max\_tokens} = \min(16384, \max(4096, 4096 + |\text{depends\_on}| \times 1024))$$
- Eliminates `KV-cache overflow` errors during multi-module builds.
- Upstream deliverables exceeding 40 lines are distilled into lean public interface contracts (`AUTO_INJECT_MAX_LINES = 40`), saving hundreds of prompt tokens.

### 5. Guidance Preservation in Self-Healing Loops
When a worker's attempt fails cleanroom checks, the System 2 Thinker previously overwrote the original blueprint with a brief critique. v2.1.2 preserves the full architectural blueprint in memory, cleanly appending targeted self-healing corrections without polluting code blocks.

---

## 🔬 Empirical Validation: The `OmniLite` Proof

To empirically validate the protocol, OmniEngine generated **OmniLite** (a complete 6-file DAG execution engine) using a single **1.5B model** (`Qwen2.5-Coder-1.5B-Instruct-Q4_K_M`) on an Intel Core i5-6300U (2 cores, 4 threads):

| Component | Responsibility | Tokens | Result |
| :--- | :--- | :--- | :--- |
| `models.py` | `TaskStatus` Enum & `Task` dataclass | 250 | **Verified Cleanroom** (Attempt 1) |
| `storage.py` | `TaskStore` with atomic file persistence | 350 | **Verified Cleanroom** (Attempt 1) |
| `graph.py` | `DependencyGraph` with Kahn's DAG batching | 480 | **Verified Cleanroom** (Attempt 1) |
| `executor.py` | `PipelineExecutor` & `PipelineResult` | 520 | **Verified Cleanroom** (Attempt 1) |
| `cli.py` | CLI with `validate`, `run`, `status` subcommands | 540 | **Verified Cleanroom** (Attempt 1) |
| `test_suite.py` | End-to-end `unittest` verification suite | 680 | **Verified Cleanroom** (Attempt 1) |

- **Milestones**: 4 out of 4 topological checkpoints verified and sealed.
- **Test Suite**: 7 out of 7 unit tests passed in 0.012 seconds (`OK`).
- **CLI Verification**:
  - `python3 cli.py validate --file pipeline.json` $\to$ **SUCCESS**
  - `python3 cli.py run --file pipeline.json --storage store.json` $\to$ **SUCCESS (2/2 passed)**
  - `python3 cli.py status --storage store.json` $\to$ **SUCCESS**

---

## 📦 Compatibility & Breaking Changes
- **Backward Compatible**: Existing interactive CLI flows, single-goal runs, and pre-pass triage remain fully supported.
- **Hardware Requirement**: Remains 100% sovereign, requiring zero cloud APIs or GPUs.
- **Rust Toolchain**: Fully conforms to `#![deny(unused_must_use)]` with 135 unit tests passing.
