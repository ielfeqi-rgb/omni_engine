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

use omni_engine::native_llama::NativeLlamaModel;
use std::time::Instant;

#[derive(Debug)]
struct BenchArgs {
    model: String,
    iterations: usize,
    base_prefix: usize,
    wrong_suffix: usize,
    new_suffix: usize,
}

#[derive(Default, Debug)]
struct Stat {
    times: Vec<f64>,
}
impl Stat {
    fn add(&mut self, val: f64) {
        self.times.push(val);
    }
    fn mean(&self) -> f64 {
        if self.times.is_empty() { return 0.0; }
        self.times.iter().sum::<f64>() / self.times.len() as f64
    }
    fn std_dev(&self) -> f64 {
        if self.times.is_empty() { return 0.0; }
        let m = self.mean();
        let variance = self.times.iter().map(|value| {
            let diff = m - (*value as f64);
            diff * diff
        }).sum::<f64>() / self.times.len() as f64;
        variance.sqrt()
    }
    fn min(&self) -> f64 {
        *self.times.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.0)
    }
    fn max(&self) -> f64 {
        *self.times.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(&0.0)
    }
}

fn generate_dummy_tokens(count: usize, start: i32) -> Vec<i32> {
    (0..count).map(|i| start + (i as i32 % 100)).collect()
}

fn main() {
    let args_vec: Vec<String> = std::env::args().collect();
    let mut model_path = String::new();
    let mut iterations = 30;
    
    let mut i = 1;
    while i < args_vec.len() {
        if args_vec[i] == "--model" && i + 1 < args_vec.len() {
            model_path = args_vec[i+1].clone();
            i += 2;
        } else if args_vec[i] == "--iterations" && i + 1 < args_vec.len() {
            iterations = args_vec[i+1].parse().unwrap_or(30);
            i += 2;
        } else {
            i += 1;
        }
    }

    if model_path.is_empty() {
        eprintln!("Usage: omni_bench --model <PATH> [--iterations <NUM>]");
        std::process::exit(1);
    }

    let args = BenchArgs {
        model: model_path,
        iterations,
        base_prefix: 256,
        wrong_suffix: 128,
        new_suffix: 128,
    };

    println!("🧪 Omni Engine - Scientific Benchmark Suite");
    println!("Model: {}", args.model);
    println!("Iterations: {}", args.iterations);
    println!("Prefix: {} | Wrong Suffix: {} | New Suffix: {}", args.base_prefix, args.wrong_suffix, args.new_suffix);
    println!("--------------------------------------------------");

    let model = NativeLlamaModel::load(&args.model, 99).expect("Failed to load model");
    
    let mut stat_monotonic = Stat::default();
    let mut stat_rollback = Stat::default();
    let mut stat_cold = Stat::default();
    
    let prefix_tokens = generate_dummy_tokens(args.base_prefix, 100);
    let wrong_tokens = generate_dummy_tokens(args.wrong_suffix, 200);
    let new_tokens = generate_dummy_tokens(args.new_suffix, 300);

    for iter in 1..=args.iterations {
        let n_batch = args.base_prefix + args.wrong_suffix + args.new_suffix + 128;
        let n_ctx = n_batch * 2;
        
        {
            let mut ctx = model.create_context(n_ctx, n_batch, 4).unwrap();
            ctx.eval_tokens(&prefix_tokens, 0).unwrap();
            let start = Instant::now();
            ctx.eval_tokens(&new_tokens, prefix_tokens.len() as i32).unwrap();
            stat_monotonic.add(start.elapsed().as_secs_f64() * 1000.0);
        }
        
        {
            let mut ctx = model.create_context(n_ctx, n_batch, 4).unwrap();
            ctx.eval_tokens(&prefix_tokens, 0).unwrap();
            ctx.eval_tokens(&wrong_tokens, prefix_tokens.len() as i32).unwrap();
            let start = Instant::now();
            let _ = ctx.kv_cache_seq_rm(0, prefix_tokens.len() as i32, -1);
            ctx.eval_tokens(&new_tokens, prefix_tokens.len() as i32).unwrap();
            stat_rollback.add(start.elapsed().as_secs_f64() * 1000.0);
        }
        
        {
            let mut ctx = model.create_context(n_ctx, n_batch, 4).unwrap();
            let mut combined = prefix_tokens.clone();
            combined.extend(new_tokens.clone());
            let start = Instant::now();
            ctx.eval_tokens(&combined, 0).unwrap();
            stat_cold.add(start.elapsed().as_secs_f64() * 1000.0);
        }
        
        if iter % 5 == 0 {
            println!("Completed {}/{} iterations...", iter, args.iterations);
        }
    }

    println!("\n📊 BENCHMARK RESULTS (N={})", args.iterations);
    println!("| Condition | Mean (ms) | StdDev (ms) | Min (ms) | Max (ms) |");
    println!("|-----------|-----------|-------------|----------|----------|");
    println!("| Monotonic (Ideal)   | {:.2} | {:.2} | {:.2} | {:.2} |", stat_monotonic.mean(), stat_monotonic.std_dev(), stat_monotonic.min(), stat_monotonic.max());
    println!("| Causal Rollback     | {:.2} | {:.2} | {:.2} | {:.2} |", stat_rollback.mean(), stat_rollback.std_dev(), stat_rollback.min(), stat_rollback.max());
    println!("| Cold Restart (Trad) | {:.2} | {:.2} | {:.2} | {:.2} |", stat_cold.mean(), stat_cold.std_dev(), stat_cold.min(), stat_cold.max());
    
    let speedup = stat_cold.mean() / stat_rollback.mean();
    let overhead = ((stat_rollback.mean() / stat_monotonic.mean()) - 1.0) * 100.0;
    
    println!("\n🚀 CONCLUSIONS:");
    println!("- Speedup over Cold Restart: {:.2}x", speedup);
    println!("- Hardware Overhead (vs Monotonic): {:.2}%", overhead);
}
