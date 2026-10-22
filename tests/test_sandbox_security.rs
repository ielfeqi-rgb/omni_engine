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

use omni_engine::planner::swarm_coordinator::{SubGoal, TaskKind};
use omni_engine::sandbox::{IsolatedJail, MemoryVfs, VfsBoundaryError};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_isolated_jail_timeout_runaway_loop() {
    let staged_files = vec![
        (PathBuf::from("loop.py"), b"while True: pass\n".to_vec()),
    ];
    let start = std::time::Instant::now();
    let res = IsolatedJail::run_in_jail(
        "python3 loop.py",
        &staged_files,
        Duration::from_millis(1500),
    );
    let elapsed = start.elapsed();

    assert!(res.is_ok(), "Jail should cleanly handle timeout error");
    let exec = res.unwrap();
    assert!(!exec.success, "Infinite loop must fail / time out");
    assert!(
        exec.stderr.contains("TIMEOUT") || exec.stderr.contains("exceeded") || !exec.success,
        "Stderr should indicate failure or timeout"
    );
    assert!(elapsed < Duration::from_secs(5), "Timeout must be enforced promptly");
}

#[test]
fn test_isolated_jail_staging_scoped_to_read_set() {
    let vfs = Arc::new(MemoryVfs::new());
    vfs.write_file("visible.py", b"print('visible')");
    vfs.write_file("hidden_secret.txt", b"CONFIDENTIAL_DATA");

    let read_set = vec![PathBuf::from("visible.py")];
    let vfs_files = vfs.all_files();
    let staged_files: Vec<(PathBuf, Vec<u8>)> = vfs_files
        .into_iter()
        .filter(|(path, _)| MemoryVfs::is_path_in_set(path, &read_set))
        .collect();

    assert_eq!(staged_files.len(), 1);
    assert_eq!(staged_files[0].0, PathBuf::from("visible.py"));

    let verify_cmd = "python3 -c \"import os; assert not os.path.exists('hidden_secret.txt'), 'Secret leaked into jail!'\"";
    let res = IsolatedJail::run_in_jail(verify_cmd, &staged_files, Duration::from_secs(10));
    assert!(res.is_ok());
    assert!(res.unwrap().success);
}

#[test]
fn test_adversarial_isolated_jail_staging_read_write_sets() {
    let vfs = Arc::new(MemoryVfs::new());
    // Authorized files
    vfs.write_file("data/input.csv", b"id,val\n1,100\n");
    vfs.write_file("config.json", b"{\"mode\": \"test\"}\n");
    vfs.write_file("output.py", b"print('processing')\n");
    // Unauthorized files (should NEVER be staged)
    vfs.write_file("secrets/master_key.pem", b"SUPER_SECRET_KEY");
    vfs.write_file(".env", b"DB_PASS=admin123");
    vfs.write_file("other_worker_target.py", b"print('unrelated')");

    let subgoal = SubGoal {
        id: 42,
        description: "Process data and produce script".to_string(),
        target_entity: "output.py".to_string(),
        kind: TaskKind::Modify,
        guidance: None,
        depends_on: vec!["data/input.csv".to_string()],
        read_set: vec![PathBuf::from("data/input.csv"), PathBuf::from("config.json")],
        write_set: vec![PathBuf::from("output.py")],
        max_tokens: 4096,
    };

    // Staging filter exactly as implemented in SwarmCoordinator
    let vfs_files = vfs.all_files();
    let staged_files: Vec<(PathBuf, Vec<u8>)> = vfs_files
        .into_iter()
        .filter(|(path, _)| {
            MemoryVfs::is_path_in_set(path, &subgoal.read_set)
                || MemoryVfs::is_path_in_set(path, &subgoal.write_set)
        })
        .collect();

    // Verify exactly 3 files staged
    assert_eq!(staged_files.len(), 3, "Only files in read_set or write_set must be staged");
    let staged_paths: Vec<&PathBuf> = staged_files.iter().map(|(p, _)| p).collect();
    assert!(staged_paths.iter().any(|p| **p == PathBuf::from("data/input.csv")));
    assert!(staged_paths.iter().any(|p| **p == PathBuf::from("config.json")));
    assert!(staged_paths.iter().any(|p| **p == PathBuf::from("output.py")));

    // Verify forbidden files are ABSENT from staging
    assert!(!staged_paths.iter().any(|p| **p == PathBuf::from("secrets/master_key.pem")));
    assert!(!staged_paths.iter().any(|p| **p == PathBuf::from(".env")));
    assert!(!staged_paths.iter().any(|p| **p == PathBuf::from("other_worker_target.py")));

    // Run verification script inside the Isolated Jail
    let test_script = r#"python3 -c "
import os

# Authorized files must exist
assert os.path.exists('data/input.csv'), 'Missing staged data/input.csv'
assert os.path.exists('config.json'), 'Missing staged config.json'
assert os.path.exists('output.py'), 'Missing staged output.py'

# Forbidden files must NOT exist
assert not os.path.exists('secrets/master_key.pem'), 'Secret master_key.pem leaked into jail!'
assert not os.path.exists('secrets'), 'Secrets directory leaked into jail!'
assert not os.path.exists('.env'), '.env leaked into jail!'
assert not os.path.exists('other_worker_target.py'), 'other_worker_target.py leaked into jail!'

print('ISOLATED_JAIL_STAGING_VERIFIED')
""#;

    let res = IsolatedJail::run_in_jail(test_script, &staged_files, Duration::from_secs(10));
    assert!(res.is_ok(), "Jail execution failed to invoke: {:?}", res.err());
    let exec = res.unwrap();
    assert!(exec.success, "Jail assertions failed: stdout={}, stderr={}", exec.stdout, exec.stderr);
    assert!(exec.stdout.contains("ISOLATED_JAIL_STAGING_VERIFIED"));
}

