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

use crate::native_llama::{NativeLlamaModel, NativeLlamaContext, NativeLlamaSampler};
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct Chunk {
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum AnalyzerResult {
    /// A diagnostic issue was found that requires the Surgeon
    IssueFound {
        line_number: usize,
        error_message: String,
        lines_context: String,
    },
    /// No issues found in this chunk or the entire file, proceed to execute
    Pass {
        execution_command: Option<String>,
    },
}

pub struct StaticAnalyzer {
    model: Arc<NativeLlamaModel>,
    ctx: Arc<Mutex<NativeLlamaContext>>,
}

impl StaticAnalyzer {
    pub fn new(model: Arc<NativeLlamaModel>, ctx: Arc<Mutex<NativeLlamaContext>>) -> Self {
        Self { model, ctx }
    }

    /// Evaluates tokens from the VFS chunk and yields an `AnalyzerResult`.
    pub fn evaluate_chunk(&self, seq_id: i32, filename: &str, chunk: &Chunk) -> Result<AnalyzerResult, String> {
        info!("Analyzing chunk lines {} to {}", chunk.start_line, chunk.end_line);
        let mut ctx = self.ctx.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;

        // Snapshot cursor so we can drop the analyzer's memory (zero footprint)
        let start_cursor = ctx.current_cursor() as i32;

        let user_prompt = format!(
            "You are a brilliant Static Code Analyzer.\n\
            Analyze this code for `{}`.\n\
            If there is a fatal structural syntax error, output exactly: `[Line X] Error: <description>`\n\
            If it looks sound, output exactly `PASS`.\n\
            Then, on a new line, output exactly: `COMMAND: <bash command to compile/run>`.\n\
            Note: The file is located in the current working directory as `{}`.\n\nCode:\n{}",
            filename, filename, chunk.content
        );
        let prompt = self.model.format_chat(None, &user_prompt);

        let tokens = self.model.tokenize(&prompt, true).map_err(|e| e.to_string())?;
        ctx.eval_tokens(&tokens, seq_id).map_err(|e| e.to_string())?;

        let mut sampler = NativeLlamaSampler::new(&crate::native_llama::SamplingConfig {
            temperature: 0.1,
            ..Default::default()
        }, self.model.n_vocab()).map_err(|e| e.to_string())?;

        let mut response = String::new();
        let mut in_think = false;
        for _ in 0..256 {
            let token = ctx.sample(&mut sampler).map_err(|e| e.to_string())?;
            let text = self.model.token_to_piece(token).unwrap_or_default();
            if text.is_empty() || self.model.is_stop_token(token) || self.model.is_stop_piece(&text) {
                break;
            }
            response.push_str(&text);
            ctx.eval_tokens(&[token], seq_id).map_err(|e| e.to_string())?;

            if response.contains("<think>") {
                in_think = true;
            }
            if response.contains("</think>") {
                in_think = false;
            }

            if !in_think && response.contains("COMMAND:") && response.split("COMMAND:").nth(1).unwrap_or("").contains('\n') {
                break;
            }
        }

        // Clean up analyzer memory immediately
        if let Err(e) = ctx.kv_cache_seq_rm(seq_id, start_cursor, -1) {
            tracing::warn!("Failed to prune analyzer KV cache: {}", e);
        }

        let clean_response = crate::planner::swarm_coordinator::strip_thought(&response);
        let res_trimmed = clean_response.trim();
        info!("Analyzer Output:\n{}", res_trimmed);

        let upper = res_trimmed.to_uppercase();
        let is_pass = (res_trimmed.starts_with("PASS") || upper.lines().next().map(|l| l.trim() == "PASS").unwrap_or(false))
            && !upper.contains("ERROR")
            && !upper.contains("FAIL")
            && !upper.contains("[LINE");

        if is_pass {
            let cmd = if let Some(idx) = res_trimmed.find("COMMAND:") {
                let end_idx = res_trimmed[idx..].find('\n').map(|i| i + idx).unwrap_or(res_trimmed.len());
                res_trimmed[idx + 8..end_idx].trim().to_string()
            } else {
                // Dynamic toolchain-discovered command
                crate::planner::swarm_coordinator::SwarmCoordinator::resolve_verification_command(filename)
            };
            Ok(AnalyzerResult::Pass { execution_command: Some(cmd) })
        } else if let Some(start_idx) = res_trimmed.find('[') {
            // Parse [Line X] or [X] error
            let end_idx = res_trimmed[start_idx..].find(']').unwrap_or(0) + start_idx;
            let mut line_str = res_trimmed[start_idx + 1..end_idx].trim();
            if line_str.to_lowercase().starts_with("line ") {
                line_str = &line_str[5..];
            }
            let line_number = line_str.parse::<usize>().unwrap_or(chunk.start_line);
            
            let err_msg = res_trimmed[end_idx + 1..].trim().trim_start_matches(':').trim_start_matches("Error:").trim_start_matches("error:").trim().to_string();
            
            Ok(AnalyzerResult::IssueFound {
                line_number,
                error_message: err_msg,
                lines_context: String::new(),
            })
        } else {
            // Check fallback for "Line X" or "line X"
            let mut found_issue = None;
            for line in res_trimmed.lines() {
                let l_lower = line.to_lowercase();
                if let Some(pos) = l_lower.find("line") {
                    let remainder = line[pos + 4..].trim_start_matches(|c: char| c == ' ' || c == ':' || c == '#');
                    let digits: String = remainder.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(line_num) = digits.parse::<usize>() {
                        let err_msg = remainder[digits.len()..]
                            .trim()
                            .trim_start_matches(':')
                            .trim_start_matches('-')
                            .trim_start_matches("Error:")
                            .trim_start_matches("error:")
                            .trim()
                            .to_string();
                        found_issue = Some((line_num, if err_msg.is_empty() { line.to_string() } else { err_msg }));
                        break;
                    }
                }
            }

            if let Some((line_number, error_message)) = found_issue {
                Ok(AnalyzerResult::IssueFound {
                    line_number,
                    error_message,
                    lines_context: String::new(),
                })
            } else {
                // Unrecognized format, assume Pass with dynamic verification command
                let cmd = crate::planner::swarm_coordinator::SwarmCoordinator::resolve_verification_command(filename);
                Ok(AnalyzerResult::Pass { execution_command: Some(cmd) })
            }
        }
    }
}
