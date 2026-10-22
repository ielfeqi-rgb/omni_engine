# E2E Test Infra: Parallel DAG Swarm Engine (omni_engine)

## Test Philosophy
- Opaque-box, requirement-driven testing based strictly on `ORIGINAL_REQUEST.md`.
- Verifies:
  1. Concurrency & makespan acceleration ($C_{max}$) on 5+ task DAGs.
  2. VFS boundary enforcement & cascading cancellation to descendants on failure.
  3. Memory bound gating, Context Boom backfilling pauses, and Zero-OOM rejection.
  4. Scheduler invariants under edge cases and adversarial scenarios.

## Feature Inventory & Test Coverage
| # | Feature | Requirement | Tier 1 (Feature) | Tier 2 (Boundary) | Tier 3 (Pairwise) | Tier 4 (Scenario) |
|---|---------|-------------|:----------------:|:-----------------:|:-----------------:|:-----------------:|
| 1 | Baseline Test Build | Compilation | 1 | 1 | 1 | - |
| 2 | SubGoal Read/Write Sets | R1 | 5 | 5 | ✓ | ✓ |
| 3 | VFS Checked Operations | R1 | 5 | 5 | ✓ | ✓ |
| 4 | VFS Runtime Boundary Enforcement | R1 | 5 | 5 | ✓ | ✓ |
| 5 | Lua Sandbox Enforcement | R1 | 5 | 5 | ✓ | ✓ |
| 6 | Global Contract & Planning | R1 | 5 | 5 | ✓ | ✓ |
| 7 | Memory Bound Gate & Zero-OOM | R3 | 5 | 5 | ✓ | ✓ |
| 8 | Dynamic KV & Context Boom Pause | R3 | 5 | 5 | ✓ | ✓ |
| 9 | Task State Machine Transitions | R4 | 5 | 5 | ✓ | ✓ |
| 10 | Descendant Cancellation Cascade | R4 | 5 | 5 | ✓ | ✓ |
| 11 | DAG & LRP Construction | R2 | 5 | 5 | ✓ | ✓ |
| 12 | Head-Pinning & Backfilling | R2 | 5 | 5 | ✓ | ✓ |
| 13 | Scoped Thread Concurrency | R2 | 5 | 5 | ✓ | ✓ |
| 14 | Stress Test & Benchmark Binary | Acceptance | 5 | 5 | ✓ | ✓ |

## Test Architecture
- Unit Tests: `cargo test --lib` (tested in `src/sandbox/vfs.rs`, `src/planner/swarm_coordinator.rs`).
- Integration Tests: `cargo test --test test_rcpsp_scheduler` (in `tests/`).
- Stress Test & Benchmark Binary: `cargo run --bin swarm_stress_test` (in `src/bin/swarm_stress_test.rs`).
- Pass/Fail Semantics:
  - `cargo check --tests` returns code 0.
  - `cargo test --lib` and `cargo test --tests` pass with zero failures.
  - `cargo run --bin swarm_stress_test` exits with code 0 and prints verification reports confirming all 4 acceptance criteria.

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Acceptance Criteria |
|---|----------|--------------------|---------------------|
| 1 | 5+ Task Parallel DAG | F2, F7, F11, F12, F13, F14 | Parallel makespan is significantly lower than sequential makespan |
| 2 | Out-of-Bounds Security Violation | F2, F3, F4, F9, F10, F15 | Task transitions to `FAILED`, direct & indirect children transition to `CANCELLED` |
| 3 | Simulated Context Boom | F7, F8, F12, F16 | Task requesting tokens when remaining KV is low pauses backfill until capacity frees |
| 4 | Zero-OOM Over-Capacity Rejection | F7, F8, F9 | Task demanding $> K_{max}$ is hard-rejected immediately before thread launch |
| 5 | Mixed Diamond DAG with Mid-Flow Failure | F9, F10, F11, F12, F13 | Independent branches complete successfully while failed branch cascades cancel |

## Dynamic Library Path Requirement
- Pre-compiled llama.cpp libraries reside in `omni_engine/bin`.
- Runner invocation must specify:
  `LD_LIBRARY_PATH=$(pwd)/bin cargo test ...`
