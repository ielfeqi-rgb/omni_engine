use omni_engine::sandbox::cleanroom::UniversalCleanroom;
use omni_engine::planner::swarm_coordinator::{
    TaskDag, SubGoal, TaskKind, StateMachineCoordinator, WorkerEvent, WorkerFinding, MemoryGate,
};
use std::path::PathBuf;

#[test]
fn test_cleanroom_acceptance_criteria_auditor() {
    println!("=== Testing Mode 4: Acceptance Criteria Auditing ===");

    let user_goal = "Build a complete interactive Todo CLI in Python: add and delete tasks, persist in tasks.json, error handling for corrupted files, interactive loop";

    // 1. Partial implementation: in-memory list only, no JSON persistence, no interactive loop, no error handling
    let partial_code = r#"
class TodoList:
    def __init__(self):
        self.tasks = []
    def add_task(self, task):
        self.tasks.append(task)
    def delete_task(self, idx):
        if 0 <= idx < len(self.tasks):
            return self.tasks.pop(idx)
        return None
"#;

    let delta1 = UniversalCleanroom::audit_acceptance_criteria(
        "todo.py",
        partial_code,
        user_goal,
    );

    println!("Delta 1 (Partial): {:?}", delta1);
    assert!(!delta1.is_accepted, "Partial implementation must not be prematurely accepted");
    assert!(delta1.missing_requirements.contains(&"File/JSON persistence".to_string()), "Must detect missing JSON persistence");
    assert!(delta1.missing_requirements.contains(&"Interactive CLI loop / user input".to_string()), "Must detect missing interactive CLI");
    assert!(delta1.missing_requirements.contains(&"Error handling and corruption recovery".to_string()), "Must detect missing error handling");
    assert!(delta1.covered_requirements.contains(&"Deletion / Removal capability".to_string()), "Must detect covered deletion requirement");

    // 2. Full extended implementation: keeps TodoList and adds JSON persistence, error handling, and interactive CLI
    let complete_code = r#"
import json
import os

class TodoList:
    def __init__(self, filename="tasks.json"):
        self.filename = filename
        self.tasks = []
        self.load()

    def add_task(self, task):
        self.tasks.append(task)
        self.save()

    def delete_task(self, idx):
        if 0 <= idx < len(self.tasks):
            t = self.tasks.pop(idx)
            self.save()
            return t
        return None

    def save(self):
        with open(self.filename, 'w') as f:
            json.dump(self.tasks, f)

    def load(self):
        if not os.path.exists(self.filename):
            return
        try:
            with open(self.filename, 'r') as f:
                self.tasks = json.load(f)
        except Exception as e:
            print(f"Error loading {self.filename}: {e}")
            self.tasks = []

def main():
    todo = TodoList()
    while True:
        try:
            cmd = input("todo> ").strip()
            if cmd == "quit":
                break
        except (EOFError, KeyboardInterrupt):
            break

if __name__ == "__main__":
    main()
"#;

    let delta2 = UniversalCleanroom::audit_acceptance_criteria(
        "todo.py",
        complete_code,
        user_goal,
    );

    println!("Delta 2 (Complete): {:?}", delta2);
    assert!(delta2.is_accepted, "Complete implementation must pass acceptance criteria audit");
    assert!(delta2.missing_requirements.is_empty(), "All requirements must be satisfied");
    assert!(delta2.covered_requirements.contains(&"File/JSON persistence".to_string()));
    assert!(delta2.covered_requirements.contains(&"Interactive CLI loop / user input".to_string()));
    assert!(delta2.covered_requirements.contains(&"Error handling and corruption recovery".to_string()));
    assert!(delta2.covered_requirements.contains(&"Deletion / Removal capability".to_string()));
}

