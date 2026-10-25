use omni_engine::planner::swarm_coordinator::{SwarmCoordinator, SwarmConfig};
use omni_engine::sandbox::cleanroom::UniversalCleanroom;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_live_acceptance_feedback_swarm() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = manifest_dir.join("models").join("qwen2.5-coder-1.5b-instruct-q4_k_m.gguf");

    if !model_path.exists() {
        eprintln!("[SKIP] Model not found at: {:?}", model_path);
        return;
    }

    println!("\n================================================================================");
    println!(">>> RUNNING LIVE ACCEPTANCE CLOSED-LOOP BENCHMARK WITH REAL MODEL");
    println!(">>> Model: {:?}", model_path.file_name().unwrap());
    println!("================================================================================");

    let output_dir = manifest_dir.join("workspace").join("live_test_calc");
    let _ = fs::remove_dir_all(&output_dir);
    let _ = fs::create_dir_all(&output_dir);

    let user_goal = "Build a complete interactive Calculator CLI in Python: basic arithmetic operations, file persistence to save calculation history in history.json, error handling for division by zero and corrupted files, interactive loop";

    let config = SwarmConfig {
        orchestrator_model_path: model_path.clone(),
        worker_model_path: model_path.clone(),
        max_subgoals: 3,
        max_steps_per_worker: 3,
        input_dir: None,
        output_dir: Some(output_dir.clone()),
        verbose: true,
        auto_approve_deps: None,
        is_modify_mode: false,
        review_plan: false,
    };

    let coordinator = SwarmCoordinator::new(config)
        .expect("Failed to initialize SwarmCoordinator with real model");

    let result = coordinator.execute_goal(user_goal)
        .expect("SwarmCoordinator execution failed");

    println!("\n[SWARM FINISHED] Elapsed: {}ms, Rollbacks: {}, Tokens Saved: {}",
        result.elapsed_ms, result.total_rollbacks, result.total_tokens_saved);

    // Verify output files in output directory
    let entries: Vec<PathBuf> = fs::read_dir(&output_dir)
        .expect("Failed to read output dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();

    println!("[OUTPUT FILES CREATED]: {:?}", entries);
    assert!(!entries.is_empty(), "At least one deliverable file must be materialized to disk");

    // Audit materialized python deliverable in UniversalCleanroom
    let py_file = entries.iter().find(|p| p.extension().map_or(false, |ext| ext == "py"));
    if let Some(target_path) = py_file {
        let code = fs::read_to_string(target_path).expect("Failed to read materialized Python file");
        println!("\n[MATERIALIZED DELIVERABLE CONTENT ({})]:\n{}", target_path.display(), code);

        let file_name = target_path.file_name().unwrap().to_str().unwrap();

        // 1. Audit syntax in Cleanroom
        let syntax_report = UniversalCleanroom::audit_code_syntax(
            file_name,
            code.as_bytes(),
        );
        println!("\n[CLEANROOM SYNTAX AUDIT]: Valid={}, ExitCode={:?}", syntax_report.is_valid, syntax_report.exit_code);
        assert!(syntax_report.is_valid, "Materialized deliverable must be syntactically valid");

        // 2. Audit acceptance criteria in Cleanroom
        let acceptance_report = UniversalCleanroom::audit_acceptance_criteria(
            file_name,
            &code,
            user_goal,
        );
        println!("\n[CLEANROOM ACCEPTANCE AUDIT]: Accepted={}, Covered={:?}, Missing={:?}",
            acceptance_report.is_accepted,
            acceptance_report.covered_requirements,
            acceptance_report.missing_requirements
        );

        // 3. Mode 3: Real Interactive Cleanroom Execution
        let staged_files = vec![(PathBuf::from(file_name), code.as_bytes().to_vec())];
        let exec_cmd = format!("python3 {}", file_name);
        let sim_inputs = vec!["add", "10", "20", "exit"];

        let interactive_report = UniversalCleanroom::execute_interactive(
            &exec_cmd,
            &staged_files,
            &sim_inputs,
            Duration::from_secs(5),
        );

        println!("\n[CLEANROOM INTERACTIVE EXECUTION REPORT]:");
        println!("  Success:   {}", interactive_report.success);
        println!("  Exit Code: {:?}", interactive_report.exit_code);
        println!("  Timed Out: {}", interactive_report.timed_out);
        println!("  Stdout:\n{}", interactive_report.full_stdout.trim());
        if !interactive_report.full_stderr.is_empty() {
            println!("  Stderr:\n{}", interactive_report.full_stderr.trim());
        }
    }
}
