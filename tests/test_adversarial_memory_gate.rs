//! Adversarial Integration Test Suite: MemoryGate & Zero-OOM Rejection Logic
//!
//! Empirical Stress Tests for Milestone 2:
//! 1. Boundary conditions: 0 tokens, exactly K_max tokens, K_max + 1 tokens.
//! 2. Degenerate K_max: K_max = 0 edge cases.
//! 3. Arithmetic overflow / underflow resistance in allocate(), release(), remaining(), utilization(), and byte calculations.
//! 4. Context Boom backpressure dynamics: pausing when K_rem < demanded, resuming when capacity freed, non-blocking backfilling.
//! 5. Pre-flight Zero-OOM rejection vs dynamic admit_task Zero-OOM rejection.
//! 6. Fault cascade with deep DAGs, wide DAGs, and diamond topologies under Zero-OOM conditions.
//! 7. Repeated / saturating release (zero double-free / memory corruption invariant).
//! 8. Invariant stress fuzzing: remaining() + active_tokens == K_max across rapid pseudo-random allocations.

use omni_engine::planner::swarm_coordinator::{
    MemoryGate, MemoryGateError, StateMachineCoordinator, SubGoal, TaskDag,
    TaskKind, TaskStatus,
};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

fn make_subgoal(
    id: usize,
    target: &str,
    depends_on: Vec<&str>,
    max_tokens: usize,
) -> SubGoal {
    SubGoal {
        id,
        description: format!("SubGoal #{} targeting {}", id, target),
        target_entity: target.to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: depends_on.into_iter().map(String::from).collect(),
        read_set: vec![],
        write_set: vec![PathBuf::from(target)],
        max_tokens,
    }
}

// ============================================================================
// PART 1: Boundary Demands: 0 tokens, exactly K_max tokens, K_max + 1 tokens
// ============================================================================

#[test]
fn test_adversarial_boundary_zero_tokens() {
    let k_max = 4096;
    let mut gate = MemoryGate::new(k_max);

    // 0 tokens request
    assert!(!gate.is_over_capacity(0), "0 tokens is not over capacity");
    assert!(gate.can_admit(0), "0 tokens can always be admitted");
    assert!(gate.allocate(0).is_ok(), "allocate(0) must succeed");
    assert_eq!(gate.active_tokens, 0, "active_tokens remains 0");
    assert_eq!(gate.remaining(), k_max, "remaining tokens remains k_max");

    // Coordinator behavior with 0 tokens task
    let sg0 = make_subgoal(1, "task_zero.rs", vec![], 0);
    let dag = TaskDag::from_subgoals(vec![sg0]).expect("Valid DAG");
    let mut coord = StateMachineCoordinator::new(dag, k_max).expect("Valid coordinator");

    assert_eq!(coord.ready_set, vec![1]);
    let abort_flag = coord.admit_task(1, 0).expect("Task demanding 0 tokens must admit");
    assert!(!abort_flag.load(Ordering::SeqCst));
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Running);
    assert_eq!(coord.records.get(&1).unwrap().allocated_tokens, 0);
    assert_eq!(coord.memory_gate.active_tokens, 0);

    // Completing 0-token task
    let unblocked = coord.complete_task(1).expect("Task 1 completes cleanly");
    assert!(unblocked.is_empty());
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Completed);
    assert_eq!(coord.memory_gate.active_tokens, 0);
    assert_eq!(coord.memory_gate.remaining(), k_max);
}

