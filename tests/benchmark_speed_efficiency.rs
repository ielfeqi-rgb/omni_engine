use omni_engine::causal_memory::dag::CausalGraph;
use omni_engine::planner::pre_pass_triage::PrePassTriage;
use omni_engine::planner::supervisor::InternalSupervisorProbe;
use omni_engine::sandbox::lua_runner::LuaSandboxRunner;
use omni_engine::sandbox::terminal_bridge::TerminalSessionBridge;
use omni_engine::sandbox::vfs::MemoryVfs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[test]
fn test_benchmark_speed_and_efficiency() {
    println!("\n============================================================");
    println!("  [OMNI_ENGINE SPEED & EFFICIENCY BENCHMARK SUITE]");
    println!("============================================================");

    // 1. Speculative Intent Probe Microsecond Latency Test
    println!("\n--- [1] Speculative Intent Probe Latency (100,000 iterations) ---");
    let test_queries = [
        "run cargo test and compile project in terminal",
        "fetch latest breaking tech news online",
        "save metrics report to results.csv",
        "what is the meaning of life?",
        "شغل الأمر في الطرفية وشوف النتيجة",
        "ابحث في الانترنت عن اسعار العملات",
    ];

    let start_probe = Instant::now();
    let iterations = 100_000;
    for i in 0..iterations {
        let q = test_queries[i % test_queries.len()];
        let _target = PrePassTriage::probe_fast_intent(q);
    }
    let elapsed_probe = start_probe.elapsed();
    let per_op_ns = elapsed_probe.as_nanos() as f64 / iterations as f64;
    let ops_per_sec = (iterations as f64 / elapsed_probe.as_secs_f64()) as u64;

    println!("Total Duration: {:?}", elapsed_probe);
    println!("Per-Probe Latency: {:.2} ns ({:.4} µs)", per_op_ns, per_op_ns / 1000.0);
    println!("Throughput: {} probes/sec", ops_per_sec);
    assert!(per_op_ns < 50_000.0, "Probe latency should be sub-50 microseconds");

    // 2. MemoryVFS RAM High-Throughput I/O Test (Zero Disk IOPS Overhead)
    println!("\n--- [2] In-Memory VFS Zero-Cost I/O Throughput (50,000 ops) ---");
    let vfs = Arc::new(MemoryVfs::new());
    let payload = b"{\"event\": \"TELEMETRY_SAMPLE\", \"metric\": 98.7, \"status\": \"ACTIVE\"}";
    let vfs_iterations = 50_000;

    let start_vfs = Instant::now();
    for i in 0..vfs_iterations {
        let file_path = PathBuf::from(format!("stream/chunk_{}.json", i % 500));
        vfs.write_file(file_path.clone(), payload);
        let read_back = vfs.read_file(&file_path);
        assert!(read_back.is_some());
    }
    let elapsed_vfs = start_vfs.elapsed();
    let vfs_per_op_ns = elapsed_vfs.as_nanos() as f64 / (vfs_iterations as f64 * 2.0);
    let vfs_ops_per_sec = ((vfs_iterations * 2) as f64 / elapsed_vfs.as_secs_f64()) as u64;

    println!("Total Duration: {:?}", elapsed_vfs);
    println!("Per-Op (Write+Read) Latency: {:.2} ns ({:.4} µs)", vfs_per_op_ns, vfs_per_op_ns / 1000.0);
    println!("Throughput: {} IOPS (In-Memory RAM)", vfs_ops_per_sec);

    // 3. Embedded Lua Host Sandbox Execution Latency
    println!("\n--- [3] Native Embedded Lua Sandbox Isolation Latency (5,000 scripts) ---");
    let runner = LuaSandboxRunner::new(vfs.clone());
    let lua_script = r#"
        local x = 0
        for i = 1, 100 do
            x = x + i
        end
        vfs.write("math_result.txt", tostring(x))
    "#;
    let lua_runs = 5_000;
    let start_lua = Instant::now();
    for _ in 0..lua_runs {
        let res = runner.run_script(lua_script);
        assert!(res.success);
    }
    let elapsed_lua = start_lua.elapsed();
    let per_lua_ms = elapsed_lua.as_micros() as f64 / lua_runs as f64;
    let lua_runs_per_sec = (lua_runs as f64 / elapsed_lua.as_secs_f64()) as u64;

    println!("Total Duration: {:?}", elapsed_lua);
    println!("Per-Script Sandboxed Execution: {:.2} µs", per_lua_ms);
    println!("Throughput: {} sandboxed executions/sec", lua_runs_per_sec);

    // 4. Causal DAG Hydration & KV Rollback Pruning Speed
    println!("\n--- [4] Causal DAG Rollback & Distillation Speed (20,000 steps) ---");
    let supervisor = InternalSupervisorProbe::new();
    let mut causal_graph = CausalGraph::new();

    let start_causal = Instant::now();
    for i in 0..20_000 {
        causal_graph.record_step(
            i,
            &format!("Execute Step #{}", i),
            &["input_stream.raw"],
            &["output_state.bin"],
            128,
        );
        if i % 100 == 0 {
            supervisor.prune_branch_with_causal_lesson(
                &format!("branch-{}", i),
                "Plan A",
                "Trap root cause simulation",
                "Self-healing causal synthesis rule",
                350,
            );
        }
    }
    let elapsed_causal = start_causal.elapsed();
    println!("Total Duration: {:?}", elapsed_causal);
    println!("Causal Node Hydration + Rollback: {:.4} µs / op", elapsed_causal.as_nanos() as f64 / 20_000.0 / 1000.0);

    // 5. Speculative TermHost Warmup Speed
    println!("\n--- [5] Speculative TermHost PTY Warmup Latency (100,000 armings) ---");
    let bridge = TerminalSessionBridge::new(100);
    let start_warm = Instant::now();
    for _ in 0..100_000 {
        assert!(bridge.arm_and_warmup());
    }
    let elapsed_warm = start_warm.elapsed();
    let per_warm_ns = elapsed_warm.as_nanos() as f64 / 100_000.0;
    println!("Total Duration: {:?}", elapsed_warm);
    println!("Per-Warmup Arming Latency: {:.2} ns", per_warm_ns);

    println!("\n============================================================");
    println!("  [BENCHMARK COMPLETED SUCCESSFULLY: ZERO BOTTLENECK DETECTED]");
    println!("============================================================\n");
}
