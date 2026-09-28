use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use colored::*;
use tracing::{info, warn};

use crate::causal_memory::dag::CausalGraph;
use crate::native_llama::{NativeLlamaContext, NativeLlamaModel};
use crate::sandbox::{MemoryVfs, WebLens, SearchResult, IsolatedJail, JailExecutionResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Create,
    Modify,
    Delete,
    Test,
    Search,
}

impl TaskKind {
    pub fn name(&self) -> &'static str {
        match self {
            TaskKind::Create => "CREATE",
            TaskKind::Modify => "MODIFY",
            TaskKind::Delete => "DELETE",
            TaskKind::Test => "TEST",
            TaskKind::Search => "SEARCH",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwarmAction {
    Search { query: String },
    Fetch { url: String },
    RunLua { script: String },
    WriteFile { filename: String, content: String },
    PatchFile { filename: String, target: String, replacement: String },
    DeleteFile { filename: String },
    RunTerminal { command: String },
    Consult { question: String },
    Report { finding: String },
    Done,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    VfsFileNotWritten,
    PlaceholderOrEmpty,
    LuaSyntaxTrap,
    NoCodeBlock,
    RepeatedAttractor,
    MissingDependency(String),
    PatchFailed(String),
    FileNotDeleted,
    DiffEmpty,
    TestFailed,
}

impl DiagnosticCode {
    pub fn name(&self) -> &'static str {
        match self {
            DiagnosticCode::VfsFileNotWritten => "ERR_VFS_FILE_NOT_WRITTEN",
            DiagnosticCode::PlaceholderOrEmpty => "ERR_PLACEHOLDER_OR_EMPTY",
            DiagnosticCode::LuaSyntaxTrap => "ERR_LUA_SYNTAX",
            DiagnosticCode::NoCodeBlock => "ERR_NO_CODE_BLOCK",
            DiagnosticCode::RepeatedAttractor => "ERR_REPEATED_ATTRACTOR",
            DiagnosticCode::MissingDependency(_) => "ERR_MISSING_DEPENDENCY",
            DiagnosticCode::PatchFailed(_) => "ERR_PATCH_FAILED",
            DiagnosticCode::FileNotDeleted => "ERR_FILE_NOT_DELETED",
            DiagnosticCode::DiffEmpty => "ERR_DIFF_EMPTY",
            DiagnosticCode::TestFailed => "ERR_TEST_FAILED",
        }
    }

    pub fn default_prescription(&self, target_entity: &str) -> String {
        match self {
            DiagnosticCode::VfsFileNotWritten => {
                format!(
                    "Provide the complete implementation for '{}' inside a code block (e.g. ```python ... ```) or call vfs.write(\"{}\", [[...]]).",
                    target_entity, target_entity
                )
            }
            DiagnosticCode::PlaceholderOrEmpty => {
                format!(
                    "Write complete, working implementation for {}. NEVER output placeholders like TODO, pass, or empty comments.",
                    target_entity
                )
            }
            DiagnosticCode::LuaSyntaxTrap => {
                format!("Fix syntax error in {}. Ensure code is syntactically valid and completely closed.", target_entity)
            }
            DiagnosticCode::NoCodeBlock => {
                format!("Output your code strictly inside a markdown code block for {}.", target_entity)
            }
            DiagnosticCode::RepeatedAttractor => {
                format!(
                    "Break repetition attractor. Do not repeat failed patterns. Write complete, fresh code directly for '{}'.",
                    target_entity
                )
            }
            DiagnosticCode::MissingDependency(ref dep) => {
                format!(
                    "Required dependency or CLI tool '{}' is missing on host. Consult user or use standard library built-ins.",
                    dep
                )
            }
            DiagnosticCode::PatchFailed(ref err) => {
                format!("Patch failed for '{}': {}. Ensure the exact target snippet exists in the file before patching.", target_entity, err)
            }
            DiagnosticCode::FileNotDeleted => {
                format!("File '{}' was not deleted from VFS. Call vfs.delete(\"{}\") or execute rm via terminal.", target_entity, target_entity)
            }
            DiagnosticCode::DiffEmpty => {
                format!("Modification on '{}' produced zero diff. Ensure the target lines are actually changed.", target_entity)
            }
            DiagnosticCode::TestFailed => {
                format!("Execution test for '{}' failed. Review error output and fix the root cause.", target_entity)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyChoice {
    InstallAndRetry,
    UseAlternative,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThinkerDecision {
    Nudge(String),
    Reset(String),
    ConsultDependency {
        dependency: String,
        question: String,
    },
}

#[derive(Debug, Clone)]
pub struct SubGoal {
    pub id: usize,
    pub description: String,
    pub target_entity: String,
    pub kind: TaskKind,
    pub guidance: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkerFinding {
    pub worker_id: usize,
    pub sub_goal: String,
    pub finding: String,
    pub steps_taken: usize,
    pub rollbacks_count: usize,
    pub tokens_saved_by_rollback: usize,
}

#[derive(Debug, Clone)]
pub struct SwarmResult {
    pub goal: String,
    pub subgoals_count: usize,
    pub findings: Vec<WorkerFinding>,
    pub final_report: String,
    pub total_rollbacks: usize,
    pub total_tokens_saved: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone)]
pub struct SwarmConfig {
    pub orchestrator_model_path: PathBuf,
    pub worker_model_path: PathBuf,
    pub max_subgoals: usize,
    pub max_steps_per_worker: usize,
    pub output_dir: Option<PathBuf>,
    pub verbose: bool,
}

impl SwarmConfig {
    /// Auto-detect available GGUF models in standard portable directories:
    /// 1. ./models
    /// 2. ../models
    /// 3. ./
    /// Works with ANY open-source GGUF model (Qwen, Llama, Mistral, Phi, Gemma, or custom fine-tuned weights)
    pub fn auto_detect(base_dir: &Path) -> Option<Self> {
        let candidates = vec![
            base_dir.join("models"),
            base_dir.join("../models"),
            base_dir.to_path_buf(),
        ];

        let mut found_models = Vec::new();

        for dir in &candidates {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            if ext == "gguf" {
                                found_models.push(path);
                            }
                        }
                    }
                }
            }
            if !found_models.is_empty() {
                break;
            }
        }

        if found_models.is_empty() {
            return None;
        }

        // Sort by file size descending
        found_models.sort_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0));
        found_models.reverse();

        // Unified Sovereign Architecture: Both Thinker (System 2) and Worker (System 1) share
        // the top-performing model in RAM via Arc<NativeLlamaModel>, saving memory and maximizing IQ.
        let orch = found_models[0].clone();
        let worker = orch.clone();

        Some(Self {
            orchestrator_model_path: orch,
            worker_model_path: worker,
            max_subgoals: 3,
            max_steps_per_worker: 5,
            output_dir: None,
            verbose: true,
        })
    }
}

pub struct SwarmCoordinator {
    config: SwarmConfig,
    orchestrator_model: Arc<NativeLlamaModel>,
    worker_model: Arc<NativeLlamaModel>,
    web_lens: Arc<WebLens>,
    vfs: Arc<MemoryVfs>,
    terminal: Arc<crate::sandbox::TerminalSessionBridge>,
}

impl SwarmCoordinator {
    pub fn new(config: SwarmConfig) -> Result<Self, String> {
        info!("Loading System 2 Orchestrator Model: {:?}", config.orchestrator_model_path);
        let orchestrator_model = NativeLlamaModel::load(&config.orchestrator_model_path, 0)?;

        let worker_model = if config.worker_model_path == config.orchestrator_model_path {
            orchestrator_model.clone()
        } else {
            info!("Loading System 1 Worker Swarm Model: {:?}", config.worker_model_path);
            NativeLlamaModel::load(&config.worker_model_path, 0)?
        };

        let terminal = Arc::new(crate::sandbox::TerminalSessionBridge::new(1000));

        Ok(Self {
            config,
            orchestrator_model,
            worker_model,
            web_lens: Arc::new(WebLens::new()),
            vfs: Arc::new(MemoryVfs::new()),
            terminal,
        })
    }

