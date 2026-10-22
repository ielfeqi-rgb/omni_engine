//! Integration Test Suite: RCPSP Scheduler, MemoryGate & State Machine (M2-3)
//!
//! Validates:
//! 1. Zero-OOM Hard Rejection: Tasks requesting > K_max tokens are rejected immediately
//!    prior to thread execution, transitioning to Failed and cancelling downstream dependencies.
//! 2. Out-of-Bounds Write Cascade: VFS runtime boundary violation transitions task to Failed
//!    and immediately cascades cancellation to all topological descendants while keeping VFS safe.
//! 3. Context Boom Pause: When aggregate active tokens approach K_max, tasks fit in K_max
//!    are paused in Ready until active tasks complete and release memory capacity.
//! 4. Diamond DAG Fault Isolation: Failure on one branch cancels only dependent nodes,
//!    leaving independent concurrent branches running to successful completion.
//! 5. Cooperative Preemption & Zero Double-Free: Abort flags are set and tokens refunded
//!    without duplicate releases when workers terminate.
//! 6. DAG Cycle Rejection: Circular dependencies are rejected by Kahn's algorithm.

use omni_engine::planner::swarm_coordinator::{
    DiagnosticCode, LrpConfig, MemoryGate, RcpspScheduler, ScheduleDecision,
    StateMachineCoordinator, SubGoal, TaskDag, TaskKind, TaskStatus,
};
use omni_engine::sandbox::MemoryVfs;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Helper: SubGoal Test Factory
// ---------------------------------------------------------------------------
fn make_test_subgoal(
    id: usize,
    target: &str,
    depends_on: Vec<&str>,
    read_set: Vec<&str>,
    write_set: Vec<&str>,
    max_tokens: usize,
) -> SubGoal {
    SubGoal {
        id,
        description: format!("SubGoal #{} targeting {}", id, target),
        target_entity: target.to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: depends_on.into_iter().map(String::from).collect(),
        read_set: read_set.into_iter().map(PathBuf::from).collect(),
        write_set: write_set.into_iter().map(PathBuf::from).collect(),
        max_tokens,
    }
}

// ---------------------------------------------------------------------------
// Test 1: Zero-OOM Hard Rejection of Over-Capacity Tasks
// ---------------------------------------------------------------------------
#[test]
fn test_rcpsp_zero_oom_hard_rejection() {
    println!("\n=== TEST 1: Zero-OOM Hard Rejection ===");
    let k_max = 4096;
    let direct_gate = MemoryGate::new(k_max);
    assert!(!direct_gate.can_admit(8192));
    assert!(direct_gate.is_over_capacity(8192));
    assert_eq!(direct_gate.remaining(), k_max);

    // Task 1 requests 8192 tokens (> k_max 4096)
    let sg1 = make_test_subgoal(1, "big_model.rs", vec![], vec![], vec!["big_model.rs"], 8192);
    // Task 2 depends on Task 1
    let sg2 = make_test_subgoal(2, "client.rs", vec!["1"], vec!["big_model.rs"], vec!["client.rs"], 1024);
    // Task 3 is independent (demands normal tokens)
    let sg3 = make_test_subgoal(3, "independent.rs", vec![], vec![], vec!["independent.rs"], 1024);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).expect("DAG must be valid");
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).expect("Coordinator init");

    // Both Task 1 and Task 3 start Ready
    assert_eq!(coordinator.ready_set, vec![1, 3]);

    // 1. Attempt to admit Task 1 -> Must be hard rejected with Zero-OOM
    let admit_res = coordinator.admit_task(1, 8192);
    assert!(admit_res.is_err(), "Over-capacity task must be rejected");
    assert_eq!(admit_res.unwrap_err(), "Zero-OOM hard rejection");

    // Verify Task 1 is FAILED with explanatory reason
    let rec1 = coordinator.records.get(&1).unwrap();
    assert_eq!(rec1.status, TaskStatus::Failed);
    assert!(
        rec1.failure_reason.as_ref().unwrap().contains("Zero-OOM rejection"),
        "Failure reason must document Zero-OOM: {:?}",
        rec1.failure_reason
    );

    // Verify descendant Task 2 was cascaded to CANCELLED
    let rec2 = coordinator.records.get(&2).unwrap();
    assert_eq!(rec2.status, TaskStatus::Cancelled);
    assert!(
        rec2.failure_reason.as_ref().unwrap().contains("Task #1 FAILED"),
        "Reason must state upstream ancestor failed"
    );

    // Verify memory gate remained completely unconsumed by the rejected task
    assert_eq!(coordinator.memory_gate.active_tokens, 0);

    // Verify Task 3 remains untouched in Ready set and can execute cleanly
    assert_eq!(coordinator.ready_set, vec![3]);
    let abort_3 = coordinator.admit_task(3, 1024).expect("Task 3 must admit");
    assert!(!abort_3.load(Ordering::SeqCst));
    assert_eq!(coordinator.memory_gate.active_tokens, 1024);
    let _ = coordinator.complete_task(3).expect("Task 3 completes");
    assert_eq!(coordinator.records.get(&3).unwrap().status, TaskStatus::Completed);
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
    assert!(coordinator.is_all_terminal());
}

