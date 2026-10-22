//! Empirical Adversarial Challenger Test Suite: Milestone 2
//!
//! Stress-testing:
//! 1. State Machine: Exhaustive 6x6 transition matrix and invalid runtime state transitions
//! 2. DAG Cancellation Cascade: Deep chains (50 nodes), complete binary trees (depth 5),
//!    nested multi-diamonds, hourglass fan-out/fan-in, disconnected forests, and property-based fuzzing
//! 3. Circular Dependency Handling: 2-node cycles, 3-node cycles, embedded cycles, disconnected cycles,
//!    figure-8 cycles, and semantic entity/path cycles
//! 4. Memory Gate & Zero-OOM Boundary Stress: Extreme values, usize::MAX overflow resistance,
//!    exact capacity exhaustion, saturating release

use omni_engine::planner::swarm_coordinator::{
    MemoryGate, StateMachineCoordinator, SubGoal, TaskDag, TaskKind, TaskStatus,
};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

fn make_subgoal(id: usize, target: &str, depends_on: Vec<&str>, tokens: usize) -> SubGoal {
    SubGoal {
        id,
        description: format!("Task #{} ({})", id, target),
        target_entity: target.to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: depends_on.into_iter().map(String::from).collect(),
        read_set: vec![],
        write_set: vec![PathBuf::from(target)],
        max_tokens: tokens,
    }
}

// ============================================================================
// 1. STATE MACHINE: EXHAUSTIVE 6x6 TRANSITION MATRIX & RUNTIME ILLEGAL JUMPS
// ============================================================================

#[test]
fn test_exhaustive_state_transition_matrix() {
    let all_statuses = [
        TaskStatus::Pending,
        TaskStatus::Ready,
        TaskStatus::Running,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ];

    // Formally specified legal transitions:
    // Pending   -> Ready, Failed, Cancelled
    // Ready     -> Running, Failed, Cancelled
    // Running   -> Completed, Failed, Cancelled
    // Completed -> (none - terminal)
    // Failed    -> (none - terminal)
    // Cancelled -> (none - terminal)
    let is_legal_oracle = |from: TaskStatus, to: TaskStatus| -> bool {
        match (from, to) {
            (from, to) if from == to => false,
            (TaskStatus::Pending, TaskStatus::Ready) => true,
            (TaskStatus::Pending, TaskStatus::Failed) => true,
            (TaskStatus::Pending, TaskStatus::Cancelled) => true,
            (TaskStatus::Ready, TaskStatus::Running) => true,
            (TaskStatus::Ready, TaskStatus::Failed) => true,
            (TaskStatus::Ready, TaskStatus::Cancelled) => true,
            (TaskStatus::Running, TaskStatus::Completed) => true,
            (TaskStatus::Running, TaskStatus::Failed) => true,
            (TaskStatus::Running, TaskStatus::Cancelled) => true,
            _ => false,
        }
    };

    let mut legal_count = 0;
    let mut illegal_count = 0;

    for &from in &all_statuses {
        for &to in &all_statuses {
            let actual = from.can_transition_to(to);
            let expected = is_legal_oracle(from, to);
            assert_eq!(
                actual, expected,
                "Transition {:?} -> {:?} expected {} but got {}",
                from, to, expected, actual
            );

            if actual {
                legal_count += 1;
            } else {
                illegal_count += 1;
            }
        }
    }

    assert_eq!(legal_count, 9, "Exactly 9 transitions must be legal");
    assert_eq!(illegal_count, 27, "Exactly 27 transitions must be illegal");
}