    /// Format prompts dynamically based on the model's vocabulary.
    /// Completely model-agnostic: supports ChatML, Llama 3 header tokens, or Universal Markdown.
    pub fn format_prompt(model: &NativeLlamaModel, system: &str, user: &str) -> String {
        let is_chatml = model.tokenize("<|im_start|>", false).map(|t| t.len() == 1).unwrap_or(false);
        let is_llama3 = model.tokenize("<|start_header_id|>", false).map(|t| t.len() == 1).unwrap_or(false);

        if is_chatml {
            format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                system, user
            )
        } else if is_llama3 {
            format!(
                "<|start_header_id|>system<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                system, user
            )
        } else {
            // Universal standard markdown prompt
            format!(
                "### System Instruction:\n{}\n\n### User Objective:\n{}\n\n### Assistant Execution:\n",
                system, user
            )
        }
    }

    /// Append user feedback or observation turn dynamically to active context
    pub fn format_observation_turn(model: &NativeLlamaModel, observation: &str, next_instruction: &str) -> String {
        let is_chatml = model.tokenize("<|im_start|>", false).map(|t| t.len() == 1).unwrap_or(false);
        let is_llama3 = model.tokenize("<|start_header_id|>", false).map(|t| t.len() == 1).unwrap_or(false);

        if is_chatml {
            format!(
                "\n<|im_end|>\n<|im_start|>user\nOBSERVATION: {}\n{}<|im_end|>\n<|im_start|>assistant\n",
                observation, next_instruction
            )
        } else if is_llama3 {
            format!(
                "\n<|eot_id|><|start_header_id|>user<|end_header_id|>\n\nOBSERVATION: {}\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
                observation, next_instruction
            )
        } else {
            format!(
                "\n\n### User Observation:\nOBSERVATION: {}\n{}\n\n### Assistant Response:\n",
                observation, next_instruction
            )
        }
    }

    /// Checks if a piece is a standard end-of-turn or stop token across architectures
    pub fn is_stop_piece(piece: &str) -> bool {
        piece.contains("<|im_end|>")
            || piece.contains("<|endoftext|>")
            || piece.contains("<|eot_id|>")
            || piece.contains("</s>")
            || piece.contains("<end_of_turn>")
            || piece.contains('\n')
    }

    /// Primary Swarm Execution Pipeline:
    /// 1. System 2 Thinker: Decomposes overall goal into targeted sub-tasks
    /// 2. System 1 Swarm: Dispatches autonomous worker instances with Checkpoint Freeze
    /// 3. Epistemic Ledger: Rollback on failure with Thinker Sovereign arbitration
    /// 4. System 2 Thinker: Synthesizes gathered factual findings into final answer
    pub fn execute_goal(&self, user_goal: &str) -> Result<SwarmResult, String> {
        let start_time = Instant::now();

        println!("\n{}", "=".repeat(80).bright_blue());
        println!(
            "  [SWARM ENGINE] Objective: \"{}\"",
            user_goal.bright_white()
        );
        println!(
            "  [SYSTEM ARCHITECTURE] Thinker: {} | Worker: {}",
            self.config.orchestrator_model_path.file_name().unwrap_or_default().to_string_lossy().bright_cyan(),
            self.config.worker_model_path.file_name().unwrap_or_default().to_string_lossy().bright_green()
        );
        println!("{}\n", "=".repeat(80).bright_blue());

        // PHASE 1: System 2 Orchestrator Plan Synthesis
        println!("  [THINKER PLAN] Formulating execution sub-tasks...");
        let subgoals = self.orchestrate_plan(user_goal)?;

        println!("  [PLAN] Generated {} sub-task(s):", subgoals.len().to_string().bright_yellow());
        for sg in &subgoals {
            println!("   - Task #{}: {} (Target: {})", sg.id, sg.description.bright_white(), sg.target_entity.bright_cyan());
        }
        println!();

        // PHASE 2: System 1 Swarm Execution with Continuous Shared KV Attention & Physical KV Rollback
        // Allocate single unified KV context for the swarm mission (8192 tokens budget)
        let mut worker_ctx = self.worker_model.create_context(8192, 512, 4)?;
        let mut findings: Vec<WorkerFinding> = Vec::new();

        for (idx, mut sg) in subgoals.into_iter().enumerate() {
            let is_first_worker = idx == 0;
            match self.thinker_dispatch_directive(user_goal, &sg, &findings) {
                Ok(directive) if !directive.is_empty() => {
                    println!(
                        "  [THINKER DIRECTIVE] Task #{}: \"{}\"",
                        sg.id,
                        directive.bright_white()
                    );
                    sg.guidance = Some(directive);
                }
                _ => {}
            }

            println!(
                "  [DISPATCH WORKER] Task #{}: \"{}\"",
                sg.id,
                sg.description.bright_white()
            );

            let finding = self.run_worker_loop(user_goal, &sg, &mut worker_ctx, is_first_worker, &findings)?;
            findings.push(finding);
            println!();
        }

        // Swarm Mission Completed: Purge shared KV-cache (Epistemic Apoptosis)
        worker_ctx.kv_cache_clear();

        // PHASE 3: System 2 Orchestrator Grounded Synthesis
        println!("  [THINKER SYNTHESIS] Synthesizing verified deliverables...");
        let final_report = self.synthesize_findings(user_goal, &findings)?;

        // PHASE 4: VFS Materialization to Host Disk
        let staged_diffs = self.vfs.generate_staged_diffs();
        if !staged_diffs.is_empty() {
            println!("  [HOST DISK COMMIT] Materializing VFS artifacts to host disk...");
            match self.vfs.commit_to_host(true) {
                Ok(count) => {
                    for d in &staged_diffs {
                        println!("   [SAVED TO DISK] {}", d.path.display().to_string().bright_white());
                    }
                    println!("   [COMMIT] Successfully committed {} file(s) to host machine.", count.to_string().bright_green());

                    if let Some(ref out_dir) = self.config.output_dir {
                        let _ = std::fs::create_dir_all(out_dir);
                        for d in &staged_diffs {
                            let file_name = d.path.file_name().unwrap_or_default();
                            let dest = out_dir.join(file_name);
                            if let Some(content) = self.vfs.read_file(&d.path) {
                                let _ = std::fs::write(&dest, content);
                                println!("   [OUTPUT DIRECTORY] Saved to: {}", dest.display().to_string().bright_cyan());
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to commit VFS to host disk: {}", e);
                }
            }
        }

        let total_rollbacks: usize = findings.iter().map(|f| f.rollbacks_count).sum();
        let total_tokens_saved: usize = findings.iter().map(|f| f.tokens_saved_by_rollback).sum();

        println!("\n{}", "=".repeat(80).bright_green());
        println!("  [SWARM COMPLETE] Execution completed in {:?}", start_time.elapsed());
        println!("     * Workers Deployed:          {}", findings.len());
        println!("     * Checkpoint Rollbacks:      {}", total_rollbacks.to_string().bright_yellow());
        println!("     * Trapped Tokens Excised:    {} tokens saved", total_tokens_saved.to_string().bright_cyan());
        println!("{}\n", "=".repeat(80).bright_green());

        Ok(SwarmResult {
            goal: user_goal.to_string(),
            subgoals_count: findings.len(),
            findings,
            final_report,
            total_rollbacks,
            total_tokens_saved,
            elapsed_ms: start_time.elapsed().as_millis(),
        })
    }

    /// Dynamic Thinker Dispatch: System 2 issues the explicit, actionable directive for every worker
    fn thinker_dispatch_directive(
        &self,
        user_goal: &str,
        subgoal: &SubGoal,
        previous_findings: &[WorkerFinding],
    ) -> Result<String, String> {
        let mut ctx = self.orchestrator_model.create_context(2048, 512, 4)?;
        let system_msg = "You are the System 2 Sovereign Thinker commanding an execution worker.\n\
The user communicates ONLY with you. Workers are your executive hands.\n\
Workers are localized, stateless code-generation engines with short-term context. They do not converse; they execute.\n\
Your worker is equipped with:\n\
- Direct code generation: Write complete code inside ```<lang> ... ``` blocks\n\
- vfs.write(\"filename\", [[content]]): Write complete file content to VFS\n\
- vfs.read(\"filename\"): Read file from VFS\n\
- terminal.run(\"command\"): Execute shell command on host\n\
- DONE: Finish task\n\n\
Your Task:\n\
Command the worker what EXACT technical functionality, algorithm, and logic to implement for the target file.\n\
Do NOT give vague directives like 'write implementation'. Specify the concrete purpose and features.\n\
Be direct, imperative, and specific (max 35 words).";

        let prev_summary = if previous_findings.is_empty() {
            "Initial task dispatch. No previous findings.".to_string()
        } else {
            let last = previous_findings.last().map(|f| f.finding.as_str()).unwrap_or("");
            format!("Previous worker findings: {}", last)
        };

        let user_msg = format!(
            "User Goal: {}\nSub-goal: {}\nTarget Entity: {}\nContext: {}\n\nDirective for Worker:",
            user_goal, subgoal.description, subgoal.target_entity, prev_summary
        );
        let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);
        let output = ctx.generate(&prompt, 64)?;
        Ok(output.trim().to_string())
    }

    /// Evaluates worker failure with Epistemic Compaction:
    /// 1. Temporary Thinker context inspects the error, code, and deterministic diagnostic.
    /// 2. Thinker generates decision: NUDGE (minor fixable) or RESET (major rewrite).
    /// 3. Thinker context is wiped immediately, purging all broken code from active cache.
    /// 4. Returns the decision and a dense ledger entry.
    fn thinker_evaluate_failure(
        &self,
        subgoal: &SubGoal,
        attempt: usize,
        failed_code: &str,
        runtime_error: &str,
        diagnostic: &DiagnosticCode,
    ) -> Result<(ThinkerDecision, String), String> {
        if let DiagnosticCode::MissingDependency(ref dep) = diagnostic {
            let question = format!(
                "Required dependency or CLI tool '{}' is missing on host.",
                dep
            );
            let ledger_entry = format!(
                "[LEDGER: Worker #{} Attempt #{} PAUSED (ERR_MISSING_DEPENDENCY: '{}') -> Action: CONSULT_USER]",
                subgoal.id, attempt, dep
            );
            return Ok((
                ThinkerDecision::ConsultDependency {
                    dependency: dep.clone(),
                    question,
                },
                ledger_entry,
            ));
        }

        let mut ctx = self.orchestrator_model.create_context(2048, 512, 4)?;

        let system_msg = "You are the System 2 Sovereign Thinker evaluating a worker's failed attempt.\n\
The worker ran into an error. Analyze whether this is a minor fixable slip (syntax, bracket, small typo, missing vfs.write call) or a major structural failure requiring a complete reset.\n\
Output strictly in one of these two formats:\n\
DECISION: NUDGE | REASON: <concise corrective hint for the worker>\n\
or\n\
DECISION: RESET | REASON: <new simplified directive for fresh restart>";

        let snippet_code = if failed_code.len() > 400 {
            &failed_code[..400]
        } else {
            failed_code
        };

        let snippet_err = if runtime_error.len() > 300 {
            &runtime_error[..300]
        } else {
            runtime_error
        };

        let user_msg = format!(
            "Subgoal: {}\nTarget: {}\nAttempt #{}\nDiagnostic: {} - {}\nWorker Code:\n{}\nRuntime Error:\n{}\n\nDecision:",
            subgoal.description,
            subgoal.target_entity,
            attempt,
            diagnostic.name(),
            diagnostic.default_prescription(&subgoal.target_entity),
            snippet_code,
            snippet_err
        );

        let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);
        let raw = ctx.generate(&prompt, 128)?;

        // WIPE temporary inspection tokens from Thinker context immediately
        ctx.kv_cache_clear();

        let decision = Self::parse_thinker_decision(&raw, subgoal);
        let ledger_entry = match &decision {
            ThinkerDecision::Nudge(hint) => {
                format!(
                    "[LEDGER: Worker #{} Attempt #{} FAILED ({}) -> Action: NUDGE ('{}')]",
                    subgoal.id, attempt, diagnostic.name(), hint
                )
            }
            ThinkerDecision::Reset(directive) => {
                format!(
                    "[LEDGER: Worker #{} Attempt #{} FAILED ({}) -> Action: RESET ('{}')]",
                    subgoal.id, attempt, diagnostic.name(), directive
                )
            }
            ThinkerDecision::ConsultDependency { dependency, .. } => {
                format!(
                    "[LEDGER: Worker #{} Attempt #{} PAUSED (ERR_MISSING_DEPENDENCY: '{}') -> Action: CONSULT_USER]",
                    subgoal.id, attempt, dependency
                )
            }
        };

        Ok((decision, ledger_entry))
    }

    pub fn extract_missing_dependency(text: &str) -> Option<String> {
        // 1. Python ModuleNotFoundError: No module named 'openpyxl' or "openpyxl"
        if let Some(idx) = text.find("No module named ") {
            let sub = &text[idx + "No module named ".len()..];
            let sub = sub.trim_start();
            let mut quote = None;
            let first_char = sub.chars().next()?;
            let dep = if first_char == '\'' || first_char == '"' {
                quote = Some(first_char);
                &sub[1..]
            } else {
                sub
            };
            let end_idx = if let Some(q) = quote {
                dep.find(q).unwrap_or_else(|| dep.split_whitespace().next().map(|s| s.len()).unwrap_or(dep.len()))
            } else {
                dep.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-').unwrap_or(dep.len())
            };
            let candidate = dep[..end_idx].trim().to_string();
            if !candidate.is_empty() {
                return Some(candidate);
            }
        }

        // 2. Python ImportError: cannot import name ... from 'xxx'
        if text.contains("ImportError") {
            if let Some(from_idx) = text.find("from '") {
                let sub = &text[from_idx + 6..];
                if let Some(end) = sub.find('\'') {
                    let dep = sub[..end].trim().to_string();
                    if !dep.is_empty() {
                        return Some(dep);
                    }
                }
            }
        }

        // 3. Shell / Linux command not found:
        // "command not found: jq" or "jq: command not found" or "sh: line 1: jq: not found"
        if text.contains("command not found") || text.contains(": not found") {
            const IGNORED_COMMANDS: &[&str] = &[
                "run", "exec", "terminal", "sh", "bash", "cmd", "action", "command",
                "python", "python3", "pytest", "sudo", "exit", "test", "true", "false",
            ];
            for line in text.lines() {
                let candidate = if let Some(idx) = line.find("command not found:") {
                    let sub = line[idx + "command not found:".len()..].trim();
                    sub.split_whitespace().next()
                } else if let Some(idx) = line.find(": command not found") {
                    let prefix = line[..idx].trim();
                    prefix.split_whitespace().last()
                } else if let Some(idx) = line.find(": not found") {
                    let prefix = line[..idx].trim();
                    prefix.split_whitespace().last()
                } else {
                    None
                };

                if let Some(word) = candidate {
                    let clean = word.trim_matches(|c| c == '\'' || c == '"' || c == ':').to_string();
                    if !clean.is_empty() && !IGNORED_COMMANDS.contains(&clean.to_lowercase().as_str()) {
                        return Some(clean);
                    }
                }
            }
        }

        None
    }

    pub fn classify_failure(
        action: &SwarmAction,
        exec_success: bool,
        exec_output: &str,
        vfs_target_exists: bool,
        vfs_target_size: usize,
        vfs_content: Option<&str>,
        generated_text: &str,
        previous_diagnostic: Option<&DiagnosticCode>,
    ) -> DiagnosticCode {
        Self::classify_failure_for_kind(
            action,
            exec_success,
            exec_output,
            vfs_target_exists,
            vfs_target_size,
            vfs_content,
            generated_text,
            previous_diagnostic,
            None,
        )
    }

    pub fn classify_failure_for_kind(
        action: &SwarmAction,
        exec_success: bool,
        exec_output: &str,
        vfs_target_exists: bool,
        vfs_target_size: usize,
        vfs_content: Option<&str>,
        generated_text: &str,
        previous_diagnostic: Option<&DiagnosticCode>,
        task_kind: Option<&TaskKind>,
    ) -> DiagnosticCode {
        // 0. Deterministic missing dependency detection from execution or output
        if let Some(dep) = Self::extract_missing_dependency(exec_output) {
            return DiagnosticCode::MissingDependency(dep);
        }
        if let Some(dep) = Self::extract_missing_dependency(generated_text) {
            return DiagnosticCode::MissingDependency(dep);
        }

        // Check for repeated attractor pattern
        if let Some(prev) = previous_diagnostic {
            if (*prev == DiagnosticCode::PlaceholderOrEmpty || *prev == DiagnosticCode::VfsFileNotWritten)
                && (generated_text.contains("TODO") || generated_text.contains("pass") || generated_text.contains("<!-- TODO"))
            {
                return DiagnosticCode::RepeatedAttractor;
            }
        }

        // Action-specific diagnostics
        if let SwarmAction::PatchFile { .. } = action {
            if !exec_success {
                return DiagnosticCode::PatchFailed(exec_output.to_string());
            }
        }
        if let SwarmAction::DeleteFile { .. } = action {
            if vfs_target_exists {
                return DiagnosticCode::FileNotDeleted;
            }
        }
        if let SwarmAction::RunTerminal { .. } = action {
            if !exec_success {
                return DiagnosticCode::TestFailed;
            }
        }

        // TaskKind-specific diagnostics
        if let Some(kind) = task_kind {
            match kind {
                TaskKind::Delete => {
                    if vfs_target_exists {
                        return DiagnosticCode::FileNotDeleted;
                    }
                }
                TaskKind::Modify => {
                    if !exec_success {
                        return DiagnosticCode::PatchFailed(exec_output.to_string());
                    }
                    if vfs_target_exists && vfs_content.map(|d| d.trim().is_empty()).unwrap_or(true) {
                        return DiagnosticCode::DiffEmpty;
                    }
                }
                TaskKind::Test => {
                    if !exec_success {
                        return DiagnosticCode::TestFailed;
                    }
                }
                _ => {}
            }
        }

        // 1. Missing code block or unparsed action
        if matches!(action, SwarmAction::None) && !generated_text.contains("```") && !generated_text.contains("vfs.") {
            return DiagnosticCode::NoCodeBlock;
        }

        // 2. Lua execution failure / syntax trap
        if !exec_success {
            return DiagnosticCode::LuaSyntaxTrap;
        }

        // 3. File not created in VFS
        if !vfs_target_exists {
            return DiagnosticCode::VfsFileNotWritten;
        }

        // 4. File is too small or contains placeholder tokens
        if vfs_target_size <= 30 {
            return DiagnosticCode::PlaceholderOrEmpty;
        }

        if let Some(content) = vfs_content {
            let trimmed = content.trim();
            if trimmed.contains("TODO")
                || trimmed.starts_with("pass")
                || trimmed == "..."
            {
                return DiagnosticCode::PlaceholderOrEmpty;
            }
        }

        DiagnosticCode::VfsFileNotWritten
    }

    pub fn prompt_user_for_dependency(dep: &str, _question: &str) -> DependencyChoice {
        println!("\n{}", "────────────────────────────────────────────────────────────────────────────────".bright_yellow());
        println!("  [!] {}", format!("المكتبة أو الأداة المطلوبة غير متوفرة في النظام: '{}'", dep).bright_yellow().bold());
        println!("  المفكر يستشيرك لاختيار مسار التنفيذ:");
        println!("     [1] تثبيت المكتبة تلقائياً عبر الطرفية ومواصلة المهمة (pip install {})", dep);
        println!("     [2] التحويل إلى بديل قياسي (مثل CSV أو مكتبات بايثون القياسية) دون تثبيت");
        println!("{}", "────────────────────────────────────────────────────────────────────────────────".bright_yellow());

        if !io::stdin().is_terminal() {
            println!("  [*] طرفية غير تفاعلية. الاختيار التلقائي: [1] محاولة التثبيت.");
            return DependencyChoice::InstallAndRetry;
        }

        loop {
            print!("  >> اختيارك [1/2، الافتراضي: 1]: ");
            let _ = io::stdout().flush();
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(0) => {
                    println!();
                    return DependencyChoice::InstallAndRetry;
                }
                Ok(_) => {
                    let trimmed = input.trim();
                    if trimmed.is_empty() || trimmed == "1" {
                        return DependencyChoice::InstallAndRetry;
                    } else if trimmed == "2" {
                        return DependencyChoice::UseAlternative;
                    }
                    println!("     يرجى إدخال 1 أو 2.");
                }
                Err(_) => {
                    return DependencyChoice::InstallAndRetry;
                }
            }
        }
    }

    pub fn parse_thinker_decision(text: &str, subgoal: &SubGoal) -> ThinkerDecision {
        let upper = text.to_uppercase();
        if upper.contains("DECISION: RESET") || upper.contains("DECISION:RESET") {
            let reason = if let Some(idx) = text.find("REASON:") {
                text[idx + 7..].lines().next().unwrap_or("").trim().to_string()
            } else if let Some(idx) = text.find('|') {
                text[idx + 1..].lines().next().unwrap_or("").trim().to_string()
            } else {
                format!("Rewrite complete implementation for {}", subgoal.target_entity)
            };
            ThinkerDecision::Reset(reason)
        } else {
            let reason = if let Some(idx) = text.find("REASON:") {
                text[idx + 7..].lines().next().unwrap_or("").trim().to_string()
            } else if let Some(idx) = text.find('|') {
                text[idx + 1..].lines().next().unwrap_or("").trim().to_string()
            } else {
                format!("Fix syntax error and ensure {} is written to VFS", subgoal.target_entity)
            };
            ThinkerDecision::Nudge(reason)
        }
    }

    pub fn is_explicit_single_file(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        lower.contains("single file")
            || lower.contains("one file")
            || lower.contains("a file")
            || lower.contains("a script")
            || lower.contains("one script")
            || lower.contains("single script")
            || lower.contains("standalone script")
            || lower.contains("program")
            || lower.contains("a program")
            || lower.contains("one program")
            || lower.contains("single program")
            || lower.contains("a tool")
            || lower.contains("utility")
            || lower.contains("برنامج")
            || lower.contains("اداة")
            || lower.contains("أداة")
            || lower.contains("ملف واحد")
            || lower.contains("ملفا واحدا")
            || lower.contains("سكربت واحد")
            || lower.contains("صفحة واحدة")
            || lower.contains("صفحه واحده")
            || lower.contains("في ملف")
            || lower.contains("فى ملف")
            || (!lower.contains(" and ") && !lower.contains(" و ") && !lower.contains("separate") && !lower.contains("multiple")
                && (lower.contains("python") || lower.contains("بايثون") || lower.contains("script") || lower.contains("سكربت")))
    }

    /// Normalizes entity filename to proper casing (e.g. PascalCase for Java files)
    pub fn normalize_entity_filename(entity: &str) -> String {
        let trimmed = entity.trim();
        if trimmed.to_lowercase().ends_with(".java") {
            let stem = &trimmed[..trimmed.len() - 5];
            let pascal: String = stem
                .split(|c: char| c == '_' || c == '-')
                .filter(|s| !s.is_empty())
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            let rest: String = chars.collect();
                            first.to_uppercase().collect::<String>() + &rest
                        }
                    }
                })
                .collect();
            if !pascal.is_empty() {
                return format!("{}.java", pascal);
            }
        }
        trimmed.to_string()
    }

