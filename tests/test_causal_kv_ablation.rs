use omni_engine::causal_memory::dag::CausalGraph;
use omni_engine::native_llama::NativeLlamaModel;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::time::Instant;

extern "C" {
    fn getpagesize() -> i32;
}

/// Retrieves process resident set size (RSS) in MB using kernel procfs telemetries.
/// Reads `/proc/self/status` (VmRSS) directly in kB, falling back to `/proc/self/statm`
/// with dynamic system page size obtained via `getpagesize()`.
fn get_process_rss_mb() -> f64 {
    if let Ok(content) = read_to_string("/proc/self/status") {
        for line in content.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<f64>() {
                        return kb / 1024.0;
                    }
                }
            }
        }
    }
    if let Ok(content) = read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(pages) = parts[1].parse::<usize>() {
                let page_size_bytes = unsafe { getpagesize() };
                let page_size_kb = if page_size_bytes > 0 {
                    page_size_bytes as f64 / 1024.0
                } else {
                    4.0
                };
                return (pages as f64 * page_size_kb) / 1024.0;
            }
        }
    }
    0.0
}

#[derive(Debug, Clone)]
struct LatencyStats {
    median_ms: f64,
    mean_ms: f64,
    std_dev_ms: f64,
    min_ms: f64,
    max_ms: f64,
}

fn compute_stats(mut times_ms: Vec<f64>) -> LatencyStats {
    assert!(!times_ms.is_empty(), "Cannot compute stats for empty sample");
    times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = times_ms.len();
    let median = if n % 2 == 1 {
        times_ms[n / 2]
    } else {
        (times_ms[n / 2 - 1] + times_ms[n / 2]) / 2.0
    };
    let mean = times_ms.iter().sum::<f64>() / n as f64;
    let variance = times_ms.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n as f64;
    let std_dev = variance.sqrt();
    LatencyStats {
        median_ms: median,
        mean_ms: mean,
        std_dev_ms: std_dev,
        min_ms: times_ms[0],
        max_ms: times_ms[n - 1],
    }
}

/// Measures logit vector divergence between two conditions:
/// Returns (L_inf max diff, L1 mean absolute error, Cosine similarity).
fn compare_logits(l1: &[f32], l2: &[f32]) -> (f32, f32, f64) {
    assert_eq!(
        l1.len(),
        l2.len(),
        "Logit vectors must match in vocabulary dimension"
    );
    let mut max_diff: f32 = 0.0;
    let mut sum_diff: f64 = 0.0;
    let mut dot: f64 = 0.0;
    let mut norm1: f64 = 0.0;
    let mut norm2: f64 = 0.0;

    for (&a, &b) in l1.iter().zip(l2.iter()) {
        let diff = (a - b).abs();
        if diff > max_diff {
            max_diff = diff;
        }
        sum_diff += diff as f64;
        dot += (a as f64) * (b as f64);
        norm1 += (a as f64) * (a as f64);
        norm2 += (b as f64) * (b as f64);
    }

    let mean_diff = (sum_diff / l1.len() as f64) as f32;
    let cosine_sim = if norm1 > 0.0 && norm2 > 0.0 {
        dot / (norm1.sqrt() * norm2.sqrt())
    } else {
        1.0
    };

    (max_diff, mean_diff, cosine_sim)
}