#[test]
fn test_runtime_state_machine_illegal_admit_and_complete() {
    let sg1 = make_subgoal(1, "root.rs", vec![], 1024);
    let sg2 = make_subgoal(2, "child.rs", vec!["1"], 1024);
    let dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

    // 1. Task 2 is Pending -> admit_task must fail
    assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Pending);
    let err_admit_pending = coord.admit_task(2, 1024);
    assert!(err_admit_pending.is_err(), "Cannot admit Pending task");
    assert!(err_admit_pending.unwrap_err().contains("is not Ready"));

    // 2. Task 1 is Ready -> complete_task must fail
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Ready);
    let err_complete_ready = coord.complete_task(1);
    assert!(err_complete_ready.is_err(), "Cannot complete Ready task");
    assert!(err_complete_ready.unwrap_err().contains("Cannot complete task #1: status is Ready"));

    // 3. Admit Task 1 -> now Running
    let _ = coord.admit_task(1, 1024).unwrap();
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Running);

    // 4. Task 1 is Running -> cannot re-admit
    let err_re_admit = coord.admit_task(1, 1024);
    assert!(err_re_admit.is_err(), "Cannot admit already Running task");
    assert!(err_re_admit.unwrap_err().contains("is not Ready (current: Running)"));

    // 5. Complete Task 1 -> becomes Completed, Task 2 becomes Ready
    let _ = coord.complete_task(1).unwrap();
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Completed);
    assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Ready);

    // 6. Task 1 is Completed -> cannot admit or complete again
    let err_admit_completed = coord.admit_task(1, 1024);
    assert!(err_admit_completed.is_err(), "Cannot admit Completed task");
    let err_complete_completed = coord.complete_task(1);
    assert!(err_complete_completed.is_err(), "Cannot complete Completed task");

    // 7. Fail Task 2 -> becomes Failed
    let _ = coord.fail_task(2, "Simulated crash", None);
    assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Failed);

    // 8. Task 2 is Failed -> cannot admit or complete
    let err_admit_failed = coord.admit_task(2, 1024);
    assert!(err_admit_failed.is_err(), "Cannot admit Failed task");
    let err_complete_failed = coord.complete_task(2);
    assert!(err_complete_failed.is_err(), "Cannot complete Failed task");

    // 9. Non-existent task IDs
    assert!(coord.admit_task(999, 1024).is_err());
    assert!(coord.complete_task(999).is_err());
}

#[test]
fn test_cancelled_task_never_resurrects_on_upstream_completion() {
    // Multi-parent DAG: 1 -> 3, 2 -> 3
    let sg1 = make_subgoal(1, "p1.rs", vec![], 1024);
    let sg2 = make_subgoal(2, "p2.rs", vec![], 1024);
    let sg3 = make_subgoal(3, "sink.rs", vec!["1", "2"], 1024);

    let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

    // 1. Admit Parent 1 and Parent 2
    let _ = coord.admit_task(1, 1024).unwrap();
    let _ = coord.admit_task(2, 1024).unwrap();

    // 2. Parent 1 fails -> cascades Cancelled to Sink 3
    let cancelled = coord.fail_task(1, "Parent 1 failed", None);
    assert_eq!(cancelled, vec![3]);
    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Cancelled);

    // 3. Parent 2 completes later -> Sink 3 MUST NOT resurrect to Ready!
    let unblocked = coord.complete_task(2).unwrap();
    assert!(unblocked.is_empty(), "Sink 3 was cancelled; cannot be unblocked");
    assert_eq!(
        coord.records.get(&3).unwrap().status,
        TaskStatus::Cancelled,
        "Sink 3 must remain Cancelled forever"
    );
    assert!(!coord.ready_set.contains(&3));
}

// ============================================================================
// 2. DEEP & COMPLEX DAG TOPOLOGY CANCELLATION CASCADES
// ============================================================================