// ---------------------------------------------------------------------------
// Test 2: Out-of-Bounds Write Cascade
// ---------------------------------------------------------------------------
#[test]
fn test_rcpsp_out_of_bounds_write_transitions_to_failed_and_cancels_descendants() {
    println!("\n=== TEST 2: Out-of-Bounds Write Cascade ===");
    let k_max = 4096;

    // Linear DAG: Task 1 (write schema.sql) -> Task 2 (write models.rs) -> Task 3 (test)
    let sg1 = make_test_subgoal(1, "schema.sql", vec![], vec![], vec!["schema.sql"], 1024);
    let sg2 = make_test_subgoal(2, "models.rs", vec!["1"], vec!["schema.sql"], vec!["models.rs"], 1024);
    let sg3 = make_test_subgoal(3, "test.rs", vec!["2"], vec!["models.rs"], vec!["test.rs"], 1024);

    let dag = TaskDag::from_subgoals(vec![sg1.clone(), sg2, sg3]).unwrap();
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).unwrap();

    // Setup VFS
    let vfs = MemoryVfs::new();

    // Admit Task 1
    let _abort_1 = coordinator.admit_task(1, 1024).unwrap();
    assert_eq!(coordinator.records.get(&1).unwrap().status, TaskStatus::Running);

    // Simulate worker attempting an unauthorized write outside write_set
    let unauthorized_path = PathBuf::from("secrets.env");
    assert!(!sg1.can_write(&unauthorized_path), "secrets.env must not be in write_set");

    let write_res = vfs.write_file_checked(&unauthorized_path, b"SECRET_KEY=123", &sg1.write_set);
    assert!(write_res.is_err(), "VFS must reject unauthorized write");
    assert!(!vfs.exists("secrets.env"), "Unauthorized file must NOT exist in VFS");

    // Coordinator catches OutOfBoundsWrite and fails Task 1
    let diag = DiagnosticCode::OutOfBoundsWrite;
    let cancelled = coordinator.fail_task(
        1,
        "VFS Boundary Violation: Attempted write to 'secrets.env' outside write_set",
        Some(diag.clone()),
    );

    // Verify cancellation cascade: both Task 2 and Task 3 must be Cancelled
    assert_eq!(cancelled, vec![2, 3]);
    assert_eq!(coordinator.records.get(&1).unwrap().status, TaskStatus::Failed);
    assert_eq!(coordinator.records.get(&1).unwrap().diagnostic_code, Some(diag));
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Cancelled);
    assert_eq!(coordinator.records.get(&3).unwrap().status, TaskStatus::Cancelled);

    // Memory tokens for Task 1 must be refunded to 0
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
    assert!(coordinator.is_all_terminal());
}