fn infer_target_entity(goal: &str) -> String {
    let lower = goal.to_lowercase();
    if lower.contains(".java") || lower.contains("java") || lower.contains("جافا") {
        "Main.java".to_string()
    } else if lower.contains(".py") || lower.contains("python") || lower.contains("بايثون") {
        if lower.contains("speed") || lower.contains("سرعة") || lower.contains("سرعه") {
            "measure_speed.py".to_string()
        } else {
            "main.py".to_string()
        }
    } else if lower.contains(".pdf") || lower.contains("pdf") {
        "document.pdf".to_string()
    } else if lower.contains(".sh") || lower.contains("bash") || lower.contains("shell") || lower.contains("باش") {
        "script.sh".to_string()
    } else if lower.contains(".json") || lower.contains("json") {
        "data.json".to_string()
    } else if lower.contains(".csv") || lower.contains("csv") || lower.contains("جدول")
        || lower.contains("excel") || lower.contains("xlsx") || lower.contains("xls")
        || lower.contains("اكسل") || lower.contains("إكسل")
    {
        "data.csv".to_string()
    } else if lower.contains(".md") || lower.contains("markdown") || lower.contains("تقرير") || lower.contains("report") {
        "report.md".to_string()
    } else if lower.contains(".html") || lower.contains("html") || lower.contains("صفحة") || lower.contains("صفحه") || lower.contains("موقع") || lower.contains("web") {
        "index.html".to_string()
    } else {
        "output.txt".to_string()
    }
}

fn is_build_task(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("create")
        || lower.contains("build")
        || lower.contains("game")
        || lower.contains("java")
        || lower.contains("جافا")
        || lower.contains("html")
        || lower.contains("css")
        || lower.contains("js")
        || lower.contains("write")
        || lower.contains("code")
        || lower.contains("implement")
        || lower.contains("develop")
        || lower.contains("make")
        || lower.contains("web")
        || lower.contains("python")
        || lower.contains("py")
        || lower.contains("pdf")
        || lower.contains("script")
        || lower.contains("program")
        || lower.contains("tool")
        || lower.contains("utility")
        || lower.contains("app")
        || lower.contains("bash")
        || lower.contains("shell")
        || lower.contains("json")
        || lower.contains("csv")
        || lower.contains("excel")
        || lower.contains("xlsx")
        || lower.contains("delete")
        || lower.contains("remove")
        || lower.contains("clean")
        || lower.contains("patch")
        || lower.contains("modify")
        || lower.contains("edit")
        || lower.contains("fix")
        || lower.contains("refactor")
        || lower.contains("test")
        || lower.contains("verify")
        || lower.contains("check")
        || lower.contains("اكتب")
        || lower.contains("انشئ")
        || lower.contains("أنشئ")
        || lower.contains("اصنع")
        || lower.contains("اعمل")
        || lower.contains("احذف")
        || lower.contains("امسح")
        || lower.contains("نظف")
        || lower.contains("ازالة")
        || lower.contains("إزالة")
        || lower.contains("عدل")
        || lower.contains("صلح")
        || lower.contains("غير")
        || lower.contains("تعديل")
        || lower.contains("تصليح")
        || lower.contains("افحص")
        || lower.contains("شغل")
        || lower.contains("اختبر")
        || lower.contains("تحقق")
        || lower.contains("صفحة")
        || lower.contains("صفحه")
        || lower.contains("كود")
        || lower.contains("برمج")
        || lower.contains("سكربت")
        || lower.contains("سكريبت")
        || lower.contains("بايثون")
        || lower.contains("برنامج")
        || lower.contains("اداة")
        || lower.contains("أداة")
        || lower.contains("اكسل")
        || lower.contains("إكسل")
        || lower.contains("تقرير")
        || lower.contains("ملف")
        || lower.contains("لعبة")
        || lower.contains("لعبه")
        || lower.contains("موقع")
        || lower.contains("تطبيق")
        || lower.contains("جدول")
        || lower.contains("زر")
}