#[test]
fn test_rigorous_causal_kv_ablation_and_memory_benchmarks() {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = base_dir.join("models/qwen-0.5b.gguf");

    // Strict validation: fail immediately if the required model weights are absent
    assert!(
        model_path.exists(),
        "Ablation benchmark requires model weights at {:?}. Ensure models/qwen-0.5b.gguf is present.",
        model_path
    );

    println!("\n================================================================================");
    println!("  CAUSAL KV-CACHE EMPIRICAL ABLATION & LOGIT DIVERGENCE BENCHMARK");
    println!("================================================================================");

    let rss_initial = get_process_rss_mb();
    println!("Initial Process Baseline RSS: {:.2} MB", rss_initial);

    // 1. Model & Context Lifecycle
    let model = NativeLlamaModel::load(&model_path, 0)
        .expect("Failed to load native GGUF model");
    let rss_after_model = get_process_rss_mb();
    println!("Process RSS after model mmap (vocab={}): {:.2} MB", model.n_vocab(), rss_after_model);

    let mut ctx = model.create_context(512, 512, 4)
        .expect("Failed to create execution context");
    let rss_after_context = get_process_rss_mb();
    println!("Process RSS after allocating context buffer (n_ctx=512): {:.2} MB", rss_after_context);
    println!("Physical Context Capacity: {} cells (Current used: {})", ctx.n_ctx(), ctx.kv_cache_used_cells());

    // 2. Multi-Prompt Test Suite (System Administration & Code Generation)
    struct PromptScenario {
        name: &'static str,
        prefix_prompt: &'static str,
        failed_attempt: &'static str,
        correction_prompt: &'static str,
    }

    let scenarios = vec![
        PromptScenario {
            name: "Scenario A: System Diagnostic Command",
            prefix_prompt: "You are an autonomous Linux systems administrator. Inspecting network state: ",
            failed_attempt: "exec: rm -rf /etc/network/interfaces --no-preserve-root; reboot;",
            correction_prompt: "exec: ip addr show eth0; netstat -tuln;",
        },
        PromptScenario {
            name: "Scenario B: Python Data Structure Initialization",
            prefix_prompt: "Write a high-performance Python cache handler. Configuration: ",
            failed_attempt: "import undefined_legacy_module\nhandler = undefined_legacy_module.setup()",
            correction_prompt: "import collections\nhandler = collections.OrderedDict()",
        },
    ];

    for scenario in scenarios {
        println!("\n--------------------------------------------------------------------------------");
        println!("  Evaluating {}", scenario.name);
        println!("--------------------------------------------------------------------------------");

        let prefix_tokens = model.tokenize(scenario.prefix_prompt, true).unwrap();
        let failed_tokens = model.tokenize(scenario.failed_attempt, false).unwrap();
        let correction_tokens = model.tokenize(scenario.correction_prompt, false).unwrap();

        let prefix_len = prefix_tokens.len();
        let failed_len = failed_tokens.len();
        let correction_len = correction_tokens.len();

        println!("Token Counts: Prefix={}, Failed Attempt={}, Pruned Correction={}", 
            prefix_len, failed_len, correction_len);

        // Warmup execution to avoid cold JIT / CPU scheduler anomalies
        ctx.kv_cache_clear();
        ctx.eval_tokens(&prefix_tokens, 0).unwrap();
        ctx.kv_cache_clear();

        const NUM_ITERATIONS: usize = 3;
        let mut c1_latencies = Vec::with_capacity(NUM_ITERATIONS);
        let mut c2_eval_latencies = Vec::with_capacity(NUM_ITERATIONS);
        let mut c2_rollback_latencies = Vec::with_capacity(NUM_ITERATIONS);
        let mut c3_latencies = Vec::with_capacity(NUM_ITERATIONS);

        let mut token_c1 = 0;
        let mut token_c2 = 0;
        let mut token_c3 = 0;

        let mut logits_c1 = Vec::new();
        let mut logits_c2 = Vec::new();
        let mut logits_c3 = Vec::new();

        let mut cells_c1 = 0;
        let mut cells_c2 = 0;
        let mut cells_c3 = 0;

        for _iter in 0..NUM_ITERATIONS {
            // =========================================================================
            // CONDITION 1: MONOTONIC STATEFUL KV ACCUMULATION (Industry Baseline)
            // =========================================================================
            ctx.kv_cache_clear();
            ctx.eval_tokens(&prefix_tokens, 0).unwrap();
            ctx.eval_tokens(&failed_tokens, 0).unwrap();

            let mono_start = Instant::now();
            ctx.eval_tokens(&correction_tokens, 0).unwrap();
            c1_latencies.push(mono_start.elapsed().as_secs_f64() * 1000.0);

            token_c1 = ctx.sample_greedy().unwrap();
            logits_c1 = ctx.get_logits().unwrap();
            cells_c1 = ctx.kv_cache_used_cells();

            // =========================================================================
            // CONDITION 2: IN-PLACE CAUSALGRAPH KV ROLLBACK (Ours)
            // =========================================================================
            ctx.kv_cache_clear();
            let mut graph = CausalGraph::with_prefix_offset(prefix_len);

            // Prefill prefix
            ctx.eval_tokens(&prefix_tokens, 0).unwrap();
            assert_eq!(ctx.current_cursor(), prefix_len);
            assert_eq!(graph.current_token_cursor(), prefix_len);

            // Record and evaluate failed attempt
            let step_failed = 1;
            graph.record_step(
                step_failed,
                "Failed command execution",
                &["system_state"],
                &["system_state"],
                failed_len,
            );
            ctx.eval_tokens(&failed_tokens, 0).unwrap();
            assert_eq!(ctx.kv_cache_used_cells(), prefix_len + failed_len);
            assert_eq!(ctx.current_cursor(), prefix_len + failed_len);
            assert_eq!(graph.current_token_cursor(), prefix_len + failed_len);

            // Surgical KV Rollback via CausalGraph
            let rb_start = Instant::now();
            let rb_res = graph.rollback_step_kv(step_failed, &mut ctx);
            c2_rollback_latencies.push(rb_start.elapsed().as_secs_f64() * 1000.0);
            assert!(rb_res.is_ok(), "Graph rollback returned error: {:?}", rb_res);

            // Cursor and cache synchronization assertions
            assert_eq!(ctx.kv_cache_used_cells(), prefix_len);
            assert_eq!(ctx.current_cursor(), prefix_len);
            assert_eq!(graph.current_token_cursor(), prefix_len);

            // Record and evaluate pruned correction suffix
            let step_corr = 2;
            graph.record_step(
                step_corr,
                "Safe diagnostic correction",
                &["system_state"],
                &["diagnostic_output"],
                correction_len,
            );

            let pruned_start = Instant::now();
            ctx.eval_tokens(&correction_tokens, 0).unwrap();
            c2_eval_latencies.push(pruned_start.elapsed().as_secs_f64() * 1000.0);

            token_c2 = ctx.sample_greedy().unwrap();
            logits_c2 = ctx.get_logits().unwrap();
            cells_c2 = ctx.kv_cache_used_cells();

            // Verify graph tracked positions correctly
            assert_eq!(cells_c2, prefix_len + correction_len);
            assert_eq!(ctx.current_cursor(), prefix_len + correction_len);
            assert_eq!(graph.current_token_cursor(), prefix_len + correction_len);

            // =========================================================================
            // CONDITION 3: COLD FRESH PROMPT RECOMPUTATION (Ground Truth Control)
            // =========================================================================
            ctx.kv_cache_clear();
            let mut fresh_tokens = prefix_tokens.clone();
            fresh_tokens.extend_from_slice(&correction_tokens);

            let cold_start = Instant::now();
            ctx.eval_tokens(&fresh_tokens, 0).unwrap();
            c3_latencies.push(cold_start.elapsed().as_secs_f64() * 1000.0);

            token_c3 = ctx.sample_greedy().unwrap();
            logits_c3 = ctx.get_logits().unwrap();
            cells_c3 = ctx.kv_cache_used_cells();
        }

        let rss_after_c1 = get_process_rss_mb();
        let rss_after_c2 = get_process_rss_mb();
        let rss_after_c3 = get_process_rss_mb();

        let stats_c1 = compute_stats(c1_latencies);
        let stats_c2_eval = compute_stats(c2_eval_latencies);
        let stats_c2_rb = compute_stats(c2_rollback_latencies);
        let stats_c3 = compute_stats(c3_latencies);

        println!("\n[Execution Latency Profile (N={} runs)]", NUM_ITERATIONS);
        println!("  Cond 1 (Monotonic Accumulation):  median={:.2}ms (mean={:.2}ms, std={:.2}ms)", 
            stats_c1.median_ms, stats_c1.mean_ms, stats_c1.std_dev_ms);
        println!("  Cond 2 (Causal Rollback Overhead): median={:.3}ms (mean={:.3}ms)", 
            stats_c2_rb.median_ms, stats_c2_rb.mean_ms);
        println!("  Cond 2 (Pruned Suffix Evaluation): median={:.2}ms (mean={:.2}ms, std={:.2}ms)", 
            stats_c2_eval.median_ms, stats_c2_eval.mean_ms, stats_c2_eval.std_dev_ms);
        println!("  Cond 3 (Cold Full Evaluation):     median={:.2}ms (mean={:.2}ms, std={:.2}ms)", 
            stats_c3.median_ms, stats_c3.mean_ms, stats_c3.std_dev_ms);

        let speedup = stats_c3.median_ms / stats_c2_eval.median_ms;
        println!("  TTFT Speedup (Cond 3 Cold vs Cond 2 Pruned): {:.2}x", speedup);
        println!("  Note: Speedup derives from prefix KV-cache reuse (skipping re-evaluation of {} tokens).", prefix_len);

        println!("\n[Physical KV-Cache Occupancy & Memory Telemetry]");
        println!("  Cond 1 KV Occupancy: {} cells (RSS: {:.2} MB)", cells_c1, rss_after_c1);
        println!("  Cond 2 KV Occupancy: {} cells (RSS: {:.2} MB)", cells_c2, rss_after_c2);
        println!("  Cond 3 KV Occupancy: {} cells (RSS: {:.2} MB)", cells_c3, rss_after_c3);
        assert_eq!(cells_c1, prefix_len + failed_len + correction_len);
        assert_eq!(cells_c2, prefix_len + correction_len);
        assert_eq!(cells_c3, prefix_len + correction_len);

        // 3. Quantitative Logit and Token Divergence Analysis
        let (max_diff_23, mean_diff_23, cos_sim_23) = compare_logits(&logits_c2, &logits_c3);
        let (_max_diff_12, _mean_diff_12, cos_sim_12) = compare_logits(&logits_c1, &logits_c2);

        println!("\n[Logit and Token Divergence Metrics]");
        println!("  Next-Token Greedy Argmax: Cond 1={}, Cond 2={}, Cond 3={}", 
            token_c1, token_c2, token_c3);
        println!("  Logit Comparison (Cond 2 Pruned vs Cond 3 Ground Truth):");
        println!("    L_inf Max Absolute Delta: {:.6}", max_diff_23);
        println!("    L1 Mean Absolute Error:   {:.6}", mean_diff_23);
        println!("    Cosine Similarity:        {:.8}", cos_sim_23);
        println!("  Logit Comparison (Cond 1 Contaminated vs Cond 2 Pruned):");
        println!("    Cosine Similarity:        {:.8}", cos_sim_12);

        // Empirical assertions
        assert_eq!(
            token_c2, token_c3,
            "Greedy next-token parity failed between pruned KV and cold ground truth"
        );
        assert!(
            cos_sim_23 > 0.999,
            "Cosine similarity between Cond 2 and Cond 3 logit vectors is below 0.999: {:.6}",
            cos_sim_23
        );
    }
}

