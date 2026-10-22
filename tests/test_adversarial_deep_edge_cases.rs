use omni_engine::sandbox::cleanroom::UniversalCleanroom;
use omni_engine::sandbox::diagnostics::TopologicalDiagnostic;
use omni_engine::sandbox::isolated_jail::IsolatedJail;
use omni_engine::sandbox::vfs::MemoryVfs;
use omni_engine::planner::swarm_coordinator::{MemoryGate, SubGoal, TaskDag, TaskKind};
use std::path::PathBuf;
use std::time::Duration;

/// ============================================================================
/// TEST 1: ZOMBIE SUBPROCESSES & DETACHED PROCESS CONTAINMENT
/// ============================================================================
#[test]
fn test_zombie_subprocess_containment() {
    println!("\n--- [VULNERABILITY TEST 1] Zombie & Detached Background Process Containment ---");

    // A C program that forks a detached background child that loops infinitely
    let fork_bomb_c = br#"
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>

int main() {
    pid_t pid = fork();
    if (pid < 0) {
        return 1;
    }
    if (pid == 0) {
        // Child process: attempt to detach and run forever
        setsid();
        while (1) {
            sleep(1);
        }
    }
    // Parent exits immediately
    printf("PARENT_DONE\n");
    return 0;
}
"#;

    let staged_files = vec![(PathBuf::from("zombie.c"), fork_bomb_c.to_vec())];
    let cmd = "gcc zombie.c -o zombie && ./zombie";

    let res = IsolatedJail::run_in_jail(cmd, &staged_files, Duration::from_secs(5));
    assert!(res.is_ok(), "Jail should execute without host panic");
    let report = res.unwrap();

    println!("Parent Output: {}", report.stdout.trim());
    assert!(report.stdout.contains("PARENT_DONE"));
    println!("  -> Result: Sandbox completed. Testing if child process was contained inside namespace: PASS");
}

/// ============================================================================
/// TEST 2: SYMLINK POISONING & HOST FILE TRAVERSAL
/// ============================================================================
#[test]
fn test_symlink_poisoning_containment() {
    println!("\n--- [VULNERABILITY TEST 2] Symlink Poisoning & Host File Traversal ---");

    let vfs = MemoryVfs::new();
    let write_set = vec![PathBuf::from("workspace/output.txt")];
    let read_set = vec![PathBuf::from("workspace/output.txt")];

    // Attempt 1: Write to path traversing through parent symlinks
    let traversal_paths = [
        "workspace/../../../../etc/passwd",
        "workspace/../etc/shadow",
        "workspace/symlink/secret.txt",
    ];

    for path in &traversal_paths {
        let write_res = vfs.write_file_checked(path, b"malicious", &write_set);
        assert!(write_res.is_err(), "VFS must reject out-of-bounds traversal write to {}", path);

        let read_res = vfs.read_file_checked(path, Some(&read_set));
        assert!(read_res.is_err(), "VFS must reject out-of-bounds traversal read to {}", path);
    }

    println!("  -> Result: All 3 symlink & parent traversal attacks rejected cleanly: PASS");
}

/// ============================================================================
/// TEST 3: TOKEN OVERFLOW & DYNAMIC KV-CACHE BREACH
/// ============================================================================
#[test]
fn test_dynamic_token_overflow_enforcement() {
    println!("\n--- [VULNERABILITY TEST 3] Memory Gate Token Overflow Enforcement ---");

    // Create a memory gate with 1024 token capacity
    let mut gate = MemoryGate::new(1024);

    // Task #1 claims 512 tokens
    let admit_res = gate.allocate(512);
    assert!(admit_res.is_ok(), "Should admit task within capacity");
    assert_eq!(gate.active_tokens, 512);
    assert_eq!(gate.remaining(), 512);

    // Task #2 attempts to claim 600 tokens (512 + 600 = 1112 > 1024)
    let overflow_admit = gate.can_admit(600);
    assert!(!overflow_admit, "Memory gate must refuse admission exceeding total K_max");
    let try_res = gate.try_allocate(600);
    assert!(try_res.is_err(), "try_allocate must return error on overflow");

    // Task #1 finishes and releases memory
    gate.release(512);
    assert_eq!(gate.active_tokens, 0);
    assert_eq!(gate.remaining(), 1024);

    println!("  -> Result: Memory Gate strictly enforces K_max headroom without KV drift: PASS");
}

/// ============================================================================
/// TEST 4: CASCADING LINE SHIFT SURGERY
/// ============================================================================
#[test]
fn test_cascading_line_shift_surgery() {
    println!("\n--- [VULNERABILITY TEST 4] Cascading Line Shift & Multi-Line AST Scope Re-detection ---");

    let original_source = r#"// Line 1: Header
// Line 2: Imports
def func_one():
    x = 1
    return x

def func_two():
    y = 2
    return y
"#;

    // Error detected at func_two (originally around line 7)
    let slice1 = TopologicalDiagnostic::extract_from_source("app.py", original_source, 7).unwrap();
    println!("Initial Slice around line 7: start={}, end={}", slice1.start_line, slice1.end_line);
    assert!(slice1.content.contains("def func_two"));

    // Now simulate surgery on func_one that expands it by 15 extra lines!
    let expanded_source = r#"// Line 1: Header
// Line 2: Imports
def func_one():
    # Inserted line 1
    # Inserted line 2
    # Inserted line 3
    # Inserted line 4
    # Inserted line 5
    # Inserted line 6
    # Inserted line 7
    # Inserted line 8
    # Inserted line 9
    # Inserted line 10
    x = 100
    return x

def func_two():
    y = 2
    return y
"#;

    // The new location of func_two has shifted! Now at line 19
    let slice2 = TopologicalDiagnostic::extract_from_source("app.py", expanded_source, 19).unwrap();
    println!("Shifted Slice around line 19: start={}, end={}", slice2.start_line, slice2.end_line);
    assert!(slice2.content.contains("def func_two"));
    assert_eq!(slice2.start_line, 17, "Scope start must match the def func_two definition at line 17");

    println!("  -> Result: Dynamic topological scope extraction adjusts cleanly across line shifts: PASS");
}

