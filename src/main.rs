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

use omni_engine::auth::KeyManager;
use omni_engine::cli;
use omni_engine::downloader::ModelDownloader;
use omni_engine::llama_manager::LlamaManager;
use omni_engine::logger::LogBuffer;
use omni_engine::tui;
use omni_engine::web_server::{create_router, AppState};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let base_dir = omni_engine::resolve_base_dir();

    // TUI Opt-in Layer (Full-Screen Graphical Terminal Dashboard)
    if args.contains(&"--tui".to_string())
        || args.contains(&"-tui".to_string())
        || (args.len() > 1 && (args[1] == "tui" || args[1] == "dashboard"))
    {
        return tui::run_tui().await;
    }

    // If CLI command is provided and handled, exit cleanly
    if args.len() > 1 && args[1] != "serve" {
        let mut cli_args = args.clone();
        cli_args.retain(|a| a != "--tui" && a != "-tui"); // just in case
        if cli::handle_cli(&cli_args, &base_dir).await? {
            return Ok(());
        }
    }

    // If running with no args, open Sovereign Terminal Console by default
    if args.len() <= 1 {
        return cli::run_interactive_console(&base_dir).await;
    }

    // Otherwise, initialize logger and run web server
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "omni_engine=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Check for custom port in args (e.g. `serve --port 8095`)
    // [GUIDANCE] Use env!("CARGO_PKG_VERSION") instead of hardcoded version strings.
    // Currently main.rs says "v2.0.0", line 81 says "v1.0.1", cli.rs says "v1.0.1".
    let mut port: u16 = 8090;
    let mut bind_host = "127.0.0.1".to_string();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            if let Ok(p) = args[i + 1].parse::<u16>() {
                port = p;
            }
            i += 1;
        } else if args[i] == "--bind" && i + 1 < args.len() {
            bind_host = args[i + 1].clone();
            i += 1;
        }
        i += 1;
    }

    println!("============================================================");
    println!("   :: OMNI AI ENGINE v{} (Sovereign Autonomous Runtime)", env!("CARGO_PKG_VERSION"));
    println!("   Core: Rust Native & Isolated Sandbox | Causal KV-Cache | Local AI");
    println!("============================================================");

    let key_manager = KeyManager::new(base_dir.clone());
    let downloader = ModelDownloader::new(base_dir.join("models"));
    let llama_manager = LlamaManager::new(base_dir.clone());
    let log_buffer = LogBuffer::new(500);

    log_buffer.push(format!("OMNI AI ENGINE v{} initialized.", env!("CARGO_PKG_VERSION")));
    log_buffer.push("Scanning hardware specs and local model repository...".to_string());

    let state = AppState {
        key_manager,
        downloader,
        llama_manager: llama_manager.clone(),
        log_buffer,
    };

    let router = create_router(state);

    let bind_addr = format!("{}:{}", bind_host, port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    println!("[+] Server Listening on: http://{}", bind_addr);
    println!("[+] Web UI Dashboard:    http://{}:{}", bind_host, port);
    println!("[+] OpenAI Endpoint:     http://{}:{}/v1/chat/completions", bind_host, port);
    println!("============================================================");

    tokio::select! {
        res = axum::serve(listener, router) => {
            if let Err(e) = res {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n[!] Graceful shutdown signal received. Stopping background services...");
            let _ = llama_manager.stop();
            println!("[+] Omni Engine stopped cleanly.");
        }
    }

    Ok(())
}

