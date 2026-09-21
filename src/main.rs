#![allow(dead_code)]

mod auth;
mod causal_memory;
mod cli;
mod downloader;
mod llama_manager;
mod logger;
mod openai_api;
mod planner;
mod sandbox;
mod system_info;
mod tui_agent;
mod web_server;

use auth::KeyManager;
use downloader::ModelDownloader;
use llama_manager::LlamaManager;
use logger::LogBuffer;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use web_server::{create_router, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let base_dir = if exe_dir.join("models").exists() || exe_dir.join("config").exists() {
        exe_dir
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    // If CLI command is provided and handled, exit cleanly
    if args.len() > 1 && args[1] != "serve" {
        if cli::handle_cli(&args, &base_dir).await? {
            return Ok(());
        }
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
    let mut port: u16 = 8090;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            if let Ok(p) = args[i + 1].parse::<u16>() {
                port = p;
            }
            i += 1;
        }
        i += 1;
    }

    println!("============================================================");
    println!("   🚀 OMNI AI ENGINE v1.0.1 (Standalone Rust Edition)   ");
    println!("   Core: llama.cpp | CLI & Web UI | OpenAI API | Auth   ");
    println!("============================================================");

    let key_manager = KeyManager::new(base_dir.clone());
    let downloader = ModelDownloader::new(base_dir.join("models"));
    let llama_manager = LlamaManager::new(base_dir.clone());
    let log_buffer = LogBuffer::new(500);

    log_buffer.push("OMNI AI ENGINE v1.0.1 initialized.".to_string());
    log_buffer.push("Scanning hardware specs and local model repository...".to_string());

    let state = AppState {
        key_manager,
        downloader,
        llama_manager,
        log_buffer,
    };

    let router = create_router(state);

    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    println!("🌐 Server Listening on: http://{}", bind_addr);
    println!("💬 Web UI Dashboard:    http://127.0.0.1:{}", port);
    println!("⚡ OpenAI Endpoint:     http://127.0.0.1:{}/v1/chat/completions", port);
    println!("============================================================");

    axum::serve(listener, router).await?;

    Ok(())
}