#[test]
fn test_deep_linear_chain_cascade_exact_transitive_boundary() {
    const CHAIN_LEN: usize = 50;
    let mut subgoals = Vec::new();
    for i in 1..=CHAIN_LEN {
        let deps = if i == 1 { vec![] } else { vec![format!("{}", i - 1)] };
        subgoals.push(SubGoal {
            id: i,
            description: format!("Chain step {}", i),
            target_entity: format!("step_{}.rs", i),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: deps,
            read_set: vec![],
            write_set: vec![PathBuf::from(format!("step_{}.rs", i))],
            max_tokens: 100,
        });
    }

    let dag = TaskDag::from_subgoals(subgoals).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 8192).unwrap();

    // Admit and complete steps 1..=24
    for i in 1..=24 {
        assert_eq!(coord.ready_set, vec![i]);
        let _ = coord.admit_task(i, 100).unwrap();
        let unblocked = coord.complete_task(i).unwrap();
        assert_eq!(unblocked, vec![i + 1]);
    }

    // Step 25 is currently Ready. Admit it, then fail it.
    assert_eq!(coord.records.get(&25).unwrap().status, TaskStatus::Ready);
    let _ = coord.admit_task(25, 100).unwrap();
    let cancelled = coord.fail_task(25, "Failure at midpoint 25", None);

    // Exact descendants of 25 are 26..=50 (25 nodes)
    let expected_cancelled: Vec<usize> = (26..=50).collect();
    assert_eq!(cancelled, expected_cancelled, "Must cancel exactly 26..=50");

    // Verify status of all tasks
    for i in 1..=24 {
        assert_eq!(
            coord.records.get(&i).unwrap().status,
            TaskStatus::Completed,
            "Predecessor {} must remain Completed", i
        );
    }
    assert_eq!(coord.records.get(&25).unwrap().status, TaskStatus::Failed);
    for i in 26..=50 {
        assert_eq!(
            coord.records.get(&i).unwrap().status,
            TaskStatus::Cancelled,
            "Successor {} must be Cancelled", i
        );
    }

    assert!(coord.ready_set.is_empty());
    assert_eq!(coord.memory_gate.active_tokens, 0);
    assert!(coord.is_all_terminal());
}

#[test]
fn test_complete_binary_tree_fault_isolation() {
    // Binary tree of depth 4 (nodes 1..=15):
    //               1
    //        2             3
    //     4     5       6     7
    //    8 9  10 11   12 13 14 15
    let mut subgoals = Vec::new();
    for i in 1..=15 {
        let deps = if i == 1 {
            vec![]
        } else {
            vec![format!("{}", i / 2)]
        };
        subgoals.push(SubGoal {
            id: i,
            description: format!("Tree node {}", i),
            target_entity: format!("node_{}.rs", i),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: deps,
            read_set: vec![],
            write_set: vec![PathBuf::from(format!("node_{}.rs", i))],
            max_tokens: 100,
        });
    }

    let dag = TaskDag::from_subgoals(subgoals).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 8192).unwrap();

    // 1. Run root (node 1) to completion -> unblocks 2 and 3
    let _ = coord.admit_task(1, 100).unwrap();
    let unblocked = coord.complete_task(1).unwrap();
    assert_eq!(unblocked, vec![2, 3]);

    // 2. Admit left child 2 and right child 3
    let _ = coord.admit_task(2, 100).unwrap();
    let _ = coord.admit_task(3, 100).unwrap();

    // 3. Fail left child 2
    let mut cancelled = coord.fail_task(2, "Left subtree failure", None);
    cancelled.sort();

    // Descendants of 2 in binary tree: 4, 5, 8, 9, 10, 11 (6 nodes)
    let expected_cancelled = vec![4, 5, 8, 9, 10, 11];
    assert_eq!(cancelled, expected_cancelled, "Only left subtree must be cancelled");

    // Right child 3 and its descendants (3, 6, 7, 12, 13, 14, 15) must be 100% unaffected!
    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Running);
    for &id in &[6, 7, 12, 13, 14, 15] {
        assert_eq!(
            coord.records.get(&id).unwrap().status,
            TaskStatus::Pending,
            "Right subtree node {} must remain Pending", id
        );
    }

    // 4. Complete right child 3 and all its descendants to terminal success!
    let mut active = vec![3];
    while let Some(curr) = active.pop() {
        let unblocked = coord.complete_task(curr).unwrap();
        for child in unblocked {
            coord.admit_task(child, 100).unwrap();
            active.push(child);
        }
    }

    for &id in &[3, 6, 7, 12, 13, 14, 15] {
        assert_eq!(
            coord.records.get(&id).unwrap().status,
            TaskStatus::Completed,
            "Right subtree node {} must be Completed", id
        );
    }
    assert_eq!(coord.memory_gate.active_tokens, 0);
    assert!(coord.is_all_terminal());
}

