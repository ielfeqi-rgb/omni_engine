# Omni Engine: Technical Whitepaper & Architectural Specification
**A Comprehensive Systems Engineering & Computational Reference**

---

## 1. Foundational Philosophy

**Omni Engine** is a high-performance, deterministic autonomous software engineering runtime written in pure Rust and coupled directly to upstream `llama.cpp` via low-level in-process C-FFI bindings.

The foundational design objective is to break the paradigm of **"blind autoregressive generation."** Rather than treating Large Language Models—especially Small Language Models (SLMs, 1.5B–7B)—as unconstrained conversational storytellers, Omni Engine constrains them into **Deterministic Processing Units (DPUs)** within a finite state machine governed by graph theory, set algebra, and static AST analysis.

```text
+-----------------------------------------------------------------------------+
|                                 OMNI ENGINE                                 |
|                                                                             |
|  +--------------------+     +-------------------+     +------------------+  |
|  |     Rust Host      | <-> | Synchronous FFI   | <-> |  libllama (C++)  |  |
|  |  (State Machine)   |     | (Atomic Step/Halt)|     | (Inference Core) |  |
|  +--------------------+     +-------------------+     +------------------+  |
|            |                                                    |           |
|            v                                                    v           |
|  +--------------------+                               +------------------+  |
|  | AST Gatekeeper     |                               | Persistent       |  |
|  | (Rust AST Scanner) |                               | KV-Cache Matrix  |  |
|  +--------------------+                               +------------------+  |
+-----------------------------------------------------------------------------+
```

---

## 2. Attention Physics & KV-Cache Mechanics

### 2.1 Attention Mask Manipulation
Orchestration of localized worker threads relies on direct, in-process manipulation of the attention mask matrix $M \in \mathbb{R}^{N \times N}$ inside the compute kernel:

$$\text{Attention}(Q, K, V) = \text{Softmax}\left(\frac{Q K^T}{\sqrt{d_k}} + M\right) V$$

Where:
* $Q \in \mathbb{R}^{L \times d_k}$ is the query matrix for the current active tokens.
* $K, V \in \mathbb{R}^{S \times d_k}$ are key and value tensors retained inside the persistent KV-cache.
* $M_{i, j}$ defines token-to-token visibility:

$$M_{i, j} = \begin{cases} 0 & \text{if token } j \text{ is visible to token } i \\ -\infty & \text{if token } j \text{ is masked / obscured} \end{cases}$$

Under the Softmax transformation:
$$\exp(-\infty) = 0$$

Masked tokens receive zero attention weight, completely eliminating their contribution to $V$ aggregation while retaining their cached representations in memory without costly eviction or recomputation.

### 2.2 Attention Sinks
To maintain numerical stability across deep transformer layers and prevent softmax degradation, Omni Engine enforces persistent visibility over the first 4 tokens of the context sequence:
$$\forall i \ge 0, \quad M_{i, 0..3} = 0$$

These tokens function as **Attention Sinks**, absorbing residual softmax mass and preventing numerical overflow or underflow across multi-turn generation.

### 2.3 Zero-Compute Memory Retention
The KV-cache is a static RAM data structure. Retaining cached tokens imposes $0.0\%$ CPU compute overhead. Dynamic context modification is executed via microsecond surgical slice operations:
* **Surgical Slice Truncation:**
  $$\text{llama\_kv\_cache\_seq\_rm}(\text{ctx}, \text{seq\_id}, p_0, p_1)$$
  Excises only the oscillating or failed token span $[p_0, p_1)$, retaining the valid prefix intact without requiring cold context recalculations.

---

## 3. Mathematical Entropy Analysis & Node Fission

### 3.1 Instantaneous Shannon Entropy Monitoring
Omni Engine does not rely on fragile heuristic regexes to detect model confusion or token looping. Instead, it computes the Shannon entropy of the softmax probability distribution at each generation step $t$:

$$H(t) = -\sum_{i=1}^{|V|} P(w_i) \log_2 P(w_i)$$

Where:
* $|V|$ is the total model vocabulary size.
* $P(w_i)$ is the computed probability for candidate token $w_i$.

### 3.2 Fission Trigger & Cutpoint Identification
The engine tracks the moving average of entropy over a sliding window of length $K$:

