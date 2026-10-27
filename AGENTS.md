# AGENTS.md — Omni Engine Autonomous Integration Specification
**Version:** 2.1.2  
**Target Audience:** Autonomous Coding Agents, LLM Orchestrators, Tool-Calling Systems (Claude, GPT-4, Cursor, Cline, Aider)  
**Encoding:** UTF-8 • Markdown Specification

---

> [!CAUTION]
> ### 🛑 CRITICAL DIRECTIVE FOR EXTERNAL LLMS (ChatGPT, Claude, Cursor, Copilot)
> 1. **DO NOT output conversational English prompts** (e.g., *"Read the project's AGENTS.md file and follow its instructions..."*) for the user to paste into the terminal. The terminal is a Linux Bash shell, **NOT an LLM chat prompt**. Pasting English sentences will crash with `bash: Read: command not found`.
> 2. **DO NOT write or generate code files yourself.**
> 3. **`omni_engine` is the local compiler & software engineer.** Your ONLY role as an external AI assistant is to construct and provide the single executable Bash command calling `./bin/omni_engine`.
>
> **The ONLY valid output to give the user is a shell execution command, e.g.:**
> ```bash
> ./bin/omni_engine run "<CONCISE_REQUIREMENTS_SUMMARY>" --output-dir ./<TARGET_DIR> -y
> ```
> *(Condense all functional requirements, unit test expectations, and file constraints into the `<goal>` argument string).*

---

## 1. System Overview & Agentic Role

Omni Engine is a sovereign, 100% offline multi-agent software engineering runtime. It executes in-process via C-FFI linking `llama.cpp` directly in RAM, operating as a deterministic compiler swarm for multi-file codebases.

When an LLM agent uses Omni Engine as a subprocess or CLI tool, it must adhere to the deterministic contracts described below.

---

## 2. Decision Tree: `run` vs. `modify`

```
Is the task operating on an existing repository or directory?
│
├── NO  --> Use 'run' (Autonomous System Synthesis from Scratch)
│           Creates new architecture, DAG, and modular files in RAM VFS.
│           Syntax: omni_engine run "<goal>" --output-dir <path>
│
└── YES --> Use 'modify' (Incremental Refactor with Zero-Wipe Guarantee)
            Preloads existing repository files into RAM VFS before planning.
            Enforces strict Tree-Sitter AST preservation (+lines / -0 regressions).
            Syntax: omni_engine modify "<goal>" --dir <path> --output-dir <path>
```

---

## 3. CLI Command Specifications & Grammar

### 3.1 Synthesis (`run` / `swarm` / `goal`)
```bash
omni_engine run "<goal_description>" [OPTIONS]
```
* **Parameters:**
  * `<goal_description>`: High-level system requirements string.
  * `--output-dir <DIR>`: Directory path where verified code deliverables will be committed (default: `./workspace`).
  * `--review`: *(Recommended for Interactive Sessions)* Prompts for human/agent review of the topological DAG plan before code generation.
  * `-y`, `--yes`: Headless / non-interactive mode. Bypasses interactive `--review` stdin prompts and auto-approves dependency healing triage. (Mandatory for CI/CD and automated agents).
  * `--subgoals <N>`: Maximum topological DAG tasks (default: 3).
  * `--steps <N>`: Maximum worker self-healing attempts per task (default: 5).
  * `--model <PATH>`: Explicit path to unified GGUF model in RAM.

### 3.2 Refactoring (`modify` / `refactor` / `patch`)
```bash
omni_engine modify "<modification_goal>" --dir <existing_project_path> [OPTIONS]
```
* **Parameters:**
  * `<modification_goal>`: Specific feature extension, bugfix, or refactoring objective.
  * `--dir <DIR>`: **[MANDATORY]** Path to the existing project directory to preload into RAM VFS.
  * `--output-dir <DIR>`: Path to commit patched deliverables (typically identical to `--dir`).
  * `--review`: Displays plan card comparing pre-existing functions against proposed delta.

### 3.3 Sovereign Maestro Architectural Plan Protocol (`--plan-file` / `--plan`)
When high-capacity Big Cloud LLMs (Claude 3.5 Sonnet, GPT-4o, Gemini 1.5 Pro) or human software architects design the system, they bypass local orchestrator planning by outputting an ultra-precise JSON blueprint.

