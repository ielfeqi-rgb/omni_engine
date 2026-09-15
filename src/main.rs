mod auth;
mod downloader;
mod llama_manager;
mod openai_api;
mod web_server;

use auth::KeyManager;
use downloader::ModelDownloader;
use llama_manager::LlamaManager;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use web_server::{create_router, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "omni_engine=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let base_dir = if exe_dir.join("models").exists() || exe_dir.join("config").exists() {
        exe_dir
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    println!("============================================================");
    println!("   🚀 OMNI AI ENGINE v1.0 (Standalone Rust Edition)   ");
    println!("   Core: llama.cpp | Web UI | OpenAI API | API Key Auth     ");
    println!("============================================================");

    let key_manager = KeyManager::new(base_dir.clone());
    let downloader = ModelDownloader::new(base_dir.join("models"));
    let llama_manager = LlamaManager::new(base_dir.clone());

    let state = AppState {
        key_manager,
        downloader,
        llama_manager,
    };

    let router = create_router(state);

    let bind_addr = "0.0.0.0:8090";
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;

    println!("🌐 Server Listening on: http://{}", bind_addr);
    println!("💬 Web UI Dashboard:    http://127.0.0.1:8090");
    println!("⚡ OpenAI Endpoint:     http://127.0.0.1:8090/v1/chat/completions");
    println!("============================================================");

    axum::serve(listener, router).await?;

    Ok(())
}