$$\bar{H}_K(t) = \frac{1}{K} \sum_{\tau=t-K+1}^t H(\tau)$$

When entropy exceeds the stability threshold:
$$\bar{H}_K(t) > H_{\text{threshold}}$$

The Rust host halts generation immediately and identifies the optimal rollback cutpoint $T_{\text{cut}}$ at the nearest preceding entropy minimum:
$$T_{\text{cut}} = \arg\min_{\tau \le t} H(\tau)$$

### 3.3 Worker Fission Protocol
* **Freeze Worker A:** Context from token 0 to $T_{\text{cut}}$ is sealed and marked read-only.
* **Spawn Worker B:** The subsequent worker thread initializes with Rotary Position Embedding (RoPE) offset at $T_{\text{cut}} + 1$.
* **Epistemic Intent Anchor:** The original architectural objective is retained at sequence prefix index 0, ensuring consistent task alignment across worker resets.

---

## 4. Deterministic DAG Scheduling & Graph Theory

### 4.1 Stepwise Forcible Halting
Project goals are decomposed dynamically without pre-baked prompt templates by forcibly intercepting generation at syntactic line boundaries:
```text
[Prompt: Project Entity 1:] 
   -> LLM generates: "Block" 
   -> Rust Interrupt Engine (Halt on '\n')
[Append to KV: Project Entity 2:] 
   -> LLM generates: "World" 
   -> Rust Interrupt Engine (Halt on '\n')
[Append to KV: Project Entity 3:] 
   -> LLM generates: "Player" 
   -> Rust Interrupt Engine (Halt on '\n')
```

### 4.2 Adjacency Matrix Construction
Inter-entity dependencies are resolved via atomic, single-token directional inquiries:
* `"To define Player, which entity is required first?"` $\to$ `World`
* `"To define World, which entity is required first?"` $\to$ `Block`

Dependencies populate an adjacency matrix $A \in \{0, 1\}^{N \times N}$, where $A_{i, j} = 1$ denotes that entity $j$ depends on entity $i$.

### 4.3 Kahn's Algorithm for Topological Scheduling
The engine applies Kahn's algorithm to resolve a deterministic, cycle-free topological execution order:

```text
L <- Empty list that will contain the sorted elements
S <- Set of all nodes with no incoming edges (In-Degree = 0)

while S is not empty:
    remove a node n from S
    append n to L
    for each node m with an edge e from n to m:
        remove edge e from the graph
        if m has no other incoming edges:
            insert m into S

if graph has edges:
    return Error (Circular Dependency Detected)
else:
    return L (Deterministic Execution Schedule)
```

### 4.4 Fractal Drill-Down Decomposition
When scheduling complex composite entities:
* **Small Models ($N_{\text{scratch}} = 0$):** Sub-components are extracted via strict stepwise line halting (e.g., `Movement`, `Interaction`).
* **Large Models:** A temporary scratchpad budget ($N_{\text{sub-scratch}} \le 100$ tokens) is allocated for internal architectural reasoning, followed by instantaneous KV truncation once module signatures are extracted.

---

## 5. AST Gatekeeping & Static Verification

### 5.1 Contract Extraction
Upon completion of a generated module, the code is passed immediately through an in-memory Abstract Syntax Tree (AST) parser (Native Zero-Dependency Rust AST/Symbol Scanner):
* $\text{AST}_{\text{provides}}$: Concrete exported types, structs, enums, functions, and traits defined within the file.
* $\text{AST}_{\text{requires}}$: External symbols and types imported or referenced without local declarations.

### 5.2 Set Algebra & Ghost Symbol Rejection
The engine applies formal set operations to verify interface contracts:

```text
                          +------------------------+
                          |   Worker Outputs File  |
                          +------------------------+
                                       |
                                       v
                          +------------------------+
                          |    Run AST Analysis    |
                          +------------------------+
                                       |
               +-----------------------+-----------------------+
               |                                               |
               v                                               v
    [Delta_ghost == Empty]                           [Delta_ghost != Empty]
               |                                               |
               v                                               v
       +---------------+                            +---------------------+
       | Commit to DAG |                            | Halt Execution      |
       +---------------+                            | In-Place Correction |
                                                    | (Inject Definition) |
                                                    +---------------------+
```

