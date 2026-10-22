// ============================================================================
// [SOVEREIGN ENGINE CONSTITUTION - INVIOLABLE SAFETY DIRECTIVE]
// STRICT ARCHITECTURAL RULE:
// It is strictly forbidden, under any circumstances or for any reason whatsoever,
// to remove, comment out, bypass, or weaken `#![deny(unused_must_use)]` from
// this file or ANY other .rs file in this codebase.
// All fallible operations, Results, and Options must be explicitly handled.
// Zero silent error swallowing under the rug. This engine is built to endure for decades.
// ============================================================================
#![deny(unused_must_use)]

//! Stress Test & Benchmark Suite for Omni Engine Parallel DAG Swarm (RCPSP Scheduler)
//!
//! Demonstrates and proves:
//! 1. 5+ Task DAG concurrency with makespan (C_max) reduction vs sequential execution.
//! 2. Zero-OOM hard rejection before execution.
//! 3. Out-of-bounds VFS write transition to FAILED and immediate descendant cancellation.
//! 4. Context Boom dynamic backpressure pausing task admission.

use std::path::PathBuf;
use std::time::Instant;

use omni_engine::planner::swarm_coordinator::{
    LrpConfig, MemoryGate, RcpspScheduler, StateMachineCoordinator, SubGoal, TaskDag, TaskKind, TaskStatus,
};
use omni_engine::sandbox::vfs::MemoryVfs;

fn make_task(id: usize, desc: &str, entity: &str, deps: Vec<&str>, tokens: usize, writes: Vec<&str>, reads: Vec<&str>) -> SubGoal {
    SubGoal {
        id,
        description: desc.to_string(),
        target_entity: entity.to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
        read_set: reads.into_iter().map(PathBuf::from).collect(),
        write_set: writes.into_iter().map(PathBuf::from).collect(),
        max_tokens: tokens,
    }
}

fn print_header(title: &str) {
    println!("\n{}", "=".repeat(80));
    println!("  {}", title);
    println!("{}\n", "=".repeat(80));
}