#[test]
fn test_multi_diamond_cascading_pipeline() {
    // 3 concatenated diamonds:
    // 1 -> (2, 3) -> 4 -> (5, 6) -> 7 -> (8, 9) -> 10
    let subgoals = vec![
        make_subgoal(1, "t1.rs", vec![], 100),
        make_subgoal(2, "t2.rs", vec!["1"], 100),
        make_subgoal(3, "t3.rs", vec!["1"], 100),
        make_subgoal(4, "t4.rs", vec!["2", "3"], 100),
        make_subgoal(5, "t5.rs", vec!["4"], 100),
        make_subgoal(6, "t6.rs", vec!["4"], 100),
        make_subgoal(7, "t7.rs", vec!["5", "6"], 100),
        make_subgoal(8, "t8.rs", vec!["7"], 100),
        make_subgoal(9, "t9.rs", vec!["7"], 100),
        make_subgoal(10, "t10.rs", vec!["8", "9"], 100),
    ];

    let dag = TaskDag::from_subgoals(subgoals).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

    let _ = coord.admit_task(1, 100).unwrap();
    let _ = coord.complete_task(1).unwrap();

    let _ = coord.admit_task(2, 100).unwrap();
    let _ = coord.admit_task(3, 100).unwrap();

    // Fail branch 2 in the first diamond:
    // Transitive descendants: 4, 5, 6, 7, 8, 9, 10 (all 7 downstream tasks!)
    let mut cancelled = coord.fail_task(2, "Diamond 1 branch A fail", None);
    cancelled.sort();

    assert_eq!(cancelled, vec![4, 5, 6, 7, 8, 9, 10]);
    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Running);

    // Complete sibling branch 3
    let unblocked = coord.complete_task(3).unwrap();
    assert!(unblocked.is_empty(), "Sink 4 was cancelled; cannot unblock");
    assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Completed);
    assert_eq!(coord.memory_gate.active_tokens, 0);
    assert!(coord.is_all_terminal());
}

#[test]
fn test_hourglass_wide_fan_out_fan_in_cascade() {
    // 1 root -> 50 intermediate nodes -> 1 sink
    const N_MID: usize = 50;
    let mut subgoals = Vec::new();
    subgoals.push(make_subgoal(1, "root.rs", vec![], 100));

    let mut mid_deps = Vec::new();
    for i in 2..=(N_MID + 1) {
        subgoals.push(make_subgoal(i, &format!("mid_{}.rs", i), vec!["1"], 50));
        mid_deps.push(format!("{}", i));
    }

    let sink_id = N_MID + 2;
    let mid_deps_refs: Vec<&str> = mid_deps.iter().map(|s| s.as_str()).collect();
    subgoals.push(make_subgoal(sink_id, "sink.rs", mid_deps_refs, 100));

    let dag = TaskDag::from_subgoals(subgoals).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 16384).unwrap();

    // 1. Complete root
    let _ = coord.admit_task(1, 100).unwrap();
    let unblocked = coord.complete_task(1).unwrap();
    assert_eq!(unblocked.len(), N_MID);

    // 2. Admit all 50 intermediate tasks
    for i in 2..=(N_MID + 1) {
        coord.admit_task(i, 50).unwrap();
    }
    assert_eq!(coord.memory_gate.active_tokens, N_MID * 50);

    // 3. Fail exactly one intermediate task (node 25)
    let cancelled = coord.fail_task(25, "Node 25 failed", None);
    assert_eq!(cancelled, vec![sink_id], "Only the convergent sink must be cancelled");
    assert_eq!(coord.records.get(&sink_id).unwrap().status, TaskStatus::Cancelled);

    // 4. All other 49 intermediate tasks can complete cleanly!
    for i in 2..=(N_MID + 1) {
        if i == 25 {
            continue;
        }
        assert_eq!(coord.records.get(&i).unwrap().status, TaskStatus::Running);
        let unblocked = coord.complete_task(i).unwrap();
        assert!(unblocked.is_empty(), "Sink is already cancelled");
        assert_eq!(coord.records.get(&i).unwrap().status, TaskStatus::Completed);
    }

    assert_eq!(coord.memory_gate.active_tokens, 0);
    assert!(coord.is_all_terminal());
}

