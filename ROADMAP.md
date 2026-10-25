# Omni Engine Development Roadmap

This document outlines the strategic architectural milestones for Omni Engine, building upon the empirically verified foundation established in **v2.1.0 (Sovereign Runtime)**.

Our roadmap adheres strictly to the **"Grounded Systems Engineering"** philosophy: we do not pursue superficial AI buzzwords. Every milestone directly expands execution throughput, language coverage, and deterministic code safety on local commodity hardware.

---

```
┌────────────────────────────────────────────────────────────────────────┐
│                   Omni Engine Evolution Trajectory                     │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
    ┌───────────────────────────────┼───────────────────────────────┐
    ▼                               ▼                               ▼
[v2.2.0: Hardware Acceleration]    [v2.3.0: Multilingual AST]      [v2.4.0: Local Git Integration]
* Dynamic CUDA / Metal Auto-Link   * Tree-Sitter for TS/Rust/Go     * Ephemeral Git Branching
* 3.8 tok/s -> 40 tok/s on GPU     * Universal Contract Distiller   * Atomic Conventional Commits
* 8GB RAM Floor Preserved          * AST-guided Regression Isolation* Zero-Network Local Workflows
```

---

## Milestone 1: Dynamic Hardware Acceleration (v2.2.0)
*Target: Multiplying throughput from 3.8 tok/s to 40+ tok/s while preserving the 8GB RAM CPU sovereign baseline.*

- [ ] **Heterogeneous Dynamic C-FFI Dispatch**
  - Implement runtime hardware discovery probe: automatically inspect host for Nvidia CUDA, Apple Silicon Metal, or Vulkan compute units.
  - Dynamically dlopen GPU backend libraries (`libggml-cuda.so`, `libggml-metal.so`) from `./bin/` without requiring recompilation of the Rust binary.
- [ ] **Adaptive KV-Cache Paging**
  - Layer unified memory paging between GPU VRAM and host RAM, maintaining the zero-OOM memory gate ($K_{max}$) across hybrid hardware.
- [ ] **Zero-GPU Fallback Resilience**
  - Seamless, zero-latency fallback to pure AVX-512/AVX2 CPU execution if GPU memory is saturated or unavailable.

---

## Milestone 2: Multilingual AST Contract Synthesis (v2.3.0)
*Target: Scaling the Zero-Wipe Invariant and $O(1)$ Context Isolation to large polyglot enterprise codebases.*

- [ ] **Universal Tree-Sitter Contract Distiller**
  - Extend `distill_interface_contract` beyond Python/JavaScript to full AST grammar support for **TypeScript**, **Rust**, **Go**, and **C++**.
  - Extract public type definitions, trait implementations, and API signatures into compact 25-line contracts, ensuring worker attention remains clamped ($O(1)$ complexity) across 50,000+ line repositories.
- [ ] **AST-Guided Dependency Graph & Targeted Testing**
  - Construct an in-memory directed acyclic graph (DAG) of caller-callee relationships across files.
  - When modifying a function, automatically identify and execute only the specific test suites covering that function's callers.

---

## Milestone 3: Sovereign Git Patch & Branch Materialization (v2.4.0)
*Target: Bridging the in-memory RAM VFS cleanroom directly into professional Git workflows.*

- [ ] **Ephemeral Cleanroom Git Worktrees**
  - Automatically checkout ephemeral local Git worktrees (`omni/task-<id>`) for worker staging.
  - Commits to host disk occur as clean, atomic Git commits with verifiable Conventional Commit messages and test pass logs.
- [ ] **Automated Interactive Diffs & Rebase Verification**
  - Surface visual interactive Git diffs in the TUI console before finalizing commits.
  - Allow developers to accept, reject, or amend specific line hunks directly from the terminal.

---

## Milestone 4: Persistent Long-Horizon Refactoring Swarms (v3.0.0)
*Target: Multi-day codebase migrations and autonomous architectural refactoring.*

- [ ] **Cross-Session Causal DAG Checkpointing**
  - Serialize and snapshot the Causal Memory DAG into local SQLite storage, enabling the swarm to pause, reboot, and resume complex multi-day refactorings without re-planning.
- [ ] **Monolith Decomposition Protocol**
  - Introduce an autonomous migration planner capable of breaking down monolithic codebases into modular micro-packages file-by-file with continuous cleanroom verification.

---

> **Guiding Invariant:** *The core text-in/verified-software-out autonomous loop, the Zero-Wipe Invariant, and `#![deny(unused_must_use)]` remain immutable across all versions.*