// ---------------------------------------------------------------------------
// Test 3: Context Boom Pauses Admission Until Capacity Frees
// ---------------------------------------------------------------------------
#[test]
fn test_rcpsp_context_boom_pausing_admission_until_capacity_frees() {
    println!("\n=== TEST 3: Context Boom Backpressure ===");
    let k_max = 4096;

    // Both tasks are root tasks
    let sg1 = make_test_subgoal(1, "heavy_task.rs", vec![], vec![], vec!["heavy_task.rs"], 3000);
    let sg2 = make_test_subgoal(2, "secondary_task.rs", vec![], vec![], vec!["secondary_task.rs"], 2000);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).unwrap();

    // Both ready initially
    assert_eq!(coordinator.ready_set, vec![1, 2]);

    // 1. Admit Task 1 (allocates 3000 tokens; remaining = 1096)
    let abort_1 = coordinator.admit_task(1, 3000).expect("Task 1 must be admitted");
    assert!(!abort_1.load(Ordering::SeqCst));
    assert_eq!(coordinator.memory_gate.active_tokens, 3000);
    assert_eq!(coordinator.memory_gate.remaining(), 1096);
    assert_eq!(coordinator.ready_set, vec![2]);

    // 2. Attempt to admit Task 2 (requests 2000 > remaining 1096, but <= k_max 4096)
    let boom_res = coordinator.admit_task(2, 2000);
    assert!(boom_res.is_err());
    assert_eq!(boom_res.unwrap_err(), "Context Boom: insufficient remaining memory");

    // Task 2 MUST NOT be failed or cancelled; it remains Ready
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Ready);
    assert_eq!(coordinator.ready_set, vec![2]);
    assert_eq!(coordinator.memory_gate.active_tokens, 3000);

    // 3. Task 1 finishes and releases its 3000 tokens
    let unblocked = coordinator.complete_task(1).expect("Task 1 completion");
    assert!(unblocked.is_empty(), "Task 2 was already ready");
    assert_eq!(coordinator.records.get(&1).unwrap().status, TaskStatus::Completed);
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
    assert_eq!(coordinator.memory_gate.remaining(), 4096);

    // 4. Retry admitting Task 2 -> Now succeeds!
    let abort_2 = coordinator.admit_task(2, 2000).expect("Task 2 admitted after memory released");
    assert!(!abort_2.load(Ordering::SeqCst));
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Running);
    assert_eq!(coordinator.memory_gate.active_tokens, 2000);
    assert!(coordinator.ready_set.is_empty());

    // Complete Task 2
    let _ = coordinator.complete_task(2).expect("Task 2 completion");
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Completed);
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
    assert!(coordinator.is_all_terminal());
}

// ---------------------------------------------------------------------------
// Test 4: Diamond DAG Fault Isolation
// ---------------------------------------------------------------------------
#[test]
fn test_rcpsp_diamond_dag_fault_isolation() {
    println!("\n=== TEST 4: Diamond DAG Fault Isolation ===");
    let k_max = 4096;

    // Diamond: 1 -> 2 -> 4 and 1 -> 3 -> 4
    let sg1 = make_test_subgoal(1, "root.rs", vec![], vec![], vec!["root.rs"], 1024);
    let sg2 = make_test_subgoal(2, "branch_a.rs", vec!["1"], vec!["root.rs"], vec!["branch_a.rs"], 1024);
    let sg3 = make_test_subgoal(3, "branch_b.rs", vec!["1"], vec!["root.rs"], vec!["branch_b.rs"], 1024);
    let sg4 = make_test_subgoal(4, "sink.rs", vec!["2", "3"], vec!["branch_a.rs", "branch_b.rs"], vec!["sink.rs"], 1024);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4]).unwrap();
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).unwrap();

    // Complete Task 1
    let _ = coordinator.admit_task(1, 1024).unwrap();
    let unblocked = coordinator.complete_task(1).unwrap();
    assert_eq!(unblocked, vec![2, 3]);

    // Admit both branches
    let _ = coordinator.admit_task(2, 1024).unwrap();
    let _ = coordinator.admit_task(3, 1024).unwrap();
    assert_eq!(coordinator.memory_gate.active_tokens, 2048);

    // Fail Branch A (Task 2)
    let cancelled = coordinator.fail_task(2, "Branch A crashed", None);
    assert_eq!(cancelled, vec![4], "Only Sink 4 must be cancelled");
    assert_eq!(coordinator.records.get(&4).unwrap().status, TaskStatus::Cancelled);

    // Branch B (Task 3) MUST continue running unaffected
    assert_eq!(coordinator.records.get(&3).unwrap().status, TaskStatus::Running);
    assert_eq!(coordinator.memory_gate.active_tokens, 1024);

    // Complete Branch B
    let _ = coordinator.complete_task(3).unwrap();
    assert_eq!(coordinator.records.get(&3).unwrap().status, TaskStatus::Completed);
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
    assert!(coordinator.is_all_terminal());
}