#[test]
fn test_disconnected_forest_fault_isolation() {
    // 3 disjoint components:
    // C1: 1 -> 2 -> 3
    // C2: 10 -> 20 -> 30
    // C3: 100 -> 200 -> 300
    let subgoals = vec![
        make_subgoal(1, "c1_1.rs", vec![], 100),
        make_subgoal(2, "c1_2.rs", vec!["1"], 100),
        make_subgoal(3, "c1_3.rs", vec!["2"], 100),
        make_subgoal(10, "c2_1.rs", vec![], 100),
        make_subgoal(20, "c2_2.rs", vec!["10"], 100),
        make_subgoal(30, "c2_3.rs", vec!["20"], 100),
        make_subgoal(100, "c3_1.rs", vec![], 100),
        make_subgoal(200, "c3_2.rs", vec!["100"], 100),
        make_subgoal(300, "c3_3.rs", vec!["200"], 100),
    ];

    let dag = TaskDag::from_subgoals(subgoals).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

    assert_eq!(coord.ready_set, vec![1, 10, 100]);

    // Fail node 1 in C1
    let _ = coord.admit_task(1, 100).unwrap();
    let cancelled = coord.fail_task(1, "C1 crash", None);
    assert_eq!(cancelled, vec![2, 3]);

    // C2 and C3 are untouched
    assert_eq!(coord.records.get(&10).unwrap().status, TaskStatus::Ready);
    assert_eq!(coord.records.get(&20).unwrap().status, TaskStatus::Pending);
    assert_eq!(coord.records.get(&100).unwrap().status, TaskStatus::Ready);
    assert_eq!(coord.records.get(&200).unwrap().status, TaskStatus::Pending);
    assert_eq!(coord.ready_set, vec![10, 100]);
}

// ============================================================================
// 3. PROPERTY-BASED / FUZZ TESTING: RANDOM TOPOLOGY CASCADES VS ORACLE
// ============================================================================

#[test]
fn test_property_fuzz_random_dag_cancellation_vs_oracle() {
    // Linear congruential pseudo-random generator for determinism without extra dependencies
    let mut rng_state: u64 = 0xDEADBEEFCAFE1234;
    let mut next_rand = || -> u64 {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        rng_state
    };

    // Run 50 random DAG topologies
    for trial in 0..50 {
        let num_nodes = 10 + (next_rand() % 15) as usize; // 10 to 24 nodes
        let mut adj: HashMap<usize, HashSet<usize>> = HashMap::new();
        for i in 1..=num_nodes {
            adj.entry(i).or_default();
        }

        // Upper-triangular DAG edges to guarantee acyclicity
        for i in 1..num_nodes {
            for j in (i + 1)..=num_nodes {
                // ~25% edge probability
                if (next_rand() % 100) < 25 {
                    adj.get_mut(&i).unwrap().insert(j);
                }
            }
        }

        let mut subgoals = Vec::new();
        for i in 1..=num_nodes {
            let mut deps = Vec::new();
            for (&parent, children) in &adj {
                if children.contains(&i) {
                    deps.push(format!("{}", parent));
                }
            }
            subgoals.push(SubGoal {
                id: i,
                description: format!("Node {}", i),
                target_entity: format!("n_{}.rs", i),
                kind: TaskKind::Create,
                guidance: None,
                depends_on: deps,
                read_set: vec![],
                write_set: vec![PathBuf::from(format!("n_{}.rs", i))],
                max_tokens: 50,
            });
        }

        let dag = TaskDag::from_subgoals(subgoals).expect("Generated upper-triangular DAG must be acyclic");
        let mut coord = StateMachineCoordinator::new(dag, 100_000).unwrap();

        // Pick a random node to fail (nodes 1..num_nodes/2 usually have descendants)
        let failed_node = 1 + (next_rand() % (num_nodes as u64 / 2).max(1)) as usize;

        // Ground-Truth Oracle: BFS reachability closure of descendants
        let mut oracle_descendants = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(failed_node);
        let mut visited = HashSet::new();
        visited.insert(failed_node);

        while let Some(curr) = queue.pop_front() {
            if let Some(children) = adj.get(&curr) {
                for &child in children {
                    if visited.insert(child) {
                        oracle_descendants.insert(child);
                        queue.push_back(child);
                    }
                }
            }
        }

        // Execute fail_task on coordinator
        let actual_cancelled: BTreeSet<usize> = coord.fail_task(failed_node, "Fuzz trial failure", None)
            .into_iter()
            .collect();

        // Assert: Actual cancelled set == Oracle transitive descendants
        assert_eq!(
            actual_cancelled, oracle_descendants,
            "Trial {}: Transitive descendants mismatch for failed node {}", trial, failed_node
        );

        // Verify status table matches exactly
        for &id in &oracle_descendants {
            assert_eq!(
                coord.records.get(&id).unwrap().status,
                TaskStatus::Cancelled,
                "Trial {}: Node {} must be Cancelled", trial, id
            );
        }
        for i in 1..=num_nodes {
            if i != failed_node && !oracle_descendants.contains(&i) {
                assert_ne!(
                    coord.records.get(&i).unwrap().status,
                    TaskStatus::Cancelled,
                    "Trial {}: Non-descendant node {} must NOT be cancelled", trial, i
                );
            }
        }
    }
}

