use omni_engine::sandbox::cleanroom::UniversalCleanroom;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_cleanroom_written_artifact_analysis() {
    println!("=== Testing Mode 1: Written Artifact Auditing ===");

    // Case 1: Complete and rigorous architectural document
    let complete_doc = r#"# Distributed Consensus Specification
## Overview
This document specifies the consensus protocol, leader election, and raft log replication.
The state machine guarantees sequential consistency across all participating nodes.

## Fault Tolerance
The system tolerates up to f failures in a cluster of 2f + 1 nodes.
Heartbeats are dispatched every 50ms to ensure liveness and detect partitions.

## Acceptance Criteria
All nodes must reach state convergence within 200ms of partition healing.
"#;
    let report1 = UniversalCleanroom::audit_written_content(
        complete_doc,
        &["consensus", "leader election", "log replication"],
    );
    println!("Report 1 (Complete): {:?}", report1);
    assert!(report1.is_complete, "Complete doc should pass validation");
    assert_eq!(report1.placeholder_count, 0);
    assert!(report1.score >= 0.8);
    assert_eq!(report1.covered_topics.len(), 3);
    assert!(report1.missing_topics.is_empty());
    assert_eq!(report1.detected_format, "Markdown");

    // Case 2: Document with incomplete placeholders and missing topics
    let incomplete_doc = r#"# Quick Draft
TODO: fill in the consensus algorithm details later.
FIXME: broken formula.
...
"#;
    let report2 = UniversalCleanroom::audit_written_content(
        incomplete_doc,
        &["consensus", "leader election"],
    );
    println!("Report 2 (Incomplete): {:?}", report2);
    assert!(!report2.is_complete, "Incomplete doc with TODOs must fail validation");
    assert!(report2.placeholder_count >= 2, "Must detect TODO and FIXME placeholders");
    assert!(report2.missing_topics.contains(&"leader election".to_string()));
}

#[test]
fn test_cleanroom_code_syntax_and_error_extraction() {
    println!("=== Testing Mode 2: Multi-Language Code & AST Analysis ===");

    // Case 1: Valid C Program
    let valid_c = br#"
#include <stdio.h>
int main(void) {
    printf("Universal Cleanroom C test passed\n");
    return 0;
}
"#;
    let report_c = UniversalCleanroom::audit_code_syntax("cleanroom_test.c", valid_c);
    println!("C Syntax Report: valid={}, exit_code={:?}", report_c.is_valid, report_c.exit_code);
    assert!(report_c.is_valid, "Valid C code should pass syntax check");
    assert!(report_c.error_line.is_none());

    // Case 2: Broken Python Code with Syntax Error at Line 3
    let broken_py = br#"def compute():
    val = 100
    print("Unclosed string
    return val
"#;
    let report_py = UniversalCleanroom::audit_code_syntax("cleanroom_broken.py", broken_py);
    println!("Python Broken Report: valid={}, error_line={:?}, error_msg={:?}",
        report_py.is_valid, report_py.error_line, report_py.error_message);
    assert!(!report_py.is_valid, "Broken Python code must fail syntax check");
    assert!(report_py.error_line.is_some(), "Must extract failing line number");
    assert!(report_py.broken_snippet.is_some(), "Must extract broken AST snippet");
}

#[test]
fn test_cleanroom_interactive_execution_with_stdin() {
    println!("=== Testing Mode 3: Real Interactive Program Execution ===");

    // An interactive Python script that reads two numbers, calculates sum and product
    let interactive_script = br#"
import sys

try:
    line1 = input("Enter first number: ")
    num1 = int(line1.strip())
    line2 = input("Enter second number: ")
    num2 = int(line2.strip())
    op = input("Enter operation (+ or *): ").strip()

    if op == '+':
        result = num1 + num2
    elif op == '*':
        result = num1 * num2
    else:
        result = 0

    print(f"RESULT: {result}")
except Exception as e:
    print(f"ERROR: {e}")
    sys.exit(1)
"#;

    let staged_files = vec![(PathBuf::from("interactive_calc.py"), interactive_script.to_vec())];
    let inputs = vec!["25", "17", "+"];

    let report = UniversalCleanroom::execute_interactive(
        "python3 interactive_calc.py",
        &staged_files,
        &inputs,
        Duration::from_secs(5),
    );

    println!("Interactive Execution Report:");
    println!("  Success: {}", report.success);
    println!("  Exit Code: {:?}", report.exit_code);
    println!("  Stdout:\n{}", report.full_stdout);
    println!("  Stderr:\n{}", report.full_stderr);

    assert!(report.success, "Interactive program must succeed with fed inputs");
    assert_eq!(report.exit_code, Some(0));
    assert!(report.full_stdout.contains("RESULT: 42"), "Must calculate 25 + 17 = 42");
    assert!(!report.timed_out);
}
