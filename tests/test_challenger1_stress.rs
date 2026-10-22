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

use omni_engine::causal_memory::dag::CausalGraph;
use omni_engine::native_llama::NativeLlamaModel;
use std::path::PathBuf;

// ============================================================================
// CAUSAL DAG EXACT TAIL ROLLBACK STRESS TESTS
// ============================================================================

#[test]
fn test_causal_dag_exact_tail_rollback_logical_scenarios() {
    // Scenario 1: Multi-step chain and leaf eviction
    let mut dag = CausalGraph::new();
    assert!(dag.record_step(1, "Step 1", &[], &["state.json"], 25));
    assert!(dag.record_step(2, "Step 2", &["state.json"], &["model.bin"], 35));
    assert!(dag.record_step(3, "Step 3", &["model.bin"], &["eval.log"], 45));
    assert!(dag.record_step(4, "Step 4", &["eval.log"], &["report.txt"], 30));

    assert_eq!(dag.current_token_cursor(), 135);
    assert!(dag.is_active_suffix(4));

    // Suffix rollback of leaf step 4
    let res = dag.rollback_step_logical(4);
    assert!(res.is_ok(), "Leaf step rollback must succeed: {:?}", res);
    assert_eq!(dag.current_token_cursor(), 105, "Cursor must rewind exactly to Step 3 end (105)");
    assert!(!dag.contains_step(4));
    assert!(dag.is_active_suffix(3), "Step 3 must now be active suffix");

    // Suffix rollback of step 3
    let res = dag.rollback_step_logical(3);
    assert!(res.is_ok());
    assert_eq!(dag.current_token_cursor(), 60, "Cursor must rewind to Step 2 end (60)");
    assert!(!dag.contains_step(3));
    assert!(dag.is_active_suffix(2));

    // Rollback step 2
    let res = dag.rollback_step_logical(2);
    assert!(res.is_ok());
    assert_eq!(dag.current_token_cursor(), 25);
    assert!(dag.is_active_suffix(1));

    // Rollback to step 0
    let res = dag.rollback_step_logical(1);
    assert!(res.is_ok());
    assert_eq!(dag.current_token_cursor(), 0, "Cursor must rewind to 0");
    assert!(!dag.contains_step(1));
}