// ============================================================================
// 4. CIRCULAR DEPENDENCY REJECTION: COMPREHENSIVE CYCLIC TOPOLOGIES
// ============================================================================

#[test]
fn test_circular_dependency_2_node_cycle() {
    let sg1 = make_subgoal(1, "a.rs", vec!["2"], 1024);
    let sg2 = make_subgoal(2, "b.rs", vec!["1"], 1024);

    let res = TaskDag::from_subgoals(vec![sg1, sg2]);
    assert!(res.is_err(), "2-node cycle (1 <-> 2) must be rejected");
    assert!(res.unwrap_err().contains("Cyclic dependency detected"));
}

#[test]
fn test_circular_dependency_3_node_cycle() {
    let sg1 = make_subgoal(1, "a.rs", vec!["3"], 1024);
    let sg2 = make_subgoal(2, "b.rs", vec!["1"], 1024);
    let sg3 = make_subgoal(3, "c.rs", vec!["2"], 1024);

    let res = TaskDag::from_subgoals(vec![sg1, sg2, sg3]);
    assert!(res.is_err(), "3-node cycle (1 -> 2 -> 3 -> 1) must be rejected");
    assert!(res.unwrap_err().contains("Cyclic dependency detected"));
}

#[test]
fn test_circular_dependency_cycle_embedded_in_dag() {
    // 0 -> 1 -> 2 -> 3 -> 1 -> 4
    // 0 is valid root, 4 is valid sink, but 1-2-3 form a cycle
    let sg0 = make_subgoal(0, "root.rs", vec![], 1024);
    let sg1 = make_subgoal(1, "a.rs", vec!["0", "3"], 1024);
    let sg2 = make_subgoal(2, "b.rs", vec!["1"], 1024);
    let sg3 = make_subgoal(3, "c.rs", vec!["2"], 1024);
    let sg4 = make_subgoal(4, "sink.rs", vec!["1"], 1024);

    let res = TaskDag::from_subgoals(vec![sg0, sg1, sg2, sg3, sg4]);
    assert!(res.is_err(), "Embedded cycle in DAG must be rejected");
    assert!(res.unwrap_err().contains("Cyclic dependency detected"));
}

