# Project: Parallel DAG Swarm Engine (omni_engine)

## Architecture
- **Coordinator & Scheduler** (`src/planner/swarm_coordinator.rs`):
  - Online RCPSP Scheduler replacing sequential worker execution loop.
  - Concurrency managed via `std::thread::scope`, with thread-isolated `NativeLlamaContext` instances spawned from shared `Arc<NativeLlamaModel>`.
  - Topological Ready Set $R_t$, critical path priority via Longest Remaining Path (LRP), Head-Pinning ($v^* = \arg\max \text{LRP}$), and EASY backfilling with zero-delay guarantee for $v^*$.
- **Memory Bound Gate** (`src/planner/swarm_coordinator.rs` & `src/native_llama/`):
  - Native KV-cache token sizing ($4 \times L \times H_{kv} \times (D/H_q)$) calculating $K_{max}$.
  - Hard rejection of tasks requesting $> K_{max}$ tokens for Zero-OOM guarantee.
  - Dynamic tracking of remaining KV capacity $K_{\text{rem}}$, pausing backfilling during Context Booms.
- **VFS Boundary Enforcement & Cleanroom Jail** (`src/sandbox/vfs.rs`, `src/sandbox/isolated_jail.rs`, `src/sandbox/cleanroom.rs`, `src/planner/swarm_coordinator.rs`):
  - `SubGoal` with strict `read_set` and `write_set`.
  - `MemoryVfs` checked APIs (`write_file_checked`, `read_file_checked`, `patch_file_checked`, `delete_file_checked`).
  - Linux Bubblewrap (`bwrap`) fail-closed cleanroom jail with `--unshare-all`, private PID/net/user namespaces, tmpfs cap, and `RLIMIT_FSIZE`.
  - Out-of-bounds error mapped to `DiagnosticCode::OutOfBoundsWrite` and aborts retry loop immediately.
- **State Machine & Fault Cascade** (`src/planner/swarm_coordinator.rs`):
  - Explicit states: `Pending`, `Ready`, `Running`, `Completed`, `Failed`, `Cancelled`.
  - Immediate forward topological cascade transitioning all descendants of a `Failed` task to `Cancelled`.

