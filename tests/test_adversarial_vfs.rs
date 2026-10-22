use omni_engine::sandbox::{MemoryVfs, VfsBoundaryError};
use omni_engine::planner::swarm_coordinator::{DiagnosticCode, SubGoal, SwarmAction, TaskKind};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_adversarial_normalize_path_basic() {
    // Current directory prefixes
    assert_eq!(MemoryVfs::normalize_path("./target"), PathBuf::from("target"));
    assert_eq!(MemoryVfs::normalize_path("././target"), PathBuf::from("target"));
    assert_eq!(MemoryVfs::normalize_path("a/./b/./c"), PathBuf::from("a/b/c"));

    // Traversal inside path
    assert_eq!(MemoryVfs::normalize_path("a/../target"), PathBuf::from("target"));
    assert_eq!(MemoryVfs::normalize_path("a/b/../../target"), PathBuf::from("target"));
    assert_eq!(MemoryVfs::normalize_path("a/b/c/../../../target"), PathBuf::from("target"));

    // Leading slashes
    assert_eq!(MemoryVfs::normalize_path("/target"), PathBuf::from("/target"));
    assert_eq!(MemoryVfs::normalize_path("/a/../target"), PathBuf::from("/target"));
    assert_eq!(MemoryVfs::normalize_path("/a/b/../../target"), PathBuf::from("/target"));
}

#[test]
fn test_adversarial_normalization_parent_dir_preservation() {
    // Verification of fixed behavior in `normalize_path`:
    // Component::ParentDir is preserved and not inadvertently popped.
    let p_two_dots = MemoryVfs::normalize_path("../../target");
    let p_three_dots = MemoryVfs::normalize_path("../../../target");
    let p_four_dots = MemoryVfs::normalize_path("../../../../target");

    println!("NORMALIZATION BEHAVIOR ON RELATIVE ASCENT:");
    println!("  '../../target' -> {:?}", p_two_dots);
    println!("  '../../../target' -> {:?}", p_three_dots);
    println!("  '../../../../target' -> {:?}", p_four_dots);

    // Parent directory components (..) are preserved on relative ascent:
    assert_eq!(p_two_dots, PathBuf::from("../../target"));
    assert_eq!(p_three_dots, PathBuf::from("../../../target"));
    assert_eq!(p_four_dots, PathBuf::from("../../../../target"));

    // Root breakout behavior:
    let pr_one = MemoryVfs::normalize_path("/../target");
    let pr_two = MemoryVfs::normalize_path("/../../target");
    println!("NORMALIZATION BEHAVIOR ON ROOT ASCENT:");
    println!("  '/../target' -> {:?}", pr_one);
    println!("  '/../../target' -> {:?}", pr_two);
    assert_eq!(pr_one, PathBuf::from("/../target"));
    assert_eq!(pr_two, PathBuf::from("/../../target"));
}

#[test]
fn test_adversarial_is_path_in_set_root_and_dot() {
    // Check behavior when allowed_set has "/"
    let allowed_root = vec![PathBuf::from("/")];
    let in_root = MemoryVfs::is_path_in_set("secret.txt", &allowed_root);
    println!("is_path_in_set('secret.txt', &['/']) = {}", in_root);
    // VULNERABILITY REPRODUCTION:
    // When allowed_set is "/", stripped_allowed is "", and Path::new("secret.txt").starts_with("") is true in Rust.
    // This empirically proves that allowed_set containing "/" bypasses all path restrictions.
    assert!(in_root, "CONFIRMED VULNERABILITY: is_path_in_set permits arbitrary files when allowed_set contains '/'");

    // Check behavior when allowed_set has "."
    let allowed_dot = vec![PathBuf::from(".")];
    let in_dot = MemoryVfs::is_path_in_set("secret.txt", &allowed_dot);
    println!("is_path_in_set('secret.txt', &['.']) = {}", in_dot);
    assert!(!in_dot, "Allowed set ['.'] does not match 'secret.txt'");
}