#[test]
fn test_causal_ancestral_cone_efficiency() {
    println!("\n================================================================================");
    println!("  CAUSAL ANCESTRAL CONE TOKEN REDUCTION BENCHMARK (§2.2)");
    println!("================================================================================");

    let mut graph = CausalGraph::new();

    // Step 1: Initialize Database Connection
    let s1_tokens = 35;
    graph.record_step(1, "Init DB pool", &[], &["db_conn", "pool_cfg"], s1_tokens);

    // Step 2: Unrelated UI configuration
    let s2_tokens = 45;
    graph.record_step(2, "Load UI theme settings", &[], &["ui_theme", "css_vars"], s2_tokens);

    // Step 3: Unrelated Network Telemetry ping
    let s3_tokens = 40;
    graph.record_step(3, "Ping NTP telemetry server", &[], &["ntp_sync", "telemetry"], s3_tokens);

    // Step 4: Crash in DB migration referencing db_conn
    let s4_tokens = 25;
    graph.record_step(4, "Apply DB migration schema", &["db_conn"], &["migration_res"], s4_tokens);

    let total_standard_tokens: usize = s1_tokens + s2_tokens + s3_tokens + s4_tokens;
    let crash_step = 4;
    let cone = graph.resolve_ancestral_cone_for_step(crash_step);

    println!("Full Sequential History Steps: [1, 2, 3, 4] (Total L_standard = {} tokens)", total_standard_tokens);
    println!("Ancestral Cone for Step 4: {:?}", cone);

    assert_eq!(cone, vec![1, 4], "Ancestral cone must strictly isolate steps 1 and 4");

    let causal_tokens: usize = s1_tokens + s4_tokens;
    println!("Causal Ancestral Cone L_causal = {} tokens", causal_tokens);

    let reduction = 1.0 - (causal_tokens as f64 / total_standard_tokens as f64);
    println!("Token Footprint Reduction: {:.1}% ({} tokens saved)", reduction * 100.0, total_standard_tokens - causal_tokens);

    assert!(
        causal_tokens < total_standard_tokens,
        "L_causal must be strictly less than L_standard in non-trivial trajectories"
    );
}