#[test]
fn test_adversarial_isolated_jail_host_confinement() {
    // Verify that IsolatedJail confines execution and prevents writing to system directories
    let staged_files = vec![
        (PathBuf::from("test.sh"), b"echo 'inside jail'".to_vec()),
    ];

    // 1. Verification that system directories are strictly read-only inside bwrap
    if IsolatedJail::is_bwrap_available() {
        let res_sys_write = IsolatedJail::run_in_jail(
            "touch /usr/malicious_binary && echo 'PWNED'",
            &staged_files,
            Duration::from_secs(5),
        ).expect("Execution should execute");
        assert!(!res_sys_write.success, "Writing to /usr MUST be denied in jail");
        assert!(!res_sys_write.stdout.contains("PWNED"));

        // 2. Verification that /etc cannot be modified
        let res_etc_write = IsolatedJail::run_in_jail(
            "touch /etc/hacked && echo 'PWNED'",
            &staged_files,
            Duration::from_secs(5),
        ).expect("Execution should execute");
        assert!(!res_etc_write.success, "Writing to /etc MUST be denied in jail");
        assert!(!res_etc_write.stdout.contains("PWNED"));
    }
}

#[test]
fn test_adversarial_subgoal_boundaries_and_traversal() {
    let sg = SubGoal {
        id: 7,
        description: "Adversarial boundary check".to_string(),
        target_entity: "src/worker.py".to_string(),
        kind: TaskKind::Modify,
        guidance: None,
        depends_on: vec!["lib/util.py".to_string()],
        read_set: vec![PathBuf::from("lib/util.py"), PathBuf::from("src/worker.py")],
        write_set: vec![PathBuf::from("src/worker.py")],
        max_tokens: 2048,
    };

    // can_write: exact and normalized
    assert!(sg.can_write("src/worker.py"));
    assert!(sg.can_write("./src/worker.py"));
    assert!(sg.can_write("src/../src/worker.py"));

    // can_write: forbidden
    assert!(!sg.can_write("lib/util.py"), "Files only in read_set cannot be written");
    assert!(!sg.can_write("src/worker.py.bak"), "Prefix match must not match different file");
    assert!(!sg.can_write("src/worker_extra.py"));
    assert!(!sg.can_write("../src/worker.py"));
    assert!(!sg.can_write("evil.sh"));

    // can_read: exact and normalized
    assert!(sg.can_read("lib/util.py"));
    assert!(sg.can_read("./lib/util.py"));
    assert!(sg.can_read("src/worker.py"));

    // can_read: forbidden
    assert!(!sg.can_read("secret.key"));
    assert!(!sg.can_read("lib/util.py.old"));
    assert!(!sg.can_read("../lib/util.py"));
}

#[test]
fn test_adversarial_vfs_checked_apis() {
    let vfs = MemoryVfs::new();
    let write_set = vec![PathBuf::from("src/allowed.rs")];
    let read_set = vec![PathBuf::from("config/safe.json")];

    // 1. Authorized write succeeds
    assert!(vfs.write_file_checked("src/allowed.rs", b"fn main() {}", &write_set).is_ok());

    // 2. Traversal write fails
    let res_traversal = vfs.write_file_checked("src/../evil.rs", b"fn evil() {}", &write_set);
    assert!(matches!(res_traversal, Err(VfsBoundaryError::UnauthorizedWrite { .. })));

    // 3. Unauthorized patch fails
    let res_patch = vfs.patch_file_checked("evil.rs", "fn evil() {}", &write_set);
    assert!(matches!(res_patch, Err(VfsBoundaryError::UnauthorizedPatch { .. })));

    // 4. Unauthorized delete fails
    let res_del = vfs.delete_file_checked("evil.rs", &write_set);
    assert!(matches!(res_del, Err(VfsBoundaryError::UnauthorizedDelete { .. })));

    // 5. Unauthorized read fails
    let res_read = vfs.read_file_checked("secret.key", Some(&read_set));
    assert!(matches!(res_read, Err(VfsBoundaryError::UnauthorizedRead { .. })));
}