#[test]
fn test_adversarial_boundary_exact_k_max_tokens() {
    let k_max = 4096;
    let mut gate = MemoryGate::new(k_max);

    // Exactly K_max tokens
    assert!(!gate.is_over_capacity(k_max), "K_max is within capacity");
    assert!(gate.can_admit(k_max), "K_max can be admitted when empty");
    assert!(gate.allocate(k_max).is_ok(), "allocate(k_max) must succeed");
    assert_eq!(gate.active_tokens, k_max, "active_tokens equals k_max");
    assert_eq!(gate.remaining(), 0, "remaining tokens must be 0");
    assert_eq!(gate.utilization(), 1.0, "utilization must be 1.0");

    // Any further request (even 1 token) must fail
    assert!(!gate.can_admit(1));
    assert!(gate.allocate(1).is_err());
    assert_eq!(
        gate.try_allocate(1),
        Err(MemoryGateError::InsufficientCapacity {
            requested: 1,
            remaining: 0,
            k_max,
        })
    );

    // Releasing K_max restores full capacity
    gate.release(k_max);
    assert_eq!(gate.active_tokens, 0);
    assert_eq!(gate.remaining(), k_max);
    assert_eq!(gate.utilization(), 0.0);

    // Coordinator with exact K_max task
    let sg_full = make_subgoal(1, "exact_full.rs", vec![], k_max);
    let sg_queued = make_subgoal(2, "queued.rs", vec![], 512);
    let dag = TaskDag::from_subgoals(vec![sg_full, sg_queued]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, k_max).unwrap();

    // Admit Task 1 for exact K_max
    let _ = coord.admit_task(1, k_max).expect("Exact K_max must admit");
    assert_eq!(coord.memory_gate.active_tokens, k_max);
    assert_eq!(coord.memory_gate.remaining(), 0);

    // Task 2 must be backpressured (Context Boom)
    let res2 = coord.admit_task(2, 512);
    assert!(res2.is_err());
    assert_eq!(res2.unwrap_err(), "Context Boom: insufficient remaining memory");
    assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Ready);

    // Complete Task 1 -> capacity frees -> Task 2 admits
    coord.complete_task(1).unwrap();
    assert_eq!(coord.memory_gate.remaining(), k_max);
    let _ = coord.admit_task(2, 512).expect("Task 2 admits after release");
    assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Running);
    assert_eq!(coord.memory_gate.active_tokens, 512);
}

#[test]
fn test_adversarial_boundary_k_max_plus_one_rejection() {
    let k_max = 4096;
    let mut gate = MemoryGate::new(k_max);

    // K_max + 1 tokens
    let req = k_max + 1;
    assert!(gate.is_over_capacity(req), "K_max + 1 must be flagged over capacity");
    assert!(!gate.can_admit(req), "K_max + 1 must not be admitted");
    assert!(gate.allocate(req).is_err(), "allocate(K_max + 1) must return Err");
    assert_eq!(
        gate.try_allocate(req),
        Err(MemoryGateError::OverCapacity {
            requested: req,
            k_max,
        })
    );
    assert_eq!(gate.active_tokens, 0, "No quota allocated");

    // Coordinator Zero-OOM rejection and cascading cancellation
    let sg_bad = make_subgoal(1, "huge.rs", vec![], req);
    let sg_child = make_subgoal(2, "child.rs", vec!["1"], 256);
    let sg_unrelated = make_subgoal(3, "independent.rs", vec![], 512);

    let dag = TaskDag::from_subgoals(vec![sg_bad, sg_child, sg_unrelated]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, k_max).unwrap();

    let res = coord.admit_task(1, req);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "Zero-OOM hard rejection");

    // Task 1 must be Failed with explicit Zero-OOM reason
    let rec1 = coord.records.get(&1).unwrap();
    assert_eq!(rec1.status, TaskStatus::Failed);
    assert!(rec1.failure_reason.as_ref().unwrap().contains("Zero-OOM rejection"));

    // Dependent child must be Cancelled
    let rec2 = coord.records.get(&2).unwrap();
    assert_eq!(rec2.status, TaskStatus::Cancelled);
    assert!(rec2.failure_reason.as_ref().unwrap().contains("Task #1 FAILED"));

    // Memory quota remains untouched
    assert_eq!(coord.memory_gate.active_tokens, 0);

    // Independent task remains Ready and operable
    assert_eq!(coord.ready_set, vec![3]);
    let _ = coord.admit_task(3, 512).unwrap();
    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Running);
    assert_eq!(coord.memory_gate.active_tokens, 512);
}