* If $\Delta_{\text{ghost\_provides}} \neq \emptyset$:
  The module is blocked from DAG commitment. The missing symbol declaration is injected directly into the worker's prompt prefix:
  ```rust
  // AST CONTRACT DIRECTIVE: Missing required symbol definition.
  pub struct Account { ... }
  ```
  The worker is constrained to complete only the missing contract definition (Strict Prefill Completion).

---

## 6. Human-in-the-Loop Triage Protocol

### 6.1 Centrality & Impact Scoring
When persistent ambiguity occurs, Omni Engine evaluates node centrality via its out-degree in the dependency matrix $A$:

| Execution Path | Mathematical Condition | Runtime Action |
|---|---|---|
| **Critical Path** | $\text{OutDegree}(v_i) \ge 3$ | Halt compute thread immediately, park KV-cache, and surface an interactive triage request to the human user. |
| **Deferred Path** | $\text{OutDegree}(v_i) < 3$ | Inject an `OMNI_DEFERRED_DECISION` marker, proceed with generation, and present the choice prior to final commit. |

### 6.2 Sovereign Rules Persistence
User decisions are permanently recorded in a structured table and injected as immutable constraints into all downstream worker contexts:
```text
[SOVEREIGN_RULES]:
- Rule #1: All financial transactions must use canonical lock ordering (min/max).
```
This guarantees zero redundant prompts for identical architectural decisions across subsequent turns.

---

## 7. OS-Level Synchronization & Zero-Power Hardware Parking

### 7.1 Zero-Power Parking via Futex
To prevent CPU spinning or thread burn while awaiting user decisions without evicting the KV-cache:
* The inference core runs on a dedicated OS thread (`std::thread`).
* When human triage is triggered, the thread creates a `tokio::sync::oneshot` channel.
* The thread executes `oneshot_rx.blocking_recv()`, triggering a Linux futex syscall:
```c
futex(uaddr, FUTEX_WAIT, val, NULL, NULL, 0);
```

```text
Compute Thread (C++ / llama.cpp)                UI Runtime (Rust / Tokio)
--------------------------------                -------------------------
[Active Token Generation]
       |
       v (Critical Ambiguity Detected)
[Create oneshot channel]
[Send TriageRequest to UI] ------------------> [Receive Event]
[Invoke: blocking_recv()]                      [Render Prompt to User]
       |                                                  |
       | (Thread parked via OS futex)                     v
       | (CPU Usage drops to 0.0%)               [Wait for User Input...]
       | (KV-Cache preserved in VRAM)                     |
       |                                                  v
       |                                         [User Submits Choice]
       | <-------------------------------------- [Send via oneshot_tx]
[Woken up by OS Scheduler]
[Inject choice into KV-Cache]
[Resume Forward Pass]
```

### 7.2 Hardware State During Parking
* **CPU Utilization:** Compute thread CPU consumption drops to strictly **$0.0\%$** as the OS moves the task into `WaitQueue`.
* **GPU Utilization:** Compute kernels suspend; GPU power drops to the idle baseline (Low P-State) while weights and KV tensors remain pinned in VRAM.
* **Wakeup Latency:** Upon receipt of user input, the OS scheduler wakes the compute thread in **$< 5\,\mu\text{s}$**, instantly resuming inference from the subsequent token.

---

## 8. Architectural Specification Summary

| Architectural Component | Mathematical & Systems Role |
| :--- | :--- |
| **Memory Retention** | Mask-driven KV-cache governance with zero periodic cache evictions. |
| **Mask Bounds** | Binary attention matrix ($0$ / $-\infty$) with guaranteed Attention Sink preservation. |
| **Hesitation Monitor** | Instantaneous Shannon entropy calculation tracking distribution collapse. |
| **Node Fission** | Entropy-triggered worker bifurcation preserving prefix intent. |
| **Topology Resolution** | Template-free Kahn's algorithm topological sort on the adjacency matrix. |
| **Stepwise Halter** | Deterministic symbol extraction via line boundary interrupts (`Halt on '\n'`). |
| **AST Verification** | Set-algebra verification of provided vs required symbols ($\Delta_{\text{ghost}} = \emptyset$). |
| **Sync Primitive** | Futex-level zero-power hardware thread parking ($0.0\%$ CPU load during triage). |

---
*Omni Engine v2.2.2 — Official Systems Architecture Specification.*