#[test]
fn test_dynamic_dag_subgoal_injection_on_acceptance_delta() {
    println!("=== Testing Dynamic Subgoal Injection & Zero-Wipe Invariant ===");

    // Initial 2-task DAG: Task 1 (Create todo.py) -> Task 2 (Test todo.py)
    let sg1 = SubGoal {
        id: 1,
        description: "Create todo.py with basic task structure".to_string(),
        target_entity: "todo.py".to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: vec![],
        read_set: vec![],
        write_set: vec![PathBuf::from("todo.py")],
        max_tokens: 2048,
    };

    let sg2 = SubGoal {
        id: 2,
        description: "Review and test todo.py in isolated Cleanroom".to_string(),
        target_entity: "todo.py".to_string(),
        kind: TaskKind::Test,
        guidance: None,
        depends_on: vec!["todo.py".to_string()],
        read_set: vec![PathBuf::from("todo.py")],
        write_set: vec![], // Strictly Read-Only (W = ∅)
        max_tokens: 2048,
    };

    let dag = TaskDag::from_subgoals(vec![sg1, sg2]).expect("Valid initial DAG");
    let mut coordinator = StateMachineCoordinator::with_memory_gate(dag, MemoryGate::new(8192))
        .expect("Coordinator initialization");

    // Initially, Task 1 is Ready, Task 2 is Pending
    assert_eq!(coordinator.ready_set, vec![1]);

    // 1. Worker 1 runs and completes Task 1
    let flag1 = coordinator.admit_task(1, 2048).expect("Admit Task 1");
    assert!(!flag1.load(std::sync::atomic::Ordering::Relaxed));
    let unblocked = coordinator.complete_task(1).expect("Complete Task 1");
    assert_eq!(unblocked, vec![2], "Task 2 should now become Ready");
    assert_eq!(coordinator.ready_set, vec![2]);

    // 2. Tester (Task 2) runs and detects Acceptance Delta
    let _flag2 = coordinator.admit_task(2, 2048).expect("Admit Task 2 (Tester)");
    
    // Simulate Acceptance Delta emitted by Tester
    let delta_desc = "Missing: File/JSON persistence, Interactive CLI loop";
    let event = WorkerEvent::AcceptanceDelta {
        task_id: 2,
        target_file: "todo.py".to_string(),
        delta: delta_desc.to_string(),
        finding: WorkerFinding {
            worker_id: 2,
            sub_goal: "Review and test todo.py".to_string(),
            finding: format!("[ACCEPTANCE DELTA] In 'todo.py': {}", delta_desc),
            steps_taken: 1,
            rollbacks_count: 0,
            tokens_saved_by_rollback: 0,
        },
    };

    // 3. Coordinator processes AcceptanceDelta: completes Task 2 and dynamically injects Modify Task 3 & Re-test Task 4
    if let WorkerEvent::AcceptanceDelta { task_id, target_file, delta, .. } = event {
        let _ = coordinator.complete_task(task_id);

        let target_path = PathBuf::from(&target_file);
        let modify_id = coordinator.inject_subgoal(
            SubGoal {
                id: 0,
                description: format!("Extend '{}' to implement: {}", target_file, delta),
                target_entity: target_file.clone(),
                kind: TaskKind::Modify,
                guidance: Some(format!("Keep all existing working code. Add {}", delta)),
                depends_on: vec![target_file.clone()],
                read_set: vec![target_path.clone()],
                write_set: vec![target_path.clone()],
                max_tokens: 2048,
            },
            vec![task_id],
        ).expect("Inject modify subgoal should succeed");
        assert_eq!(modify_id, 3, "New Modify task should have ID 3");

        let retest_id = coordinator.inject_subgoal(
            SubGoal {
                id: 0,
                description: format!("Re-audit '{}' in Cleanroom", target_file),
                target_entity: target_file.clone(),
                kind: TaskKind::Test,
                guidance: Some("Verify 100% acceptance criteria satisfied".to_string()),
                depends_on: vec![target_file.clone()],
                read_set: vec![target_path.clone()],
                write_set: vec![],
                max_tokens: 2048,
            },
            vec![modify_id],
        ).expect("Inject retest subgoal should succeed");
        assert_eq!(retest_id, 4, "New Test task should have ID 4");

        // Modify task has all dependencies completed (Task 2 is completed) -> Must be immediately Ready!
        assert_eq!(coordinator.ready_set, vec![3], "Modify Task 3 must be Ready immediately");

        // 4. Worker 3 runs Modify task and completes
        let _flag3 = coordinator.admit_task(3, 2048).expect("Admit Modify Task 3");
        let unblocked3 = coordinator.complete_task(3).expect("Complete Modify Task 3");
        assert_eq!(unblocked3, vec![4], "Re-audit Task 4 should become Ready after Task 3");
        assert_eq!(coordinator.ready_set, vec![4]);

        // 5. Final Re-Audit passes
        let _flag4 = coordinator.admit_task(4, 2048).expect("Admit Re-audit Task 4");
        let unblocked4 = coordinator.complete_task(4).expect("Complete Re-audit Task 4");
        assert!(unblocked4.is_empty());
        assert!(coordinator.is_all_terminal(), "All tasks including dynamic extensions must reach terminal status");
    }
}