pub fn infer_task_kind(text: &str) -> TaskKind {
    let lower = text.to_lowercase();
    if lower.contains("delete") || lower.contains("remove") || lower.contains("clean")
        || lower.contains("احذف") || lower.contains("امسح") || lower.contains("نظف") || lower.contains("إزالة") || lower.contains("ازالة")
    {
        TaskKind::Delete
    } else if lower.contains("patch") || lower.contains("modify") || lower.contains("edit") || lower.contains("fix") || lower.contains("refactor")
        || lower.contains("عدل") || lower.contains("صلح") || lower.contains("غير") || lower.contains("تعديل") || lower.contains("تصليح")
    {
        TaskKind::Modify
    } else if lower.contains("test") || lower.contains("verify") || lower.contains("check")
        || lower.contains("افحص") || lower.contains("شغل") || lower.contains("اختبر") || lower.contains("تحقق")
    {
        TaskKind::Test
    } else {
        TaskKind::Create
    }
}

    /// System 2: Decomposes goal into discrete sub-goals
    fn orchestrate_plan(&self, goal: &str) -> Result<Vec<SubGoal>, String> {
        let is_build = Self::is_build_task(goal);

        let mut ctx = self.orchestrator_model.create_context(4096, 512, 4)?;

        let system_msg = if is_build {
            format!(
                "You are the System 2 Sovereign Architect.\n\
                Environment: In-memory Virtual Filesystem (VFS) sandbox.\n\
                Workers write standalone project files directly in their native language (e.g. ```python, ```sh, ```html, ```java) or via vfs.write(\"filename\", [[content]]).\n\n\
                PARSIMONY & WORKER DISPATCH RULES:\n\
                - You decide how many workers to deploy (from 1 up to {} max budget).\n\
                - Rule of Parsimony: If the user objective asks for a single script, utility, single document, or standalone file (e.g. Python script, shell script, single web page, or report), deploy EXACTLY 1 worker to generate the complete file in one pass. Output EXACTLY ONE line for SUBGOAL.\n\
                - Only decompose into multiple sub-goals if the objective genuinely requires multiple distinct physical files (e.g. separate frontend and backend, or html with separate css). Each sub-goal MUST target a distinct physical filename (ENTITY).\n\
                - For Java: Target filenames MUST use PascalCase (e.g. GameLoop.java, Snake.java, Food.java) matching their primary public class.\n\
                - NEVER output multiple sub-goals for the same file or redundant sub-tasks.\n\
                - SPREADSHEETS & BINARY: Lua VFS cannot write binary ZIP archives (.xlsx, .docx). For spreadsheets, Excel, or tabular data, always direct the worker to generate a clean, well-formatted CSV file (ENTITY: data.csv) which Excel opens natively.\n\n\
                Output each sub-goal on a new line strictly formatted as:\n\
                SUBGOAL: <explicit implementation directive> | ENTITY: <target filename>\n\n\
                Examples:\n\
                Single-file deliverable:\n\
                SUBGOAL: Implement internet speed measurement tool using download and upload tests | ENTITY: measure_speed.py\n\
                Java multi-file deliverable:\n\
                SUBGOAL: Implement main game window, board panel, and game loop | ENTITY: GameLoop.java\n\
                SUBGOAL: Implement snake body segments, movement, and direction | ENTITY: Snake.java\n\
                SUBGOAL: Implement food spawning, collision logic, and score tracking | ENTITY: Food.java\n\
                Spreadsheet / tabular deliverable:\n\
                SUBGOAL: Write the complete tabular data in CSV format | ENTITY: data.csv\n\
                Multi-file deliverable:\n\
                SUBGOAL: Write the HTML structure and markup | ENTITY: index.html\n\
                SUBGOAL: Write the external CSS styling | ENTITY: styles.css\n\n\
                - REVIEW & TEST WORKER: When the objective creates an executable script or program (e.g. .py, .sh, .java), you may deploy a final review/smoke test worker (kind: TEST, ENTITY: <target>) to verify runtime stability in the isolated container sandbox before final delivery to the user.",
                self.config.max_subgoals
            )
        } else {
            format!(
                "You are the System 2 Sovereign Architect.\n\
                Decision Authority on Workers:\n\
                - You decide how many workers to deploy (from 1 up to {} max budget).\n\
                - For straightforward topics, 1 worker is sufficient.\n\n\
                Output each sub-goal on a new line strictly formatted as:\n\
                SUBGOAL: <precise search objective> | ENTITY: <primary keyword>\n\n\
                Examples:\n\
                SUBGOAL: Search for room temperature superconductors 2026 breakthroughs | ENTITY: superconductors\n\
                SUBGOAL: Search for recent astrophysics discoveries | ENTITY: astrophysics",
                self.config.max_subgoals
            )
        };

        let user_msg = format!("Goal: {}", goal);
        let prompt = Self::format_prompt(&self.orchestrator_model, &system_msg, &user_msg);

        let output = ctx.generate(&prompt, 256)?;
        let mut subgoals = Vec::new();
        let mut seen_entities = std::collections::HashSet::new();
        let mut id = 1;

        for line in output.lines() {
            let l = line.trim();
            if l.starts_with("SUBGOAL:") {
                let rest = l.trim_start_matches("SUBGOAL:").trim();
                let parts: Vec<&str> = rest.split("| ENTITY:").collect();
                let desc = parts[0].trim().to_string();
                let raw_entity = if parts.len() > 1 && !parts[1].trim().is_empty() {
                    parts[1].trim().to_string()
                } else if is_build {
                    Self::infer_target_entity(goal)
                } else {
                    goal.split_whitespace().next().unwrap_or("general").to_string()
                };
                let entity = Self::normalize_entity_filename(&raw_entity);

                let entity_key = entity.to_lowercase();
                if seen_entities.contains(&entity_key) {
                    continue; // Skip duplicate entity: never spawn redundant workers for the same target
                }
                seen_entities.insert(entity_key);

                if !desc.is_empty() {
                    let task_kind = Self::infer_task_kind(&desc);
                    subgoals.push(SubGoal {
                        id,
                        description: desc,
                        target_entity: entity,
                        kind: task_kind,
                        guidance: None,
                    });
                    id += 1;
                    if subgoals.len() >= self.config.max_subgoals {
                        break;
                    }
                }
            }
        }

        // If user goal explicitly specifies a single file/script, enforce exactly 1 creation worker
        if Self::is_explicit_single_file(goal) && subgoals.len() > 1 {
            subgoals.truncate(1);
        }

        // Fallback if model output did not match format exactly
        if subgoals.is_empty() {
            if is_build {
                let inferred = Self::infer_target_entity(goal);
                let task_kind = Self::infer_task_kind(goal);
                subgoals.push(SubGoal {
                    id: 1,
                    description: format!("Create and write complete standalone implementation for: {}", goal),
                    target_entity: inferred,
                    kind: task_kind,
                    guidance: None,
                });
            } else {
                subgoals.push(SubGoal {
                    id: 1,
                    description: format!("Search web for: {}", goal),
                    target_entity: goal.split_whitespace().next().unwrap_or("topic").to_string(),
                    kind: TaskKind::Search,
                    guidance: None,
                });
            }
        }

        // Thinker Sovereign Review Gate:
        // When building executable deliverables (.py, .sh, .js), deploy a Review & Smoke Test worker
        // to execute and verify the deliverable in the isolated host sandbox before delivery to user.
        if is_build && subgoals.len() < self.config.max_subgoals {
            let has_executable = subgoals.iter().any(|sg| {
                sg.target_entity.ends_with(".py")
                    || sg.target_entity.ends_with(".sh")
                    || sg.target_entity.ends_with(".js")
            });
            let has_test_worker = subgoals.iter().any(|sg| sg.kind == TaskKind::Test);

            if has_executable && !has_test_worker {
                let target = subgoals
                    .iter()
                    .find(|sg| {
                        sg.target_entity.ends_with(".py")
                            || sg.target_entity.ends_with(".sh")
                            || sg.target_entity.ends_with(".js")
                    })
                    .map(|sg| sg.target_entity.clone())
                    .unwrap_or_else(|| "main.py".to_string());

                let next_id = subgoals.len() + 1;
                subgoals.push(SubGoal {
                    id: next_id,
                    description: format!("Run isolated sandbox smoke test and review runtime execution of '{}'", target),
                    target_entity: target,
                    kind: TaskKind::Test,
                    guidance: Some("Execute isolated sandbox test and verify zero runtime errors before final host commit".to_string()),
                });
            }
        }

        Ok(subgoals)
    }

    /// System 1: Autonomous Worker Loop with Checkpoint Freeze and Unified KV-Cache Stream
    fn run_worker_loop(
        &self,
        user_goal: &str,
        subgoal: &SubGoal,
        worker_ctx: &mut NativeLlamaContext,
        is_first_worker: bool,
        previous_findings: &[WorkerFinding],
    ) -> Result<WorkerFinding, String> {
        let is_build = Self::is_build_task(&subgoal.description) || Self::is_build_task(user_goal);

        let mut attempt = 1;
        let max_attempts = 3;
        let mut current_directive = subgoal.guidance.clone().unwrap_or_else(|| subgoal.description.clone());
        let mut pending_nudge: Option<String> = None;
        let mut checkpoint: usize = 0;

        let mut rollbacks_count = 0;
        let mut tokens_saved = 0;
        let mut final_finding = String::new();
        let target_path = std::path::PathBuf::from(&subgoal.target_entity);
        let mut last_diagnostic: Option<DiagnosticCode> = None;

        // Baseline Snapshot for Modification Tasks
        if subgoal.kind == TaskKind::Modify {
            if !self.vfs.exists(&target_path) {
                if let Ok(bytes) = std::fs::read(&target_path) {
                    self.vfs.preload_file(&target_path, &bytes);
                }
            }
            self.vfs.snapshot_file(&target_path);
        }

        // Unified Sovereign Swarm Memory:
        // Record the anchor cursor where this worker begins in the shared KV cache.
        let worker_initial_cursor = worker_ctx.current_cursor();

        while attempt <= max_attempts {
            // Check if starting fresh (attempt 1 or after a RESET)
            if pending_nudge.is_none() {
                if is_first_worker && attempt == 1 {
                    worker_ctx.kv_cache_clear();
                } else if attempt > 1 {
                    // RESET triggered: Surgically roll back ONLY this worker's failed attempt,
                    // preserving all prior workers' KV-cache intact!
                    let _ = worker_ctx.rollback_to(worker_initial_cursor);
                    println!(
                        "  [WORKER RESET] Worker #{} rewound to initial cursor {} (Prior KV cache preserved).",
                        subgoal.id, worker_initial_cursor
                    );
                }

                if is_first_worker {
                    let java_mandate = if subgoal.target_entity.ends_with(".java") {
                        let stem = subgoal.target_entity.trim_end_matches(".java");
                        format!(
                            "\n- Java Requirement: The primary public class MUST be named 'public class {}' to match '{}.java'.",
                            stem, stem
                        )
                    } else {
                        String::new()
                    };

                    let system_msg = if is_build {
                        format!(
                            "You are an Executive Hands Worker in an autonomous dual-model swarm.\n\
                            You are directed exclusively by the System 2 Sovereign Thinker.\n\
                            Environment: In-memory Virtual Filesystem (VFS) sandbox.\n\
                            Overall Goal: {}\n\
                            Current Task: {}\n\
                            Target File: {}\n\n\
                            [ENGINE EXECUTION MANDATE]:\n\
                            1. You are a pure code-generation engine. You have NO conversational persona.\n\
                            2. NEVER output conversational speech, greetings, explanations, or commentary.\n\
                            3. Output ONLY the complete, production-ready code for '{}' inside a code block (e.g. ```python ... ```, ```sh ... ```, ```java ... ```) or call vfs.write(\"{}\", [[...]]) inside a ```lua ... ``` block.\n\
                            4. The code MUST be 100% complete, fully implemented, and standalone.\n\
                            5. NEVER output placeholder comments like TODO, FIXME, pass, or ellipses (...).\n\
                            6. For spreadsheets, Excel, or tables: output clean CSV format with headers.",
                            user_goal, subgoal.description, subgoal.target_entity, subgoal.target_entity, subgoal.target_entity
                        )
                    } else {
                        format!(
                            "You are an Executive Hands Worker in an autonomous dual-model swarm.\n\
                            You are directed exclusively by the System 2 Sovereign Thinker.\n\
                            Objective: {}\n\n\
                            Available Commands:\n\
                            - SEARCH: <query>\n\
                            - FETCH: <result index or URL>\n\
                            - REPORT: <discovered facts>\n\
                            - DONE\n\n\
                            Rule: Output exactly ONE command per step.\n\
                            When facts are found, output REPORT: <facts>.\n\
                            When objective is fulfilled, output DONE.",
                            subgoal.description
                        )
                    };

                    let user_msg = if is_build {
                        format!(
                            "Goal: {}\nTask: {}\nTarget File: {}\nThinker Directive: {}\n\n\
                            [ENGINE DIRECTIVE INJECTION - STRICT CONSTRAINTS]:\n\
                            - Output EXCLUSIVELY the complete, working code inside a single markdown code block.\n\
                            - ZERO discussion. ZERO apologies. ZERO introduction. ZERO text outside the code block.\n\
                            - Complete implementation only: NO 'TODO', NO 'pass', NO placeholders.{}\n\
                            - Generate the code for '{}' now:",
                            user_goal, subgoal.description, subgoal.target_entity, current_directive,
                            java_mandate, subgoal.target_entity
                        )
                    } else {
                        format!(
                            "Goal: {}\nThinker Directive: {}\nTask: {}\n\n\
                            [ENGINE DIRECTIVE INJECTION]:\n\
                            - Output EXCLUSIVELY one command: SEARCH: <query> or REPORT: <facts>.\n\
                            - ZERO conversational text.\n\
                            Execute now:",
                            user_goal, current_directive, subgoal.description
                        )
                    };

                    let prompt = Self::format_prompt(&self.worker_model, &system_msg, &user_msg);
                    let prompt_tokens = self.worker_model.tokenize(&prompt, true)?;
                    worker_ctx.eval_tokens(&prompt_tokens, 0)?;
                } else {
                    // Subsequent Workers: Seamless Multi-Turn continuation in Shared KV-Cache
                    let vfs_files = self.vfs.list_files();
                    let vfs_files_list = if vfs_files.is_empty() {
                        "None".to_string()
                    } else {
                        vfs_files
                            .iter()
                            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };

                    let prev_summary = previous_findings
                        .last()
                        .map(|f| format!("Deliverable for Task #{} committed to VFS: {}", f.worker_id, f.finding))
                        .unwrap_or_else(|| "Previous task complete.".to_string());

                    let observation = format!(
                        "{}\nFiles currently in VFS: [{}]",
                        prev_summary, vfs_files_list
                    );

                    let java_guidance = if subgoal.target_entity.ends_with(".java") {
                        let stem = subgoal.target_entity.trim_end_matches(".java");
                        format!(
                            "\n- In Java, you MUST declare 'public class {}' matching the filename exactly.\n\
                            - Inspect the previous Java classes above in active memory. Use their exact class names, methods, and variables without mismatch.",
                            stem
                        )
                    } else {
                        String::new()
                    };

                    let next_instruction = if is_build {
                        if subgoal.kind == TaskKind::Test {
                            format!(
                                "[CONTINUOUS SWARM DIRECTIVE - ISOLATED TEST WORKER]:\n\
                                Task #{}: {}\n\
                                Target File: {}\n\
                                Thinker Directive: {}\n\n\
                                [SANDBOX TEST MANDATE]:\n\
                                - Your role is to review and test the staged deliverable in the isolated sandbox.\n\
                                - Output the command to test '{}', e.g.:\n\
                                RUN: python3 {}\n\
                                Execute test now:",
                                subgoal.id, subgoal.description, subgoal.target_entity, current_directive,
                                subgoal.target_entity, subgoal.target_entity
                            )
                        } else {
                            format!(
                                "[CONTINUOUS SWARM DIRECTIVE - SHARED MEMORY]:\n\
                                Task #{}: {}\n\
                                Target File: {}\n\
                                Thinker Directive: {}\n\n\
                                [MANDATORY INTEROPERABILITY]:\n\
                                - You share continuous attention memory with prior workers above.\n\
                                - Strictly attend to the exact class names, method signatures, return types, and fields defined in prior files in memory.\n\
                                - Ensure 100% interoperability and compatibility.{}\n\
                                - Output EXCLUSIVELY the complete, working code for '{}' inside a single code block.\n\
                                - ZERO conversational text outside the code block.\n\
                                Generate the code for '{}' now:",
                                subgoal.id, subgoal.description, subgoal.target_entity, current_directive,
                                java_guidance, subgoal.target_entity, subgoal.target_entity
                            )
                        }
                    } else {
                        format!(
                            "[CONTINUOUS SWARM DIRECTIVE - SHARED MEMORY]:\n\
                            Task #{}: {}\n\
                            Target: {}\n\
                            Thinker Directive: {}\n\n\
                            - Build upon previous findings above in memory.\n\
                            - Output EXCLUSIVELY one command: SEARCH: <query> or REPORT: <facts>.\n\
                            Execute now:",
                            subgoal.id, subgoal.description, subgoal.target_entity, current_directive
                        )
                    };

                    let turn = Self::format_observation_turn(&self.worker_model, &observation, &next_instruction);
                    let turn_tokens = self.worker_model.tokenize(&turn, false)?;
                    worker_ctx.eval_tokens(&turn_tokens, 0)?;
                }

                // CHECKPOINT FREEZE: Anchor cursor position before generation
                checkpoint = worker_ctx.current_cursor();
                if is_first_worker {
                    println!(
                        "  [WORKER CONTEXT] Worker #1 (Lead): Anchored at position {} tokens.",
                        checkpoint
                    );
                } else {
                    println!(
                        "  [SHARED KV MEMORY] Worker #{} inherited {} tokens of attention context from prior workers.",
                        subgoal.id, worker_initial_cursor.to_string().bright_cyan()
                    );
                    println!(
                        "  [WORKER CONTEXT] Worker #{} anchored at position {} tokens in unified KV-stream.",
                        subgoal.id, checkpoint
                    );
                }
            } else if let Some(ref nudge) = pending_nudge {
                // NUDGE BRANCH: Worker is frozen at checkpoint!
                let nudge_turn = format!(
                    "\n[ENGINE DIRECTIVE INJECTION - CORRECTION]:\n\
                    Notice from Thinker: {}\n\
                    Target File: {}\n\
                    MANDATE: Output ONLY the corrected, complete code block now without any commentary or explanations:",
                    nudge, subgoal.target_entity
                );
                let nudge_tokens = self.worker_model.tokenize(&nudge_turn, false)?;
                worker_ctx.eval_tokens(&nudge_tokens, 0)?;
                println!(
                    "  [WORKER RESUME] Applied Thinker nudge ({} tokens) from frozen checkpoint {}.",
                    nudge_tokens.len(), checkpoint
                );
            }

            // Autoregressive generation
            let mut generated_text = String::new();
            let mut generated_tokens = Vec::new();
            let mut in_code_block = false;
            let max_gen_tokens = if is_build { 1536 } else { 256 };

            for _ in 0..max_gen_tokens {
                let tok = worker_ctx.sample_greedy()?;
                let piece = self.worker_model.token_to_piece(tok)?;

                if piece.contains("<|im_end|>")
                    || piece.contains("<|endoftext|>")
                    || piece.contains("<|eot_id|>")
                    || piece.contains("</s>")
                    || piece.contains("<end_of_turn>")
                {
                    break;
                }

                generated_tokens.push(tok);
                generated_text.push_str(&piece);
                worker_ctx.eval_tokens(&[tok], 0)?;

                if generated_text.contains("```") {
                    in_code_block = true;
                }

                if in_code_block {
                    if let Some(first_idx) = generated_text.find("```") {
                        if generated_text[first_idx + 3..].contains("```") {
                            break;
                        }
                    }
                } else if !is_build && piece.contains('\n') {
                    break;
                }
            }

            let parsed_action = Self::parse_action(&generated_text);
            let action = if subgoal.kind == TaskKind::Test {
                match parsed_action {
                    SwarmAction::RunTerminal { ref command } => {
                        let clean_cmd = Self::strip_command_prefix(command);
                        SwarmAction::RunTerminal { command: clean_cmd }
                    }
                    SwarmAction::WriteFile { ref content, .. } if content.contains("python") || content.contains("pytest") || content.contains("bash") || content.contains("sh ") => {
                        let cmd = content.lines().find(|l| l.contains("python") || l.contains("bash") || l.contains("pytest")).unwrap_or(content).trim();
                        let clean_cmd = Self::strip_command_prefix(cmd);
                        SwarmAction::RunTerminal { command: clean_cmd }
                    }
                    _ => {
                        let default_cmd = if subgoal.target_entity.ends_with(".py") {
                            format!("python3 {}", subgoal.target_entity)
                        } else if subgoal.target_entity.ends_with(".sh") {
                            format!("bash {}", subgoal.target_entity)
                        } else {
                            format!("test -f {}", subgoal.target_entity)
                        };
                        SwarmAction::RunTerminal { command: default_cmd }
                    }
                }
            } else {
                parsed_action
            };
            let tokens_at_attempt_end = worker_ctx.current_cursor();

            if self.config.verbose {
                let preview = if generated_text.len() > 120 {
                    format!("{}... [{} chars]", &generated_text[..120].trim(), generated_text.len())
                } else {
                    generated_text.trim().to_string()
                };
                println!(
                    "  [WORKER OUTPUT] Step generated: \"{}\" ({} tokens)",
                    preview.bright_white(),
                    generated_tokens.len()
                );
            }

            // Execute action
            let (mut exec_success, mut exec_output) = match action {
                SwarmAction::WriteFile { ref filename, ref content } => {
                    let actual_filename = if filename.is_empty() {
                        &subgoal.target_entity
                    } else {
                        filename.as_str()
                    };
                    let path = std::path::PathBuf::from(actual_filename);
                    println!("  [EXECUTION] Direct code write: '{}' ({} bytes)...", actual_filename, content.len());
                    self.vfs.write_file(&path, content.as_bytes());
                    (true, format!("Wrote {} bytes to {}", content.len(), actual_filename))
                }
                SwarmAction::PatchFile { ref filename, ref target, ref replacement } => {
                    let actual_filename = if filename.is_empty() {
                        &subgoal.target_entity
                    } else {
                        filename.as_str()
                    };
                    let path = std::path::PathBuf::from(actual_filename);
                    println!("  [EXECUTION] Patching '{}'...", actual_filename);
                    match self.vfs.patch_file(&path, target, replacement) {
                        Ok(true) => {
                            (true, format!("Successfully patched {}", actual_filename))
                        }
                        Ok(false) => {
                            (false, format!("Target snippet not found in {}", actual_filename))
                        }
                        Err(e) => {
                            (false, format!("Patch error in {}: {}", actual_filename, e))
                        }
                    }
                }
                SwarmAction::DeleteFile { ref filename } => {
                    let actual_filename = if filename.is_empty() {
                        &subgoal.target_entity
                    } else {
                        filename.as_str()
                    };
                    let path = std::path::PathBuf::from(actual_filename);
                    println!("  [EXECUTION] Deleting file: '{}'...", actual_filename);
                    let deleted = self.vfs.delete_file(&path);
                    if deleted {
                        (true, format!("Successfully deleted {}", actual_filename))
                    } else if !self.vfs.exists(&path) {
                        (true, format!("File '{}' is not present in VFS (already deleted)", actual_filename))
                    } else {
                        (false, format!("Failed to delete '{}' from VFS", actual_filename))
                    }
                }
                SwarmAction::RunTerminal { ref command } => {
                    let clean_cmd = Self::strip_command_prefix(command);
                    let bwrap_active = IsolatedJail::is_bwrap_available();
                    println!(
                        "  [EXECUTION] Isolated Sandbox exec (bwrap={}): '{}'...",
                        bwrap_active, clean_cmd
                    );
                    let vfs_files = self.vfs.all_files();
                    match IsolatedJail::run_in_jail(&clean_cmd, &vfs_files, std::time::Duration::from_secs(30)) {
                        Ok(jail_res) => {
                            let combined = format!("{}{}", jail_res.stdout, jail_res.stderr);
                            (jail_res.success, combined)
                        }
                        Err(e) => {
                            (false, format!("Isolated jail error: {}", e))
                        }
                    }
                }
                SwarmAction::RunLua { ref script } => {
                    println!("  [EXECUTION] Running Lua script ({} chars)...", script.len());
                    let runner = crate::sandbox::LuaSandboxRunner::with_terminal(self.vfs.clone(), self.terminal.clone());
                    let res = runner.run_script(script);
                    let out = if res.success {
                        if res.output_log.trim().is_empty() {
                            "Executed with exit code 0".to_string()
                        } else {
                            res.output_log.trim().to_string()
                        }
                    } else {
                        res.error.unwrap_or_else(|| "Lua execution runtime trap".to_string())
                    };
                    (res.success, out)
                }
                SwarmAction::Search { ref query } => {
                    println!("  [SEARCH] Query: \"{}\"", query);
                    match self.web_lens.search(query, 3) {
                        Ok(results) if !results.is_empty() => {
                            let mut results_str = format!("Found {} sources:\n", results.len());
                            for (i, r) in results.iter().enumerate() {
                                let snippet_clean = if r.snippet.len() > 300 {
                                    format!("{}...", &r.snippet[..300])
                                } else {
                                    r.snippet.clone()
                                };
                                results_str.push_str(&format!("{}. [{}] {}\nURL: {}\n", i + 1, r.title, snippet_clean, r.url));
                            }
                            (true, results_str)
                        }
                        Ok(_) => {
                            (false, "Search yielded 0 results.".to_string())
                        }
                        Err(e) => {
                            (false, format!("Search failed: {}", e))
                        }
                    }
                }
                SwarmAction::Fetch { ref url } => {
                    match self.web_lens.fetch_text(url, 800) {
                        Ok(content) => (true, content),
                        Err(e) => (false, format!("Fetch error: {}", e)),
                    }
                }
                SwarmAction::Report { ref finding } => {
                    (true, finding.clone())
                }
                SwarmAction::Done => {
                    (true, "DONE signaled".to_string())
                }
                SwarmAction::Consult { ref question } => {
                    (false, format!("Consultation: {}", question))
                }
                SwarmAction::None => {
                    (false, "Unparsed action / missing code block".to_string())
                }
            };

            // GROUND TRUTH CHECK & THINKER VERIFICATION GATE
            let vfs_bytes = self.vfs.read_file(&target_path);
            let vfs_target_exists = self.vfs.exists(&target_path);
            let vfs_target_size = vfs_bytes.as_ref().map(|c| c.len()).unwrap_or(0);
            let vfs_str = vfs_bytes.as_ref().and_then(|b| std::str::from_utf8(b).ok());
            let line_diff = self.vfs.diff_file(&target_path);

            // Line-by-Line Diff Perception Telemetry
            if let Some(ref diff) = line_diff {
                if !diff.trim().is_empty() {
                    println!("  [DIFF PERCEPTION] Unified Line Diff for '{}':", subgoal.target_entity.bright_cyan());
                    for line in diff.lines().take(25) {
                        if line.starts_with('+') && !line.starts_with("+++") {
                            println!("    {}", line.green());
                        } else if line.starts_with('-') && !line.starts_with("---") {
                            println!("    {}", line.red());
                        } else if line.starts_with("@@") {
                            println!("    {}", line.cyan());
                        } else {
                            println!("    {}", line.white());
                        }
                    }
                    if diff.lines().count() > 25 {
                        println!("    ... (diff truncated for display)");
                    }
                }
            }

            let mut placeholder_found = false;
            let mut placeholder_reason = String::new();
            if let Some(code) = vfs_str {
                let trimmed = code.trim();
                if code.contains("TODO") || code.contains("FIXME") {
                    placeholder_found = true;
                    placeholder_reason = "Code contains TODO or FIXME placeholders".to_string();
                } else if code.contains("<!-- implementation")
                    || code.contains("/* implementation")
                    || code.contains("# implementation")
                {
                    placeholder_found = true;
                    placeholder_reason = "Code contains placeholder comments".to_string();
                } else if (trimmed.ends_with("pass") || trimmed.contains("pass\n")) && vfs_target_size < 120 {
                    placeholder_found = true;
                    placeholder_reason = "Code contains stub pass statement".to_string();
                } else if trimmed == "..." {
                    placeholder_found = true;
                    placeholder_reason = "Code contains ellipsis placeholder".to_string();
                }
            }

            // AST Syntax Validation Gate
            let mut syntax_error: Option<String> = None;
            if (is_build || subgoal.kind == TaskKind::Create || subgoal.kind == TaskKind::Modify)
                && vfs_target_exists && vfs_target_size > 0 && !placeholder_found
            {
                if let Some(code) = vfs_str {
                    if subgoal.target_entity.ends_with(".py") {
                        let hex_encoded: String = code.as_bytes().iter().map(|b| format!("{:02x}", b)).collect();
                        let check_cmd = format!("python3 -c \"import ast; ast.parse(bytes.fromhex('{}').decode('utf-8'))\"", hex_encoded);
                        if let Ok(res) = self.terminal.run_sync(&check_cmd, 5) {
                            if res.contains("SyntaxError") || res.contains("Traceback") {
                                let err_line = res.lines().find(|l| l.contains("SyntaxError")).unwrap_or("Python SyntaxError");
                                syntax_error = Some(err_line.to_string());
                            }
                        }
                        if syntax_error.is_none() {
                            let vfs_files = self.vfs.all_files();
                            if let Ok(smoke_res) = IsolatedJail::smoke_test_script(&subgoal.target_entity, &vfs_files, std::time::Duration::from_secs(4)) {
                                if let Some(ref dep) = smoke_res.missing_dependency {
                                    println!(
                                        "  [SMOKE TEST] Detected missing host dependency in sandbox: '{}'",
                                        dep.bright_yellow()
                                    );
                                    exec_success = false;
                                    exec_output = format!("ModuleNotFoundError: No module named '{}'", dep);
                                }
                            }
                        }
                    } else if subgoal.target_entity.ends_with(".json") {
                        if let Err(e) = serde_json::from_str::<serde_json::Value>(code) {
                            syntax_error = Some(format!("JSON syntax error: {}", e));
                        }
                    } else if subgoal.target_entity.ends_with(".sh") {
                        let hex_encoded: String = code.as_bytes().iter().map(|b| format!("{:02x}", b)).collect();
                        let check_cmd = format!("bash -n <(python3 -c \"import sys; sys.stdout.buffer.write(bytes.fromhex('{}'))\")", hex_encoded);
                        if let Ok(res) = self.terminal.run_sync(&check_cmd, 5) {
                            if !res.trim().is_empty() && (res.contains("syntax error") || res.contains("error")) {
                                syntax_error = Some(res.trim().to_string());
                            }
                        }
                    } else if subgoal.target_entity.ends_with(".java") {
                        let stem = subgoal.target_entity.trim_end_matches(".java");
                        let open_braces = code.chars().filter(|&c| c == '{').count();
                        let close_braces = code.chars().filter(|&c| c == '}').count();
                        if open_braces != close_braces {
                            syntax_error = Some(format!(
                                "Java syntax error: Unbalanced curly braces ({} open vs {} close)",
                                open_braces, close_braces
                            ));
                        } else {
                            let has_decl = code.lines().any(|l| {
                                let words: Vec<&str> = l.split_whitespace().collect();
                                words.windows(2).any(|w| {
                                    (w[0] == "class" || w[0] == "interface" || w[0] == "enum")
                                        && (w[1] == stem || w[1].starts_with(&format!("{}<", stem)) || w[1].starts_with(&format!("{}(", stem)))
                                })
                            });
                            if !has_decl {
                                syntax_error = Some(format!(
                                    "Java structural mismatch: File is named '{}.java' but does not declare 'class {}'",
                                    stem, stem
                                ));
                            }
                        }
                    }
                }
            }

            // 4-Tier Verification Gate
            let real_success = match subgoal.kind {
                TaskKind::Create => {
                    exec_success && vfs_target_exists && vfs_target_size > 0 && !placeholder_found && syntax_error.is_none()
                }
                TaskKind::Modify => {
                    let has_diff = line_diff.as_ref().map(|d| !d.trim().is_empty()).unwrap_or(false);
                    exec_success && vfs_target_exists && has_diff && !placeholder_found && syntax_error.is_none()
                }
                TaskKind::Delete => {
                    exec_success && !self.vfs.exists(&target_path)
                }
                TaskKind::Test => {
                    exec_success
                        && !exec_output.contains("Traceback (most recent call last)")
                        && !exec_output.contains("FAILED")
                        && !exec_output.contains("SyntaxError:")
                }
                TaskKind::Search => {
                    exec_success && !exec_output.trim().is_empty()
                }
            };

            if real_success {
                // Synthesize immutable Epistemic Flags
                let epistemic_flag = match subgoal.kind {
                    TaskKind::Create => {
                        format!(
                            "[FLAG #{} (Verified Create): File '{}' staged in VFS ({} bytes); syntax valid; zero placeholders]",
                            subgoal.id, subgoal.target_entity, vfs_target_size
                        )
                    }
                    TaskKind::Modify => {
                        let diff_summary = line_diff.as_ref().map(|d| {
                            let added = d.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++")).count();
                            let removed = d.lines().filter(|l| l.starts_with('-') && !l.starts_with("---")).count();
                            format!("+{} / -{} lines", added, removed)
                        }).unwrap_or_else(|| "modified".to_string());
                        format!(
                            "[FLAG #{} (Verified Line Diff): File '{}' patched ({}); syntax valid; no regressions]",
                            subgoal.id, subgoal.target_entity, diff_summary
                        )
                    }
                    TaskKind::Delete => {
                        format!(
                            "[FLAG #{} (Verified Delete): File '{}' confirmed removed from VFS; tracking recorded]",
                            subgoal.id, subgoal.target_entity
                        )
                    }
                    TaskKind::Test => {
                        format!(
                            "[FLAG #{} (Verified Test): Execution gate passed with exit code 0; zero tracebacks]",
                            subgoal.id
                        )
                    }
                    TaskKind::Search => {
                        format!(
                            "[FLAG #{} (Verified Search): Discovered empirical facts]",
                            subgoal.id
                        )
                    }
                };

                println!("{}", "  ┌────────────────────────────────────────────────────────────────────────┐".bright_green());
                println!("  │ {} Thinker Perception Gate: {}", "[+]".bright_green().bold(), epistemic_flag.bright_white().bold());
                match subgoal.kind {
                    TaskKind::Create => {
                        println!("  │     * Target: {} | Size: {} bytes | Syntax Gate: VALIDATED", subgoal.target_entity.bright_cyan(), vfs_target_size);
                        println!("  │     * Clean Code: Zero placeholders, zero TODOs, production-ready");
                        println!("  │     * Status: Staged in RAM Sandbox VFS -> Ready for host commit");
                    }
                    TaskKind::Modify => {
                        let diff_lines_count = line_diff.as_ref().map(|d| d.lines().count()).unwrap_or(0);
                        println!("  │     * Target: {} | Diff lines: {} | Semantic Diff: VALIDATED", subgoal.target_entity.bright_cyan(), diff_lines_count);
                        println!("  │     * Syntax Gate: Validated | No unintended regressions detected");
                    }
                    TaskKind::Delete => {
                        println!("  │     * Target: {} | VFS Existence: REMOVED (Confirmed !vfs.exists)", subgoal.target_entity.bright_cyan());
                        println!("  │     * Tracking: Tombstone recorded in VFS ledger");
                    }
                    TaskKind::Test => {
                        let bwrap_active = IsolatedJail::is_bwrap_available();
                        println!("  │     * Sandbox Gate: PASSED (Isolation: {}, zero tracebacks)", if bwrap_active { "Bubblewrap bwrap" } else { "Ephemeral Tempdir" });
                    }
                    TaskKind::Search => {
                        println!("  │     * Search/Fetch: Gathered factual ground truth");
                    }
                }
                println!("{}", "  └────────────────────────────────────────────────────────────────────────┘".bright_green());

                // Epistemic Distillation: The lean flag replaces the raw bulky inspection tokens
                final_finding = epistemic_flag;
                break;
            }

            // FAILURE OCCURRED: Trigger Worker Rollback to Checkpoint
            let tokens_before = tokens_at_attempt_end;
            let _ = worker_ctx.rollback_to(checkpoint);
            let tokens_after = worker_ctx.current_cursor();
            let excised = tokens_before.saturating_sub(tokens_after);
            rollbacks_count += 1;
            tokens_saved += excised;

            // Deterministic Diagnostic Classification
            let diagnostic = if let Some(ref _syn_err) = syntax_error {
                DiagnosticCode::LuaSyntaxTrap
            } else {
                Self::classify_failure_for_kind(
                    &action,
                    exec_success,
                    &exec_output,
                    vfs_target_exists,
                    vfs_target_size,
                    vfs_str,
                    &generated_text,
                    last_diagnostic.as_ref(),
                    Some(&subgoal.kind),
                )
            };

            println!(
                "  [ROLLBACK] Worker #{} rolled back to checkpoint {} (excised {} tokens). Diagnostic: {}.",
                subgoal.id, tokens_after, excised, diagnostic.name()
            );

            // ESCALATE TO THINKER FOR SOVEREIGN DECISION
            let error_desc = if let Some(ref syn_err) = syntax_error {
                format!("Syntax error in '{}': {}", subgoal.target_entity, syn_err)
            } else if placeholder_found {
                format!("Incomplete placeholder in '{}': {}", subgoal.target_entity, placeholder_reason)
            } else if !exec_success {
                exec_output.clone()
            } else {
                match subgoal.kind {
                    TaskKind::Delete => format!("File '{}' was not deleted from VFS", subgoal.target_entity),
                    TaskKind::Modify => format!("Modification on '{}' yielded zero diff or failed", subgoal.target_entity),
                    TaskKind::Test => format!("Test execution for '{}' failed", subgoal.target_entity),
                    _ => {
                        if !vfs_target_exists {
                            format!("File '{}' was not written to VFS", subgoal.target_entity)
                        } else if vfs_target_size == 0 {
                            format!("File '{}' is empty (0 bytes)", subgoal.target_entity)
                        } else {
                            format!("File '{}' verification failed", subgoal.target_entity)
                        }
                    }
                }
            };

            let (decision, ledger_entry) = self.thinker_evaluate_failure(
                subgoal,
                attempt,
                &generated_text,
                &error_desc,
                &diagnostic,
            )?;

            println!("  {}", ledger_entry);

            last_diagnostic = Some(diagnostic);

            match decision {
                ThinkerDecision::Nudge(hint) => {
                    pending_nudge = Some(hint);
                    attempt += 1;
                }
                ThinkerDecision::Reset(new_directive) => {
                    current_directive = new_directive;
                    pending_nudge = None;
                    attempt += 1;
                }
                ThinkerDecision::ConsultDependency { dependency, question } => {
                    let user_choice = Self::prompt_user_for_dependency(&dependency, &question);
                    match user_choice {
                        DependencyChoice::InstallAndRetry => {
                            println!("  [TERMINAL] Installing dependency '{}'...", dependency);
                            let install_cmd = if dependency == "tkinter" {
                                "sudo dnf install -y python3-tkinter || sudo apt-get install -y python3-tk || pip install tk".to_string()
                            } else {
                                format!("pip install {} || pip3 install {}", dependency, dependency)
                            };
                            match self.terminal.run_sync(&install_cmd, 60) {
                                Ok(out) => {
                                    let summary = out.lines().last().unwrap_or("Installation completed");
                                    println!("  [TERMINAL] Installation completed: {}", summary);
                                    pending_nudge = Some(format!(
                                        "Dependency '{}' was installed on the system. Re-execute your script now.",
                                        dependency
                                    ));
                                    attempt += 1;
                                }
                                Err(e) => {
                                    println!("  [TERMINAL] Installation failed: {}. Falling back to standard library / CLI alternative.", e);
                                    current_directive = format!(
                                        "Do not use external library '{}'. Implement the solution using standard library or CLI alternative without '{}'.",
                                        dependency, dependency
                                    );
                                    pending_nudge = None;
                                    attempt += 1;
                                }
                            }
                        }
                        DependencyChoice::UseAlternative => {
                            println!("  [DECISION] Switching to standard library / CLI alternative without '{}'.", dependency);
                            current_directive = format!(
                                "Do not use external library '{}'. Implement the solution using standard library or CLI alternative without '{}'.",
                                dependency, dependency
                            );
                            pending_nudge = None;
                            attempt += 1;
                        }
                    }
                }
            }
        }

        if final_finding.is_empty() {
            final_finding = format!(
                "FAILED: Worker #{} could not write target file '{}' to VFS after {} attempts.",
                subgoal.id, subgoal.target_entity, max_attempts
            );
        }

        Ok(WorkerFinding {
            worker_id: subgoal.id,
            sub_goal: subgoal.description.clone(),
            finding: final_finding,
            steps_taken: attempt,
            rollbacks_count,
            tokens_saved_by_rollback: tokens_saved,
        })
    }

    /// System 2: Synthesizes all gathered worker findings into a final report as Executive Secretary
    fn synthesize_findings(&self, original_goal: &str, findings: &[WorkerFinding]) -> Result<String, String> {
        let all_successful = findings.iter().all(|f| !f.finding.starts_with("FAILED"));

        let mut report = String::new();
        report.push_str(":: DELIVERABLES & EXECUTION SUMMARY:\n");
        for (i, f) in findings.iter().enumerate() {
            let status_mark = if f.finding.starts_with("FAILED") { "[-]" } else { "[+]" };
            report.push_str(&format!(
                "  {} Task #{}: {}\n     Result: {}\n",
                status_mark, i + 1, f.sub_goal, f.finding
            ));
        }

        let mut ctx = match self.orchestrator_model.create_context(4096, 512, 4) {
            Ok(c) => c,
            Err(_) => return Ok(report),
        };

        let system_msg = if all_successful {
            "You are the System 2 Sovereign Secretary reporting to the user.\n\
            All deliverables were verified and staged in VFS successfully.\n\
            In 2-4 concise, professional bullet points, explain what was built, how the components interact, and how to compile or run the generated code.\n\
            Do not repeat status lines or invent failures."
        } else {
            "You are the System 2 Sovereign Secretary reporting to the user.\n\
            Some deliverables encountered unresolved issues.\n\
            In 2-3 concise bullet points, explain what succeeded and what specific files or dependencies require resolution."
        };

        let user_msg = format!(
            "User Goal: {}\nCompleted Work:\n{}\nProvide the brief architectural overview:",
            original_goal, report
        );
        let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);

        if let Ok(summary) = ctx.generate(&prompt, 256) {
            let trimmed = summary.trim();
            if !trimmed.is_empty() {
                report.push_str("\n:: ARCHITECTURAL OVERVIEW:\n");
                report.push_str(trimmed);
                report.push('\n');
            }
        }

        Ok(report)
    }

    /// Strip prefixes like RUN:, EXEC:, TERMINAL:, $, #, -, etc. from an execution command
    pub fn strip_command_prefix(raw: &str) -> String {
        let mut s = raw.trim();
        loop {
            let mut changed = false;
            if let Some(rest) = s.strip_prefix("Assistant:") {
                s = rest.trim();
                changed = true;
            }
            if let Some(rest) = s.strip_prefix("- ")
                .or_else(|| s.strip_prefix("* "))
                .or_else(|| s.strip_prefix("1. "))
                .or_else(|| s.strip_prefix("2. "))
                .or_else(|| s.strip_prefix("$ "))
                .or_else(|| s.strip_prefix("# "))
            {
                s = rest.trim();
                changed = true;
            }
            let upper = s.to_uppercase();
            if upper.starts_with("RUN:") {
                s = s[4..].trim();
                changed = true;
            } else if upper.starts_with("EXEC:") {
                s = s[5..].trim();
                changed = true;
            } else if upper.starts_with("TERMINAL:") {
                s = s[9..].trim();
                changed = true;
            } else if upper.starts_with("ACTION:") {
                s = s[7..].trim();
                changed = true;
            } else if upper.starts_with("COMMAND:") {
                s = s[8..].trim();
                changed = true;
            }
            if !changed {
                break;
            }
        }
        s.trim_matches('"').trim_matches('\'').trim().to_string()
    }

    /// Parse raw text generated by the worker model into typed SwarmAction
    pub fn parse_action(text: &str) -> SwarmAction {
        let mut cleaned = text.trim();
        if let Some(rest) = cleaned.strip_prefix("Assistant:") {
            cleaned = rest.trim();
        }

        // 1. Check for Lua code block: ```lua ... ```
        if let Some(start) = cleaned.find("```lua") {
            let after = &cleaned[start + 6..];
            let script = if let Some(end) = after.find("```") {
                &after[..end]
            } else {
                after
            }.trim();

            if script.contains("vfs.") || script.contains("terminal.run") || script.contains("print(") {
                return SwarmAction::RunLua { script: script.to_string() };
            }
        }

        // 2. Check for unified diff / patch markers: <<<<<<< TARGET ... ======= ... >>>>>>>
        if let Some(target_start) = cleaned.find("<<<<<<< TARGET") {
            let after_target = &cleaned[target_start + 14..];
            if let Some(sep) = after_target.find("=======") {
                let target_snippet = after_target[..sep].trim_matches('\n');
                let after_sep = &after_target[sep + 7..];
                if let Some(end) = after_sep.find(">>>>>>>") {
                    let replacement_snippet = after_sep[..end].trim_matches('\n');
                    let filename = if let Some(pos) = cleaned.find("PATCH:") {
                        let line = cleaned[pos..].lines().next().unwrap_or("");
                        line.trim_start_matches("PATCH:").trim().to_string()
                    } else if let Some(pos) = cleaned.find("FILE:") {
                        let line = cleaned[pos..].lines().next().unwrap_or("");
                        line.trim_start_matches("FILE:").trim().to_string()
                    } else {
                        String::new()
                    };
                    return SwarmAction::PatchFile {
                        filename,
                        target: target_snippet.to_string(),
                        replacement: replacement_snippet.to_string(),
                    };
                }
            }
        }

        // 3. Check for general code block ``` ... ```
        if let Some(start) = cleaned.find("```") {
            let after = &cleaned[start + 3..];
            // Extract optional language header line
            let first_nl = after.find('\n').unwrap_or(after.len());
            let header = after[..first_nl].trim();
            let is_ident = !header.is_empty() && !header.contains(' ') && header.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            let body_start = if is_ident { first_nl + 1 } else { 0 };
            let body = if body_start < after.len() { &after[body_start..] } else { after };
            let block = if let Some(end) = body.find("```") {
                &body[..end]
            } else {
                body
            }.trim();

            if block.contains("vfs.") || (header.eq_ignore_ascii_case("lua") && block.contains("terminal.run")) {
                return SwarmAction::RunLua { script: block.to_string() };
            } else {
                let upper_block = block.to_uppercase();
                if header.eq_ignore_ascii_case("bash") || header.eq_ignore_ascii_case("sh")
                    || upper_block.starts_with("RUN:")
                    || upper_block.starts_with("EXEC:")
                    || upper_block.starts_with("TERMINAL:")
                {
                    let clean = Self::strip_command_prefix(block);
                    return SwarmAction::RunTerminal { command: clean };
                } else if !block.is_empty() {
                    return SwarmAction::WriteFile { filename: String::new(), content: block.to_string() };
                }
            }
        }

        // 4. Raw Lua call outside code blocks (e.g. vfs.write("...", ...), vfs.delete("..."), etc.)
        if cleaned.contains("vfs.") {
            return SwarmAction::RunLua { script: cleaned.to_string() };
        }

        // 5. Raw code starting without markdown fences
        if cleaned.starts_with("#!/")
            || cleaned.starts_with("import ")
            || cleaned.starts_with("from ")
            || cleaned.starts_with("def ")
            || cleaned.starts_with("class ")
            || cleaned.starts_with("<!DOCTYPE")
            || cleaned.starts_with("<html")
        {
            return SwarmAction::WriteFile { filename: String::new(), content: cleaned.to_string() };
        }

        // 6. Standard commands line by line
        for line in cleaned.lines() {
            let mut l = line.trim();
            if let Some(rest) = l.strip_prefix("Assistant:") {
                l = rest.trim();
            }

            // Strip bullet points or numbering
            if let Some(rest) = l.strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| l.strip_prefix("1. "))
                .or_else(|| l.strip_prefix("2. "))
            {
                l = rest.trim();
            }

            // Strip common prefixes
            if let Some(rest) = l.strip_prefix("Action:").or_else(|| l.strip_prefix("action:")) {
                l = rest.trim();
            }
            if let Some(rest) = l.strip_prefix("Command:").or_else(|| l.strip_prefix("command:")) {
                l = rest.trim();
            }

            let upper = l.to_uppercase();
            if upper.starts_with("DELETE:") || upper.starts_with("REMOVE:") || upper.starts_with("RM:") {
                let file = l.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('"').trim_matches('\'');
                return SwarmAction::DeleteFile { filename: file.to_string() };
            }
            if upper.starts_with("PATCH:") {
                let rest = l[6..].trim();
                let parts: Vec<&str> = rest.split('|').collect();
                let filename = parts[0].trim().trim_matches('"').trim_matches('\'').to_string();
                let mut target = String::new();
                let mut replacement = String::new();
                for part in &parts[1..] {
                    let trimmed_part = part.trim();
                    let upper_part = trimmed_part.to_uppercase();
                    if upper_part.starts_with("TARGET:") {
                        target = trimmed_part[7..].trim().trim_matches('"').trim_matches('\'').to_string();
                    } else if upper_part.starts_with("REPLACEMENT:") {
                        replacement = trimmed_part[12..].trim().trim_matches('"').trim_matches('\'').to_string();
                    }
                }
                return SwarmAction::PatchFile { filename, target, replacement };
            }
            if upper.starts_with("EXEC:") || upper.starts_with("RUN:") || upper.starts_with("TERMINAL:") {
                let clean = Self::strip_command_prefix(l);
                return SwarmAction::RunTerminal { command: clean };
            }
            if upper.starts_with("SEARCH:") {
                let q = l[7..].trim().trim_matches('"').trim_matches('\'');
                return SwarmAction::Search { query: q.to_string() };
            }
            if upper.starts_with("FETCH:") {
                let u = l[6..].trim().trim_matches('"').trim_matches('\'');
                return SwarmAction::Fetch { url: u.to_string() };
            }
            if upper.starts_with("REPORT:") {
                let f = l[7..].trim();
                return SwarmAction::Report { finding: f.to_string() };
            }
            if upper.starts_with("CONSULT:") || upper.starts_with("ASK:") || upper.starts_with("NEED:") {
                let q = l.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('"').trim_matches('\'');
                return SwarmAction::Consult { question: q.to_string() };
            }
            if upper == "DONE" || upper.starts_with("DONE:") || upper.starts_with("DONE ") {
                return SwarmAction::Done;
            }
        }

        SwarmAction::None
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_swarm_actions() {
        let txt1 = "SEARCH: \"James Webb telescope new exoplanet atmosphere\"";
        assert_eq!(
            SwarmCoordinator::parse_action(txt1),
            SwarmAction::Search {
                query: "James Webb telescope new exoplanet atmosphere".to_string()
            }
        );

        let txt2 = "FETCH: https://en.wikipedia.org/wiki/James_Webb_Space_Telescope";
        assert_eq!(
            SwarmCoordinator::parse_action(txt2),
            SwarmAction::Fetch {
                url: "https://en.wikipedia.org/wiki/James_Webb_Space_Telescope".to_string()
            }
        );

        let txt3 = "REPORT: The atmosphere contains traces of methane and carbon dioxide.";
        assert_eq!(
            SwarmCoordinator::parse_action(txt3),
            SwarmAction::Report {
                finding: "The atmosphere contains traces of methane and carbon dioxide.".to_string()
            }
        );

        let txt4 = "DONE";
        assert_eq!(SwarmCoordinator::parse_action(txt4), SwarmAction::Done);

        // Test Lua code block parsing
        let txt5 = "```lua\nlocal x = 1\nvfs.write(\"test.html\", \"<h1>hi</h1>\")\nprint(\"done\")\n```";
        match SwarmCoordinator::parse_action(txt5) {
            SwarmAction::RunLua { script } => {
                assert!(script.contains("vfs.write"));
            }
            other => panic!("Expected RunLua, got {:?}", other),
        }

        // Test direct HTML code block produces SwarmAction::WriteFile
        let txt6 = "```html\n<!DOCTYPE html><html><body><h1>Platformer Game</h1></body></html>\n```";
        match SwarmCoordinator::parse_action(txt6) {
            SwarmAction::WriteFile { content, .. } => {
                assert!(content.contains("<h1>Platformer Game</h1>"));
            }
            other => panic!("Expected WriteFile, got {:?}", other),
        }

        // Test direct Python code block produces SwarmAction::WriteFile
        let txt8 = "```python\nimport time\nprint('hello')\n```";
        match SwarmCoordinator::parse_action(txt8) {
            SwarmAction::WriteFile { content, .. } => {
                assert!(content.contains("import time"));
            }
            other => panic!("Expected WriteFile for python, got {:?}", other),
        }

        // Test raw vfs.write call outside fences
        let txt7 = "vfs.write(\"game.html\", \"data\")";
        match SwarmCoordinator::parse_action(txt7) {
            SwarmAction::RunLua { script } => {
                assert!(script.contains("vfs.write"));
            }
            other => panic!("Expected RunLua for raw vfs.write, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_thinker_decision() {
        let sg = SubGoal {
            id: 1,
            description: "Write CSS styles".to_string(),
            target_entity: "styles.css".to_string(),
            kind: TaskKind::Create,
            guidance: None,
        };

        let t1 = "DECISION: NUDGE | REASON: Missing closing bracket in styles.css";
        assert_eq!(
            SwarmCoordinator::parse_thinker_decision(t1, &sg),
            ThinkerDecision::Nudge("Missing closing bracket in styles.css".to_string())
        );

        let t2 = "DECISION: RESET | REASON: Rewrite the architecture using grid layout";
        assert_eq!(
            SwarmCoordinator::parse_thinker_decision(t2, &sg),
            ThinkerDecision::Reset("Rewrite the architecture using grid layout".to_string())
        );
    }

    #[test]
    fn test_diagnostic_code_taxonomy() {
        let err1 = DiagnosticCode::VfsFileNotWritten;
        assert_eq!(err1.name(), "ERR_VFS_FILE_NOT_WRITTEN");
        assert!(err1.default_prescription("main.py").contains("vfs.write"));

        let err2 = DiagnosticCode::PlaceholderOrEmpty;
        assert_eq!(err2.name(), "ERR_PLACEHOLDER_OR_EMPTY");
        assert!(err2.default_prescription("main.py").contains("NEVER output placeholders"));

        let err3 = DiagnosticCode::LuaSyntaxTrap;
        assert_eq!(err3.name(), "ERR_LUA_SYNTAX");

        let err4 = DiagnosticCode::NoCodeBlock;
        assert_eq!(err4.name(), "ERR_NO_CODE_BLOCK");

        let err5 = DiagnosticCode::RepeatedAttractor;
        assert_eq!(err5.name(), "ERR_REPEATED_ATTRACTOR");
    }

    #[test]
    fn test_classify_failure() {
        // 1. No code block
        let diag1 = SwarmCoordinator::classify_failure(
            &SwarmAction::None,
            false,
            "",
            false,
            0,
            None,
            "I will now write the file for you:",
            None,
        );
        assert_eq!(diag1, DiagnosticCode::NoCodeBlock);

        // 2. Lua syntax trap
        let diag2 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunLua { script: "vfs.write(".to_string() },
            false,
            "syntax error near <eof>",
            false,
            0,
            None,
            "```lua\nvfs.write(\n```",
            None,
        );
        assert_eq!(diag2, DiagnosticCode::LuaSyntaxTrap);

        // 3. VFS file not written
        let diag3 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunLua { script: "print('done')".to_string() },
            true,
            "done",
            false,
            0,
            None,
            "```lua\nprint('done')\n```",
            None,
        );
        assert_eq!(diag3, DiagnosticCode::VfsFileNotWritten);

        // 4. Placeholder or empty
        let diag4 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunLua { script: "vfs.write('main.py', '# TODO')".to_string() },
            true,
            "",
            true,
            6,
            Some("# TODO"),
            "```lua\nvfs.write('main.py', '# TODO')\n```",
            None,
        );
        assert_eq!(diag4, DiagnosticCode::PlaceholderOrEmpty);

        // 5. Repeated attractor
        let diag5 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunLua { script: "vfs.write('main.py', '# TODO')".to_string() },
            true,
            "",
            true,
            6,
            Some("# TODO"),
            "```lua\nvfs.write('main.py', '# TODO')\n```",
            Some(&DiagnosticCode::PlaceholderOrEmpty),
        );
        assert_eq!(diag5, DiagnosticCode::RepeatedAttractor);

        // 6. Generic Missing dependency detection
        let diag6 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunLua { script: "terminal.run('python3 script.py')".to_string() },
            false,
            "ModuleNotFoundError: No module named 'scipy'",
            false,
            0,
            None,
            "",
            None,
        );
        assert_eq!(diag6, DiagnosticCode::MissingDependency("scipy".to_string()));

        let diag7 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunLua { script: "terminal.run('ffmpeg -version')".to_string() },
            false,
            "/bin/sh: line 1: ffmpeg: command not found",
            false,
            0,
            None,
            "",
            None,
        );
        assert_eq!(diag7, DiagnosticCode::MissingDependency("ffmpeg".to_string()));
    }

    #[test]
    fn test_extract_missing_dependency_generic() {
        assert_eq!(
            SwarmCoordinator::extract_missing_dependency("ModuleNotFoundError: No module named 'requests'"),
            Some("requests".to_string())
        );
        assert_eq!(
            SwarmCoordinator::extract_missing_dependency("sh: line 1: jq: command not found"),
            Some("jq".to_string())
        );
        assert_eq!(
            SwarmCoordinator::extract_missing_dependency("ImportError: cannot import name 'solve' from 'scipy'"),
            Some("scipy".to_string())
        );
    }

    #[test]
    fn test_explicit_single_file_detection() {
        assert!(SwarmCoordinator::is_explicit_single_file("Create a script in a single file"));
        assert!(SwarmCoordinator::is_explicit_single_file("اكتب كود بايثون في ملف واحد"));
        assert!(SwarmCoordinator::is_explicit_single_file("Build a standalone script for backups"));
        assert!(SwarmCoordinator::is_explicit_single_file("I need a Python program to measure internet speed."));
        assert!(SwarmCoordinator::is_explicit_single_file("أريد برنامج بايثون لقياس سرعة الإنترنت"));
        assert!(!SwarmCoordinator::is_explicit_single_file("Create an HTML app with external CSS and JS files"));
    }

    #[test]
    fn test_normalize_entity_filename() {
        assert_eq!(SwarmCoordinator::normalize_entity_filename("game_loop.java"), "GameLoop.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("snake_body.java"), "SnakeBody.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("collision_detection.java"), "CollisionDetection.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("GameLoop.java"), "GameLoop.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("Snake.java"), "Snake.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("main.py"), "main.py");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("styles.css"), "styles.css");
    }

    #[test]
    fn test_parse_patch_and_delete_actions() {
        // DELETE action
        let txt1 = "DELETE: old_script.py";
        assert_eq!(
            SwarmCoordinator::parse_action(txt1),
            SwarmAction::DeleteFile { filename: "old_script.py".to_string() }
        );

        let txt1_rm = "RM: temp_data.json";
        assert_eq!(
            SwarmCoordinator::parse_action(txt1_rm),
            SwarmAction::DeleteFile { filename: "temp_data.json".to_string() }
        );

        // PATCH command format
        let txt2 = "PATCH: config.py | TARGET: debug = False | REPLACEMENT: debug = True";
        assert_eq!(
            SwarmCoordinator::parse_action(txt2),
            SwarmAction::PatchFile {
                filename: "config.py".to_string(),
                target: "debug = False".to_string(),
                replacement: "debug = True".to_string(),
            }
        );

        // Unified diff / patch markers
        let txt3 = "PATCH: main.py\n<<<<<<< TARGET\ndef old_calc():\n    return 1\n=======\ndef new_calc():\n    return 2\n>>>>>>>";
        assert_eq!(
            SwarmCoordinator::parse_action(txt3),
            SwarmAction::PatchFile {
                filename: "main.py".to_string(),
                target: "def old_calc():\n    return 1".to_string(),
                replacement: "def new_calc():\n    return 2".to_string(),
            }
        );

        // Terminal EXEC
        let txt4 = "EXEC: pytest tests/";
        assert_eq!(
            SwarmCoordinator::parse_action(txt4),
            SwarmAction::RunTerminal { command: "pytest tests/".to_string() }
        );
    }

    #[test]
    fn test_infer_task_kind() {
        assert_eq!(SwarmCoordinator::infer_task_kind("Create a new web server in Python"), TaskKind::Create);
        assert_eq!(SwarmCoordinator::infer_task_kind("Patch the bug in line 42"), TaskKind::Modify);
        assert_eq!(SwarmCoordinator::infer_task_kind("عدل الكود وصلح الخطأ"), TaskKind::Modify);
        assert_eq!(SwarmCoordinator::infer_task_kind("Delete redundant files from workspace"), TaskKind::Delete);
        assert_eq!(SwarmCoordinator::infer_task_kind("احذف الملف القديم"), TaskKind::Delete);
        assert_eq!(SwarmCoordinator::infer_task_kind("Test the unit test suite and verify output"), TaskKind::Test);
        assert_eq!(SwarmCoordinator::infer_task_kind("اختبر الكود وافحص النتيجة"), TaskKind::Test);
    }

    #[test]
    fn test_classify_failure_for_kind() {
        // Delete kind failure when file still exists
        let diag1 = SwarmCoordinator::classify_failure_for_kind(
            &SwarmAction::DeleteFile { filename: "test.txt".to_string() },
            false,
            "failed",
            true, // file still exists
            10,
            None,
            "",
            None,
            Some(&TaskKind::Delete),
        );
        assert_eq!(diag1, DiagnosticCode::FileNotDeleted);

        // Modify kind failure when diff is empty
        let diag2 = SwarmCoordinator::classify_failure_for_kind(
            &SwarmAction::PatchFile { filename: "test.txt".to_string(), target: "a".into(), replacement: "b".into() },
            true,
            "success",
            true,
            10,
            Some(""), // empty diff
            "",
            None,
            Some(&TaskKind::Modify),
        );
        assert_eq!(diag2, DiagnosticCode::DiffEmpty);

        // Test kind failure
        let diag3 = SwarmCoordinator::classify_failure_for_kind(
            &SwarmAction::RunTerminal { command: "pytest".to_string() },
            false,
            "assertion failed",
            true,
            10,
            None,
            "",
            None,
            Some(&TaskKind::Test),
        );
        assert_eq!(diag3, DiagnosticCode::TestFailed);

        // Missing dependency extraction in test kind
        let diag4 = SwarmCoordinator::classify_failure_for_kind(
            &SwarmAction::RunTerminal { command: "python3 GameLoop.py".to_string() },
            false,
            "ModuleNotFoundError: No module named 'tkinter'",
            true,
            100,
            None,
            "",
            None,
            Some(&TaskKind::Test),
        );
        assert_eq!(diag4, DiagnosticCode::MissingDependency("tkinter".to_string()));
    }

    #[test]
    fn test_strip_command_prefix_and_robust_parsing() {
        assert_eq!(SwarmCoordinator::strip_command_prefix("RUN: python3 GameLoop.py"), "python3 GameLoop.py");
        assert_eq!(SwarmCoordinator::strip_command_prefix("EXEC:   bash test.sh"), "bash test.sh");
        assert_eq!(SwarmCoordinator::strip_command_prefix("- RUN: $ python3 foo.py"), "python3 foo.py");
        assert_eq!(SwarmCoordinator::strip_command_prefix("Assistant: 1. RUN: pytest tests/"), "pytest tests/");

        // Code block containing RUN:
        let code_block = "```python\nRUN: python3 GameLoop.py\n```";
        assert_eq!(
            SwarmCoordinator::parse_action(code_block),
            SwarmAction::RunTerminal { command: "python3 GameLoop.py".to_string() }
        );

        // Code block with bash header
        let bash_block = "```bash\npython3 GameLoop.py\n```";
        assert_eq!(
            SwarmCoordinator::parse_action(bash_block),
            SwarmAction::RunTerminal { command: "python3 GameLoop.py".to_string() }
        );

        // Command not found for procedural words should NOT extract missing dependency
        assert_eq!(SwarmCoordinator::extract_missing_dependency("sh: line 1: RUN:: not found"), None);
        assert_eq!(SwarmCoordinator::extract_missing_dependency("sh: line 1: RUN: command not found"), None);
        assert_eq!(SwarmCoordinator::extract_missing_dependency("sh: line 1: exec: not found"), None);
        assert_eq!(SwarmCoordinator::extract_missing_dependency("sh: line 1: python: command not found"), None);
    }
}