#[test]
fn test_adversarial_degenerate_k_max_zero() {
    let mut gate = MemoryGate::new(0);

    // With K_max = 0
    assert_eq!(gate.remaining(), 0);
    assert_eq!(gate.utilization(), 0.0, "K_max=0 must not divide by zero");

    // 0 tokens on K_max = 0
    assert!(!gate.is_over_capacity(0));
    assert!(gate.can_admit(0));
    assert!(gate.allocate(0).is_ok());

    // 1 token on K_max = 0
    assert!(gate.is_over_capacity(1));
    assert!(!gate.can_admit(1));
    assert!(gate.allocate(1).is_err());
    assert_eq!(
        gate.try_allocate(1),
        Err(MemoryGateError::OverCapacity {
            requested: 1,
            k_max: 0,
        })
    );
}

// ============================================================================
// PART 2: Arithmetic Overflow and Underflow Resistance
// ============================================================================

#[test]
fn test_adversarial_overflow_resistance_in_allocate() {
    let k_max = 4096;
    let mut gate = MemoryGate::new(k_max);

    // Extreme token requests (usize::MAX, usize::MAX - 1)
    assert!(gate.is_over_capacity(usize::MAX));
    assert!(!gate.can_admit(usize::MAX));
    assert!(gate.allocate(usize::MAX).is_err());
    assert_eq!(
        gate.try_allocate(usize::MAX),
        Err(MemoryGateError::OverCapacity {
            requested: usize::MAX,
            k_max,
        })
    );

    // Partial allocation, then attempt overflow addition
    gate.allocate(2048).unwrap();
    assert_eq!(gate.active_tokens, 2048);

    // Requesting (usize::MAX - 1000): saturating_add must not wrap around to 0
    assert!(gate.is_over_capacity(usize::MAX - 1000));
    assert!(!gate.can_admit(usize::MAX - 1000));
    assert!(gate.allocate(usize::MAX - 1000).is_err());
    assert_eq!(gate.active_tokens, 2048, "active_tokens must remain unchanged");
    assert_eq!(gate.remaining(), 2048);
}

#[test]
fn test_adversarial_underflow_resistance_in_release() {
    let k_max = 4096;
    let mut gate = MemoryGate::new(k_max);

    gate.allocate(1000).unwrap();
    assert_eq!(gate.active_tokens, 1000);

    // Release exact
    gate.release(1000);
    assert_eq!(gate.active_tokens, 0);
    assert_eq!(gate.remaining(), k_max);

    // Release on empty gate (must saturate to 0, not underflow to usize::MAX)
    gate.release(1);
    assert_eq!(gate.active_tokens, 0);
    assert_eq!(gate.remaining(), k_max);

    gate.release(usize::MAX);
    assert_eq!(gate.active_tokens, 0);
    assert_eq!(gate.remaining(), k_max);

    // Allocate 500, release 10_000
    gate.allocate(500).unwrap();
    assert_eq!(gate.active_tokens, 500);
    gate.release(10_000);
    assert_eq!(gate.active_tokens, 0);
    assert_eq!(gate.remaining(), k_max);
}

#[test]
fn test_adversarial_remaining_saturating_invariants() {
    let mut gate = MemoryGate::new(4096);
    gate.active_tokens = 5000; // Artificially corrupted active > k_max
    assert_eq!(gate.remaining(), 0, "remaining() must saturate to 0 if active > k_max");
    assert_eq!(gate.utilization(), 1.0, "utilization() must clamp to 1.0");

    gate.active_tokens = 0;
    assert_eq!(gate.remaining(), 4096);
    assert_eq!(gate.utilization(), 0.0);
}

#[test]
fn test_adversarial_byte_calculation_edge_cases() {
    // analytical_bytes_per_token division by zero safety
    let bpt_zero_hq = MemoryGate::analytical_bytes_per_token(28, 2, 1536, 0);
    assert_eq!(bpt_zero_hq, 0, "h_q=0 must safely return 0 without division by zero");

    // calculate_k_max division by zero safety
    let k_max_zero_bpt = MemoryGate::calculate_k_max(8192, 1024 * 1024, 0);
    assert_eq!(k_max_zero_bpt, 8192, "bytes_per_token=0 must safely fallback to n_ctx_train");

    // Zero budget
    let k_max_zero_budget = MemoryGate::calculate_k_max(8192, 0, 28_672);
    assert_eq!(k_max_zero_budget, 0, "budget=0 must yield k_max=0");

    // Zero context train
    let k_max_zero_ctx = MemoryGate::calculate_k_max(0, 1024 * 1024 * 1024, 28_672);
    assert_eq!(k_max_zero_ctx, 0, "n_ctx_train=0 must yield k_max=0");
}