#[test]
fn test_causal_dag_exact_tail_rollback_with_physical_context() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = manifest_dir.join("models").join("qwen-0.5b.gguf");

    if !model_path.exists() {
        eprintln!("Skipping physical context test: model not found at {:?}", model_path);
        return;
    }

    let model = NativeLlamaModel::load(&model_path, 0).expect("Failed to load model");
    let mut ctx = model.create_context(512, 512, 4).expect("Failed to create context");

    // Evaluate tokens for 4 distinct steps
    let step1_text = "Task 1: Initialize database schema with primary keys.";
    let step2_text = "Task 2: Insert initial user profiles and permissions.";
    let step3_text = "Task 3: Compute aggregate metrics across user accounts.";
    let step4_text = "Task 4: Export summary analytics to external CSV file.";

    let toks1 = model.tokenize(step1_text, true).unwrap();
    let toks2 = model.tokenize(step2_text, false).unwrap();
    let toks3 = model.tokenize(step3_text, false).unwrap();
    let toks4 = model.tokenize(step4_text, false).unwrap();

    let n1 = toks1.len();
    let n2 = toks2.len();
    let n3 = toks3.len();
    let n4 = toks4.len();

    println!("Token counts: step1={}, step2={}, step3={}, step4={}", n1, n2, n3, n4);

    let mut dag = CausalGraph::new();

    // Step 1
    ctx.eval_tokens(&toks1, 0).unwrap();
    assert!(dag.record_step_with_context(1, "Step 1", &[], &["db"], &ctx));
    assert_eq!(dag.current_token_cursor(), n1);
    assert_eq!(ctx.current_cursor(), n1);
    assert_eq!(ctx.kv_cache_used_cells(), n1);

    // Step 2
    ctx.eval_tokens(&toks2, 0).unwrap();
    assert!(dag.record_step_with_context(2, "Step 2", &["db"], &["users"], &ctx));
    assert_eq!(dag.current_token_cursor(), n1 + n2);
    assert_eq!(ctx.current_cursor(), n1 + n2);
    assert_eq!(ctx.kv_cache_used_cells(), n1 + n2);

    // Step 3
    ctx.eval_tokens(&toks3, 0).unwrap();
    assert!(dag.record_step_with_context(3, "Step 3", &["users"], &["metrics"], &ctx));
    assert_eq!(dag.current_token_cursor(), n1 + n2 + n3);
    assert_eq!(ctx.current_cursor(), n1 + n2 + n3);
    assert_eq!(ctx.kv_cache_used_cells(), n1 + n2 + n3);

    // Step 4
    ctx.eval_tokens(&toks4, 0).unwrap();
    assert!(dag.record_step_with_context(4, "Step 4", &["metrics"], &["csv"], &ctx));
    let total_tokens = n1 + n2 + n3 + n4;
    assert_eq!(dag.current_token_cursor(), total_tokens);
    assert_eq!(ctx.current_cursor(), total_tokens);
    assert_eq!(ctx.kv_cache_used_cells(), total_tokens);

    // ========================================================================
    // Scenario A: Evict leaf step 4 with context (strict tail-rollback)
    // ========================================================================
    println!("\n--- Evicting leaf step 4 ---");
    let ok = dag.evict_step_with_context(4, step4_text, Some(&toks4), Some(&mut ctx)).unwrap();
    assert!(ok);

    let expected_pos_3 = n1 + n2 + n3;
    assert_eq!(
        ctx.kv_cache_used_cells(),
        expected_pos_3,
        "KV cache cells must physically match Step 3 boundary"
    );
    assert_eq!(
        ctx.current_cursor(),
        expected_pos_3,
        "Context cursor must rewind to Step 3 boundary"
    );
    assert_eq!(
        dag.current_token_cursor(),
        expected_pos_3,
        "DAG cursor must match Step 3 boundary"
    );
    assert!(dag.is_step_evicted(4).unwrap());
    assert_eq!(dag.step_token_range(4), Some((0, 0)));

    // ========================================================================
    // Scenario B: Evict step 3 with context
    // ========================================================================
    println!("\n--- Evicting step 3 ---");
    let ok = dag.evict_step_with_context(3, step3_text, Some(&toks3), Some(&mut ctx)).unwrap();
    assert!(ok);

    let expected_pos_2 = n1 + n2;
    assert_eq!(ctx.kv_cache_used_cells(), expected_pos_2);
    assert_eq!(ctx.current_cursor(), expected_pos_2);
    assert_eq!(dag.current_token_cursor(), expected_pos_2);
    assert!(dag.is_step_evicted(3).unwrap());

    // ========================================================================
    // Scenario C: Continuation - Evaluate new alternative branch from Step 2
    // ========================================================================
    println!("\n--- Continuation: Evaluating alternative branch ---");
    let alt_text = "Task 3 (Alt): Generate mock testing data instead.";
    let alt_toks = model.tokenize(alt_text, false).unwrap();
    let alt_len = alt_toks.len();

    ctx.eval_tokens(&alt_toks, 0).unwrap();
    assert!(dag.record_step_with_context(5, "Step 3 Alt", &["users"], &["mock"], &ctx));

    let expected_alt_pos = expected_pos_2 + alt_len;
    assert_eq!(ctx.kv_cache_used_cells(), expected_alt_pos);
    assert_eq!(ctx.current_cursor(), expected_alt_pos);
    assert_eq!(dag.current_token_cursor(), expected_alt_pos);

    // ========================================================================
    // Scenario D: Mid-span tail eviction (evicting Step 2 when Step 5 exists)
    // ========================================================================
    println!("\n--- Mid-span tail rollback: Evicting Step 2 ---");
    let ok = dag.evict_step_with_context(2, step2_text, Some(&toks2), Some(&mut ctx)).unwrap();
    assert!(ok);

    let expected_pos_1 = n1;
    assert_eq!(
        ctx.kv_cache_used_cells(),
        expected_pos_1,
        "KV cache cells must truncate to Step 2 start (p0 = n1)"
    );
    assert_eq!(ctx.current_cursor(), expected_pos_1);
    assert_eq!(dag.current_token_cursor(), expected_pos_1);
    // Trailing step 5 must be pruned from DAG
    assert!(!dag.contains_step(5), "Trailing step 5 must be pruned by tail rollback");

    // ========================================================================
    // Scenario E: Rollback to step 0
    // ========================================================================
    println!("\n--- Rollback to step 0: Evicting Step 1 ---");
    let ok = dag.evict_step_with_context(1, step1_text, Some(&toks1), Some(&mut ctx)).unwrap();
    assert!(ok);

    assert_eq!(ctx.kv_cache_used_cells(), 0, "KV cache must be exactly 0 cells");
    assert_eq!(ctx.current_cursor(), 0, "Context cursor must be 0");
    assert_eq!(dag.current_token_cursor(), 0, "DAG cursor must be 0");
}
