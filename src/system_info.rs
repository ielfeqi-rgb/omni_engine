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

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSpecs {
    pub cpu_cores: usize,
    pub total_ram_gb: f32,
    pub free_ram_gb: f32,
    pub recommended_params: String,
    pub max_recommended_size: String,
}

pub fn get_system_specs() -> SystemSpecs {
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let mut total_ram_kb: u64 = 0;
    let mut free_ram_kb: u64 = 0;
    let mut available_ram_kb: u64 = 0;

    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_ram_kb = parse_mem_line(line);
            } else if line.starts_with("MemFree:") {
                free_ram_kb = parse_mem_line(line);
            } else if line.starts_with("MemAvailable:") {
                available_ram_kb = parse_mem_line(line);
            }
        }
    }

    if available_ram_kb == 0 {
        available_ram_kb = free_ram_kb;
    }

    let total_ram_gb = total_ram_kb as f32 / (1024.0 * 1024.0);
    let free_ram_gb = available_ram_kb as f32 / (1024.0 * 1024.0);

    let (recommended_params, max_recommended_size) = estimate_max_model(total_ram_gb);

    SystemSpecs {
        cpu_cores,
        total_ram_gb,
        free_ram_gb,
        recommended_params,
        max_recommended_size,
    }
}

fn parse_mem_line(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

fn estimate_max_model(ram_gb: f32) -> (String, String) {
    if ram_gb < 6.0 {
        (
            "1.5B - 3B Parameters (GGUF Q4_K_M)".to_string(),
            "Suitable for small models such as Qwen2.5-1.5B or Llama-3.2-3B".to_string(),
        )
    } else if ram_gb < 12.0 {
        (
            "7B - 8B Parameters (GGUF Q4_K_M)".to_string(),
            "Suitable for medium models such as Llama-3-8B, Mistral-7B, or Qwen2.5-7B".to_string(),
        )
    } else if ram_gb < 24.0 {
        (
            "13B - 14B Parameters (GGUF Q4_K_M)".to_string(),
            "Suitable for strong models such as Qwen2.5-14B or DeepSeek-R1-Distill-14B".to_string(),
        )
    } else if ram_gb < 48.0 {
        (
            "30B - 34B Parameters (GGUF Q4_K_M)".to_string(),
            "Suitable for large models such as Command-R 35B or Qwen2.5-32B".to_string(),
        )
    } else {
        (
            "70B+ Parameters (GGUF Q4_K_M)".to_string(),
            "Ultra-powerful machine! Capable of running massive models such as Llama-3-70B or Qwen-72B".to_string(),
        )
    }
}
