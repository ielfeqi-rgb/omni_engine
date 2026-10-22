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

use crate::native_llama::{NativeLlamaModel, NativeLlamaContext, NativeLlamaSampler, SamplingConfig};
use crate::planner::surgeon::Diagnostic;
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct ExecutionResult {
    pub exit_code: i32,
    pub output: String, // Combined stdout + stderr
}

pub struct TerminalOracle {
    model: Arc<NativeLlamaModel>,
    ctx: Arc<Mutex<NativeLlamaContext>>,
}

impl TerminalOracle {
    pub fn new(model: Arc<NativeLlamaModel>, ctx: Arc<Mutex<NativeLlamaContext>>) -> Self {
        Self { model, ctx }
    }

    /// Securely runs the command inside IsolatedJail sandbox
    pub fn execute_command(&self, command: &str, filename: &str, vfs_code: &str) -> ExecutionResult {
        info!("Oracle executing in sandbox: {}", command);
        let vfs_files = vec![(std::path::PathBuf::from(filename), vfs_code.as_bytes().to_vec())];
        match crate::sandbox::isolated_jail::IsolatedJail::run_in_jail(
            command,
            &vfs_files,
            std::time::Duration::from_secs(15),
        ) {
            Ok(result) => {
                let code = result.exit_code.unwrap_or(-1);
                let mut combined = result.stdout;
                if !result.stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push_str("\n--- STDERR ---\n");
                    }
                    combined.push_str(&result.stderr);
                }
                ExecutionResult { exit_code: code, output: combined }
            }
            Err(e) => ExecutionResult { exit_code: -1, output: e },
        }
    }

    /// If exit_code != 0, the AI reads the terminal output and writes a micro-diagnostic
    pub fn diagnose_crash(
        &self,
        source_seq_id: i32,
        oracle_seq_id: i32,
        terminal_output: &str,
    ) -> Result<Option<Diagnostic>, String> {
        info!("Oracle AI diagnosing crash on seq_id={}...", oracle_seq_id);

        let mut ctx = self.ctx.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;
        let source_max_pos = ctx.current_cursor() as i32; 
        ctx.kv_cache_seq_cp(source_seq_id, oracle_seq_id, 0, source_max_pos);

        let user_prompt = format!(
            "You are an expert compiler diagnostician. The program crashed with the following output:\n{}\n\nIdentify the exact line number causing the error, and provide a tiny fix instruction. Format your response exactly as:\n[Line X] error: your fix instruction here\nIf you cannot determine the line, respond with UNKNOWN.",
            terminal_output
        );
        let prompt = self.model.format_chat(None, &user_prompt);

        let prompt_tokens = self.model.tokenize(&prompt, true)?;
        ctx.eval_tokens(&prompt_tokens, oracle_seq_id)?;

        let config = SamplingConfig { temperature: 0.1, ..Default::default() };
        let mut sampler = NativeLlamaSampler::new(&config, self.model.n_vocab())?;

        let mut response = String::new();
        let mut in_think = false;
        for _ in 0..256 {
            let token = ctx.sample(&mut sampler)?;
            let text = self.model.token_to_piece(token).unwrap_or_default();
            
            if text.is_empty() || self.model.is_stop_token(token) || self.model.is_stop_piece(&text) {
                break;
            }

            response.push_str(&text);
            ctx.eval_tokens(&[token], oracle_seq_id)?;

            if response.contains("<think>") {
                in_think = true;
            }
            if response.contains("</think>") {
                in_think = false;
            }

            if !in_think && response.contains("</think>") {
                let after_think = response.split("</think>").last().unwrap_or("");
                if after_think.contains('\n') && after_think.trim().len() > 5 {
                    break;
                }
            } else if !in_think && !response.contains("<think>") && text.contains('\n') && response.trim().len() > 5 {
                break;
            }
        }

        ctx.kv_cache_seq_rm(oracle_seq_id, 0, -1)?;
        let clean_response = crate::planner::swarm_coordinator::strip_thought(&response);
        let trimmed_response = clean_response.trim();
        info!("Oracle Diagnosis: {}", trimmed_response);

        if trimmed_response.contains("UNKNOWN") {
            return Ok(None);
        }

        // 1. Parse "[Line X] error: message" or "[X] error: message"
        if let Some(start_idx) = trimmed_response.find('[') {
            if let Some(close_idx) = trimmed_response.find(']') {
                if start_idx < close_idx {
                    let mut line_str = &trimmed_response[start_idx + 1..close_idx];
                    if line_str.to_lowercase().starts_with("line ") {
                        line_str = &line_str[5..];
                    }
                    if let Ok(line_num) = line_str.trim().parse::<usize>() {
                        let msg = trimmed_response[close_idx + 1..]
                            .trim()
                            .trim_start_matches(':')
                            .trim_start_matches("error:")
                            .trim_start_matches("Error:")
                            .trim()
                            .to_string();
                        return Ok(Some(Diagnostic { line_number: line_num, error_message: msg }));
                    }
                }
            }
        }

        // 2. Resilient fallback parsing: look for lines or phrases like "Line 12: error", "line 12 - error", "at line 12"
        for line in trimmed_response.lines() {
            let l_lower = line.to_lowercase();
            if let Some(pos) = l_lower.find("line") {
                let remainder = line[pos + 4..].trim_start_matches(|c: char| c == ' ' || c == ':' || c == '#');
                let digits: String = remainder.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(line_num) = digits.parse::<usize>() {
                    let err_msg = remainder[digits.len()..]
                        .trim()
                        .trim_start_matches(':')
                        .trim_start_matches('-')
                        .trim_start_matches("error:")
                        .trim_start_matches("Error:")
                        .trim()
                        .to_string();
                    let final_msg = if err_msg.is_empty() {
                        line.to_string()
                    } else {
                        err_msg
                    };
                    return Ok(Some(Diagnostic { line_number: line_num, error_message: final_msg }));
                }
            }
        }

        Ok(None)
    }

    /// Verifies the program's STDOUT against the original system goal
    pub fn verify_behavior(
        &self,
        source_seq_id: i32,
        oracle_seq_id: i32,
        original_goal: &str,
        terminal_output: &str,
    ) -> Result<bool, String> {
        info!("Oracle AI verifying behavioral alignment...");
        
        let mut ctx = self.ctx.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;
        let source_max_pos = ctx.current_cursor() as i32; 
        ctx.kv_cache_seq_cp(source_seq_id, oracle_seq_id, 0, source_max_pos);

        let user_prompt = format!(
            "Original Goal:\n{}\n\nProgram Output:\n{}\n\nDoes this output successfully accomplish the goal? Respond with only PASS or FAIL.",
            original_goal, terminal_output
        );
        let prompt = self.model.format_chat(None, &user_prompt);

        let prompt_tokens = self.model.tokenize(&prompt, true)?;
        ctx.eval_tokens(&prompt_tokens, oracle_seq_id)?;

        let config = SamplingConfig { temperature: 0.1, ..Default::default() };
        let mut sampler = NativeLlamaSampler::new(&config, self.model.n_vocab())?;

        let mut response = String::new();
        for _ in 0..128 {
            let token = ctx.sample(&mut sampler)?;
            let text = self.model.token_to_piece(token).unwrap_or_default();
            if text.is_empty() || self.model.is_stop_token(token) || self.model.is_stop_piece(&text) { break; }
            response.push_str(&text);
            ctx.eval_tokens(&[token], oracle_seq_id)?;

            let stripped = crate::planner::swarm_coordinator::strip_thought(&response);
            if stripped.to_uppercase().contains("PASS") || stripped.to_uppercase().contains("FAIL") {
                break;
            }
        }

        ctx.kv_cache_seq_rm(oracle_seq_id, 0, -1)?;
        let stripped = crate::planner::swarm_coordinator::strip_thought(&response).trim().to_string();
        let upper = stripped.to_uppercase();
        let has_pass = upper.starts_with("PASS") || upper.split_whitespace().any(|w| w.trim_matches(|c: char| !c.is_alphanumeric()) == "PASS");
        let has_negation_or_fail = upper.contains("FAIL") 
            || upper.contains("NOT PASS") 
            || upper.contains("DOES NOT") 
            || upper.contains("FAILED")
            || upper.contains("UNSUCCESSFUL")
            || upper.contains("ERROR");
        let passed = has_pass && !has_negation_or_fail;
        info!("Oracle Behavioral Verification: {}", if passed { "PASS" } else { "FAIL" });
        Ok(passed)
    }
}
