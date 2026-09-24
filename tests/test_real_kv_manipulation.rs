use omni_engine::native_llama::NativeLlamaModel;
use std::path::PathBuf;

#[test]
fn test_end_to_end_real_kv_cache_manipulation() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = manifest_dir.join("models").join("qwen-0.5b.gguf");

    if !model_path.exists() {
        eprintln!("Skipping test: model not found at {:?}", model_path);
        return;
    }

    println!("\n=== [1] Loading Real GGUF Model via Native C FFI ===");
    let model = NativeLlamaModel::load(&model_path, 0).expect("Failed to load model weights");
    println!("Model loaded successfully from: {:?}", model_path);

    println!("\n=== [2] Creating Real Execution Context (512 tokens) ===");
    let mut ctx = model.create_context(512, 512, 4).expect("Failed to create context");
    
    // Check baseline KV state
    let initial_cells = ctx.kv_cache_used_cells();
    let initial_tokens = ctx.kv_cache_token_count();
    println!("Baseline KV Cache: used_cells={}, token_count={}", initial_cells, initial_tokens);
    assert_eq!(initial_cells, 0, "Initial KV cache must be empty");

    println!("\n=== [3] Tokenizing Real Text Prompt ===");
    let prompt = "Explain the difference between mutable and immutable memory in systems programming:";
    let tokens = model.tokenize(prompt, true).expect("Tokenization failed");
    println!("Prompt tokenized into {} tokens", tokens.len());
    assert!(!tokens.is_empty(), "Tokens must not be empty");

    println!("\n=== [4] Evaluating Tokens into Transformer Tensor Memory ===");
    ctx.eval_tokens(&tokens, 0).expect("Evaluation failed");
    let after_eval_cells = ctx.kv_cache_used_cells();
    let after_eval_tokens = ctx.kv_cache_token_count();
    println!("After Decode KV Cache: used_cells={}, token_count={}", after_eval_cells, after_eval_tokens);
    assert_eq!(after_eval_cells, tokens.len(), "KV cells must match evaluated token count");

    println!("\n=== [5] Executing Surgical Causal KV Rollback (O(1) memory excise) ===");
    // Remove the last 5 tokens from KV cache
    let n_tokens = tokens.len() as i32;
    let rollback_start = n_tokens - 5;
    let rollback_end = n_tokens;
    
    let rm_result = ctx.kv_cache_seq_rm(0, rollback_start, rollback_end)
        .expect("Rollback call failed");
    assert!(rm_result, "Rollback must return true");

    let after_rollback_cells = ctx.kv_cache_used_cells();
    println!("After KV Rollback: used_cells={}", after_rollback_cells);
    assert_eq!(
        after_rollback_cells,
        tokens.len() - 5,
        "KV cache cells must physically decrease after causal rollback!"
    );

    println!("\n=== [6] Executing Swarm Branching (Forking KV Cache State) ===");
    // Fork current sequence 0 to sequence 1
    ctx.kv_cache_seq_cp(0, 1, 0, after_rollback_cells as i32);
    let after_branch_cells = ctx.kv_cache_used_cells();
    let after_branch_tokens = ctx.kv_cache_token_count();
    println!("After Swarm Fork (Seq 0 -> Seq 1): used_cells={}, token_count={}", after_branch_cells, after_branch_tokens);
    assert_eq!(after_branch_cells, after_rollback_cells, "Cell count stays constant (shared memory)");
    assert_eq!(after_branch_tokens, after_rollback_cells * 2, "Token count doubles because cells belong to 2 sequences!");

    println!("\n=== [7] Executing Full Epistemic Apoptosis (Purging KV Cache) ===");
    ctx.kv_cache_clear();
    let after_clear_cells = ctx.kv_cache_used_cells();
    let after_clear_tokens = ctx.kv_cache_token_count();
    println!("After Epistemic Apoptosis: used_cells={}, token_count={}", after_clear_cells, after_clear_tokens);
    assert_eq!(after_clear_cells, 0, "KV cache must be physically reset to 0 cells!");
    assert_eq!(after_clear_tokens, 0, "KV cache must be physically reset to 0 tokens!");

    println!("\n>>> REAL KV-CACHE MANIPULATION TEST PASSED 100% WITH PHYSICAL TENSORS! <<<\n");
}
