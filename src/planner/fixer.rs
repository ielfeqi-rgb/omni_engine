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
use crate::sandbox::diagnostics::AstSlice;
use crate::sandbox::MemoryVfs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::info;

/// A single diagnostic point output by the Compiler Oracle or Static Analyzer
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub line_number: usize,
    pub error_message: String,
}

/// Result of an in-situ repair attempt executed by the Sovereign Fixer
#[derive(Debug, Clone)]
pub struct FixAttemptReport {
    pub success: bool,
    pub patch_applied: bool,
    pub summary: String,
    pub target_file: String,
    pub error_line: Option<usize>,
}

/// The Sovereign CodeFixer is a premier cognitive maintenance agent.
/// Operates on isolated sequence clones, performs targeted AST/FIM repair
/// with strict Attention Masking and DAG contract adherence, leaving 0 memory
/// footprint on the primary Worker stream.
pub struct CodeFixer {
    model: Arc<NativeLlamaModel>,
    ctx: Arc<Mutex<NativeLlamaContext>>,
}

/// Backwards compatibility alias
pub type MicroSurgeon = CodeFixer;

impl CodeFixer {
    pub fn new(model: Arc<NativeLlamaModel>, ctx: Arc<Mutex<NativeLlamaContext>>) -> Self {
        Self { model, ctx }
    }

    /// Performs syntax surgery on an isolated line using FIM (Fill-in-the-Middle) / Prefix Completion.
    pub fn perform_surgery(
        &mut self,
        source_seq_id: i32,
        surgery_seq_id: i32,
        lines_context: &str,
        diagnostic: &Diagnostic,
    ) -> Result<String, String> {
        let mut ctx = self.ctx.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;
        Self::perform_surgery_direct(
            &self.model,
            &mut ctx,
            source_seq_id,
            surgery_seq_id,
            lines_context,
            diagnostic,
        )
    }

    /// Direct in-context syntax surgery on an isolated line using FIM (Fill-in-the-Middle) / Prefix Completion.
    pub fn perform_surgery_direct(
        model: &NativeLlamaModel,
        ctx: &mut NativeLlamaContext,
        source_seq_id: i32,
        surgery_seq_id: i32,
        lines_context: &str,
        diagnostic: &Diagnostic,
    ) -> Result<String, String> {
        info!("Spawning CodeFixer on seq_id={} to fix line {}", surgery_seq_id, diagnostic.line_number);

        let source_max_pos = ctx.current_cursor() as i32; 
        ctx.kv_cache_seq_cp(source_seq_id, surgery_seq_id, 0, source_max_pos);

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

        let comment_prefix = if lines_context.contains("def ") || lines_context.contains("import ") { "#" } else { "//" };
        prefix.push_str(&format!("{} [Fixer Correction Needed]: {}\n", comment_prefix, diagnostic.error_message));

        let micro_prompt = if let (Some(pre), Some(suf), Some(mid)) = (&model.fim_pre, &model.fim_suf, &model.fim_mid) {
            format!("{}{}{}{}{}", pre, prefix, suf, suffix, mid)
        } else {
            prefix.clone()
        };

        let prompt_tokens = model.tokenize(&micro_prompt, true)?;
        ctx.eval_tokens(&prompt_tokens, surgery_seq_id)?;

        let config = SamplingConfig {
            temperature: 0.1,
            ..Default::default()
        };
        let mut sampler = NativeLlamaSampler::new(&config, model.n_vocab())?;

        let mut fixed_line = String::new();
        for _ in 0..150 {
            let token = ctx.sample(&mut sampler)?;
            let text = model.token_to_piece(token).unwrap_or_default();
            
            if text.is_empty() || model.is_stop_token(token) || model.is_stop_piece(&text) {
                break;
            }

            fixed_line.push_str(&text);
            ctx.eval_tokens(&[token], surgery_seq_id)?;
            
            if text.contains('\n') {
                break;
            }
        }

        ctx.kv_cache_seq_rm(surgery_seq_id, 0, -1)?;
        info!("CodeFixer seq_id={} terminated and cache dropped.", surgery_seq_id);

        let clean_fixed = crate::planner::swarm_coordinator::strip_thought(&fixed_line);
        Ok(clean_fixed.trim_end().to_string())
    }

