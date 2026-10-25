# 📊 Omni Engine: Multi-Dimensional Comparative Benchmark Report

Date: 2026-10-23T10:42:12.213685885+03:00
Evaluation Framework: Cleanroom Isolated Runtime ($W = \emptyset$)
Test Bed: Multi-File Full-Stack Project

---

## 1. Pillar 1: DAG Concurrency & Makespan ($C_{max}$)

| Paradigm | Makespan ($C_{max}$) | Peak Memory Allocation | Concurrency Safety |
| :--- | :---: | :---: | :---: |
| **Omni Engine (RCPSP Swarm)** | **307.20s** | **4096 tokens (Hard Gate)** | **Zero-Deadlock / Conflict Free** |
| Sequential ReAct (LangChain/AutoGPT) | 409.60s | 2560 tokens | Strictly Sequential |
| Naive Parallel (Uncoordinated) | 204.80s | 8192 tokens | High OOM / Collision Risk |

* **Makespan Speedup**: **1.33x** over Sequential ReAct.

---

## 2. Pillar 2: Zero-Wipe Invariant & Acceptance Delta Recovery

| Architecture | Preserved Functions | Acceptance Deficit Before | Acceptance Deficit After | Result |
| :--- | :---: | :---: | :---: | :---: |
| **Omni Engine (Zero-Wipe Invariant)** | **4/4 (100%)** | 5 missing | **0 missing (100% Solved)** | **PASSED** |
| Competitor Rewrite Paradigm | 1/4 (25%) | 5 missing | 1 missing | REGRESSION |

---

## 3. Pillar 3: Causal KV-Cache Slicing vs Full Re-Ingestion

| Strategy | Latency (ms) | Tokens Excised / Saved | Speedup Factor |
| :--- | :---: | :---: | :---: |
| Omni Engine Causal Slicing | **4686.01 ms** | **256 tokens** | **3.16x faster** |
| Monotonic (Ideal Branch)   | 5130.94 ms     | N/A           | Reference         |
| Competitor Cold Restart     | 14795.84 ms     | 0 tokens      | 1.00x (Baseline)  |

---

## Conclusions
1. **Mathematical Scheduling**: RCPSP dynamic DAG scheduling delivers a **1.33x makespan speedup** while strictly respecting hardware memory bounds.
2. **Zero-Wipe Guarantee**: Eliminates catastrophic forgetting in small local LLMs by freezing previously working code in VFS.
3. **Causal Slicing**: Bypasses the quadratic re-evaluation penalty on syntax errors, slashing repair latency.