// ---------------------------------------------------------------------------
// Test 5: Cooperative Preemption and Token Restitution
// ---------------------------------------------------------------------------
#[test]
fn test_rcpsp_cooperative_preemption_and_memory_restitution() {
    println!("\n=== TEST 5: Cooperative Preemption & Restitution ===");
    let k_max = 4096;

    let sg1 = make_test_subgoal(1, "task_1.rs", vec![], vec![], vec!["task_1.rs"], 1024);
    let sg2 = make_test_subgoal(2, "task_2.rs", vec!["1"], vec!["task_1.rs"], vec!["task_2.rs"], 2048);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).unwrap();

    // Start Task 1
    let _ = coordinator.admit_task(1, 1024).unwrap();

    // For testing preemption of a running child: manually set Task 2 as Running
    let abort_flag_2 = {
        let rec2 = coordinator.records.get_mut(&2).unwrap();
        rec2.status = TaskStatus::Running;
        rec2.allocated_tokens = 2048;
        rec2.abort_flag.clone()
    };
    coordinator.memory_gate.allocate(2048).unwrap();
    assert_eq!(coordinator.memory_gate.active_tokens, 3072);

    // Task 1 fails
    coordinator.fail_task(1, "Unexpected panic", None);

    // Verify Task 2 received preemption abort signal
    assert!(abort_flag_2.load(Ordering::SeqCst), "abort_flag must be true");
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Cancelled);
    assert_eq!(coordinator.records.get(&2).unwrap().allocated_tokens, 0);

    // Memory for both tasks must be completely reclaimed
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
}

// ---------------------------------------------------------------------------
// Test 6: DAG Cycle Rejection
// ---------------------------------------------------------------------------
#[test]
fn test_rcpsp_dag_cycle_rejection() {
    println!("\n=== TEST 6: DAG Cycle Rejection ===");
    // Circular dependency: 1 -> 2 -> 3 -> 1
    let sg1 = make_test_subgoal(1, "a.rs", vec!["3"], vec![], vec!["a.rs"], 1024);
    let sg2 = make_test_subgoal(2, "b.rs", vec!["1"], vec![], vec!["b.rs"], 1024);
    let sg3 = make_test_subgoal(3, "c.rs", vec!["2"], vec![], vec!["c.rs"], 1024);

    let dag_res = TaskDag::from_subgoals(vec![sg1, sg2, sg3]);
    assert!(dag_res.is_err(), "Cycle must be caught by Kahn's algorithm");
    let err = dag_res.unwrap_err();
    assert!(err.contains("Cyclic dependency detected"), "Error must report cycle: {}", err);
}

// ---------------------------------------------------------------------------
// Test 7: Parallel Execution & Makespan Reduction on 5-Task DAG
// ---------------------------------------------------------------------------
#[test]
fn test_scoped_concurrency_parallel_execution_makespan() {
    println!("\n=== TEST 7: Parallel Execution & Makespan Reduction ===");
    // DAG: 1 (Root) -> [2, 3, 4] (3 Concurrent Branches) -> 5 (Sink)
    let k_max = 8192;
    let sg1 = make_test_subgoal(1, "root.rs", vec![], vec![], vec!["root.rs"], 1024);
    let sg2 = make_test_subgoal(2, "branch_a.rs", vec!["1"], vec!["root.rs"], vec!["branch_a.rs"], 1024);
    let sg3 = make_test_subgoal(3, "branch_b.rs", vec!["1"], vec!["root.rs"], vec!["branch_b.rs"], 1024);
    let sg4 = make_test_subgoal(4, "branch_c.rs", vec!["1"], vec!["root.rs"], vec!["branch_c.rs"], 1024);
    let sg5 = make_test_subgoal(5, "sink.rs", vec!["2", "3", "4"], vec!["branch_a.rs", "branch_b.rs", "branch_c.rs"], vec!["sink.rs"], 1024);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4, sg5]).unwrap();
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).unwrap();

    let start = Instant::now();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut active = std::collections::HashMap::new();

    std::thread::scope(|s| {
        let _abort_1 = coordinator.admit_task(1, 1024).unwrap();
        coordinator.complete_task(1).unwrap();

        assert_eq!(coordinator.ready_set, vec![2, 3, 4]);

        for &id in &[2, 3, 4] {
            let abort = coordinator.admit_task(id, 1024).unwrap();
            active.insert(id, abort);
            let tx_c = tx.clone();
            s.spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(50));
                tx_c.send(id).unwrap();
            });
        }

        assert_eq!(coordinator.memory_gate.active_tokens, 3072);

        for _ in 0..3 {
            let done_id = rx.recv().unwrap();
            active.remove(&done_id);
            coordinator.complete_task(done_id).unwrap();
        }

        assert_eq!(coordinator.ready_set, vec![5]);
        coordinator.admit_task(5, 1024).unwrap();
        coordinator.complete_task(5).unwrap();
    });

    let elapsed = start.elapsed();
    println!("Concurrent branch execution elapsed: {:?}", elapsed);
    assert!(coordinator.is_all_terminal());
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
    assert!(elapsed.as_millis() < 140, "Branches must execute concurrently (makespan < 140ms vs 150ms sequential)!");
}