## Code Layout
- `src/sandbox/vfs.rs`: `MemoryVfs`, `VfsBoundaryError`, checked read/write operations, lexical normalization, Two-Phase atomic commit pass (`commit_diffs_atomic`).
- `src/sandbox/isolated_jail.rs`: Linux Bubblewrap isolation jail, `safe_join` path traversal defense, resource caps, and TTY detachment.
- `src/sandbox/cleanroom.rs`: Universal language cleanroom verification and sheet parsing.
- `src/planner/swarm_coordinator.rs`: `SubGoal`, `TaskStatus`, `RcpspScheduler`, LRP calculation, Pinned EASY backfilling, `std::thread::scope` execution engine, State Machine.
- `src/bin/swarm_stress_test.rs`: Stress test and benchmark binary verifying 5+ task DAG makespan, out-of-bounds cascade, Context Boom backfill pause, Zero-OOM rejection.
- `tests/test_rcpsp_scheduler.rs`: Dedicated integration test suite for scheduler, memory gate, and state machine.
- `tests/test_sandbox_security.rs`: VFS sandbox boundary enforcement tests.
- `tests/test_adversarial_vfs.rs`: VFS path normalization and traversal resistance tests.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Baseline Test Compile Fix | Fix missing `depends_on` in `SubGoal` instantiation at line 2589 | M1 | Survey 1, 2, 3 (VERIFIED) |
| 2 | SubGoal Read/Write Sets | Add `read_set` and `write_set` to `SubGoal` with fallback inference | M1 | R1, Survey 1, 2 (VERIFIED) |
| 3 | VFS Checked Operations | Implement `write_file_checked`, `patch_file_checked`, `delete_file_checked`, `read_file_checked` in `MemoryVfs` | M1 | R1, Survey 2 (VERIFIED) |
| 4 | VFS Runtime Enforcement | Enforce `write_set` in `SwarmCoordinator` actions (`WriteFile`, `PatchFile`, `DeleteFile`) and filter jail staging | M1 | R1, Survey 2 (VERIFIED) |
| 5 | Lua Sandbox Enforcement | Intercept VFS modifications in `lua_runner.rs` and raise `VFS_BOUNDARY_VIOLATION` | M1 | R1, Survey 2 (VERIFIED) |
| 6 | Global Contract & Planning | Update `generate_global_contract` to emit read/write sets and fix unused variable warning | M1 | R1, Survey 1 (VERIFIED) |
| 7 | Memory Bound Gate | Derive $K_{max}$ from model geometry and hard-reject tasks exceeding $K_{max}$ (Zero-OOM) | M2 | R3, Survey 3 |
| 8 | Dynamic KV Tracking | Track remaining KV capacity $K_{\text{rem}}$ and pause backfilling on Context Boom | M2 | R3, Survey 3 |
| 9 | Task State Machine | Implement `TaskStatus` (`Pending`, `Ready`, `Running`, `Completed`, `Failed`, `Cancelled`) | M2 | R4, Survey 1, 3 |
| 10 | Descendant Cancellation | Forward topological BFS/DFS cascade to cancel all descendants of `Failed` task | M2 | R4, Survey 1, 2, 3 |
| 11 | DAG & LRP Construction | Build task DAG, compute $d_v^{max} = MAX\_TOKENS / r_{min}$, and compute critical path LRP for ready set | M3 | R2, Survey 1 |
| 12 | Head-Pinning & Backfilling | Pin $v^* = \arg\max \text{LRP}$ with shadow reservation; backfill smaller tasks without delaying $v^*$ | M3 | R2, Survey 1 |
| 13 | Scoped Thread Concurrency | Refactor `SwarmCoordinator::execute_goal` to use `std::thread::scope` with private `NativeLlamaContext`s | M3 | R2, Survey 1, 3 |
| 14 | Stress Test & Benchmark Binary | Create `src/bin/swarm_stress_test.rs` executing 5+ tasks concurrently with makespan comparison | M4 | Acceptance Criteria |
| 15 | Out-of-Bounds Failure Proof | Integration test proving out-of-bounds write transitions task to `FAILED` and cancels children | M4 | Acceptance Criteria |
| 16 | Context Boom Pause Proof | Integration test proving KV over-allocation correctly pauses backfilling | M4 | Acceptance Criteria |
| 17 | Adversarial Hardening | Coverage and boundary verification via Challenger and Forensic Auditor | M5 | Integrity & QA |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | VFS Enforcement & Read/Write Sets (R1) | Features 1, 2, 3, 4, 5, 6 | none | DONE |
| 2 | Memory Bound Gate & State Machine (R3, R4) | Features 7, 8, 9, 10 | M1 | DONE |
| 3 | RCPSP Scheduler (Pinned EASY Backfilling) (R2) | Features 11, 12, 13 | M1, M2 | IN_PROGRESS |
| 4 | Acceptance Tests & Benchmarks (src/bin) | Features 14, 15, 16 | M3 | PLANNED |
| 5 | Adversarial Hardening & Final Audit | Feature 17 | M4 | PLANNED |

## Interface Contracts
### `SubGoal` Data Model
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubGoal {
    pub id: usize,
    pub description: String,
    pub target_entity: String,
    pub kind: TaskKind,
    pub guidance: Option<String>,
    pub depends_on: Vec<String>,
    pub read_set: Vec<PathBuf>,
    pub write_set: Vec<PathBuf>,
    pub max_tokens: usize,
}
```

### `MemoryVfs` Checked API (`src/sandbox/vfs.rs`)
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VfsBoundaryError {
    UnauthorizedWrite { path: PathBuf, allowed: Vec<PathBuf> },
    UnauthorizedRead { path: PathBuf, allowed: Vec<PathBuf> },
    UnauthorizedDelete { path: PathBuf, allowed: Vec<PathBuf> },
    UnauthorizedPatch { path: PathBuf, allowed: Vec<PathBuf> },
}
```

### State Machine & Memory Gate Contracts (`src/planner/swarm_coordinator.rs`)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct MemoryGate {
    pub k_max: usize,
    pub active_tokens: usize,
}

impl MemoryGate {
    pub fn new(k_max: usize) -> Self;
    pub fn can_admit(&self, requested: usize) -> bool;
    pub fn is_over_capacity(&self, requested: usize) -> bool;
    pub fn allocate(&mut self, tokens: usize) -> Result<(), ()>;
    pub fn release(&mut self, tokens: usize);
    pub fn remaining(&self) -> usize;
}
```

## Build & Test Execution Environment
- Native shared libraries (`libllama.so`, `libggml.so`, `libggml-cpu.so`, `libggml-base.so`) reside in `omni_engine/bin`.
- All `cargo check`, `cargo test`, and `cargo run` commands MUST include `LD_LIBRARY_PATH`:
  `LD_LIBRARY_PATH=$(pwd)/bin cargo test ...`
  `LD_LIBRARY_PATH=$(pwd)/bin cargo run ...`