#[test]
fn test_adversarial_is_path_in_set() {
    let allowed = vec![PathBuf::from("src/main.rs"), PathBuf::from("target")];

    // Allowed exact and normalized
    assert!(MemoryVfs::is_path_in_set("src/main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("./src/main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("src/./main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("src/utils/../main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("a/../src/main.rs", &allowed));

    // Leading slash variations
    assert!(MemoryVfs::is_path_in_set("/src/main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("/target", &allowed));

    // Boundary escapes MUST NOT be in set
    assert!(!MemoryVfs::is_path_in_set("src/other.rs", &allowed));
    assert!(!MemoryVfs::is_path_in_set("src/main.rs.bak", &allowed));
    assert!(!MemoryVfs::is_path_in_set("src/main_rs", &allowed));
    assert!(!MemoryVfs::is_path_in_set("main.rs", &allowed));
    assert!(!MemoryVfs::is_path_in_set("evil/../src/other.rs", &allowed));
    assert!(!MemoryVfs::is_path_in_set("/etc/passwd", &allowed));
    assert!(!MemoryVfs::is_path_in_set("../src/main.rs", &allowed));

    // Disjoint files with prefix similarity
    assert!(!MemoryVfs::is_path_in_set("target_extra", &allowed));
    assert!(!MemoryVfs::is_path_in_set("target_backup", &allowed));
    assert!(!MemoryVfs::is_path_in_set("target2", &allowed));

    // Empty allowed set
    assert!(!MemoryVfs::is_path_in_set("src/main.rs", &[]));
    assert!(!MemoryVfs::is_path_in_set("target", &[]));
}

#[test]
fn test_adversarial_vfs_checked_operations() {
    let vfs = MemoryVfs::new();
    let write_set = vec![PathBuf::from("output/report.txt")];

    // 1. Authorized write succeeds
    assert!(vfs.write_file_checked("output/report.txt", b"ok", &write_set).is_ok());

    // 2. Traversal attempt to escape output directory
    let res = vfs.write_file_checked("output/../evil.sh", b"bad", &write_set);
    assert!(matches!(res, Err(VfsBoundaryError::UnauthorizedWrite { .. })));
    assert!(!vfs.exists("evil.sh"));

    // 3. Disjoint file attempt
    let res2 = vfs.write_file_checked("output_backup/report.txt", b"bad", &write_set);
    assert!(matches!(res2, Err(VfsBoundaryError::UnauthorizedWrite { .. })));

    // 4. Unauthorized patch
    let res3 = vfs.patch_file_checked("evil.sh", "foo", &write_set);
    assert!(matches!(res3, Err(VfsBoundaryError::UnauthorizedPatch { .. })));

    // 5. Unauthorized delete
    let res4 = vfs.delete_file_checked("output/../report.txt", &write_set);
    assert!(matches!(res4, Err(VfsBoundaryError::UnauthorizedDelete { .. })));

    // 6. Unauthorized read
    let read_set = vec![PathBuf::from("config.json")];
    let res5 = vfs.read_file_checked("private.pem", Some(&read_set));
    assert!(matches!(res5, Err(VfsBoundaryError::UnauthorizedRead { .. })));
}

#[test]
fn test_adversarial_vfs_boundary_violations() {
    let vfs = Arc::new(MemoryVfs::new());
    let allowed_write = vec![PathBuf::from("workspace/code.py")];
    let allowed_read = vec![PathBuf::from("workspace/input.txt")];

    vfs.write_file("workspace/input.txt", b"some input");
    vfs.write_file("secret.env", b"API_KEY=12345");

    // 1. Legitimate write with traversal within bounds
    let res = vfs.write_file_checked("workspace/sub/../code.py", b"print('safe')", &allowed_write);
    assert!(res.is_ok(), "Normalized path within write_set must succeed: {:?}", res);

    // 2. Escape via ../ to secret.env
    let res_evil = vfs.write_file_checked("workspace/../secret.env", b"HACKED", &allowed_write);
    assert!(matches!(res_evil, Err(VfsBoundaryError::UnauthorizedWrite { .. })));
    assert_ne!(vfs.read_string("secret.env").unwrap(), "HACKED");

    // 3. Unauthorized read attempt
    let res_read = vfs.read_file_checked("secret.env", Some(&allowed_read));
    assert!(matches!(res_read, Err(VfsBoundaryError::UnauthorizedRead { .. })));

    // 4. Unauthorized delete attempt
    let res_del = vfs.delete_file_checked("secret.env", &allowed_write);
    assert!(matches!(res_del, Err(VfsBoundaryError::UnauthorizedDelete { .. })));
    assert!(vfs.exists("secret.env"));

    // 5. Unauthorized patch attempt
    let res_patch = vfs.patch_file_checked("secret.env", "TARGET: API_KEY | REPLACEMENT: LEAK", &allowed_write);
    assert!(matches!(res_patch, Err(VfsBoundaryError::UnauthorizedPatch { .. })));

    // 6. Escape attempts with arbitrary unauthorized paths
    for escape_path in &["../outside.txt", "/etc/passwd", "workspace_backup/code.py"] {
        let res = vfs.write_file_checked(escape_path, b"malicious", &allowed_write);
        assert!(matches!(res, Err(VfsBoundaryError::UnauthorizedWrite { .. })));
        assert!(!vfs.exists(escape_path));
    }
}

#[test]
fn test_adversarial_coordinator_diagnostics() {
    // 1. Diagnostic code name and prescription
    let diag = DiagnosticCode::OutOfBoundsWrite;
    assert_eq!(diag.name(), "ERR_OUT_OF_BOUNDS_WRITE");
    let presc = diag.default_prescription("data.db");
    assert!(presc.contains("write_set"));

    // 2. classify_failure_for_kind for ERR_OUT_OF_BOUNDS_WRITE
    let err_out = "ERR_OUT_OF_BOUNDS_WRITE: Unauthorized write to 'bad.py'. Path is not in write_set: []";
    let classified = omni_engine::planner::swarm_coordinator::SwarmCoordinator::classify_failure_for_kind(
        &SwarmAction::WriteFile { filename: "bad.py".to_string(), content: "foo".to_string() },
        false,
        err_out,
        false,
        0,
        None,
        "failed",
        None,
        Some(&TaskKind::Create),
    );
    assert_eq!(classified, DiagnosticCode::OutOfBoundsWrite);

    // 3. classify_failure_for_kind for VFS_BOUNDARY_VIOLATION
    let boundary_err = "Runtime error: VFS_BOUNDARY_VIOLATION: Unauthorized write to 'bad.py'.";
    let classified_boundary = omni_engine::planner::swarm_coordinator::SwarmCoordinator::classify_failure_for_kind(
        &SwarmAction::RunTerminal { command: "python3 bad.py".to_string() },
        false,
        boundary_err,
        false,
        0,
        None,
        "failed",
        None,
        Some(&TaskKind::Create),
    );
    assert_eq!(classified_boundary, DiagnosticCode::OutOfBoundsWrite);
}

#[test]
fn test_adversarial_subgoal_boundary_consistency() {
    let sg = SubGoal {
        id: 1,
        description: "Test boundary".to_string(),
        target_entity: "src/model.rs".to_string(),
        kind: TaskKind::Create,
        guidance: None,
        depends_on: vec![],
        read_set: vec![PathBuf::from("src/model.rs")],
        write_set: vec![PathBuf::from("src/model.rs")],
        max_tokens: 2048,
    };

    // Exact match
    assert!(sg.can_write("src/model.rs"));
    assert!(sg.can_read("src/model.rs"));

    // Disjoint
    assert!(!sg.can_write("src/lib.rs"));
    assert!(!sg.can_read("src/lib.rs"));

    // MemoryVfs::is_path_in_set normalizes
    assert!(MemoryVfs::is_path_in_set("./src/model.rs", &sg.write_set));
    assert!(MemoryVfs::is_path_in_set("src/../src/model.rs", &sg.write_set));
    assert!(!MemoryVfs::is_path_in_set("src/../src/lib.rs", &sg.write_set));
}

#[test]
fn test_adversarial_extended_traversal_attacks() {
    let allowed = vec![
        PathBuf::from("workspace/project"),
        PathBuf::from("crates/core/src/lib.rs"),
    ];

    // Allowed accesses
    assert!(MemoryVfs::is_path_in_set("workspace/project/main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("workspace/project/sub/file.txt", &allowed));
    assert!(MemoryVfs::is_path_in_set("./workspace/project/main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("workspace/./project/main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("workspace/project/../project/main.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("crates/core/src/lib.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("./crates/core/src/lib.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("/crates/core/src/lib.rs", &allowed));
    assert!(MemoryVfs::is_path_in_set("crates/core/src/../src/lib.rs", &allowed));

    // Traversal attack vectors: MUST BE BLOCKED
    // 1. Double-dot breakouts:
    assert!(!MemoryVfs::is_path_in_set("../../secret.env", &allowed));
    assert!(!MemoryVfs::is_path_in_set("../../../etc/shadow", &allowed));
    assert!(!MemoryVfs::is_path_in_set("workspace/project/../../secret.env", &allowed));
    assert!(!MemoryVfs::is_path_in_set("workspace/project/../../../secret.env", &allowed));
    assert!(!MemoryVfs::is_path_in_set("workspace/../../outside.txt", &allowed));
    assert!(!MemoryVfs::is_path_in_set("crates/core/src/../../outside.txt", &allowed));
    assert!(!MemoryVfs::is_path_in_set("crates/core/src/../../../../outside.txt", &allowed));

    // 2. Traversal with interleaved current dirs:
    assert!(!MemoryVfs::is_path_in_set("./workspace/./project/../../secret.env", &allowed));
    assert!(!MemoryVfs::is_path_in_set("workspace/./.././secret.env", &allowed));

    // 3. Root traversal breakouts:
    assert!(!MemoryVfs::is_path_in_set("/../secret.env", &allowed));
    assert!(!MemoryVfs::is_path_in_set("/../../secret.env", &allowed));
    assert!(!MemoryVfs::is_path_in_set("/workspace/project/../../secret.env", &allowed));

    // 4. Sibling directory prefix attacks:
    assert!(!MemoryVfs::is_path_in_set("workspace/project_extra/main.rs", &allowed));
    assert!(!MemoryVfs::is_path_in_set("workspace/project2/main.rs", &allowed));
    assert!(!MemoryVfs::is_path_in_set("workspace/projects/main.rs", &allowed));
    assert!(!MemoryVfs::is_path_in_set("workspace/project.bak", &allowed));
    assert!(!MemoryVfs::is_path_in_set("crates/core/src/lib.rs.bak", &allowed));
    assert!(!MemoryVfs::is_path_in_set("crates/core/src/lib_rs", &allowed));

    // 5. Special filenames:
    assert!(!MemoryVfs::is_path_in_set(".../outside.txt", &allowed));
    assert!(!MemoryVfs::is_path_in_set("..hidden/outside.txt", &allowed));
    assert!(!MemoryVfs::is_path_in_set(".", &allowed));
    assert!(!MemoryVfs::is_path_in_set("", &allowed));
}

#[test]
fn test_adversarial_checked_operations_attacks() {
    let vfs = MemoryVfs::new();
    let write_set = vec![PathBuf::from("sandbox/app.rs")];
    let read_set = vec![PathBuf::from("sandbox/data.json")];

    // Seed data
    vfs.write_file("sandbox/data.json", b"{\"key\": \"val\"}");
    vfs.write_file("secret.key", b"SUPER_SECRET");

    // Traversal writes
    let res = vfs.write_file_checked("sandbox/../secret.key", b"OVERWRITTEN", &write_set);
    assert!(matches!(res, Err(VfsBoundaryError::UnauthorizedWrite { .. })));
    assert_eq!(vfs.read_string("secret.key").unwrap(), "SUPER_SECRET");

    let res2 = vfs.write_file_checked("../../secret.key", b"OVERWRITTEN", &write_set);
    assert!(matches!(res2, Err(VfsBoundaryError::UnauthorizedWrite { .. })));

    let res3 = vfs.write_file_checked("sandbox/app.rs/../../secret.key", b"OVERWRITTEN", &write_set);
    assert!(matches!(res3, Err(VfsBoundaryError::UnauthorizedWrite { .. })));

    // Traversal deletes
    let res4 = vfs.delete_file_checked("sandbox/../secret.key", &write_set);
    assert!(matches!(res4, Err(VfsBoundaryError::UnauthorizedDelete { .. })));
    assert!(vfs.exists("secret.key"));

    // Traversal patches
    let res5 = vfs.patch_file_checked("sandbox/../secret.key", "SUPER", &write_set);
    assert!(matches!(res5, Err(VfsBoundaryError::UnauthorizedPatch { .. })));

    // Traversal reads
    let res6 = vfs.read_file_checked("sandbox/../secret.key", Some(&read_set));
    assert!(matches!(res6, Err(VfsBoundaryError::UnauthorizedRead { .. })));

    let res7 = vfs.read_file_checked("../../secret.key", Some(&read_set));
    assert!(matches!(res7, Err(VfsBoundaryError::UnauthorizedRead { .. })));

    // Authorized operations with normalization
    assert!(vfs.write_file_checked("./sandbox/app.rs", b"fn main() {}", &write_set).is_ok());
    assert!(vfs.write_file_checked("sandbox/sub/../app.rs", b"fn main() {}", &write_set).is_ok());
    assert!(vfs.read_file_checked("./sandbox/data.json", Some(&read_set)).is_ok());
    assert!(vfs.read_file_checked("sandbox/foo/../data.json", Some(&read_set)).is_ok());
}

#[test]
fn test_adversarial_vfs_extended_attacks() {
    let vfs = Arc::new(MemoryVfs::new());
    let write_set = vec![PathBuf::from("build/out.log")];
    let read_set = vec![PathBuf::from("build/in.txt")];

    vfs.write_file("build/in.txt", b"input data");
    vfs.write_file("etc/passwd", b"root:x:0:0");
    vfs.write_file("var/log/syslog", b"system logs");

    // Traversal write attacks
    let write_attacks = [
        "build/../etc/passwd",
        "../../etc/passwd",
        "build/sub/../../etc/passwd",
        "/etc/passwd",
        "build_extra/out.log",
    ];

    for attack in write_attacks {
        let res = vfs.write_file_checked(attack, b"pwned", &write_set);
        assert!(matches!(res, Err(VfsBoundaryError::UnauthorizedWrite { .. })), "Attack must fail: {}", attack);
    }

    let del_res = vfs.delete_file_checked("build/../etc/passwd", &write_set);
    assert!(matches!(del_res, Err(VfsBoundaryError::UnauthorizedDelete { .. })));

    let patch_res = vfs.patch_file_checked("build/../etc/passwd", "TARGET: root | REPLACEMENT: hacked", &write_set);
    assert!(matches!(patch_res, Err(VfsBoundaryError::UnauthorizedPatch { .. })));

    assert_eq!(vfs.read_string("etc/passwd").unwrap(), "root:x:0:0");

    // Read traversal attack
    let read_res = vfs.read_file_checked("build/../var/log/syslog", Some(&read_set));
    assert!(matches!(read_res, Err(VfsBoundaryError::UnauthorizedRead { .. })));
}