```bash
omni_engine run --plan-file ./omni_plan.json --output-dir <DIR> [OPTIONS]
```
* **Parameters:**
  * `--plan-file <PATH>`: Path to structured JSON blueprint (`omni_plan.json`).
  * `--plan "<JSON_OR_SPEC>"`: Inline JSON or pipe-delimited subgoal specification.
  * `--concurrency <N>`: Maximum parallel worker threads capped to CPU physical cores (default: 2).

#### The Maestro JSON Blueprint Schema:
```json
{
  "project": "ProjectName",
  "language": "python",
  "concurrency_limit": 2,
  "nodes": [
    {
      "id": 1,
      "entity": "models.py",
      "kind": "CREATE",
      "responsibility": "Concise single-sentence file responsibility.",
      "depends_on": [],
      "imports": ["from dataclasses import dataclass", "from typing import List"],
      "symbols": {
        "classes": [
          {
            "name": "Record",
            "type": "dataclass",
            "fields": [{"name": "id", "type": "str"}, {"name": "val", "type": "int"}],
            "methods": [{"name": "to_dict", "returns": "dict"}]
          }
        ]
      },
      "invariants": [
        "Zero third-party dependencies: strictly standard library.",
        "Deterministic JSON serialization."
      ],
      "acceptance_tests": [
        "Instantiate Record, serialize to dict, verify field types."
      ]
    },
    {
      "id": 2,
      "entity": "storage.py",
      "kind": "CREATE",
      "responsibility": "Atomic state persistence layer.",
      "depends_on": ["models.py"],
      "imports": ["import json", "from models import Record"],
      "symbols": {
        "classes": [
          {
            "name": "Store",
            "methods": [{"name": "save", "params": ["record: Record"], "returns": "None"}]
          }
        ]
      },
      "invariants": [
        "MUST import Record from models; NEVER re-declare Record class.",
        "Atomic disk persistence via temporary file and os.replace."
      ],
      "acceptance_tests": [
        "Save and load records round-trip."
      ]
    }
  ]
}
```

#### Why This Decoupled Protocol Wins:
1. **Zero Hallucination:** Local 1.5B/3B workers receive exact AST symbols, type signatures, and import lines directly in their prompt.
2. **Zero Orchestrator Latency:** Planning time drops from 45 seconds to 0 milliseconds.
3. **Hardware Efficiency:** Execution runs at maximum physical CPU memory throughput without context bloat ($O(1)$ context isolation).

---

## 4. Human-in-the-Loop Steering Protocol (`--review`)

When `--review` is active, the engine pauses after the Thinker synthesizes the DAG and prints an architecture card to `stdout`:

```text
================================================================================
                    :: ARCHITECTURAL PLAN REVIEW & STEERING ::                  
================================================================================
Goal: ...
Deliverables: [models.py, repository.py, cli.py]
DAG Subgoals:
  1. [MODIFY] models.py: Add priority field (LOW, MEDIUM, HIGH)
  2. [MODIFY] repository.py: Add filter_by_priority and export_to_json
  3. [MODIFY] cli.py: Wire CLI commands
================================================================================
[?] Approve this plan to dispatch workers? [Y/n/critique]:
```

### Agent Steering Rules:
1. **To Approve:** Send `"yes"` or `"y"` or `"\n"` to `stdin`.  
   *Action:* The engine seals the plan into Causal Memory and dispatches worker threads.
2. **To Reject & Steer:** Send `"no <critique>"` or `"<critique>"` to `stdin`.  
   *Action:* The engine executes **Causal KV-Rollback**, surgically excises the unapproved plan from the KV-cache (~6ms), re-prompts the Thinker incorporating the critique, and prints Plan v2 for re-evaluation.

---

## 5. Architectural Invariants & Rigorous Guarantees

### 5.1 The Zero-Wipe Invariant
In `modify` mode, the engine treats pre-existing working logic as immutable invariants:
* **Structural Guarantee (AST Symbol Invariant):** The Tree-Sitter AST validator locks all pre-existing public classes, methods, and functions. If any pre-existing public symbol disappears from the staged file in RAM VFS, the deliverable is rejected and the worker is rolled back in KV-cache to retry with an additive delta.
* **Behavioral Guarantee (Cleanroom Test Execution):** Preserving AST signatures alone does not prove behavioral correctness. Semantic regression prevention is enforced by executing the project's test suite inside the Linux Bubblewrap sandbox (`bwrap`) before any byte is committed to disk.

