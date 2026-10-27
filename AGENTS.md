# AGENTS.md — Omni Engine Autonomous Integration Specification
**Version:** 2.1.1  
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
  * `--review`: *(Recommended)* Prompts for human/agent review of the topological DAG plan before code generation.
  * `-y`, `--yes`: Non-interactive mode. Automatically approves missing standard library triage.
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

## 5. Architectural Invariants & Guarantees

### 5.1 The Zero-Wipe Invariant
In `modify` mode, the engine treats pre-existing working logic as immutable invariants:
* Workers are instructed: `[MANDATORY PRESERVATION INVARIANT: You MUST preserve all existing working logic]`.
* Output diffs must be purely additive or surgical replacements (`+lines / -0 regressions`).
* The Cleanroom AST validator checks that no existing public functions or classes were dropped. If any function disappears, the worker is rolled back in KV-cache and instructed to retry with additive delta.

### 5.2 $O(1)$ Context Isolation Engine
When modifying large codebases (50+ files / 50K lines):
* The agent does **NOT** feed the entire codebase into context.
* Omni Engine's `distill_interface_contract` extracts public types, schemas, and signatures into a 25-line contract.
* Worker attention window is strictly clamped to **1,500 – 2,500 tokens**, preventing context degradation and out-of-memory crashes.

### 5.3 Causal KV-Rollback
* Transient reasoning tokens (e.g., `<think>...</think>`) are excised upon completion via Turn-Sealing, reclaiming **+3,800 tokens**.
* Failed worker generations are rewound via `kv_cache_seq_rm`, excising only the erroneous step while preserving ancestral context.

---

## 6. Exit Codes

| Exit Code | Meaning | Remediation |
| :---: | :--- | :--- |
| `0` | Success. Code deliverables verified and committed to disk. | Read files from `--output-dir`. |
| `1` | General runtime / arguments error. | Check command syntax via `omni_engine --help`. |
| `2` | Cleanroom verification failure. | Check Bubblewrap sandbox or compiler toolchains. |
| `130` | Terminated by user / SIGINT. | Clean up ephemeral temp files in RAM. |

---

## 7. Machine Integration Example (JSON Tool Calling)

For agents with Bash / Process execution tools:

```json
{
  "tool": "run_command",
  "parameters": {
    "CommandLine": "./bin/omni_engine modify \"Add rate limiting middleware\" --dir ./backend --output-dir ./backend -y",
    "toolSummary": "Incremental rate limiting extension with zero-wipe preservation"
  }
}
```

---

## 8. Anti-Pattern: The LLM Prompt-Pasting Trap (Negative Example)

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