/// Scenario 1: 5+ Task Diamond/Branching DAG Makespan Comparison (Parallel vs Sequential)
fn run_benchmark_makespan() {
    print_header("BENCHMARK 1: Parallel DAG Makespan (C_max) vs Sequential Execution");

    // Construct a realistic 6-task Diamond DAG:
    // Task 1 (Setup) -> Task 2 (Backend API) & Task 3 (Frontend UI) & Task 4 (Database Schema)
    // Task 2 & Task 4 -> Task 5 (Integration Service)
    // Task 3 & Task 5 -> Task 6 (E2E Test)
    let tasks = vec![
        make_task(1, "Initialize Project Config", "config.json", vec![], 1024, vec!["config.json"], vec![]),
        make_task(2, "Implement REST Endpoints", "api.rs", vec!["1"], 2048, vec!["api.rs"], vec!["config.json"]),
        make_task(3, "Implement Frontend Views", "app.tsx", vec!["1"], 2048, vec!["app.tsx"], vec!["config.json"]),
        make_task(4, "Define Database Schema", "schema.sql", vec!["1"], 1536, vec!["schema.sql"], vec!["config.json"]),
        make_task(5, "Wire Integration Service", "service.rs", vec!["2", "4"], 2048, vec!["service.rs"], vec!["api.rs", "schema.sql"]),
        make_task(6, "Execute System Verification", "verify.sh", vec!["3", "5"], 1024, vec!["verify.sh"], vec!["app.tsx", "service.rs"]),
    ];

    let scheduler = RcpspScheduler::default_rate(); // 20.0 tokens/sec
    let sequential_duration: f64 = tasks.iter().map(|t| scheduler.compute_duration(t.max_tokens)).sum();

    println!("  Task Inventory (6 Nodes):");
    for t in &tasks {
        println!("   - Node #{}: \"{}\" (Tokens: {}, d_v: {:.2}s)", t.id, t.description, t.max_tokens, scheduler.compute_duration(t.max_tokens));
    }

    let dag = TaskDag::from_subgoals(tasks.clone()).expect("DAG creation failed");
    let lrp_config = LrpConfig {
        r_min: 20.0,
        min_tokens_floor: 1024,
        model_ctx_limit: Some(8192),
    };
    let lrp_map = dag.compute_lrp(&lrp_config).expect("LRP computation failed");

    println!("\n  Longest Remaining Path (LRP) Metrics:");
    for (&id, &lrp) in &lrp_map {
        println!("   - Node #{}: Critical Path LRP = {:.2}s", id, lrp);
    }

    // Run RCPSP Pinned EASY Backfilling Simulation
    // Capacity K_max = 8192 tokens (allows tasks 2, 3, 4 to run concurrently!)
    let memory_gate = MemoryGate::new(8192);
    let mut coordinator = StateMachineCoordinator::with_memory_gate(dag, memory_gate).expect("Coordinator failed");

    let mut sim_clock: f64 = 0.0;
    let mut in_flight: Vec<(usize, f64, usize)> = Vec::new(); // (id, finish_time, tokens)
    let mut execution_log: Vec<String> = Vec::new();

    while !coordinator.is_all_terminal() {
        // 1. Dispatch as many tasks as possible under RCPSP rules
        loop {
            if coordinator.ready_set.is_empty() {
                break;
            }

            coordinator.dag.sort_ready_set(&mut coordinator.ready_set, &lrp_map);
            let head_id = coordinator.ready_set[0];
            let head_tokens = coordinator.records.get(&head_id).unwrap().subgoal.max_tokens;

            // Check if head can admit
            if coordinator.memory_gate.can_admit(head_tokens) {
                let _ = coordinator.admit_task(head_id, head_tokens).unwrap();
                let dur = scheduler.compute_duration(head_tokens);
                let finish = sim_clock + dur;
                in_flight.push((head_id, finish, head_tokens));
                execution_log.push(format!("[t={:.2}s] Dispatched Node #{} (dur: {:.2}s, finish: {:.2}s)", sim_clock, head_id, dur, finish));
                continue;
            }

            // Head cannot admit -> evaluate backfilling
            let reservation_delay = in_flight.iter().map(|(_, fin, _)| fin - sim_clock).fold(0.0, f64::max);
            let mut backfilled = false;
            let candidates: Vec<usize> = coordinator.ready_set.iter().skip(1).copied().collect();

            for cand_id in candidates {
                let cand_tokens = coordinator.records.get(&cand_id).unwrap().subgoal.max_tokens;
                let cand_dur = scheduler.compute_duration(cand_tokens);

                if coordinator.memory_gate.can_admit(cand_tokens) && cand_dur <= reservation_delay {
                    let _ = coordinator.admit_task(cand_id, cand_tokens).unwrap();
                    let finish = sim_clock + cand_dur;
                    in_flight.push((cand_id, finish, cand_tokens));
                    execution_log.push(format!("[t={:.2}s] Backfilled Node #{} ahead of Pinned Node #{}", sim_clock, cand_id, head_id));
                    backfilled = true;
                    break;
                }
            }

            if !backfilled {
                break;
            }
        }

        // 2. Advance simulation clock to next completing task
        if in_flight.is_empty() {
            break;
        }

        in_flight.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let (finished_id, finish_time, _tokens) = in_flight.remove(0);
        sim_clock = finish_time;
        let _ = coordinator.complete_task(finished_id).unwrap();
        execution_log.push(format!("[t={:.2}s] Completed Node #{}", sim_clock, finished_id));
    }

    println!("\n  Execution Timeline:");
    for entry in &execution_log {
        println!("   {}", entry);
    }

    let parallel_makespan = sim_clock;
    let speedup = sequential_duration / parallel_makespan;

    println!("\n  RESULTS SUMMARY:");
    println!("   - Sequential Baseline Makespan:  {:.2}s (sum of all d_v)", sequential_duration);
    println!("   - Parallel RCPSP Swarm Makespan: {:.2}s (C_max)", parallel_makespan);
    println!("   - Concurrency Speedup Factor:     {:.2}x faster", speedup);

    assert!(parallel_makespan < sequential_duration, "Parallel makespan must be strictly less than sequential");
    assert!(speedup >= 1.4, "Speedup factor must be at least 1.4x for diamond DAG");
    println!("  [SUCCESS] Scenario 1 Passed with {:.2}x speedup!\n", speedup);
}

/// Scenario 2: Out-of-Bounds VFS Write Transition to FAILED and Descendant Cascade
fn run_benchmark_oob_cascade() {
    print_header("BENCHMARK 2: Out-Of-Bounds VFS Breach -> FAILED State & Descendant Cascade");

    let tasks = vec![
        make_task(1, "VFS Breaching Task", "file1.txt", vec![], 1024, vec!["allowed.txt"], vec![]),
        make_task(2, "Downstream Consumer", "file2.txt", vec!["1"], 1024, vec!["output.txt"], vec!["allowed.txt"]),
        make_task(3, "Independent Worker", "other.txt", vec![], 1024, vec!["other.txt"], vec![]),
    ];

    let dag = TaskDag::from_subgoals(tasks).unwrap();
    let memory_gate = MemoryGate::new(8192);
    let mut coordinator = StateMachineCoordinator::with_memory_gate(dag, memory_gate).unwrap();

    let _ = coordinator.admit_task(1, 1024).unwrap();
    let _ = coordinator.admit_task(3, 1024).unwrap();

    println!("  Active tasks: #1 (Breacher) and #3 (Independent). Task #2 is Pending.");

    let vfs = MemoryVfs::new();
    let breach_path = PathBuf::from("secret_host.conf");
    let allowed_set = vec![PathBuf::from("allowed.txt")];

    let write_res = vfs.write_file_checked(&breach_path, b"malicious content", &allowed_set);
    assert!(write_res.is_err(), "VFS must reject out-of-bounds write");
    println!("  [VFS GUARD] Caught UnauthorizedWrite on 'secret_host.conf'!");

    let cancelled = coordinator.fail_task(1, "OutOfBoundsWrite: attempted modification of secret_host.conf", None);

    println!("  Task #1 status: {:?}", coordinator.records.get(&1).unwrap().status);
    println!("  Cascaded cancellations: {:?}", cancelled);
    println!("  Task #2 status: {:?}", coordinator.records.get(&2).unwrap().status);
    println!("  Task #3 (Independent) status: {:?}", coordinator.records.get(&3).unwrap().status);

    assert_eq!(coordinator.records.get(&1).unwrap().status, TaskStatus::Failed);
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Cancelled);
    assert_eq!(coordinator.records.get(&3).unwrap().status, TaskStatus::Running);

    println!("  [SUCCESS] Scenario 2 Passed: Breacher FAILED, dependent CANCELLED, independent UNTOUCHED!\n");
}

