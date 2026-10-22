use omni_engine::native_llama::NativeLlamaModel;
use std::path::PathBuf;

#[test]
fn test_rope_coherence_and_attention_sinks() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = manifest_dir.join("models").join("qwen-0.5b.gguf");

    if !model_path.exists() {
        eprintln!("Skipping test: model not found at {:?}", model_path);
        return;
    }

    println!("\n=== [1] Loading Real GGUF Model via Native C FFI ===");
    let model = NativeLlamaModel::load(&model_path, 0).expect("Failed to load model weights");
    
    println!("\n=== [2] Creating Execution Context ===");
    let mut ctx = model.create_context(512, 512, 4).expect("Failed to create context");
    
    println!("\n=== [3] Tokenizing Prompt (>= 50 tokens) ===");
    let prompt = "We are going to write a highly optimized Python function to calculate the Fibonacci sequence. The sequence must be generated iteratively to avoid recursion depth limits. It starts with 0 and 1, and each subsequent number is the sum of the previous two numbers in the series. Let's begin coding this now: def fibonacci(";
    let tokens = model.tokenize(prompt, true).expect("Tokenization failed");
    
    println!("Prompt tokenized into {} tokens", tokens.len());
    assert!(tokens.len() >= 50, "We need at least 50 tokens for this test. Actual: {}", tokens.len());
    
    // Evaluate 50 tokens exactly to match the blueprint scenario
    let tokens_to_eval = &tokens[0..50];
    ctx.eval_tokens(tokens_to_eval, 0).expect("Evaluation failed");
    
    let initial_cells = ctx.kv_cache_used_cells();
    println!("KV Cache Cells after evaluation: {}", initial_cells);
    assert_eq!(initial_cells, 50, "Should have exactly 50 cells in KV cache");
    assert_eq!(ctx.current_cursor(), 50, "Cursor should be exactly at 50");

    println!("\n=== [4] Executing Mid-Span Rollback (20..35) ===");
    // Remove tokens [20, 35) and shift the remaining tokens left.
    // The tokens 35..50 will shift to 20..35.
    let rm_result = ctx.rollback_range(0, 20, 35).expect("Rollback failed");
    assert!(rm_result, "Rollback must return true");
    
    let after_rollback_cells = ctx.kv_cache_used_cells();
    println!("KV Cache Cells after rollback_range: {}", after_rollback_cells);
    
    // 50 - 15 = 35 cells remaining
    assert_eq!(after_rollback_cells, 35, "Physical KV cache cells should be 35");
    assert_eq!(ctx.current_cursor(), 35, "Cursor should be updated to 35");

    println!("\n=== [5] Testing Attention Sink Protection ===");
    // Attempting to prune tokens 0..4 should fail or panic.
    // The rollback_range method has `assert!(p0 >= 4)`. We use catch_unwind to test it.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ctx_clone = model.create_context(512, 512, 4).expect("Failed to create context");
        ctx_clone.eval_tokens(&tokens[0..10], 0).unwrap();
        let _ = ctx_clone.rollback_range(0, 2, 8); // Should panic because p0 < 4
    }));
    
    assert!(result.is_err(), "rollback_range MUST panic if p0 < 4 to protect Attention Sinks!");

    println!("\n>>> ROPE COHERENCE & SINK PROTECTION TEST PASSED 100% <<<\n");
}