    /// Performs AST-level macro-surgery on a broken function or class.
    pub fn perform_macro_surgery(
        &mut self,
        source_seq_id: i32,
        surgery_seq_id: i32,
        ast_slice: &AstSlice,
        diagnostic: &Diagnostic,
    ) -> Result<String, String> {
        let mut ctx = self.ctx.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;
        Self::perform_macro_surgery_direct(
            &self.model,
            &mut ctx,
            source_seq_id,
            surgery_seq_id,
            ast_slice,
            diagnostic,
        )
    }

    /// Direct in-context AST macro-surgery on a broken function, class, or code block.
    pub fn perform_macro_surgery_direct(
        model: &NativeLlamaModel,
        ctx: &mut NativeLlamaContext,
        _source_seq_id: i32,
        surgery_seq_id: i32,
        ast_slice: &AstSlice,
        diagnostic: &Diagnostic,
    ) -> Result<String, String> {
        info!("Spawning Macro-Fixer on seq_id={} to fix lines {}-{}", surgery_seq_id, ast_slice.start_line, ast_slice.end_line);

        let user_prompt = format!(
            "The following code block from `{}` has a runtime/syntax error.\n\n\
            ```\n{}\n```\n\n\
            Error:\n{}\n\n\
            Please provide the fully corrected code block (and ONLY the code block). Use standard ``` code fences.",
            ast_slice.file_path, ast_slice.content, diagnostic.error_message
        );
        let micro_prompt = model.format_chat(None, &user_prompt);

        let prompt_tokens = model.tokenize(&micro_prompt, true)?;
        ctx.eval_tokens(&prompt_tokens, surgery_seq_id)?;

        let config = SamplingConfig {
            temperature: 0.2,
            ..Default::default()
        };
        let mut sampler = NativeLlamaSampler::new(&config, model.n_vocab())?;

        let mut fixed_block = String::new();
        let mut in_code_block = false;
        
        for _ in 0..1024 {
            let token = ctx.sample(&mut sampler)?;
            let text = model.token_to_piece(token).unwrap_or_default();
            
            if text.is_empty() || model.is_stop_token(token) || model.is_stop_piece(&text) {
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

        ctx.kv_cache_seq_rm(surgery_seq_id, 0, -1)?;
        info!("Macro-Fixer seq_id={} terminated and cache dropped.", surgery_seq_id);

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

    /// Primary Sovereign In-Situ Repair Engine:
    /// Executes targeted Analyze -> Read -> Edit cycle on active VFS code with strict Attention Masking.
    /// Preserves 100% of unbroken code while applying localized compiler/runtime corrections.
    pub fn repair_in_situ_direct(
        model: &NativeLlamaModel,
        ctx: &mut NativeLlamaContext,
        vfs: &MemoryVfs,
        target_file: &str,
        error_line: Option<usize>,
        error_message: &str,
        raw_compiler_output: &str,
        dag_contract: Option<&str>,
        write_set: &[PathBuf],
        fixer_seq_id: i32,
        attempt: usize,
    ) -> Result<FixAttemptReport, String> {
        let path = PathBuf::from(target_file);
        let full_code = vfs.read_string(&path)
            .ok_or_else(|| format!("Target file '{}' not found in VFS for in-situ repair", target_file))?;

        if full_code.trim().is_empty() {
            return Ok(FixAttemptReport {
                success: false,
                patch_applied: false,
                summary: "Target file is empty, cannot repair in-situ".to_string(),
                target_file: target_file.to_string(),
                error_line,
            });
        }

        let lines: Vec<&str> = full_code.lines().collect();
        let total_lines = lines.len();

        // 0. Deterministic Compiler Help Suggestion extraction (Tier 0 - Zero LLM Hallucination)
        // If rustc or static analyzer explicitly gave the exact import statement or fix, inject immediately!
        for line in raw_compiler_output.lines() {
            let tr = line.trim();
            // Case 1: rustc diff suggestions like: " 1 + use std::time::Instant;" or "+ use std::time::Instant;"
            if let Some(plus_idx) = tr.find('+') {
                let candidate = tr[plus_idx + 1..].trim();
                if (candidate.starts_with("use ") || candidate.starts_with("import ")) && candidate.ends_with(';') {
                    let import_stmt = candidate.trim_matches('`').trim();
                    if !full_code.contains(import_stmt) {
                        let updated = format!("{}\n{}", import_stmt, full_code);
                        vfs.write_file(&path, updated.as_bytes());
                        info!("Sovereign CodeFixer deterministically injected rustc suggested import '{}' into '{}'", import_stmt, target_file);
                        return Ok(FixAttemptReport {
                            success: true,
                            patch_applied: true,
                            summary: format!("Injected suggested import '{}' into '{}'", import_stmt, target_file),
                            target_file: target_file.to_string(),
                            error_line,
                        });
                    }
                }
            }
            // Case 2: Inline suggestions like: "help: consider importing this struct: `use std::time::Instant;`"
            if tr.starts_with("help: consider importing this") || tr.starts_with("help: consider adding") {
                if let Some(col) = tr.find(':') {
                    let after = tr[col + 1..].trim();
                    if let Some(second_col) = after.find(':') {
                        let import_stmt = after[second_col + 1..].trim().trim_matches('`').trim();
                        if (import_stmt.starts_with("use ") || import_stmt.starts_with("import ")) && import_stmt.ends_with(';') {
                            if !full_code.contains(import_stmt) {
                                let updated = format!("{}\n{}", import_stmt, full_code);
                                vfs.write_file(&path, updated.as_bytes());
                                info!("Sovereign CodeFixer deterministically injected rustc suggested import '{}' into '{}'", import_stmt, target_file);
                                return Ok(FixAttemptReport {
                                    success: true,
                                    patch_applied: true,
                                    summary: format!("Injected suggested import '{}' into '{}'", import_stmt, target_file),
                                    target_file: target_file.to_string(),
                                    error_line,
                                });
                            }
                        }
                    }
                }
            }
        }

        // 0b. Duplicate declaration excision: if a symbol is defined multiple times at error_line, excise duplicate AST block
        if error_message.contains("defined multiple times") {
            if let Some(err_l) = error_line {
                if let Ok(slice) = crate::sandbox::diagnostics::TopologicalDiagnostic::extract_from_source(target_file, &full_code, err_l) {
                    let slice_lines: Vec<&str> = slice.content.lines().collect();
                    if let Some(first_line) = slice_lines.first() {
                        let first_tr = first_line.trim();
                        if first_tr.starts_with("enum ") || first_tr.starts_with("pub enum ") 
                            || first_tr.starts_with("struct ") || first_tr.starts_with("pub struct ") 
                            || first_tr.starts_with("fn ") || first_tr.starts_with("pub fn ") 
                        {
                            let start_idx = slice.start_line.saturating_sub(1);
                            let end_idx = slice.end_line.min(lines.len());
                            let mut updated_lines = lines[..start_idx].to_vec();
                            if end_idx < lines.len() {
                                updated_lines.extend_from_slice(&lines[end_idx..]);
                            }
                            let updated_code = updated_lines.join("\n");
                            vfs.write_file(&path, updated_code.as_bytes());
                            info!("Sovereign CodeFixer excised duplicate declaration block (lines {}-{}) in '{}'", slice.start_line, slice.end_line, target_file);
                            return Ok(FixAttemptReport {
                                success: true,
                                patch_applied: true,
                                summary: format!("Excised duplicate declaration block at lines {}-{} in '{}'", slice.start_line, slice.end_line, target_file),
                                target_file: target_file.to_string(),
                                error_line,
                            });
                        }
                    }
                }
            }
        }

        // 1. Attention Masking: Extract Focal Scope around error_line via Topological AstSlice or Window
        let ast_slice_opt = if let Some(err_l) = error_line {
            crate::sandbox::diagnostics::TopologicalDiagnostic::extract_from_source(target_file, &full_code, err_l).ok()
        } else {
            None
        };

        let (focal_start, focal_end, focal_content) = if let Some(ref slice) = ast_slice_opt {
            (slice.start_line, slice.end_line, slice.content.clone())
        } else if let Some(err_l) = error_line {
            let start = if err_l > 15 { err_l - 15 } else { 1 };
            let end = (err_l + 15).min(total_lines);
            let slice = lines[start - 1..end].join("\n");
            (start, end, slice)
        } else {
            let end = total_lines.min(60);
            let slice = lines[0..end].join("\n");
            (1, end, slice)
        };

        // Extract top imports header if distinct from focal window (for Rust/Python)
        let mut header_context = String::new();
        if focal_start > 25 {
            let header_slice: Vec<&str> = lines.iter()
                .take(25)
                .filter(|l| {
                    let tr = l.trim();
                    tr.starts_with("use ") || tr.starts_with("import ") || tr.starts_with("from ") || tr.starts_with("#include")
                })
                .copied()
                .collect();
            if !header_slice.is_empty() {
                header_context = format!("\n[EXISTING IMPORTS HEADER (Lines 1-25)]:\n```\n{}\n```\n", header_slice.join("\n"));
            }
        }

        let ext = Path::new(target_file).extension().and_then(|e| e.to_str()).unwrap_or("text");

        // 1b. Extract Read-Only Preceding Anchor Block
        let preceding_anchor_opt = crate::sandbox::diagnostics::TopologicalDiagnostic::extract_preceding_anchor(target_file, &full_code, focal_start);
        let preceding_anchor_formatted = if let Some(ref anchor) = preceding_anchor_opt {
            format!(
                "\n[VERIFIED READ-ONLY PRECEDING ANCHOR (Lines {}-{}) - Do NOT modify or repeat this block]:\n```{ext}\n{}\n```\n",
                anchor.start_line, anchor.end_line, anchor.content
            )
        } else {
            String::new()
        };

        // 2. Extract Compiler Suggestions from raw output
        let compiler_suggestions: Vec<&str> = raw_compiler_output.lines()
            .filter(|l| {
                let lower = l.to_lowercase();
                lower.contains("help:") || lower.contains("suggestion:") || lower.contains("consider") || lower.contains("-->") || lower.contains("error[")
            })
            .take(10)
            .collect();
        let suggestions_formatted = if !compiler_suggestions.is_empty() {
            format!("\n[COMPILER DIAGNOSTICS & HINTS]:\n{}\n", compiler_suggestions.join("\n"))
        } else {
            String::new()
        };

        let contract_formatted = dag_contract.map(|c| {
            format!("\n[DAG CONTRACT INVARIANT]:\n{}\n(MANDATE: Keep all public signatures intact. Do not break downstream callers.)\n", c)
        }).unwrap_or_default();

        // 3. Assemble Cognitive Attention-Masked Prompt
        let system_directive = "You are the Sovereign Code Fixer. Your sole duty is targeted in-situ code repair.\n\
            Preserve all existing functions, signatures, and unbroken logic.\n\
            Output ONLY the targeted correction without conversation.";

        let user_prompt = format!(
            "Target File: `{}` (Attempt {} of 3)\n\
            Diagnostic Error: {}\n\
            Error Line: {:?}\n\
            {}{}{}{}\n\
            [FOCAL CODE BLOCK (Lines {}-{})]:\n```{ext}\n{}\n```\n\n\
            INSTRUCTION:\n\
            Fix the diagnostic error above in this code block.\n\
            Provide the complete corrected replacement block inside standard ```{ext} ... ``` code fences.\n\
            Preserve all unaffected code, signatures, and imports.\n\
            Output the corrected code block now:",
            target_file, attempt, error_message, error_line,
            contract_formatted, suggestions_formatted, header_context, preceding_anchor_formatted,
            focal_start, focal_end, focal_content,
            ext=ext
        );

        let chat_prompt = model.format_chat(Some(system_directive), &user_prompt);

        // 4. Evaluate on Fixer sequence clone
        ctx.kv_cache_seq_rm(fixer_seq_id, 0, -1)?;
        let prompt_tokens = model.tokenize(&chat_prompt, true)?;
        ctx.eval_tokens(&prompt_tokens, fixer_seq_id)?;

        let config = SamplingConfig {
            temperature: 0.15,
            ..Default::default()
        };
        let mut sampler = NativeLlamaSampler::new(&config, model.n_vocab())?;

        let mut output_buf = String::new();
        let mut in_fence = false;
        let mut think_tokens_count = 0;
        let max_think_budget = 192;

        for _ in 0..1024 {
            let token = ctx.sample(&mut sampler)?;
            let text = model.token_to_piece(token).unwrap_or_default();
            if text.is_empty() || model.is_stop_token(token) || model.is_stop_piece(&text) {
                break;
            }
            output_buf.push_str(&text);
            ctx.eval_tokens(&[token], fixer_seq_id)?;

            let has_open_think = output_buf.contains("<think>") || output_buf.contains("<thought>");
            let has_close_think = output_buf.contains("</think>") || output_buf.contains("</thought>");
            let in_think = has_open_think && !has_close_think;

            if in_think {
                think_tokens_count += 1;
                if think_tokens_count >= max_think_budget {
                    let force_close = "\n</think>\n";
                    output_buf.push_str(force_close);
                    if let Ok(force_tokens) = model.tokenize(force_close, false) {
                        let _ = ctx.eval_tokens(&force_tokens, fixer_seq_id);
                    }
                }
                continue;
            }

            if output_buf.contains("```") && !in_fence {
                in_fence = true;
            } else if in_fence {
                if let Some(first_fence) = output_buf.find("```") {
                    if output_buf[first_fence + 3..].contains("```") {
                        break;
                    }
                }
            } else if output_buf.contains(">>>>>>>") {
                break;
            }
        }
        ctx.kv_cache_seq_rm(fixer_seq_id, 0, -1)?;

        let clean_output = crate::planner::swarm_coordinator::strip_thought(&output_buf);

        // 5. Apply Patch to VFS via Multi-Tiered Strategy
        // Tier 1: Check for explicit PATCH block
        if clean_output.contains("<<<<<<< TARGET") {
            if let Ok(_) = vfs.patch_file_checked(&path, &clean_output, write_set) {
                info!("Sovereign CodeFixer successfully applied PATCH block to '{}'", target_file);
                return Ok(FixAttemptReport {
                    success: true,
                    patch_applied: true,
                    summary: format!("Applied targeted patch to '{}'", target_file),
                    target_file: target_file.to_string(),
                    error_line,
                });
            }
        }

        // Tier 2: Code block replacement for the extracted focal block
        if let Some(start) = clean_output.find("```") {
            let mut start_idx = start + 3;
            if let Some(nl) = clean_output[start_idx..].find('\n') {
                start_idx += nl + 1;
            }
            if let Some(end) = clean_output[start_idx..].find("```") {
                let block_code = clean_output[start_idx..start_idx + end].trim();
                if !block_code.is_empty() {
                    // Direct focal block replacement
                    if let Ok(true) = vfs.patch_file(&path, &focal_content, block_code) {
                        info!("Sovereign CodeFixer applied focal block replacement to '{}'", target_file);
                        return Ok(FixAttemptReport {
                            success: true,
                            patch_applied: true,
                            summary: format!("Applied focal block replacement to '{}' (lines {}-{})", target_file, focal_start, focal_end),
                            target_file: target_file.to_string(),
                            error_line,
                        });
                    }

                    // Trimmed focal block replacement
                    if let Ok(true) = vfs.patch_file(&path, focal_content.trim(), block_code) {
                        info!("Sovereign CodeFixer applied trimmed block replacement to '{}'", target_file);
                        return Ok(FixAttemptReport {
                            success: true,
                            patch_applied: true,
                            summary: format!("Applied trimmed block replacement to '{}'", target_file),
                            target_file: target_file.to_string(),
                            error_line,
                        });
                    }

                    // Direct line-range replacement (Guaranteed exact in-situ block splice)
                    if focal_start >= 1 && focal_end <= lines.len() && focal_start <= focal_end {
                        let mut updated_lines = lines[..focal_start - 1].to_vec();
                        updated_lines.push(block_code);
                        if focal_end < lines.len() {
                            updated_lines.extend_from_slice(&lines[focal_end..]);
                        }
                        let updated = updated_lines.join("\n");
                        vfs.write_file(&path, updated.as_bytes());
                        info!("Sovereign CodeFixer applied exact line-range block replacement ({}-{}) to '{}'", focal_start, focal_end, target_file);
                        return Ok(FixAttemptReport {
                            success: true,
                            patch_applied: true,
                            summary: format!("Applied line-range replacement to '{}' (lines {}-{})", target_file, focal_start, focal_end),
                            target_file: target_file.to_string(),
                            error_line,
                        });
                    }

                    // Single-line targeted replacement if model output is a single statement
                    if let Some(err_l) = error_line {
                        if block_code.lines().count() <= 2 {
                            if let Some(err_line_str) = lines.get(err_l.saturating_sub(1)) {
                                if let Ok(true) = vfs.patch_file(&path, err_line_str.trim(), block_code) {
                                    info!("Sovereign CodeFixer applied single-line replacement at line {} in '{}'", err_l, target_file);
                                    return Ok(FixAttemptReport {
                                        success: true,
                                        patch_applied: true,
                                        summary: format!("Applied single-line replacement at line {} in '{}'", err_l, target_file),
                                        target_file: target_file.to_string(),
                                        error_line,
                                    });
                                }
                            }
                        }
                    }

                    // Missing import statement insertion
                    if (block_code.starts_with("use ") || block_code.starts_with("import ")) && !full_code.contains(block_code) {
                        let updated = format!("{}\n{}", block_code, full_code);
                        vfs.write_file(&path, updated.as_bytes());
                        info!("Sovereign CodeFixer injected missing import into '{}'", target_file);
                        return Ok(FixAttemptReport {
                            success: true,
                            patch_applied: true,
                            summary: format!("Injected missing import '{}' into '{}'", block_code, target_file),
                            target_file: target_file.to_string(),
                            error_line,
                        });
                    }

                    // Missing symbol definition insertion (e.g. enum / struct)
                    if (block_code.contains("enum ") || block_code.contains("struct ")) && !full_code.contains(block_code) {
                        let insert_pos = lines.iter()
                            .position(|l| l.trim().starts_with("pub struct ") || l.trim().starts_with("struct ") || l.trim().starts_with("impl "))
                            .unwrap_or(lines.len());
                        let mut updated_lines = lines[..insert_pos].to_vec();
                        updated_lines.push(block_code);
                        updated_lines.push("");
                        updated_lines.extend_from_slice(&lines[insert_pos..]);
                        let updated = updated_lines.join("\n");
                        vfs.write_file(&path, updated.as_bytes());
                        info!("Sovereign CodeFixer injected missing symbol definition into '{}'", target_file);
                        return Ok(FixAttemptReport {
                            success: true,
                            patch_applied: true,
                            summary: format!("Injected missing definition into '{}'", target_file),
                            target_file: target_file.to_string(),
                            error_line,
                        });
                    }
                }
            }
        }

        // Tier 3: Deterministic Compiler Help Suggestion extraction (e.g. rustc "help: consider importing...")
        for line in raw_compiler_output.lines() {
            let tr = line.trim();
            if tr.starts_with("help: consider importing this") || tr.starts_with("help: consider adding") {
                if let Some(col) = tr.find(':') {
                    let after = tr[col + 1..].trim();
                    if let Some(second_col) = after.find(':') {
                        let import_stmt = after[second_col + 1..].trim().trim_matches('`').trim();
                        if (import_stmt.starts_with("use ") || import_stmt.starts_with("import ")) && import_stmt.ends_with(';') {
                            if !full_code.contains(import_stmt) {
                                let updated = format!("{}\n{}", import_stmt, full_code);
                                vfs.write_file(&path, updated.as_bytes());
                                info!("Sovereign CodeFixer deterministically injected rustc suggested import '{}' into '{}'", import_stmt, target_file);
                                return Ok(FixAttemptReport {
                                    success: true,
                                    patch_applied: true,
                                    summary: format!("Injected suggested import '{}' into '{}'", import_stmt, target_file),
                                    target_file: target_file.to_string(),
                                    error_line,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(FixAttemptReport {
            success: false,
            patch_applied: false,
            summary: format!("No matching patch or block replacement could be applied to '{}'", target_file),
            target_file: target_file.to_string(),
            error_line,
        })
    }
}
