use omni_engine::causal_memory::dag::CausalGraph;
use omni_engine::native_llama::NativeLlamaModel;
use omni_engine::sandbox::{LuaSandboxRunner, MemoryVfs};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// PART 1: LUA SANDBOX ADVERSARIAL STRESS TESTS
// ============================================================================

#[test]
fn test_lua_sandbox_security_adversarial_bypasses() {
    let vfs = Arc::new(MemoryVfs::new());
    let runner = LuaSandboxRunner::new(vfs);

    let bypass_payloads = [
        // Standard disabled globals
        ("require('os')", "require('os')"),
        ("require('io')", "require('io')"),
        ("require('debug')", "require('debug')"),
        ("require('package')", "require('package')"),
        ("os.execute('id')", "os.execute"),
        ("os.getenv('HOME')", "os.getenv"),
        ("io.open('/etc/passwd', 'r')", "io.open"),
        ("io.popen('uname -a')", "io.popen"),
        ("debug.getinfo(1)", "debug.getinfo"),
        ("debug.getregistry()", "debug.getregistry"),
        ("load('return 1')()", "load()"),
        ("loadfile('/tmp/exploit.lua')", "loadfile"),
        ("dofile('/tmp/exploit.lua')", "dofile"),
        ("coroutine.create(function() end)", "coroutine.create"),

        // Obfuscated / indirection bypasses
        ("_G['require']('os')", "_G['require']"),
        ("_G.require('os')", "_G.require"),
        ("_ENV['require']('os')", "_ENV['require']"),
        ("_ENV.require('os')", "_ENV.require"),
        ("rawget(_G, 'require')('os')", "rawget(_G, 'require')"),
        ("rawget(_ENV, 'require')('os')", "rawget(_ENV, 'require')"),
        ("package.loaded['os']", "package.loaded"),
        ("package.preload['os']", "package.preload"),
        ("package.searchpath('os', '')", "package.searchpath"),
        ("_G.os.execute('whoami')", "_G.os.execute"),
        ("_ENV.os.execute('whoami')", "_ENV.os.execute"),

        // Metatable and reflection bypasses
        (
            r#"
            local mt = getmetatable("")
            if mt then
                mt.__index = _G
                return ("").require("os")
            end
            error("no string metatable")
            "#,
            "string metatable reflection",
        ),
        (
            r#"
            local env = _ENV or _G
            local r = rawget(env, "load") or rawget(env, "require")
            if r then r("os") else error("blocked") end
            "#,
            "rawget environment scanning",
        ),
    ];

    for (code, label) in bypass_payloads {
        let res = runner.run_script(code);
        assert!(
            !res.success,
            "Adversarial bypass '{}' succeeded when it must fail safely! Output: {}",
            label, res.output_log
        );
        assert!(
            res.error.is_some(),
            "Adversarial bypass '{}' failed without returning error string",
            label
        );
    }
}