// ---------------------------------------------------------------------------
// Test 8: Thread-Local Context Isolation
// ---------------------------------------------------------------------------
#[test]
fn test_scoped_concurrency_thread_local_context_isolation() {
    println!("\n=== TEST 8: Thread-Local Context Isolation ===");
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::scope(|s| {
        for worker_id in 0..4 {
            let tx_c = tx.clone();
            s.spawn(move || {
                let mut local_buffer = vec![worker_id as u8; 1024];
                for i in 0..1024 {
                    local_buffer[i] = (worker_id * 10) as u8;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                assert!(local_buffer.iter().all(|&b| b == (worker_id * 10) as u8));
                tx_c.send(worker_id).unwrap();
            });
        }
    });

    let mut completed = Vec::new();
    for _ in 0..4 {
        completed.push(rx.recv().unwrap());
    }
    assert_eq!(completed.len(), 4);
}

// ---------------------------------------------------------------------------
// Test 9: Active Out-of-Bounds Preemption of Concurrent Descendant via abort_flag
// ---------------------------------------------------------------------------
#[test]
fn test_scoped_concurrency_out_of_bounds_aborts_running_descendant() {
    println!("\n=== TEST 9: Out-of-Bounds Preemption ===");
    let k_max = 4096;
    let sg1 = make_test_subgoal(1, "bad_writer.rs", vec![], vec![], vec!["bad_writer.rs"], 1024);
    let sg2 = make_test_subgoal(2, "descendant.rs", vec!["1"], vec!["bad_writer.rs"], vec!["descendant.rs"], 1024);

    let dag = TaskDag::from_subgoals(vec![sg1.clone(), sg2]).unwrap();
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).unwrap();
    let vfs = MemoryVfs::new();

    let _abort_1 = coordinator.admit_task(1, 1024).unwrap();

    let abort_2 = {
        let rec2 = coordinator.records.get_mut(&2).unwrap();
        rec2.status = TaskStatus::Running;
        rec2.allocated_tokens = 1024;
        rec2.abort_flag.clone()
    };
    coordinator.memory_gate.allocate(1024).unwrap();
    assert_eq!(coordinator.memory_gate.active_tokens, 2048);

    let write_res = vfs.write_file_checked(&PathBuf::from("unauthorized.env"), b"EVIL", &sg1.write_set);
    assert!(write_res.is_err());

    let cancelled = coordinator.fail_task(1, "Out of bounds write", Some(DiagnosticCode::OutOfBoundsWrite));
    assert_eq!(cancelled, vec![2]);

    assert!(abort_2.load(Ordering::SeqCst), "Descendant abort flag must be true");
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Cancelled);
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
}

