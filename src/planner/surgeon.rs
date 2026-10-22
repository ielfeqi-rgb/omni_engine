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
use std::sync::{Arc, Mutex};
use tracing::info;

/// A single diagnostic point output by the Compiler Oracle or Static Analyzer
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub line_number: usize,
    pub error_message: String,
}

/// The MicroSurgeon is an ephemeral agent. It spawns, fixes a single line using sequence cloning,
/// and terminates, leaving 0 memory footprint on the main Causal Tree.
pub struct MicroSurgeon {
    model: Arc<NativeLlamaModel>,
    ctx: Arc<Mutex<NativeLlamaContext>>,
}

impl MicroSurgeon {
    pub fn new(model: Arc<NativeLlamaModel>, ctx: Arc<Mutex<NativeLlamaContext>>) -> Self {
        Self {
            model,
            ctx,
        }
    }

    /// Performs syntax surgery on an isolated line using FIM (Fill-in-the-Middle) / Prefix Completion.
    pub fn perform_surgery(
        &mut self,
        source_seq_id: i32,
        surgery_seq_id: i32,
        lines_context: &str,
        diagnostic: &Diagnostic,
    ) -> Result<String, String> {
        info!("Spawning Micro-Surgeon on seq_id={} to fix line {}", surgery_seq_id, diagnostic.line_number);

        let mut ctx = self.ctx.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;
        let source_max_pos = ctx.current_cursor() as i32; 
        ctx.kv_cache_seq_cp(source_seq_id, surgery_seq_id, 0, source_max_pos);

        // 2. Build Prefix and Suffix
        let mut prefix = String::new();
        let mut suffix = String::new();

        let lines: Vec<&str> = lines_context.lines().collect();
        let target_idx = if diagnostic.line_number > 0 { diagnostic.line_number - 1 } else { 0 };

        for (i, line) in lines.iter().enumerate() {
            if i < target_idx {
                prefix.push_str(line);
                prefix.push('\n');
            } else if i > target_idx {
                suffix.push_str(line);
                suffix.push('\n');
            }
        }

        // We inject the error as a contextual code comment right before the completion!
        let comment_prefix = if lines_context.contains("def ") || lines_context.contains("import ") { "#" } else { "//" };
        prefix.push_str(&format!("{} [Surgeon Fix Needed]: {}\n", comment_prefix, diagnostic.error_message));

        let micro_prompt = if let (Some(pre), Some(suf), Some(mid)) = (&self.model.fim_pre, &self.model.fim_suf, &self.model.fim_mid) {
            format!("{}{}{}{}{}", pre, prefix, suf, suffix, mid)
        } else {
            // Ultimate Universal Fallback: Pure Prefix Completion
            prefix.clone()
        };

        let prompt_tokens = self.model.tokenize(&micro_prompt, true)?;
        ctx.eval_tokens(&prompt_tokens, surgery_seq_id)?;

        let config = SamplingConfig {
            temperature: 0.1,
            ..Default::default()
        };
        let mut sampler = NativeLlamaSampler::new(&config, self.model.n_vocab())?;

        // 3. Generate the fixed line (Pure text completion, no chat markup!)
        let mut fixed_line = String::new();
        for _ in 0..150 {
            let token = ctx.sample(&mut sampler)?;
            let text = self.model.token_to_piece(token).unwrap_or_default();
            
            if text.is_empty() || self.model.is_stop_token(token) || self.model.is_stop_piece(&text) {
                break;
            }

            fixed_line.push_str(&text);
            ctx.eval_tokens(&[token], surgery_seq_id)?;
            
            if text.contains('\n') {
                break; // Stop at first newline
            }
        }

        // 4. Terminate the Micro-Surgeon (Drop Cache)
        ctx.kv_cache_seq_rm(surgery_seq_id, 0, -1)?;
        info!("Micro-Surgeon seq_id={} terminated and cache dropped.", surgery_seq_id);

        let clean_fixed = crate::planner::swarm_coordinator::strip_thought(&fixed_line);
        Ok(clean_fixed.trim_end().to_string())
    }
}

use crate::sandbox::diagnostics::AstSlice;

impl MicroSurgeon {
    /// Performs AST-level macro-surgery on a broken function or class.
    pub fn perform_macro_surgery(
        &mut self,
        _source_seq_id: i32,
        surgery_seq_id: i32,
        ast_slice: &AstSlice,
        diagnostic: &Diagnostic,
    ) -> Result<String, String> {
        info!("Spawning Macro-Surgeon on seq_id={} to fix lines {}-{}", surgery_seq_id, ast_slice.start_line, ast_slice.end_line);

        let mut ctx = self.ctx.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;

        // 1. Build the prompt
        let user_prompt = format!(
            "The following code block from `{}` has a runtime/syntax error.\n\n\
            ```\n{}\n```\n\n\
            Error:\n{}\n\n\
            Please provide the fully corrected code block (and ONLY the code block). Use standard ``` code fences.",
            ast_slice.file_path, ast_slice.content, diagnostic.error_message
        );
        let micro_prompt = self.model.format_chat(None, &user_prompt);

        let prompt_tokens = self.model.tokenize(&micro_prompt, true)?;
        ctx.eval_tokens(&prompt_tokens, surgery_seq_id)?;

        let config = SamplingConfig {
            temperature: 0.2,
            ..Default::default()
        };
        let mut sampler = NativeLlamaSampler::new(&config, self.model.n_vocab())?;

        // 2. Generate the fixed block
        let mut fixed_block = String::new();
        let mut in_code_block = false;
        
        for _ in 0..1024 {
            let token = ctx.sample(&mut sampler)?;
            let text = self.model.token_to_piece(token).unwrap_or_default();
            
            if text.is_empty() || self.model.is_stop_token(token) || self.model.is_stop_piece(&text) {
                break;
            }

            fixed_block.push_str(&text);
            ctx.eval_tokens(&[token], surgery_seq_id)?;
            
            if fixed_block.contains("```") && !in_code_block {
                in_code_block = true;
            } else if in_code_block {
                if let Some(first_idx) = fixed_block.find("```") {
                    if fixed_block[first_idx + 3..].contains("```") {
                        break;
                    }
                }
            }
        }

        // 3. Terminate the Micro-Surgeon (Drop Cache)
        ctx.kv_cache_seq_rm(surgery_seq_id, 0, -1)?;
        info!("Macro-Surgeon seq_id={} terminated and cache dropped.", surgery_seq_id);

        // 4. Extract code from fences
        let mut clean_code = fixed_block.clone();
        if let Some(start) = clean_code.find("```") {
            let mut start_idx = start + 3;
            if let Some(nl) = clean_code[start_idx..].find('\n') {
                start_idx += nl + 1;
            }
            clean_code = clean_code[start_idx..].to_string();
            
            if let Some(end) = clean_code.find("```") {
                clean_code = clean_code[..end].to_string();
            }
        }
        let clean_code = crate::planner::swarm_coordinator::strip_thought(&clean_code);
        Ok(clean_code.trim().to_string())
    }
}
