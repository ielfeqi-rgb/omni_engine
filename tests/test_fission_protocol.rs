use omni_engine::native_llama::{NativeLlamaModel, NativeLlamaSampler};
use omni_engine::planner::supervisor::{DynamicThresholds, EntropyMonitor};
use std::path::PathBuf;

#[test]
fn test_live_fission_experiment() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let model_path = manifest_dir.join("models").join("qwen2.5-1.5b.gguf");
    if !model_path.exists() {
        println!("Skipping: Model not found at {:?}", model_path);
        return;
    }

    println!("\x1b[36;1m=== [OMNI ENGINE: LIVE FISSION PROTOCOL EXPERIMENT] ===\x1b[0m");
    let model = NativeLlamaModel::load(&model_path, 0).unwrap();
    let mut ctx = model.create_context(1024, 1024, 4).unwrap();
    let mut sampler = NativeLlamaSampler::greedy(model.n_vocab()).unwrap();
    
    // We create a complex multi-file prompt that simulates a complex project
    let prompt = r#"# File: backend/server.py
from flask import Flask, jsonify

app = Flask(__name__)

@app.route('/api/v1/status')
def status():
    # Return healthy status json
    return jsonify({"status": "ok"})

# File: backend/database.py
import sqlite3

def connect():
    conn = sqlite3.connect('test.db')
    return conn

# File: frontend/app.js
// Fetch status and render
async function checkStatus() {
    let res = await fetch('/api/v1/status');
    let data = await res.json();
    if (data.status === 'ok') {
        console.log("Server is healthy!");
    } else {
"#;

    let mut thresholds = DynamicThresholds::default();
    thresholds.z_score_trigger = 1.2;  // Lowered for demonstration to force trigger
    thresholds.bifurcation_margin = 0.5; // Very loose to force trigger
    
    let mut monitor = EntropyMonitor::new(thresholds);
    let tokens = model.tokenize(prompt, true).unwrap();
    ctx.eval_tokens(&tokens, 0).unwrap();
    
    println!("\x1b[32m[CONTEXT LOADED] Evaluating multi-file AST context ({} tokens)...\x1b[0m\n", tokens.len());
    
    let mut generated_text = String::new();
    let mut cursor_before_generation = tokens.len();
    
    let mut i = 0;
    let mut fissions_triggered = 0;
    
    while i < 25 {
        let ent_res = ctx.compute_live_entropy().unwrap();
        
        if let Some(msg) = monitor.analyze_fission(ent_res.entropy, ent_res.top_1_prob, ent_res.top_2_prob) {
            println!("\n{}", msg);
            println!("\x1b[33m[DECISION] The engine hesitated. Current generation looks uncertain.\x1b[0m");
            println!("\x1b[33m[SURGERY] Rolling back the last 5 tokens of hesitation...\x1b[0m");
            
            let p0 = (cursor_before_generation + i).saturating_sub(5).max(4);
            let p1 = cursor_before_generation + i;
            ctx.rollback_range(0, p0 as i32, p1 as i32).unwrap();
            
            println!("\x1b[32;1m[DAG] Branched! Resuming generation cleanly from stable state...\x1b[0m\n");
            
            cursor_before_generation = ctx.kv_cache_token_count();
            i = 0;
            fissions_triggered += 1;
            monitor.history.clear(); // Reset monitor history after fission
            
            if fissions_triggered >= 2 {
                println!("\n\x1b[36;1m=== [EXPERIMENT SUCCESSFUL: MULTIPLE FISSIONS HANDLED WITHOUT CRASH] ===\x1b[0m");
                return;
            }
        }
        
        monitor.push_entropy(ent_res.entropy);
        
        let token = ctx.sample(&mut sampler).unwrap();
        sampler.accept(token);
        ctx.eval_tokens(&[token], 0).unwrap();
        
        let piece = model.token_to_piece(token).unwrap();
        print!("{}", piece);
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        
        generated_text.push_str(&piece);
        i += 1;
    }
    
    println!("\n\n\x1b[36;1m=== [EXPERIMENT FINISHED] ===\x1b[0m");
    if fissions_triggered == 0 {
        println!("Note: Model was very confident, no fissions triggered. Lower thresholds if testing is desired.");
    }
}