// ---------------------------------------------------------------------------
// Test 10: Dynamic Admission & Backfilling under Context Boom Memory Pressure
// ---------------------------------------------------------------------------
#[test]
fn test_scoped_concurrency_context_boom_dynamic_admission() {
    println!("\n=== TEST 10: Context Boom Dynamic Admission ===");
    let k_max = 4096;
    let sg1 = make_test_subgoal(1, "t1.rs", vec![], vec![], vec!["t1.rs"], 3000);
    let sg2 = make_test_subgoal(2, "t2.rs", vec![], vec![], vec!["t2.rs"], 3000);
    let sg3 = make_test_subgoal(3, "t3.rs", vec![], vec![], vec!["t3.rs"], 800);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).unwrap();

    coordinator.admit_task(1, 3000).unwrap();
    assert_eq!(coordinator.memory_gate.remaining(), 1096);

    let boom = coordinator.admit_task(2, 3000);
    assert!(boom.is_err());

    let _abort_3 = coordinator.admit_task(3, 800).expect("Task 3 backfills cleanly");
    assert_eq!(coordinator.memory_gate.active_tokens, 3800);
    assert_eq!(coordinator.memory_gate.remaining(), 296);

    coordinator.complete_task(1).unwrap();
    coordinator.complete_task(3).unwrap();
    assert_eq!(coordinator.memory_gate.remaining(), 4096);

    coordinator.admit_task(2, 3000).expect("Task 2 admitted after memory freed");
    coordinator.complete_task(2).unwrap();
    assert!(coordinator.is_all_terminal());
}

// ---------------------------------------------------------------------------
// Test 11: Worker Thread Panic Containment via catch_unwind
// ---------------------------------------------------------------------------
#[test]
fn test_scoped_concurrency_worker_panic_containment() {
    println!("\n=== TEST 11: Worker Panic Containment ===");
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::scope(|s| {
        let tx1 = tx.clone();
        s.spawn(move || {
            let res = std::panic::catch_unwind(|| {
                "Worker 1 clean output"
            });
            tx1.send(res.is_ok()).unwrap();
        });

        let tx2 = tx.clone();
        s.spawn(move || {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                panic!("Simulated worker panic!");
            }));
            tx2.send(res.is_err()).unwrap();
        });
    });

    let res1 = rx.recv().unwrap();
    let res2 = rx.recv().unwrap();
    assert!(res1, "Worker 1 must succeed");
    assert!(res2, "Worker 2 panic must be safely caught without crashing process");
}

// ---------------------------------------------------------------------------
// Test 12: Concurrent MemoryVfs Read/Write Operations
// ---------------------------------------------------------------------------
#[test]
fn test_scoped_concurrency_vfs_parallel_read_write_safety() {
    println!("\n=== TEST 12: Concurrent MemoryVfs Safety ===");
    let vfs = Arc::new(MemoryVfs::new());
    vfs.write_file_checked(&PathBuf::from("shared.txt"), b"INITIAL", &[PathBuf::from("shared.txt")]).unwrap();

    std::thread::scope(|s| {
        for _ in 0..5 {
            let vfs_c = vfs.clone();
            s.spawn(move || {
                for _ in 0..20 {
                    let data = vfs_c.read_file_checked(&PathBuf::from("shared.txt"), Some(&[PathBuf::from("shared.txt")])).unwrap();
                    assert!(!data.unwrap().is_empty());
                }
            });
        }

        for w in 0..2 {
            let vfs_c = vfs.clone();
            s.spawn(move || {
                let filename = PathBuf::from(format!("out_{}.txt", w));
                vfs_c.write_file_checked(&filename, format!("DATA_{}", w).as_bytes(), &[filename.clone()]).unwrap();
            });
        }
    });

    assert!(vfs.exists("out_0.txt"));
    assert!(vfs.exists("out_1.txt"));
}

