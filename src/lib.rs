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

pub mod native_llama;


pub mod auth;
pub mod causal_memory;
pub mod cli;
pub mod downloader;
pub mod llama_manager;
pub mod logger;
pub mod openai_api;
pub mod planner;
pub mod sandbox;
pub mod system_info;
pub mod tui_agent;
pub mod web_server;
pub mod tui;
pub mod vector_memory;

/// Reliably resolves the project base directory containing models/, config/, and bin/
/// regardless of whether the executable is invoked from the project root, bin/, target/release/, or target/debug/.
pub fn resolve_base_dir() -> std::path::PathBuf {
    use std::path::PathBuf;
    // 1. If cwd has Cargo.toml -> project root!
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("Cargo.toml").exists() {
            return cwd;
        }
        let mut curr = cwd.clone();
        while let Some(parent) = curr.parent() {
            if parent.join("Cargo.toml").exists() {
                return parent.to_path_buf();
            }
            curr = parent.to_path_buf();
        }
    }
    // 2. Check current_exe location ancestors for Cargo.toml or bin directory
    if let Ok(exe) = std::env::current_exe() {
        let mut curr = exe.clone();
        while let Some(parent) = curr.parent() {
            if parent.join("Cargo.toml").exists() {
                return parent.to_path_buf();
            }
            curr = parent.to_path_buf();
        }
        if let Some(parent) = exe.parent() {
            if parent.join("libllama.so").exists() || parent.join("libllama.dylib").exists() {
                return parent.to_path_buf();
            }
        }
    }
    // 3. Fallback
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
