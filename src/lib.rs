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