// ---------------------------------------------------------------------------
// Test 13: Zero Double-Free & Strict Token Balance Invariant
// ---------------------------------------------------------------------------
#[test]
fn test_scoped_concurrency_zero_double_free_token_balance() {
    println!("\n=== TEST 13: Zero Double-Free Invariant ===");
    let k_max = 4096;
    let sg1 = make_test_subgoal(1, "a.rs", vec![], vec![], vec!["a.rs"], 1024);
    let sg2 = make_test_subgoal(2, "b.rs", vec!["1"], vec!["a.rs"], vec!["b.rs"], 2048);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
    let mut coordinator = StateMachineCoordinator::new(dag, k_max).unwrap();

    coordinator.admit_task(1, 1024).unwrap();
    assert_eq!(coordinator.memory_gate.active_tokens, 1024);

    {
        let rec2 = coordinator.records.get_mut(&2).unwrap();
        rec2.status = TaskStatus::Running;
        rec2.allocated_tokens = 2048;
    }
    coordinator.memory_gate.allocate(2048).unwrap();
    assert_eq!(coordinator.memory_gate.active_tokens, 3072);

    coordinator.fail_task(1, "Task 1 crashed", None);

    assert_eq!(coordinator.memory_gate.active_tokens, 0);
    assert_eq!(coordinator.memory_gate.remaining(), k_max);

    let dup_res = coordinator.complete_task(2);
    assert!(dup_res.is_err());
    assert_eq!(coordinator.memory_gate.active_tokens, 0);
}

// ---------------------------------------------------------------------------
// Test 14: LRP Critical Path & Priority-Sorted Ready Queue
// ---------------------------------------------------------------------------
#[test]
fn test_rcpsp_lrp_critical_path_and_priority_ready_queue() {
    println!("\n=== TEST 14: LRP Critical Path & Priority Ready Queue ===");
    let sg1 = make_test_subgoal(1, "root.rs", vec![], vec![], vec!["root.rs"], 2048);
    let sg2 = make_test_subgoal(2, "heavy_branch.rs", vec!["1"], vec![], vec!["heavy_branch.rs"], 4096);
    let sg3 = make_test_subgoal(3, "light_branch.rs", vec!["1"], vec![], vec!["light_branch.rs"], 2048);
    let sg4 = make_test_subgoal(4, "sink.rs", vec!["2", "3"], vec![], vec!["sink.rs"], 2048);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4]).unwrap();
    let config = LrpConfig::new(1.0, 2048, None);
    let lrp_table = dag.compute_lrp(&config).unwrap();

    assert_eq!(lrp_table.get(&4).copied().unwrap(), 2048.0);
    assert_eq!(lrp_table.get(&2).copied().unwrap(), 4096.0 + 2048.0);
    assert_eq!(lrp_table.get(&3).copied().unwrap(), 2048.0 + 2048.0);
    assert_eq!(lrp_table.get(&1).copied().unwrap(), 2048.0 + 6144.0);

    let mut coord = StateMachineCoordinator::with_memory_gate_and_lrp(dag, MemoryGate::new(8192), config).unwrap();
    assert_eq!(coord.head_task(), Some(1));
    let _ = coord.admit_task(1, 2048).unwrap();
    let unblocked = coord.complete_task(1).unwrap();
    assert_eq!(unblocked, vec![2, 3]);
    assert_eq!(coord.head_task(), Some(2));
}

// ---------------------------------------------------------------------------
// Test 15: Online RCPSP Scheduler with Pinned EASY Backfilling
// ---------------------------------------------------------------------------
#[test]
fn test_rcpsp_online_pinned_easy_backfilling_full_cycle() {
    println!("\n=== TEST 15: Online Pinned EASY Backfilling ===");
    let scheduler = RcpspScheduler::new(20.0);
    let sg1 = make_test_subgoal(1, "active.rs", vec![], vec![], vec!["active.rs"], 3000);
    let sg2 = make_test_subgoal(2, "head_pinned.rs", vec![], vec![], vec!["head_pinned.rs"], 3000);
    let sg3 = make_test_subgoal(3, "backfill.rs", vec![], vec![], vec!["backfill.rs"], 500);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

    coord.admit_task(1, 3000).unwrap();
    assert_eq!(coord.memory_gate.remaining(), 1096);

    let mut lrp_map = HashMap::new();
    lrp_map.insert(2, 200.0);
    lrp_map.insert(3, 50.0);

    let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
    match decision {
        ScheduleDecision::PinnedWithBackfill {
            pinned_head_id,
            backfilled_task_ids,
            reservation_delay_secs,
        } => {
            assert_eq!(pinned_head_id, 2);
            assert_eq!(backfilled_task_ids, vec![3]);
            assert!(reservation_delay_secs > 0.0);
        }
        other => panic!("Expected PinnedWithBackfill, got {:?}", other),
    }

    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Running);
    assert_eq!(coord.memory_gate.active_tokens, 3500);
}
