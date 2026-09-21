use crate::auth::KeyManager;
use crate::llama_manager::LlamaManager;
use crate::openai_api::{ChatCompletionRequest, ChatMessage};
use crate::system_info::get_system_specs;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

pub async fn handle_cli(args: &[String], base_dir: &PathBuf) -> Result<bool, Box<dyn std::error::Error>> {
    if args.len() <= 1 {
        // No CLI subcommand passed, run web server
        return Ok(false);
    }

    let cmd = args[1].as_str();

    match cmd {
        "-h" | "--help" | "help" => {
            print_help();
            Ok(true)
        }
        "-v" | "--version" | "version" => {
            println!("omni_engine v1.0.1 (Rust Standalone)");
            Ok(true)
        }
        "status" => {
            print_status(&base_dir);
            Ok(true)
        }
        "models" => {
            print_models(&base_dir);
            Ok(true)
        }
        "start" => {
            handle_start(&args[2..], &base_dir);
            Ok(true)
        }
        "stop" => {
            handle_stop(&base_dir);
            Ok(true)
        }
        "ask" => {
            handle_ask(&args[2..], &base_dir).await;
            Ok(true)
        }
        "chat" => {
            handle_chat(&args[2..], &base_dir).await;
            Ok(true)
        }
        "keys" => {
            handle_keys(&args[2..], &base_dir);
            Ok(true)
        }
        "serve" => {
            // User explicitly wants to serve web UI / API
            Ok(false)
        }
        unknown => {
            eprintln!("❌ Unknown command: '{}'", unknown);
            eprintln!("Run 'omni_engine --help' for a list of available commands.");
            Ok(true)
        }
    }
}