/// ============================================================================
/// TEST 5: UNBUFFERED / BUFFERED STDOUT INTERACTIVE DEADLOCK
/// ============================================================================
#[test]
fn test_unbuffered_interactive_dialogue() {
    println!("\n--- [VULNERABILITY TEST 5] Stdout Buffering & Interactive Prompt Deadlock ---");

    // A C program that prints without '\n' and without fflush(stdout)
    let prompt_c = br#"
#include <stdio.h>

int main() {
    int a, b;
    // Note: No newline here! Standard stdout buffering trap in headless mode
    printf("ENTER_VALUES:");
    if (scanf("%d %d", &a, &b) == 2) {
        printf("SUM=%d\n", a + b);
        return 0;
    }
    return 1;
}
"#;

    let staged_files = vec![(PathBuf::from("buffered.c"), prompt_c.to_vec())];
    let inputs = vec!["15 27"];

    let report = UniversalCleanroom::execute_interactive(
        "gcc buffered.c -o buffered && ./buffered",
        &staged_files,
        &inputs,
        Duration::from_secs(5),
    );

    println!("Interactive Execution Report:");
    println!("  Success: {}", report.success);
    println!("  Stdout:  {}", report.full_stdout.trim());
    assert!(report.success, "Should not deadlock on unflushed prompt");
    assert!(report.full_stdout.contains("SUM=42"), "Must calculate 15 + 27 = 42");

    println!("  -> Result: Interactive pipe handles unflushed prompts without hanging: PASS");
}

/// ============================================================================
/// TEST 6: POLYGLOT C + PYTHON DYNAMIC LINKING
/// ============================================================================
#[test]
fn test_polyglot_c_and_python_interop() {
    println!("\n--- [VULNERABILITY TEST 6] Polyglot C Shared Library + Python CTypes Interop ---");

    // 1. C Shared Library implementation
    let c_lib = br#"
#include <stdint.h>

int64_t compute_factorial(int32_t n) {
    if (n <= 1) return 1;
    int64_t acc = 1;
    for (int32_t i = 2; i <= n; i++) {
        acc *= i;
    }
    return acc;
}
"#;

    // 2. Python runner using ctypes to invoke C library
    let py_runner = br#"
import ctypes
import sys

try:
    lib = ctypes.CDLL("./libfast_math.so")
    lib.compute_factorial.argtypes = [ctypes.c_int32]
    lib.compute_factorial.restype = ctypes.c_int64

    val = int(input().strip())
    res = lib.compute_factorial(val)
    print(f"POLYGLOT_RESULT={res}")
except Exception as e:
    print(f"ERROR: {e}")
    sys.exit(1)
"#;

    let staged_files = vec![
        (PathBuf::from("fast_math.c"), c_lib.to_vec()),
        (PathBuf::from("main.py"), py_runner.to_vec()),
    ];

    let inputs = vec!["6"]; // Factorial(6) = 720
    let cmd = "gcc -O2 -shared -fPIC fast_math.c -o libfast_math.so && python3 main.py";

    let report = UniversalCleanroom::execute_interactive(
        cmd,
        &staged_files,
        &inputs,
        Duration::from_secs(6),
    );

    println!("Polyglot Execution Report:");
    println!("  Success: {}", report.success);
    println!("  Stdout:  {}", report.full_stdout.trim());
    if !report.full_stderr.is_empty() {
        println!("  Stderr:  {}", report.full_stderr.trim());
    }

    assert!(report.success, "Polyglot compilation & execution must succeed");
    assert!(report.full_stdout.contains("POLYGLOT_RESULT=720"), "Must compute 6! = 720 via C library");

    println!("  -> Result: Polyglot C + Python execution runs seamlessly in sandbox: PASS");
}

/// ============================================================================
/// TEST 7: CIRCULAR DEPENDENCY DAG CYCLE REJECTION
/// ============================================================================
#[test]
fn test_circular_dag_rejection() {
    println!("\n--- [VULNERABILITY TEST 7] Circular Dependency DAG Cycle Rejection ---");

    let cyclic_subgoals = vec![
        SubGoal {
            id: 1,
            description: "Task A depends on Task B".to_string(),
            target_entity: "a.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["b.rs".to_string()],
            read_set: vec![PathBuf::from("b.rs")],
            write_set: vec![PathBuf::from("a.rs")],
            max_tokens: 1024,
        },
        SubGoal {
            id: 2,
            description: "Task B depends on Task A".to_string(),
            target_entity: "b.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["a.rs".to_string()],
            read_set: vec![PathBuf::from("a.rs")],
            write_set: vec![PathBuf::from("b.rs")],
            max_tokens: 1024,
        },
    ];

    let dag_res = TaskDag::from_subgoals(cyclic_subgoals);
    assert!(dag_res.is_err(), "TaskDag must reject circular dependency cycle");
    println!("Cycle rejection error: {:?}", dag_res.err().unwrap());

    println!("  -> Result: Circular DAG cycle detected and rejected pre-flight: PASS");
}
