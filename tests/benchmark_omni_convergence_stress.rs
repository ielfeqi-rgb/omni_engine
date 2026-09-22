use omni_engine::auth::KeyManager;
use omni_engine::causal_memory::dag::CausalGraph;
use omni_engine::planner::pre_pass_triage::{PrePassTriage, SpeculativeTarget};
use omni_engine::planner::supervisor::InternalSupervisorProbe;
use omni_engine::sandbox::browser_lens::{ActionTargetType, BrowserTerminalLens, InteractiveElement};
use omni_engine::sandbox::lua_runner::LuaSandboxRunner;
use omni_engine::sandbox::terminal_bridge::TerminalSessionBridge;
use omni_engine::sandbox::vfs::MemoryVfs;
use omni_engine::sandbox::WasmPrimitiveSandbox;
use omni_engine::system_info::get_system_specs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_omni_convergence_complete_stress_suite() {
    println!("\n================================================================================");
    println!("  [OMNI CONVERGENCE HARDCORE STRESS SUITE: INTERNAL & EXTERNAL RESILIENCE]");
    println!("================================================================================");

    let start_all = Instant::now();

    // 1. Hardware & Operating Environment Telemetry
    println!("\n--- [SYSTEM DISCOVERY]: Probing Hardware Substrate ---");
    let specs = get_system_specs();
    println!("  CPU Cores Detected: {}", specs.cpu_cores);
    println!("  Total RAM: {:.2} GB, Free RAM: {:.2} GB", specs.total_ram_gb, specs.free_ram_gb);
    println!("  Model Recommendation: {} ({})", specs.recommended_params, specs.max_recommended_size);
    assert!(specs.cpu_cores >= 1);

    // 2. Auth & Multi-Tenant Cryptographic Key Validation (10,000 checks)
    println!("\n--- [AUTH LAYER]: High-Frequency API Key Verification (10,000 requests) ---");
    let key_mgr = KeyManager::new(PathBuf::from("/tmp/omni_stress_keys"));
    let valid_auth_count = Arc::new(AtomicUsize::new(0));

    let key_mgr_clone = key_mgr.clone();
    let counter_clone = valid_auth_count.clone();

    let start_auth = Instant::now();
    let auth_handles: Vec<_> = (0..4)
        .map(|worker_id| {
            let km = key_mgr_clone.clone();
            let cnt = counter_clone.clone();
            tokio::spawn(async move {
                for i in 0..2500 {
                    let test_token = if (i + worker_id) % 2 == 0 {
                        "Bearer omni_sk_invalid_token_attempt"
                    } else {
                        "Bearer omni_sk_invalid_test"
                    };
                    if !km.validate_key(test_token) {
                        cnt.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for h in auth_handles {
        h.await.unwrap();
    }
    println!("  Auth verification completed: {} checks in {:?}", valid_auth_count.load(Ordering::SeqCst), start_auth.elapsed());

    // 3. Pure WASM Stack Machine Sandboxed Execution Stress
    println!("\n--- [WASM SANDBOX]: Multi-Module Pure Computation Stress ---");
    let wasm_sandbox = WasmPrimitiveSandbox::new();
    let wasm_add_bytes: [u8; 41] = [
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7e,
        0x03, 0x02, 0x01, 0x00,
        0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
        0x0a, 0x09, 0x01, 0x07, 0x00,
        0x20, 0x00, 0x20, 0x01, 0x7c, 0x0b
    ];
    let outcome = wasm_sandbox.execute_pure_wasm(&wasm_add_bytes, "add", &[250_000, 250_000]);
    println!("  WASM Add(250000, 250000) Outcome: Success={}, Return={:?}, Time={}µs", outcome.success, outcome.return_value, outcome.duration_us);
    assert_eq!(outcome.return_value, Some(500_000));

    // 4. 2D Terminal ASCII Browser Lens Canvas Rendering (1,000 interactive pages)
    println!("\n--- [BROWSER LENS]: High-Frequency 2D Projection & Action Resolution ---");
    let lens = BrowserTerminalLens::new(88, 12);
    let start_lens = Instant::now();
    for i in 0..1_000 {
        let elements = vec![
            InteractiveElement {
                id: 1,
                target_type: ActionTargetType::Input,
                label: "Search Domain".to_string(),
                selector: "#search".to_string(),
                current_value: Some(format!("query_{}", i)),
            },
            InteractiveElement {
                id: 2,
                target_type: ActionTargetType::Button,
                label: "Submit Order".to_string(),
                selector: "#btn-submit".to_string(),
                current_value: None,
            },
        ];
        let proj = lens.project_canvas("Stress Page", "https://omni.internal", elements);
        assert!(proj.ascii_grid.contains("OMNI BROWSER LENS"));
    }
    println!("  1,000 ASCII Browser Views rendered in {:?}", start_lens.elapsed());

    // 5. Host Terminal Bridge Asynchronous PTY Ring Buffer Stress
    println!("\n--- [TERMINAL BRIDGE]: Real Host Shell Subprocess & Output Streaming ---");
    let term_bridge = Arc::new(TerminalSessionBridge::new(200));
    assert!(term_bridge.arm_and_warmup());

    let (job_id, _) = term_bridge.execute("for i in $(seq 1 30); do echo \"TELEMETRY_BEAT_$i\"; done");
    let report = term_bridge
        .wait_and_inspect(job_id, std::time::Duration::from_secs(10))
        .expect("Terminal job failed");
    println!("  Terminal Job #{} Status: {:?}, Lines Captured: {}", job_id, report.status, report.stdout_lines.len());
    assert!(report.is_success);
    assert!(report.stdout_lines.iter().any(|l| l.contains("TELEMETRY_BEAT_30")));

    // 6. Complete End-to-End Evolutionary AI: Causal Traps -> Rollback -> Apoptosis -> Ancestral Reincarnation
    println!("\n--- [EVOLUTIONARY COGNITION]: Multi-Trap Apoptosis & Rebirth Loop ---");
    let vfs = Arc::new(MemoryVfs::new());
    let lua_runner = LuaSandboxRunner::with_terminal(vfs.clone(), term_bridge.clone());
    let supervisor = InternalSupervisorProbe::new();
    let mut causal_graph = CausalGraph::new();

    // Speculative Intent Probe Check
    let query = "compile rust engine in terminal and save result to report.txt";
    let intent_target = PrePassTriage::probe_fast_intent(query);
    println!("  Speculative Intent for query '{}' -> {:?}", query, intent_target);
    assert_eq!(intent_target, SpeculativeTarget::TermHost);

    // Generation 1: Executes 3 attempts that trap in the sandbox
    let gen1_traps = [
        "local invalid = nil; return #invalid",
        "web.unknown_protocol_call()",
        "error('Recursive stack overflow in hypothesis 3')",
    ];

    for (idx, trap_script) in gen1_traps.iter().enumerate() {
        causal_graph.record_step(idx, &format!("Gen1 Hypothesis #{}", idx + 1), &[], &["state.tmp"], 150);
        let res = lua_runner.run_script(trap_script);
        assert!(!res.success);

        let lesson = supervisor.prune_branch_with_causal_lesson(
            &format!("gen1-hypo-{}", idx + 1),
            "Hypothesis Trapped",
            res.error.as_ref().unwrap(),
            "Do not repeat trapped syntax/API",
            300,
        );
        println!("  Gen 1 Step #{} Trapped -> Purged {} tokens, Distilled Lesson #{}", idx + 1, lesson.active_tokens_saved, lesson.lesson_id);
    }

    // Trigger Epistemic Apoptosis: Wipe the rotten generation
    println!("\n  [APOPTOSIS]: Generation 1 failed 3 times. Triggering programmed cell death...");
    let testament_stream = r#"
        - CRITICAL: Never call undefined global functions
        - RULE: Pipe valid string payloads into vfs.write directly
        - LAW: Verify terminal exit status before proceeding
    "#;
    let testament = supervisor.trigger_epistemic_apoptosis(testament_stream);
    println!("  Gen 1 Wiped! Ancestral raw testament tokens preserved: {} chars", testament.testament_tokens_raw.len());

    // Generation 2 Rebirth: Spawns with fresh slate and ancestral rules
    causal_graph.record_step(100, "Generation 2 Epigenetic Rebirth", &["ancestral_testament"], &["final_report.txt"], 50);
    let gen2_safe_script = r#"
        local final_message = "OMNI_ENGINE_STRESS_TEST_PASSED: Complete convergence established."
        vfs.write("final_report.txt", final_message)
        print(final_message)
    "#;
    let gen2_res = lua_runner.run_script(gen2_safe_script);
    assert!(gen2_res.success);

    let final_content = vfs.read_string(PathBuf::from("final_report.txt")).expect("Final report must exist");
    println!("  Generation 2 Proof in VFS: '{}'", final_content);
    assert!(final_content.contains("OMNI_ENGINE_STRESS_TEST_PASSED"));

    println!("\n================================================================================");
    println!("  [CONVERGENCE VERDICT]: ALL INTERNAL & EXTERNAL MODULES STOOD ROCK-SOLID!");
    println!("  Total Suite Duration: {:?}", start_all.elapsed());
    println!("================================================================================\n");
}