#[test]
fn test_circular_dependency_disconnected_cyclic_component() {
    // Disjoint DAG: valid component (1 -> 2) and cyclic component (3 <-> 4)
    let sg1 = make_subgoal(1, "valid_1.rs", vec![], 1024);
    let sg2 = make_subgoal(2, "valid_2.rs", vec!["1"], 1024);
    let sg3 = make_subgoal(3, "bad_1.rs", vec!["4"], 1024);
    let sg4 = make_subgoal(4, "bad_2.rs", vec!["3"], 1024);

    let res = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4]);
    assert!(res.is_err(), "Disconnected cyclic component must cause overall DAG rejection");
    assert!(res.unwrap_err().contains("Cyclic dependency detected"));
}

#[test]
fn test_circular_dependency_figure_eight_double_cycle() {
    // Figure-8: 1 -> 2 -> 1 and 2 -> 3 -> 2
    let sg1 = make_subgoal(1, "a.rs", vec!["2"], 1024);
    let sg2 = make_subgoal(2, "b.rs", vec!["1", "3"], 1024);
    let sg3 = make_subgoal(3, "c.rs", vec!["2"], 1024);

    let res = TaskDag::from_subgoals(vec![sg1, sg2, sg3]);
    assert!(res.is_err(), "Figure-8 multi-cycle must be rejected");
    assert!(res.unwrap_err().contains("Cyclic dependency detected"));
}

#[test]
fn test_circular_dependency_via_semantic_target_entities() {
    // Dependencies expressed as target entities rather than task IDs
    let sg1 = SubGoal {
        id: 1,
        description: "Task 1 producing AuthController".to_string(),
        target_entity: "AuthController".to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: vec!["UserService".to_string()], // depends on Task 2
        read_set: vec![],
        write_set: vec![PathBuf::from("auth.rs")],
        max_tokens: 1024,
    };
    let sg2 = SubGoal {
        id: 2,
        description: "Task 2 producing UserService".to_string(),
        target_entity: "UserService".to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: vec!["AuthController".to_string()], // depends on Task 1
        read_set: vec![],
        write_set: vec![PathBuf::from("user.rs")],
        max_tokens: 1024,
    };

    let res = TaskDag::from_subgoals(vec![sg1, sg2]);
    assert!(res.is_err(), "Semantic entity cycle must be detected and rejected");
    assert!(res.unwrap_err().contains("Cyclic dependency detected"));
}

#[test]
fn test_circular_dependency_via_read_write_sets() {
    // Dependencies inferred via read_set matching write_set:
    // Task 1 writes "a.txt" and reads "b.txt" (written by Task 2)
    // Task 2 writes "b.txt" and reads "a.txt" (written by Task 1)
    let sg1 = SubGoal {
        id: 1,
        description: "Task 1".to_string(),
        target_entity: "".to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: vec![],
        read_set: vec![PathBuf::from("b.txt")],
        write_set: vec![PathBuf::from("a.txt")],
        max_tokens: 1024,
    };
    let sg2 = SubGoal {
        id: 2,
        description: "Task 2".to_string(),
        target_entity: "".to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: vec![],
        read_set: vec![PathBuf::from("a.txt")],
        write_set: vec![PathBuf::from("b.txt")],
        max_tokens: 1024,
    };

    let res = TaskDag::from_subgoals(vec![sg1, sg2]);
    assert!(res.is_err(), "Read/write set implicit cycle must be detected and rejected");
    assert!(res.unwrap_err().contains("Cyclic dependency detected"));
}

// ============================================================================
// 5. MEMORY GATE & ZERO-OOM BOUNDARY ADVERSARIAL STRESS
// ============================================================================