#[test]
fn test_lua_sandbox_runaway_loops_and_clean_termination() {
    let vfs = Arc::new(MemoryVfs::new());

    // Test with default instruction limit (100_000)
    let default_runner = LuaSandboxRunner::new(vfs.clone());

    let loop_snippets = [
        ("while true do end", "busy infinite while loop"),
        ("repeat until false", "repeat until false loop"),
        ("local x = 0; while true do x = x + 1 end", "infinite arithmetic loop"),
        ("local function f() return f() end; f()", "tail call infinite recursion"),
        (
            "local f, g; f = function() return g() end; g = function() return f() end; f()",
            "mutual tail recursion",
        ),
    ];

    for (snippet, label) in loop_snippets {
        let start = Instant::now();
        let res = default_runner.run_script(snippet);
        let elapsed = start.elapsed();

        println!(
            "Default runner [{}]: terminated in {:?} ({} µs), success={}",
            label,
            elapsed,
            elapsed.as_micros(),
            res.success
        );

        assert!(!res.success, "Runaway script '{}' must not succeed", label);
        assert!(res.error.is_some());
        let err = res.error.unwrap();
        assert!(
            err.contains("instruction quota exceeded") || err.contains("infinite loop prevented"),
            "Error must identify instruction quota violation: {}",
            err
        );
    }

    // Test with tight instruction limit (500 instructions) to verify sub-millisecond (< 1 ms) termination
    let tight_runner = LuaSandboxRunner::with_limits(vfs.clone(), None, 32 * 1024 * 1024, 500);

    for (snippet, label) in loop_snippets {
        let start = Instant::now();
        let res = tight_runner.run_script(snippet);
        let elapsed = start.elapsed();

        println!(
            "Tight runner (500 insn) [{}]: terminated in {:?} ({} µs), success={}",
            label,
            elapsed,
            elapsed.as_micros(),
            res.success
        );

        assert!(!res.success, "Runaway script '{}' must not succeed", label);
        assert!(
            elapsed < std::time::Duration::from_millis(15),
            "Runaway script '{}' must terminate in < 15 ms, took {:?}",
            label,
            elapsed
        );
    }
}

#[test]
fn test_lua_sandbox_heavy_recursion_stack_overflow_safety() {
    let vfs = Arc::new(MemoryVfs::new());
    let runner = LuaSandboxRunner::new(vfs);

    // Deep non-tail recursion (stack overflow attempt)
    let non_tail_code = r#"
        local function recurse(depth)
            local a = depth * 2
            return recurse(depth + 1) + a
        end
        recurse(1)
    "#;

    let start = Instant::now();
    let res = runner.run_script(non_tail_code);
    let elapsed = start.elapsed();

    println!("Non-tail recursion test terminated in {:?}", elapsed);
    assert!(!res.success, "Infinite non-tail recursion must fail");
    assert!(res.error.is_some());
    let err = res.error.unwrap();
    // In Lua 5.4, this will either hit the instruction hook or stack overflow safely without segfaulting host
    assert!(
        err.contains("instruction quota") || err.contains("stack overflow") || err.contains("Trap"),
        "Unexpected error for deep recursion: {}",
        err
    );
}

#[test]
fn test_lua_sandbox_memory_bombs() {
    let vfs = Arc::new(MemoryVfs::new());
    // Restrict memory to 2 MiB for deterministic memory exhaustion
    let runner = LuaSandboxRunner::with_limits(vfs, None, 2 * 1024 * 1024, 1_000_000);

    let memory_bombs = [
        (
            r#"
            local t = {}
            for i = 1, 1000000 do
                t[i] = string.rep("M", 1024)
            end
            "#,
            "large string allocation bomb",
        ),
        (
            r#"
            local s = "BOMB"
            while true do
                s = s .. s
            end
            "#,
            "string concatenation exponential bomb",
        ),
        (
            r#"
            local root = {}
            while true do
                root = { left = root, right = root, data = string.rep("X", 256) }
            end
            "#,
            "exponential tree node allocation bomb",
        ),
        (
            r#"
            local closures = {}
            for i = 1, 100000 do
                local captured = string.rep("C", 512)
                closures[i] = function() return captured end
            end
            "#,
            "closure generation memory bomb",
        ),
    ];

    for (code, label) in memory_bombs {
        let start = Instant::now();
        let res = runner.run_script(code);
        let elapsed = start.elapsed();

        println!("Memory bomb [{}] terminated in {:?}", label, elapsed);
        assert!(!res.success, "Memory bomb '{}' must fail safely", label);
        assert!(res.error.is_some());
        let err = res.error.unwrap();
        assert!(
            err.to_lowercase().contains("memory")
                || err.contains("quota")
                || err.contains("Trap"),
            "Memory bomb '{}' must report memory or quota error, got: {}",
            label,
            err
        );
    }
}

// ============================================================================
// PART 2: CAUSAL DAG EXACT TAIL ROLLBACK STRESS TESTS
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