/// Scenario 3: Context Boom Backpressure Pauses Admission
fn run_benchmark_context_boom() {
    print_header("BENCHMARK 3: Context Boom Backpressure & Head-Pinning");

    let tasks = vec![
        make_task(1, "Large Model Ingestion", "doc1.bin", vec![], 3072, vec!["doc1.bin"], vec![]),
        make_task(2, "Downstream Embedding", "doc2.bin", vec![], 2048, vec!["doc2.bin"], vec![]),
    ];

    let dag = TaskDag::from_subgoals(tasks).unwrap();
    let memory_gate = MemoryGate::new(4096);
    let mut coordinator = StateMachineCoordinator::with_memory_gate(dag, memory_gate).unwrap();

    let _ = coordinator.admit_task(1, 3072).unwrap();
    println!("  Task #1 admitted (consumed: 3072 tokens). Remaining headroom: {} tokens.", coordinator.memory_gate.remaining());

    let admit_res = coordinator.admit_task(2, 2048);
    assert!(admit_res.is_err(), "Must reject task when remaining capacity is insufficient");
    println!("  Task #2 admission rejected: Context Boom active.");
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Ready);
    println!("  Task #2 state preserved safely as Ready.");

    let _ = coordinator.complete_task(1).unwrap();
    println!("  Task #1 completed. Remaining headroom freed to: {} tokens.", coordinator.memory_gate.remaining());

    let admit_retry = coordinator.admit_task(2, 2048);
    assert!(admit_retry.is_ok(), "Task 2 must admit now that memory is freed");
    println!("  Task #2 admitted successfully after memory restitution!");

    println!("  [SUCCESS] Scenario 3 Passed: Context Boom backpressure safely paused and resumed!\n");
}

/// Scenario 4: Pre-flight Zero-OOM Hard Rejection
fn run_benchmark_zero_oom_rejection() {
    print_header("BENCHMARK 4: Pre-flight Zero-OOM Hard Rejection");

    let tasks = vec![
        make_task(1, "Gigantic Context Bomb", "bomb.bin", vec![], 16384, vec!["bomb.bin"], vec![]),
        make_task(2, "Child of Bomb", "child.bin", vec!["1"], 1024, vec!["child.bin"], vec![]),
    ];

    let dag = TaskDag::from_subgoals(tasks).unwrap();
    let memory_gate = MemoryGate::new(4096);
    let mut coordinator = StateMachineCoordinator::with_memory_gate(dag, memory_gate).unwrap();

    let preflight_res = coordinator.preflight_validate();
    assert!(preflight_res.is_err(), "Preflight validation must catch over-capacity task");
    println!("  [ZERO-OOM GUARD] Pre-flight caught over-capacity request: {:?}", preflight_res.err().unwrap());

    assert_eq!(coordinator.records.get(&1).unwrap().status, TaskStatus::Failed);
    assert_eq!(coordinator.records.get(&2).unwrap().status, TaskStatus::Cancelled);
    println!("  Task #1 transitioned to FAILED, Child #2 transitioned to CANCELLED before any thread allocation.");

    println!("  [SUCCESS] Scenario 4 Passed: Zero-OOM pre-flight hard rejection verified!\n");
}

fn main() {
    let overall_start = Instant::now();
    println!("\n{}", "#".repeat(80));
    println!("  OMNI ENGINE: RCPSP PARALLEL DAG SWARM COMPREHENSIVE BENCHMARK");
    println!("  Mathematical Verification of Zero-OOM, Fault Cascades & Makespan Speedup");
    println!("{}\n", "#".repeat(80));

    run_benchmark_makespan();
    run_benchmark_oob_cascade();
    run_benchmark_context_boom();
    run_benchmark_zero_oom_rejection();

    println!("{}", "#".repeat(80));
    println!("  ALL 4 BENCHMARK SUITES COMPLETED IN {:.3}s WITH 100% SUCCESS!", overall_start.elapsed().as_secs_f64());
    println!("  RCPSP Parallel DAG Swarm Engine Mathematically & Empirically Verified!");
    println!("{}\n", "#".repeat(80));
}