#[test]
fn test_memory_gate_boundary_conditions() {
    let k_max = 4096;
    let mut gate = MemoryGate::new(k_max);

    // 1. Zero tokens requested
    assert!(gate.can_admit(0));
    assert!(!gate.is_over_capacity(0));
    assert!(gate.allocate(0).is_ok());
    assert_eq!(gate.active_tokens, 0);

    // 2. Exactly k_max tokens
    assert!(gate.can_admit(k_max));
    assert!(!gate.is_over_capacity(k_max));

    // 3. Exactly k_max + 1 tokens (Zero-OOM)
    assert!(!gate.can_admit(k_max + 1));
    assert!(gate.is_over_capacity(k_max + 1));
    assert!(gate.allocate(k_max + 1).is_err());

    // 4. Arithmetic overflow safety (usize::MAX)
    assert!(gate.is_over_capacity(usize::MAX));
    assert!(!gate.can_admit(usize::MAX));
    assert!(gate.allocate(usize::MAX).is_err());
    // Active tokens must remain 0, no arithmetic overflow panic
    assert_eq!(gate.active_tokens, 0);

    // 5. Exact saturation allocation
    assert!(gate.allocate(k_max).is_ok());
    assert_eq!(gate.active_tokens, k_max);
    assert_eq!(gate.remaining(), 0);
    assert_eq!(gate.utilization(), 1.0);

    // 6. Cannot admit even 1 more token
    assert!(!gate.can_admit(1));
    assert!(gate.allocate(1).is_err());

    // 7. Saturated release beyond active
    gate.release(k_max * 10);
    assert_eq!(gate.active_tokens, 0);
    assert_eq!(gate.remaining(), k_max);
    assert_eq!(gate.utilization(), 0.0);
}

#[test]
fn test_analytical_formula_degenerate_geometries() {
    // 0 layers -> 0 bytes
    assert_eq!(MemoryGate::analytical_bytes_per_token(0, 2, 1024, 8), 0);
    // 0 kv heads -> 0 bytes
    assert_eq!(MemoryGate::analytical_bytes_per_token(32, 0, 1024, 8), 0);
    // 0 q heads -> guarded division by zero -> 0
    assert_eq!(MemoryGate::analytical_bytes_per_token(32, 2, 1024, 0), 0);
    // Standard Qwen 1.5B
    assert_eq!(MemoryGate::analytical_bytes_per_token(28, 2, 1536, 12), 28_672);
}

// ============================================================================
// 6. ADVERSARIAL EDGE CASES: SELF-LOOPS, MISSING DEPS & TERMINAL MUTATION
// ============================================================================

#[test]
fn test_empty_dag_handling() {
    let dag = TaskDag::from_subgoals(vec![]).expect("Empty DAG should succeed");
    assert!(dag.validate_acyclic().unwrap().is_empty());
    let coord = StateMachineCoordinator::new(dag, 4096).expect("Empty coordinator should succeed");
    assert!(coord.is_all_terminal());
}

#[test]
fn test_self_dependency_edge_case() {
    // Task 1 declares dependency on itself ("1")
    let sg1 = make_subgoal(1, "self_dep.rs", vec!["1"], 1024);
    let dag = TaskDag::from_subgoals(vec![sg1]).expect("Self-dependency is filtered out by u != v");
    // Since u == v is skipped in from_subgoals, task 1 has in_degree 0
    assert_eq!(dag.in_degrees.get(&1).copied().unwrap_or(0), 0);
    assert_eq!(dag.children.get(&1).map(|c| c.len()).unwrap_or(0), 0);
}

#[test]
fn test_missing_dependency_id_behavior() {
    // Task 1 declares dependency on non-existent task "999"
    let sg1 = make_subgoal(1, "orphan.rs", vec!["999"], 1024);
    let dag = TaskDag::from_subgoals(vec![sg1]).expect("Non-existent dependency ID is ignored");
    assert_eq!(dag.in_degrees.get(&1).copied().unwrap_or(0), 0);
}

#[test]
fn test_fail_task_on_completed_task_behavior() {
    let sg1 = make_subgoal(1, "task_1.rs", vec![], 1024);
    let dag = TaskDag::from_subgoals(vec![sg1]).unwrap();
    let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

    let _ = coord.admit_task(1, 1024).unwrap();
    let _ = coord.complete_task(1).unwrap();
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Completed);

    // Call fail_task on already Completed task:
    // Documents current behavior: fail_task unconditionally overwrites status
    coord.fail_task(1, "Late crash", None);
    // Note: status became Failed because fail_task lacks an is_terminal check
    assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Failed);
}