// ============================================================================
// PART 3: Context Boom Backpressure Dynamics & Resumption
// ============================================================================

#[test]
fn test_adversarial_context_boom_backpressure_and_backfilling() {
    let k_max = 4096;

    // Three tasks:
    // Task 1: 3000 tokens (head task)
    // Task 3: 1500 tokens (cannot fit while Task 1 runs: 3000 + 1500 = 4500 > 4096)
    // Task 4: 500 tokens (can fit alongside Task 1: 3000 + 500 = 3500 <= 4096)
    let sg1 = make_subgoal(1, "task1.rs", vec![], 3000);
    let sg3 = make_subgoal(3, "task3.rs", vec![], 1500);
    let sg4 = make_subgoal(4, "task4.rs", vec![], 500);

    let dag = TaskDag::from_subgoals(vec![sg1, sg3, sg4]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, k_max).unwrap();

    // 1. Admit Task 1 (allocates 3000)
    let _ = coord.admit_task(1, 3000).expect("Task 1 admitted");
    assert_eq!(coord.memory_gate.remaining(), 1096);

    // 2. Task 3 demands 1500 (> remaining 1096, but <= k_max 4096)
    let boom = coord.admit_task(3, 1500);
    assert!(boom.is_err());
    assert_eq!(boom.unwrap_err(), "Context Boom: insufficient remaining memory");
    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Ready, "Task 3 must remain Ready");
    assert_eq!(coord.memory_gate.remaining(), 1096);

    // 3. Task 4 demands 500 (<= remaining 1096) -> Backfill admits Task 4!
    let _ = coord.admit_task(4, 500).expect("Task 4 admitted via backfill");
    assert_eq!(coord.records.get(&4).unwrap().status, TaskStatus::Running);
    assert_eq!(coord.memory_gate.active_tokens, 3500);
    assert_eq!(coord.memory_gate.remaining(), 596);

    // 4. Complete Task 1 (releases 3000 tokens)
    coord.complete_task(1).expect("Task 1 complete");
    assert_eq!(coord.memory_gate.active_tokens, 500);
    assert_eq!(coord.memory_gate.remaining(), 3596);

    // 5. Now Task 3 can be admitted cleanly!
    let _ = coord.admit_task(3, 1500).expect("Task 3 admitted after capacity freed");
    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Running);
    assert_eq!(coord.memory_gate.active_tokens, 2000);
    assert_eq!(coord.memory_gate.remaining(), 2096);

    // Complete Task 3 and Task 4
    coord.complete_task(3).unwrap();
    coord.complete_task(4).unwrap();
    assert_eq!(coord.memory_gate.active_tokens, 0);
    assert_eq!(coord.memory_gate.remaining(), k_max);
    assert!(coord.is_all_terminal());
}

// ============================================================================
// PART 4: Deep and Wide Fault Cascade under Zero-OOM
// ============================================================================

#[test]
fn test_adversarial_deep_chain_zero_oom_cascade() {
    let k_max = 4096;

    // Linear chain: 1 -> 2 -> 3 -> 4 -> 5
    // Task 1 demands 8192 (> 4096)
    let sg1 = make_subgoal(1, "t1.rs", vec![], 8192);
    let sg2 = make_subgoal(2, "t2.rs", vec!["1"], 1024);
    let sg3 = make_subgoal(3, "t3.rs", vec!["2"], 1024);
    let sg4 = make_subgoal(4, "t4.rs", vec!["3"], 1024);
    let sg5 = make_subgoal(5, "t5.rs", vec!["4"], 1024);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4, sg5]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, k_max).unwrap();

    let res = coord.admit_task(1, 8192);
    assert!(res.is_err());

    // Task 1 must be Failed
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Failed);

    // Tasks 2, 3, 4, 5 must all be Cancelled
    for id in 2..=5 {
        let rec = coord.records.get(&id).unwrap();
        assert_eq!(
            rec.status,
            TaskStatus::Cancelled,
            "Task #{} must be Cancelled in cascade",
            id
        );
        assert!(
            rec.failure_reason.as_ref().unwrap().contains("Topological ancestor"),
            "Task #{} must document ancestor failure",
            id
        );
    }

    assert!(coord.ready_set.is_empty());
    assert_eq!(coord.memory_gate.active_tokens, 0);
    assert!(coord.is_all_terminal());
}