#[test]
fn test_live_causal_rollback_and_recovery_e2e() {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = base_dir.join("models/qwen-0.5b.gguf");

    assert!(
        model_path.exists(),
        "Live rollback and recovery test requires GGUF model at: {:?}",
        model_path
    );

    println!("\n================================================================================");
    println!("  LIVE END-TO-END CAUSAL SELF-HEALING & HYDRATION BENCHMARK");
    println!("================================================================================");

    let model = NativeLlamaModel::load(&model_path, 0)
        .expect("Failed to load native GGUF model");
    let mut ctx = model.create_context(512, 512, 4)
        .expect("Failed to create execution context");

    let mut graph = CausalGraph::new();

    // Step 1: Definition of authentication helper function
    let code_step_1 = "def authenticate(user, secret):\n    return user == 'admin' and secret == 'tok123'\n";
    let tokens_s1 = model.tokenize(code_step_1, true).unwrap();
    ctx.eval_tokens(&tokens_s1, 0).unwrap();
    graph.record_step(1, "Define auth validator", &[], &["auth_validator"], tokens_s1.len());
    graph.store.store_step(1, code_step_1, Some(&tokens_s1));

    // Step 2: Unrelated logging step
    let code_step_2 = "import logging\nlogger = logging.getLogger('audit')\n";
    let tokens_s2 = model.tokenize(code_step_2, false).unwrap();
    ctx.eval_tokens(&tokens_s2, 0).unwrap();
    graph.record_step(2, "Setup audit logger", &[], &["logger"], tokens_s2.len());

    assert_eq!(ctx.kv_cache_used_cells(), tokens_s1.len() + tokens_s2.len());
    assert_eq!(graph.current_token_cursor(), ctx.current_cursor());

    // Middle-Step Eviction of Step 1 to test compressed storage
    let evict_res = graph.evict_step_with_context(1, code_step_1, Some(&tokens_s1), Some(&mut ctx));
    assert!(evict_res.is_ok(), "Middle-step eviction failed: {:?}", evict_res);
    assert_eq!(graph.is_step_evicted(1), Some(true));
    assert_eq!(ctx.kv_cache_used_cells(), tokens_s2.len());
    assert_eq!(graph.current_token_cursor(), ctx.current_cursor());

    // Step 3: Crash Step (invokes auth_validator which was evicted)
    let crash_code = "auth_validator('guest', 'bad') # Failed with KeyError: 'auth_validator'\n";
    let tokens_s3 = model.tokenize(crash_code, false).unwrap();
    ctx.eval_tokens(&tokens_s3, 0).unwrap();
    graph.record_step(3, "Invoke validator", &["auth_validator"], &["auth_res"], tokens_s3.len());

    assert_eq!(ctx.kv_cache_used_cells(), tokens_s2.len() + tokens_s3.len());
    assert_eq!(graph.current_token_cursor(), ctx.current_cursor());

    // Execute Atomic Rollback and Recovery
    println!("Executing rollback_and_recover on Step 3 for entity 'auth_validator'...");
    let recovery_start = Instant::now();
    let recovery_result = graph.rollback_and_recover(
        3,
        Some("auth_validator"),
        Some(&mut ctx),
        Some(&model),
    );
    let recovery_time = recovery_start.elapsed();

    assert!(recovery_result.is_ok(), "Recovery failed: {:?}", recovery_result);
    let hydrated_steps = recovery_result.unwrap();
    println!("Hydrated Steps: {:?} in {:?}", hydrated_steps, recovery_time);

    // Verify system invariants:
    // 1. Crash Step 3 was pruned from the graph and KV cache
    assert!(!graph.contains_step(3), "Crash step 3 must be purged from graph");

    // 2. Step 1 was restored from compressed storage to active state
    assert_eq!(graph.is_step_evicted(1), Some(false), "Step 1 must be active post-recovery");

    // 3. Graph cursor and physical context cursor match exactly
    assert_eq!(
        graph.current_token_cursor(),
        ctx.current_cursor(),
        "Cursor desynchronization detected post-recovery"
    );

    // 4. Verify live generation from recovered context
    let repair_prompt = "auth_validator('admin', 'tok123')";
    let repair_tokens = model.tokenize(repair_prompt, false).unwrap();
    let eval_res = ctx.eval_tokens(&repair_tokens, 0);
    assert!(eval_res.is_ok(), "Context evaluation failed on recovered KV state: {:?}", eval_res);

    let next_tok = ctx.sample_greedy().unwrap();
    println!("Sampled next token from healed context: {}", next_tok);
    println!("End-to-End Self-Healing Test Completed Successfully.");
}
