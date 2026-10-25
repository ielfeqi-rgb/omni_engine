# AGENTS.md — Omni Engine Autonomous Integration Specification
**Version:** 2.1.0  
**Target Audience:** Autonomous Coding Agents, LLM Orchestrators, Tool-Calling Systems (Claude, GPT-4, Cursor, Cline, Aider)  
**Encoding:** UTF-8 • Markdown Specification

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
    "CommandLine": "omni_engine modify \"Add rate limiting middleware\" --dir ./backend --output-dir ./backend -y",
    "toolSummary": "Incremental rate limiting extension with zero-wipe preservation"
  }
}
```