#[test]
fn test_adversarial_preflight_validate_multi_rejection() {
    let k_max = 4096;

    // Two tasks exceed k_max: Task 1 and Task 3
    let sg1 = make_subgoal(1, "t1.rs", vec![], 5000);
    let sg2 = make_subgoal(2, "t2.rs", vec!["1"], 1024);
    let sg3 = make_subgoal(3, "t3.rs", vec![], 6000);
    let sg4 = make_subgoal(4, "t4.rs", vec!["3"], 1024);
    let sg5 = make_subgoal(5, "t5.rs", vec![], 1024); // Valid independent task

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4, sg5]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, k_max).unwrap();

    let pre_res = coord.preflight_validate();
    assert!(pre_res.is_err(), "preflight_validate must catch both over-capacity tasks");
    let errors = pre_res.unwrap_err();
    assert_eq!(errors.len(), 2);

    // Tasks 1 and 3 are Failed
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Failed);
    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Failed);

    // Tasks 2 and 4 are Cancelled
    assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Cancelled);
    assert_eq!(coord.records.get(&4).unwrap().status, TaskStatus::Cancelled);

    // Task 5 remains Ready and can be admitted
    assert_eq!(coord.ready_set, vec![5]);
    assert_eq!(coord.records.get(&5).unwrap().status, TaskStatus::Ready);
    let _ = coord.admit_task(5, 1024).expect("Task 5 admits");
    assert_eq!(coord.records.get(&5).unwrap().status, TaskStatus::Running);
    assert_eq!(coord.memory_gate.active_tokens, 1024);
    coord.complete_task(5).unwrap();
    assert!(coord.is_all_terminal());
}

// ============================================================================
// PART 5: Invariant Stress Fuzzing (Rapid Allocation / Release Cycles)
// ============================================================================

#[test]
fn test_adversarial_memory_gate_invariants_under_stress() {
    let k_max = 16384;
    let mut gate = MemoryGate::new(k_max);

    // Deterministic pseudo-random sequence
    let mut state: u64 = 0x123456789ABCDEF0;
    let mut rng = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        state
    };

    let mut allocated_blocks: Vec<usize> = Vec::new();

    for _ in 0..1000 {
        let op = rng() % 3;
        match op {
            0 => {
                // Attempt allocation of random tokens up to 4096
                let req = (rng() % 4096) as usize;
                let remaining_before = gate.remaining();
                let can = gate.can_admit(req);
                let alloc_res = gate.allocate(req);

                if can {
                    assert!(alloc_res.is_ok());
                    allocated_blocks.push(req);
                } else {
                    assert!(alloc_res.is_err());
                    assert_eq!(gate.remaining(), remaining_before);
                }
            }
            1 => {
                // Release an allocated block if available
                if let Some(block) = allocated_blocks.pop() {
                    gate.release(block);
                }
            }
            2 => {
                // Arbitrary release (potentially larger than active)
                let rand_release = (rng() % 10000) as usize;
                gate.release(rand_release);
                // Clean tracking blocks if released more than active
                if gate.active_tokens == 0 {
                    allocated_blocks.clear();
                }
            }
            _ => unreachable!(),
        }

        // Strict Invariants
        assert!(
            gate.active_tokens <= gate.k_max,
            "active_tokens ({}) must not exceed k_max ({})",
            gate.active_tokens,
            gate.k_max
        );
        assert_eq!(
            gate.remaining() + gate.active_tokens,
            gate.k_max,
            "remaining + active must equal k_max exactly"
        );
        assert!(gate.utilization() >= 0.0 && gate.utilization() <= 1.0);
    }
}