pub fn print_help() {
    println!(r#"
================================================================================
   🚀 OMNI AI ENGINE - Terminal CLI & Engine Controller
================================================================================
USAGE:
    omni_engine [COMMAND] [OPTIONS]

COMMANDS:
    status                      Display system hardware specs & llama-server status
    models                      List all installed GGUF models in models/ directory
    start <model> [OPTIONS]     Start llama-server with the specified GGUF model
    stop                        Stop the currently running llama-server
    ask "<prompt>" [OPTIONS]    Send a single prompt to the running engine and print reply
    chat                        Start an interactive multi-turn chat session in the terminal
    keys <subcommand>           Manage API keys (list, new, revoke)
    serve [OPTIONS]             Launch the full Web UI and OpenAI HTTP proxy server

OPTIONS FOR 'start':
    --port <PORT>               Port for llama-server (default: 8081)
    --threads <N>               CPU threads to allocate (default: physical cores)
    --ctx <SIZE>                Context window size in tokens (default: 2048)

OPTIONS FOR 'ask':
    --port <PORT>               Target llama-server port (default: 8081)
    --temp <FLOAT>              Temperature between 0.0 and 1.0 (default: 0.7)
    --max-tokens <N>            Maximum output tokens (default: 1024)

KEYS SUBCOMMANDS:
    keys list                   List all active API keys
    keys new <name>             Generate a new API key with a descriptive label
    keys revoke <id>            Revoke and remove an existing API key by ID

GENERAL OPTIONS:
    -h, --help                  Show this help guide
    -v, --version               Show engine version

EXAMPLES:
    omni_engine status
    omni_engine models
    omni_engine start qwen-0.5b.gguf --ctx 2048
    omni_engine ask "What is quantum computing?"
    omni_engine chat
    omni_engine stop
    omni_engine serve
================================================================================
"#);
}

pub fn print_status(base_dir: &PathBuf) {
    let specs = get_system_specs();
    let llama_manager = LlamaManager::new(base_dir.clone());
    let status = llama_manager.status();

    println!("================================================================================");
    println!("                    💻 HARDWARE & SYSTEM DIAGNOSTICS                            ");
    println!("================================================================================");
    println!("  CPU Logical Cores:     {}", specs.cpu_cores);
    println!("  Total RAM:             {:.2} GB", specs.total_ram_gb);
    println!("  Available / Free RAM:  {:.2} GB", specs.free_ram_gb);
    println!("  Recommended Tier:      {}", specs.recommended_params);
    println!("  Model Sizing Note:     {}", specs.max_recommended_size);
    println!("--------------------------------------------------------------------------------");
    println!("                    🦙 LLAMA.CPP INFERENCE BACKEND                              ");
    println!("--------------------------------------------------------------------------------");
    println!("  Engine State:          {}", if status.is_running { "🟢 RUNNING" } else { "🔴 STOPPED" });
    if status.is_running {
        println!("  Process ID (PID):      {}", status.pid.map(|p| p.to_string()).unwrap_or_else(|| "N/A".into()));
        println!("  Backend Port:          {}", status.port);
        println!("  Active Model:          {}", status.active_model.as_deref().unwrap_or("Unknown"));
    }
    println!("  Binary Available:      {}", if status.is_binary_available { "✅ Yes" } else { "❌ No" });
    if let Some(bin) = status.binary_path {
        println!("  Binary Path:           {}", bin);
    }
    println!("  Models in Repository:  {}", status.available_models.len());
    println!("================================================================================");
}

pub fn print_models(base_dir: &PathBuf) {
    let llama_manager = LlamaManager::new(base_dir.clone());
    let models = llama_manager.list_available_models();

    println!("================================================================================");
    println!("                      📦 LOCAL GGUF MODEL REPOSITORY                            ");
    println!("================================================================================");

    if models.is_empty() {
        println!("  No GGUF models found in: {}", base_dir.join("models").display());
        println!("  💡 Tip: Place .gguf models into the 'models/' directory or use the Web UI downloader.");
    } else {
        println!("  {:<4} {:<40} {:<12}", "#", "Model Filename", "File Size");
        println!("  {}", "-".repeat(60));
        for (i, m) in models.iter().enumerate() {
            let path = base_dir.join("models").join(m);
            let size_str = if let Ok(meta) = fs::metadata(&path) {
                let mb = meta.len() as f64 / (1024.0 * 1024.0);
                if mb >= 1024.0 {
                    format!("{:.2} GB", mb / 1024.0)
                } else {
                    format!("{:.1} MB", mb)
                }
            } else {
                "Unknown".to_string()
            };
            println!("  {:<4} {:<40} {:<12}", i + 1, m, size_str);
        }
    }
    println!("================================================================================");
}

pub fn handle_start(args: &[String], base_dir: &PathBuf) {
    if args.is_empty() {
        eprintln!("❌ Error: Missing model name.");
        eprintln!("Usage: omni_engine start <model_filename.gguf> [--port 8081] [--threads N] [--ctx 2048]");
        return;
    }

    let model_name = args[0].clone();
    let mut port: u16 = 8081;
    let mut threads: usize = 0;
    let mut ctx_size: usize = 2048;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(8081);
                    i += 1;
                }
            }
            "--threads" | "-t" => {
                if i + 1 < args.len() {
                    threads = args[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "--ctx" | "-c" => {
                if i + 1 < args.len() {
                    ctx_size = args[i + 1].parse().unwrap_or(2048);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let llama_manager = LlamaManager::new(base_dir.clone());
    let status = llama_manager.status();
    if status.is_running {
        println!("⚠️  llama-server is already running (PID: {}).", status.pid.unwrap_or(0));
        println!("Run 'omni_engine stop' first if you want to switch models.");
        return;
    }

    println!("⏳ Launching llama-server with model '{}'...", model_name);
    println!("   Port: {}, Context: {} tokens, Threads: {}", port, ctx_size, if threads == 0 { "Auto (Physical Cores)" } else { "Custom" });

    match llama_manager.start(model_name.clone(), port, threads, ctx_size) {
        Ok(pid) => {
            println!("✅ Successfully started llama-server backend!");
            println!("   Process ID: {}", pid);
            println!("   Active Model: {}", model_name);
            println!("   Endpoint:   http://127.0.0.1:{}/v1/chat/completions", port);
        }
        Err(e) => {
            eprintln!("❌ Failed to start llama-server: {}", e);
        }
    }
}

pub fn handle_stop(base_dir: &PathBuf) {
    let llama_manager = LlamaManager::new(base_dir.clone());
    let status = llama_manager.status();

    if !status.is_running {
        println!("ℹ️  llama-server is not currently running.");
        return;
    }

    print!("⏳ Stopping llama-server (PID: {})... ", status.pid.unwrap_or(0));
    let _ = io::stdout().flush();

    match llama_manager.stop() {
        Ok(_) => {
            println!("✅ Stopped successfully.");
        }
        Err(e) => {
            println!("❌ Error stopping engine: {}", e);
        }
    }
}

pub async fn handle_ask(args: &[String], base_dir: &PathBuf) {
    if args.is_empty() {
        eprintln!("❌ Error: Prompt cannot be empty.");
        eprintln!("Usage: omni_engine ask \"What is quantum computing?\" [--temp 0.7] [--max-tokens 1024]");
        return;
    }

    let prompt = args[0].clone();
    let mut port: u16 = 8081;
    let mut temperature: f32 = 0.7;
    let mut max_tokens: u32 = 1024;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(8081);
                    i += 1;
                }
            }
            "--temp" => {
                if i + 1 < args.len() {
                    temperature = args[i + 1].parse().unwrap_or(0.7);
                    i += 1;
                }
            }
            "--max-tokens" => {
                if i + 1 < args.len() {
                    max_tokens = args[i + 1].parse().unwrap_or(1024);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Verify engine is running
    let llama_manager = LlamaManager::new(base_dir.clone());
    let status = llama_manager.status();
    if !status.is_running {
        eprintln!("⚠️  llama-server engine is not running.");
        eprintln!("Please start a model first with: omni_engine start <model.gguf>");
        return;
    }

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let req = ChatCompletionRequest {
        model: status.active_model.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        temperature: Some(temperature),
        stream: Some(false),
        max_tokens: Some(max_tokens),
    };

    print!("🤖 Thinking... ");
    let _ = io::stdout().flush();

    let mut attempts = 0;
    loop {
        match client.post(&url).json(&req).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    if let Ok(json_res) = res.json::<serde_json::Value>().await {
                        print!("\r                      \r");
                        if let Some(content) = json_res["choices"][0]["message"]["content"].as_str() {
                            println!("{}", content.trim());
                        } else {
                            println!("{}", serde_json::to_string_pretty(&json_res).unwrap_or_default());
                        }
                    } else {
                        eprintln!("\n❌ Failed to parse JSON response from engine.");
                    }
                    break;
                } else if res.status().as_u16() == 503 && attempts < 10 {
                    attempts += 1;
                    print!("\r⏳ Warming up model tensors... ");
                    let _ = io::stdout().flush();
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue;
                } else {
                    let err_text = res.text().await.unwrap_or_default();
                    eprintln!("\n❌ Engine returned error (HTTP): {}", err_text);
                    break;
                }
            }
            Err(e) => {
                if attempts < 5 {
                    attempts += 1;
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
                eprintln!("\n❌ Failed to connect to engine at {}: {}", url, e);
                break;
            }
        }
    }
}

pub async fn handle_chat(args: &[String], base_dir: &PathBuf) {
    let mut port: u16 = 8081;
    if !args.is_empty() {
        if let Ok(p) = args[0].parse::<u16>() {
            port = p;
        }
    }

    crate::tui_agent::run_tui_session(port, base_dir).await;
}

pub fn handle_keys(args: &[String], base_dir: &PathBuf) {
    let key_manager = KeyManager::new(base_dir.clone());

    if args.is_empty() || args[0] == "list" {
        let keys = key_manager.list_keys();
        println!("================================================================================");
        println!("                         🗝️  OMNI API KEYS REPOSITORY                           ");
        println!("================================================================================");
        if keys.is_empty() {
            println!("  No API keys found.");
        } else {
            println!("  {:<38} {:<24} {:<42}", "Key ID", "Name", "Secret Token");
            println!("  {}", "-".repeat(106));
            for k in keys {
                println!("  {:<38} {:<24} {:<42}", k.id, k.name, k.key);
            }
        }
        println!("================================================================================");
    } else if args[0] == "new" {
        let name = if args.len() > 1 {
            args[1..].join(" ")
        } else {
            "CLI Key".to_string()
        };
        let new_key = key_manager.create_key(name);
        println!("✅ Successfully created new API key!");
        println!("  ID:     {}", new_key.id);
        println!("  Name:   {}", new_key.name);
        println!("  Secret: {}", new_key.key);
    } else if args[0] == "revoke" {
        if args.len() < 2 {
            eprintln!("❌ Error: Missing Key ID to revoke.");
            eprintln!("Usage: omni_engine keys revoke <KEY_ID>");
            return;
        }
        let id = &args[1];
        if key_manager.revoke_key(id) {
            println!("✅ Successfully revoked API key ID: {}", id);
        } else {
            eprintln!("❌ Key ID '{}' not found.", id);
        }
    } else {
        eprintln!("❌ Unknown keys subcommand: '{}'", args[0]);
        eprintln!("Usage: omni_engine keys [list | new <name> | revoke <id>]");
    }
}
