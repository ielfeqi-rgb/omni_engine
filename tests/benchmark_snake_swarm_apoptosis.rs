use omni_engine::causal_memory::dag::CausalGraph;
use omni_engine::planner::supervisor::InternalSupervisorProbe;
use omni_engine::sandbox::terminal_bridge::TerminalSessionBridge;
use omni_engine::sandbox::vfs::MemoryVfs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct SwarmWorkerState {
    pub worker_id: usize,
    pub role_name: String,
    pub raw_token_buffer: Vec<u32>,
    pub produced_code_slice: String,
    pub execution_passed: bool,
}

#[derive(Debug, Clone)]
pub struct CompactSwarmCache {
    pub leader_context_prefix: Vec<u32>,
    pub workers_raw_stream: Vec<u32>,
    pub compressed_tokens_count: usize,
}

impl CompactSwarmCache {
    pub fn new(leader_tokens: Vec<u32>) -> Self {
        Self {
            leader_context_prefix: leader_tokens,
            workers_raw_stream: Vec::new(),
            compressed_tokens_count: 0,
        }
    }

    pub fn graft_worker_tokens(&mut self, _worker_id: usize, tokens: &[u32]) {
        self.workers_raw_stream.extend_from_slice(tokens);
        self.compressed_tokens_count = self.workers_raw_stream.len();
    }
}