### 5.2 Constant-Bounded Attention Window ($O(1)$ Attention Ceiling)
When modifying large codebases (50+ files / 50,000+ lines):
* The agent does **NOT** feed the entire codebase into context.
* Omni Engine's `distill_interface_contract` scans dependency files ($O(N)$ AST extraction over repository size) and distills them into compact 25-line type schemas and API interfaces.
* **Result:** Worker attention window remains strictly clamped to a constant **1,500 – 2,500 tokens** regardless of repository depth, eliminating context degradation and out-of-memory crashes.
* **$O(1)$ Tail KV-Rollback:** Slicing unapproved turns or syntax errors from physical RAM is $O(1)$ via `kv_cache_seq_rm`.

### 5.3 Causal KV-Rollback & Turn-Sealing
* Transient reasoning tokens (e.g., `<think>...</think>`) generated by models like DeepSeek-R1 or Qwen3 are excised upon completion via Turn-Sealing, reclaiming **+3,800 tokens**.
* Failed worker generations are rewound via `kv_cache_seq_rm`, excising only the erroneous step while preserving ancestral context.

---

## 6. Granular Exit Codes

| Exit Code | Meaning | Remediation |
| :---: | :--- | :--- |
| `0` | **Success**: Code deliverables passed cleanroom tests and committed to disk. | Read verified files from `--output-dir`. |
| `1` | **CLI / Syntax Error**: Invalid arguments or flags. | Check command syntax via `omni_engine --help`. |
| `2` | **Cleanroom Failure**: Code failed compilation or sandbox tests after max healing retries. | Inspect compiler/test stderr logs emitted in terminal output. |
| `3` | **Model Error**: No GGUF model detected or failed to initialize context in RAM. | Run `omni_engine download --qwen` or provide `--model <PATH>`. |
| `4` | **Zero-OOM Memory Gate Rejection**: Task requested more tokens than maximum safe KV capacity ($K_{max}$). | Split task into smaller subgoals or increase physical RAM. |
| `130` | **SIGINT**: Interrupted by user or calling process. | Clean up ephemeral temp files in RAM. |

---

## 7. Machine Integration Example (JSON Tool Calling)

For agents with Bash / Process execution tools:

```json
{
  "tool": "run_command",
  "parameters": {
    "CommandLine": "./bin/omni_engine modify \"Add rate limiting middleware\" --dir ./backend --output-dir ./backend_patched -y",
    "toolSummary": "Incremental rate limiting extension with zero-wipe preservation"
  }
}
```

> [!TIP]
> **Commit Gate Best Practice for Agents:**  
> When running non-interactively with `-y`, specify a distinct `--output-dir` (e.g., `./backend_patched`) so the calling agent or human can review `git diff` before merging, or ensure the working tree is clean (`git status --porcelain`) before modifying in-place.

---

## 8. Host Prerequisites & Sandboxing Contract

To maintain absolute transparency:
1. **Inference Runtime (Zero Server Prerequisites):** Omni Engine requires **NO external daemon processes**, background HTTP servers, Docker containers, or Python runtimes for inference. `llama.cpp` is linked directly in-process via C-FFI.
2. **Cleanroom Sandbox Prerequisites:** To execute and audit generated code safely without risking host filesystem corruption, the host system requires:
   * **Bubblewrap (`bwrap`)**: Linux user namespace container isolation (`sudo apt install bubblewrap` / `sudo dnf install bubblewrap`).
   * **Target Language Compilers / Interpreters**: The runtime needed to execute your target project's tests (e.g., `python3`, `cargo`, `node`).
   * *Fail-Closed Guarantee:* If Bubblewrap is missing, Omni Engine refuses to run untrusted code on the physical host by default, unless explicitly authorized via `--allow-unsandboxed`.

---

## 9. Anti-Pattern: The LLM Prompt-Pasting Trap (Negative Example)

When an end-user asks an external assistant:  
*"Help me create a Python calculator using Omni Engine and follow AGENTS.md"*

### ❌ WRONG (What a hallucinating assistant does):
The assistant generates conversational prompt instructions telling the user to paste this into bash:
```text
Read the project's AGENTS.md file and follow its instructions exactly. Create a new Python project named mini_calculator... Requirements: 1. Create calculator.py...
```
*(Result: Immediate failure. Linux Bash interprets `Read` as an unrecognized executable and aborts).*

### ✅ RIGHT (What a correct assistant does):
The assistant condenses all specifications into the executable CLI command:
```bash
./bin/omni_engine run "Create a complete Python mini_calculator project with calculator.py (implementing addition, subtraction, multiplication, and safe division by zero handling), test_calculator.py using unittest, and README.md. Verify by running python -m unittest." --output-dir ./workspace/mini_calculator -y
```