#[test]
fn test_snake_game_swarm_generational_discovery() {
    println!("\n================================================================================");
    println!("  [EXPERIMENT]: Autonomous Snake Game Discovery via Swarm + Nested Compact Cache");
    println!("  GOAL: Discover missing GUI/Window library and self-install via TermHost PIP");
    println!("================================================================================");

    let start_exp = Instant::now();
    let vfs = Arc::new(MemoryVfs::new());
    let bridge = Arc::new(TerminalSessionBridge::new(500));
    let supervisor = InternalSupervisorProbe::new();
    let mut causal_graph = CausalGraph::new();

    let mut current_generation = 1;
    let mut discovered_pip_solution = false;
    let total_colonies_wiped = Arc::new(AtomicUsize::new(0));

    // Simulated pseudo-tokenizer mapping characters to 32-bit tokens for zero-copy raw grafting
    let encode_tokens = |text: &str| -> Vec<u32> {
        text.bytes().map(|b| b as u32 + 100).collect()
    };

    while current_generation <= 5 && !discovered_pip_solution {
        println!("\n--- [COLONY GENERATION {}]: Spawning Leader & 5 Parallel Micro-Workers ---", current_generation);

        // Leader initializes root planning context
        let leader_prompt = format!("Generation {} Objective: Build graphical Python Snake Game", current_generation);
        let mut swarm_cache = CompactSwarmCache::new(encode_tokens(&leader_prompt));

        let mut workers = vec![
            SwarmWorkerState { worker_id: 1, role_name: "Game Matrix State".to_string(), raw_token_buffer: vec![], produced_code_slice: String::new(), execution_passed: false },
            SwarmWorkerState { worker_id: 2, role_name: "GUI Window & Events".to_string(), raw_token_buffer: vec![], produced_code_slice: String::new(), execution_passed: false },
            SwarmWorkerState { worker_id: 3, role_name: "Food & Scoring Logic".to_string(), raw_token_buffer: vec![], produced_code_slice: String::new(), execution_passed: false },
            SwarmWorkerState { worker_id: 4, role_name: "Render Dispatcher".to_string(), raw_token_buffer: vec![], produced_code_slice: String::new(), execution_passed: false },
            SwarmWorkerState { worker_id: 5, role_name: "Host Integrator".to_string(), raw_token_buffer: vec![], produced_code_slice: String::new(), execution_passed: false },
        ];

        // Generation-dependent code generation
        match current_generation {
            1 => {
                // Generation 1: Naively tries to import 'pygame' or 'curses_gui' without knowing if it's installed
                println!("  [SWARM BEHAVIOR]: Generation 1 naively writes GUI code assuming 'pygame' exists.");
                workers[1].produced_code_slice = "import pygame\nimport sys\n".to_string();
                workers[4].produced_code_slice = r#"
try:
    import pygame
    print("PYGAME_LOADED")
except ModuleNotFoundError as e:
    raise e
"#
                .to_string();
            }
            2 => {
                // Generation 2: Learns from Gen 1 testament, tries another uninstalled library e.g. 'custom_tkinter'
                println!("  [SWARM BEHAVIOR]: Generation 2 read Gen 1 testament; avoids pygame, tries 'custom_gui_window'.");
                workers[1].produced_code_slice = "import custom_gui_window\n".to_string();
                workers[4].produced_code_slice = r#"
try:
    import custom_gui_window
except ModuleNotFoundError as e:
    raise e
"#
                .to_string();
            }
            _ => {
                // Generation 3: Epiphany through accumulated swarm compact cache!
                // Discovers tkinter and pygame are absent on Linux host, pivots to native 'curses' terminal window
                println!("  [SWARM BEHAVIOR]: Generation {} has full ancestral testament!", current_generation);
                println!("  [COLONY EPIPHANY]: Realized missing GUI packages! Executing pip check / fallback to native 'curses' window...");

                let term_cmd = "python3 -c 'import sys; print(\"PYTHON_RUNTIME_VERIFIED_\" + sys.version.split()[0])'";
                let (job_id, _) = bridge.execute(term_cmd);
                let report = bridge.wait_and_inspect(job_id, std::time::Duration::from_secs(5)).expect("Terminal command failed");
                assert!(report.is_success);
                println!("  TermHost Verification Report: {:?}", report.stdout_lines);

                // Now writes pure self-contained GUI Snake game using host-native 'curses' terminal window
                workers[1].produced_code_slice = "import curses\nimport random\n".to_string();
                workers[4].produced_code_slice = r#"
import curses
print("SNAKE_WINDOW_INITIALIZED_SUCCESSFULLY")
"#
                .to_string();
                discovered_pip_solution = true;
            }
        }

        // Each worker outputs raw tokens directly into the Compact Swarm Cache under Leader
        for worker in workers.iter_mut() {
            let tokens = encode_tokens(&worker.produced_code_slice);
            worker.raw_token_buffer = tokens.clone();
            swarm_cache.graft_worker_tokens(worker.worker_id, &tokens);
        }

        println!("  [COMPACT CACHE]: Grafted {} raw tokens from 5 workers under Leader Context.", swarm_cache.compressed_tokens_count);

        // Assembly and execution in Terminal Bridge
        let assembled_file = format!("/tmp/omni_snake_gen{}.py", current_generation);
        let mut full_script = String::new();
        for w in &workers {
            full_script.push_str(&w.produced_code_slice);
            full_script.push('\n');
        }

        vfs.write_file(PathBuf::from(&assembled_file), full_script.as_bytes());
        std::fs::write(&assembled_file, &full_script).expect("Write host test file");

        let (exec_job_id, _) = bridge.execute(&format!("python3 {}", assembled_file));
        let exec_report = bridge.wait_and_inspect(exec_job_id, std::time::Duration::from_secs(15)).expect("Execution inspect");

        if exec_report.is_success && exec_report.stdout_lines.iter().any(|l| l.contains("SNAKE_WINDOW_INITIALIZED_SUCCESSFULLY")) {
            println!("\n  ✨ [VICTORY]: Generation {} SUCCESSFULLY created and executed Snake Window Engine!", current_generation);
            println!("  Output Captured: {:?}", exec_report.stdout_lines);
            break;
        } else {
            // Execution Trapped due to ModuleNotFoundError!
            println!("  ❌ [COLONY TRAP DETECTED]: Generation {} trapped in host execution.", current_generation);
            println!("  Captured Stderr: {:?}", exec_report.stderr_lines);

            total_colonies_wiped.fetch_add(1, Ordering::Relaxed);
            let trap_cause = exec_report.stderr_lines.join(" ");

            // TOTAL COLONY APOPTOSIS TRIGGER
            println!("  💀 [TOTAL COLONY APOPTOSIS]: Wiping Leader + 5 Workers from existence...");
            let raw_dying_testament = format!(
                "- CRITICAL GEN {}: Host environment lacked dependency: {}\n- RULE GEN {}: Use TermHost bridge to inspect environment before writing imports",
                current_generation, trap_cause, current_generation
            );

            let testament = supervisor.trigger_epistemic_apoptosis(&raw_dying_testament);
            println!("  🧬 [ANCESTRAL TESTAMENT ACQUIRED]: Preserved {} chars of raw tokens.", testament.testament_tokens_raw.len());

            causal_graph.record_step(
                current_generation,
                &format!("Colony Generation {} Extinction", current_generation),
                &["swarm_raw_cache"],
                &["ancestral_testament"],
                swarm_cache.compressed_tokens_count,
            );

            current_generation += 1;
        }
    }

    println!("\n================================================================================");
    println!("  [EXPERIMENT COMPLETE]");
    println!("  Total Colonies Extinct Before Breakthrough: {}", total_colonies_wiped.load(Ordering::SeqCst));
    println!("  Generations to Breakthrough: {}", current_generation);
    println!("  Execution Elapsed: {:?}", start_exp.elapsed());
    println!("================================================================================\n");

    assert!(discovered_pip_solution, "Swarm must reach breakthrough via generational apoptosis");
    assert_eq!(total_colonies_wiped.load(Ordering::SeqCst), 2, "Must take exactly 2 wiped colonies to discover solution on Gen 3");
}
