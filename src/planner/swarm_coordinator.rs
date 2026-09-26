use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use colored::*;
use tracing::{info, warn};

use crate::causal_memory::dag::CausalGraph;
use crate::native_llama::{NativeLlamaContext, NativeLlamaModel};
use crate::sandbox::{MemoryVfs, WebLens, SearchResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwarmAction {
    Search { query: String },
    Fetch { url: String },
    RunLua { script: String },
    Consult { question: String },
    Report { finding: String },
    Done,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThinkerDecision {
    Nudge(String),
    Reset(String),
}

#[derive(Debug, Clone)]
pub struct SubGoal {
    pub id: usize,
    pub description: String,
    pub target_entity: String,
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

        // Largest model as orchestrator, smallest as worker
        let orch = found_models[0].clone();
        let worker = found_models.last().cloned().unwrap_or_else(|| orch.clone());

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
        terminal.arm_and_warmup();

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

        // PHASE 2: System 1 Swarm Execution with Thinker Steering & Physical KV Rollback
        let mut findings: Vec<WorkerFinding> = Vec::new();

        for mut sg in subgoals {
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

            let finding = self.run_worker_loop(user_goal, &sg)?;
            findings.push(finding);
            println!();
        }

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
        let mut ctx = self.orchestrator_model.create_context(2048, 256, 4)?;
        let system_msg = "You are the System 2 Sovereign Thinker commanding an execution worker.\n\
The user communicates ONLY with you. Workers are your executive hands.\n\
Your worker is equipped with:\n\
- vfs.write(\"filename\", [[content]]): Write complete file content to VFS\n\
- vfs.read(\"filename\"): Read file from VFS\n\
- terminal.run(\"command\"): Execute shell command on host\n\
- DONE: Finish task\n\n\
Your Task:\n\
Take the user's goal and issue a single, concrete, explicit operational directive commanding the worker what exact tool to call and what file/content to implement.\n\
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
    /// 1. Temporary Thinker context inspects the error and code.
    /// 2. Thinker generates decision: NUDGE (minor fixable) or RESET (major rewrite).
    /// 3. Thinker context is wiped immediately, purging all broken code from active cache.
    /// 4. Returns the decision and a dense ledger entry.
    fn thinker_evaluate_failure(
        &self,
        subgoal: &SubGoal,
        attempt: usize,
        failed_code: &str,
        runtime_error: &str,
    ) -> Result<(ThinkerDecision, String), String> {
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
            "Subgoal: {}\nTarget: {}\nAttempt #{}\nWorker Code:\n{}\nRuntime Error:\n{}\n\nDecision:",
            subgoal.description, subgoal.target_entity, attempt, snippet_code, snippet_err
        );

        let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);
        let raw = ctx.generate(&prompt, 128)?;

        // WIPE temporary inspection tokens from Thinker context immediately
        ctx.kv_cache_clear();

        let decision = Self::parse_thinker_decision(&raw, subgoal);
        let ledger_entry = match &decision {
            ThinkerDecision::Nudge(hint) => {
                format!("[LEDGER: Worker #{} Attempt #{} FAILED -> Action: NUDGE ('{}')]", subgoal.id, attempt, hint)
            }
            ThinkerDecision::Reset(directive) => {
                format!("[LEDGER: Worker #{} Attempt #{} FAILED -> Action: RESET ('{}')]", subgoal.id, attempt, directive)
            }
        };

        Ok((decision, ledger_entry))
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

fn infer_target_entity(goal: &str) -> String {
    let lower = goal.to_lowercase();
    if lower.contains(".py") || lower.contains("python") || lower.contains("بايثون") {
        "main.py".to_string()
    } else if lower.contains(".pdf") || lower.contains("pdf") {
        "document.pdf".to_string()
    } else if lower.contains(".sh") || lower.contains("bash") || lower.contains("shell") || lower.contains("باش") {
        "script.sh".to_string()
    } else if lower.contains(".json") || lower.contains("json") {
        "data.json".to_string()
    } else if lower.contains(".csv") || lower.contains("csv") || lower.contains("جدول") {
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
        || lower.contains("bash")
        || lower.contains("shell")
        || lower.contains("json")
        || lower.contains("csv")
        || lower.contains("اكتب")
        || lower.contains("انشئ")
        || lower.contains("أنشئ")
        || lower.contains("اصنع")
        || lower.contains("اعمل")
        || lower.contains("صفحة")
        || lower.contains("صفحه")
        || lower.contains("كود")
        || lower.contains("برمج")
        || lower.contains("سكربت")
        || lower.contains("سكريبت")
        || lower.contains("بايثون")
        || lower.contains("تقرير")
        || lower.contains("ملف")
        || lower.contains("لعبة")
        || lower.contains("لعبه")
        || lower.contains("موقع")
        || lower.contains("تطبيق")
        || lower.contains("زر")
}

    /// System 2: Decomposes goal into discrete sub-goals
    fn orchestrate_plan(&self, goal: &str) -> Result<Vec<SubGoal>, String> {
        let is_build = Self::is_build_task(goal);

        let mut ctx = self.orchestrator_model.create_context(4096, 512, 4)?;

        let system_msg = if is_build {
            format!(
                "You are the System 2 Sovereign Architect.\n\
                Environment: In-memory Virtual Filesystem (VFS) sandbox.\n\
                Workers write standalone project files via Lua: vfs.write(\"filename\", [[content]]).\n\n\
                Decision Authority on Workers:\n\
                - You decide how many workers to deploy (from 1 up to {} max budget).\n\
                - For single-file deliverables (e.g. Python scripts, PDF generators, shell utilities, or standalone apps): Deploy exactly 1 worker to generate the complete file in one pass.\n\
                - Only decompose into multiple sub-goals if the objective genuinely requires separate, independent modules or files.\n\
                - Provide each worker with an explicit, self-contained implementation directive.\n\n\
                Output each sub-goal on a new line strictly formatted as:\n\
                SUBGOAL: <explicit implementation directive> | ENTITY: <target filename>\n\n\
                Examples:\n\
                SUBGOAL: Write the complete Python automation script | ENTITY: main.py\n\
                SUBGOAL: Write the PDF generation pipeline or document source | ENTITY: document.pdf\n\
                SUBGOAL: Write the standalone web application | ENTITY: index.html\n\
                SUBGOAL: Write the shell maintenance utility | ENTITY: script.sh",
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
        let mut id = 1;

        for line in output.lines() {
            let l = line.trim();
            if l.starts_with("SUBGOAL:") {
                let rest = l.trim_start_matches("SUBGOAL:").trim();
                let parts: Vec<&str> = rest.split("| ENTITY:").collect();
                let desc = parts[0].trim().to_string();
                let entity = if parts.len() > 1 && !parts[1].trim().is_empty() {
                    parts[1].trim().to_string()
                } else if is_build {
                    Self::infer_target_entity(goal)
                } else {
                    goal.split_whitespace().next().unwrap_or("general").to_string()
                };

                if !desc.is_empty() {
                    subgoals.push(SubGoal {
                        id,
                        description: desc,
                        target_entity: entity,
                        guidance: None,
                    });
                    id += 1;
                    if subgoals.len() >= self.config.max_subgoals {
                        break;
                    }
                }
            }
        }

        // Fallback if model output did not match format exactly
        if subgoals.is_empty() {
            if is_build {
                let inferred = Self::infer_target_entity(goal);
                subgoals.push(SubGoal {
                    id: 1,
                    description: format!("Create and write complete standalone implementation for: {}", goal),
                    target_entity: inferred,
                    guidance: None,
                });
            } else {
                subgoals.push(SubGoal {
                    id: 1,
                    description: format!("Search web for: {}", goal),
                    target_entity: goal.split_whitespace().next().unwrap_or("topic").to_string(),
                    guidance: None,
                });
            }
        }

        Ok(subgoals)
    }

    /// System 1: Autonomous Worker Loop with Checkpoint Freeze and Thinker Sovereign Decisions
    fn run_worker_loop(&self, _user_goal: &str, subgoal: &SubGoal) -> Result<WorkerFinding, String> {
        let is_build = Self::is_build_task(&subgoal.description);
        let mut worker_ctx = self.worker_model.create_context(4096, 512, 4)?;

        let mut attempt = 1;
        let max_attempts = 3;
        let mut current_directive = subgoal.guidance.clone().unwrap_or_else(|| subgoal.description.clone());
        let mut pending_nudge: Option<String> = None;
        let mut checkpoint: usize = 0;

        let mut rollbacks_count = 0;
        let mut tokens_saved = 0;
        let mut final_finding = String::new();
        let target_path = std::path::PathBuf::from(&subgoal.target_entity);

        while attempt <= max_attempts {
            // Check if starting fresh (attempt 1 or after a RESET)
            if pending_nudge.is_none() {
                worker_ctx.kv_cache_clear();

                let system_msg = if is_build {
                    format!(
                        "You are an Executive Hands Worker in an autonomous dual-model swarm.\n\
                        You are directed exclusively by the System 2 Sovereign Thinker.\n\
                        Environment: In-memory Virtual Filesystem (VFS) sandbox.\n\
                        Objective: {}\n\n\
                        Available Tools:\n\
                        - vfs.write(\"filename\", [[content]]): Write complete file content to VFS\n\
                        - vfs.read(\"filename\"): Read file from VFS\n\
                        - terminal.run(\"command\"): Execute shell command on host\n\
                        - print(\"message\"): Log execution output\n\
                        - DONE: Signal that the objective is complete\n\n\
                        Rules:\n\
                        1. You execute the Thinker's direct instructions using your tools.\n\
                        2. Output your Lua code inside a ```lua ... ``` block.\n\
                        3. Put complete, functional code or data inside [[ ... ]]. NEVER output placeholder comments like <!-- TODO --> or <!-- implementation -->.\n\
                        4. Desktop GUI libraries (gui.*, window.*) DO NOT EXIST. Implement web applications, scripts, or system tasks directly via vfs.write.\n\
                        5. Always output DONE after fulfilling the objective.",
                        subgoal.description
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
                        "Thinker Directive: {}\nTarget File: {}\nGoal: {}\n\nExecute this directive now by calling the appropriate tool inside a ```lua ... ``` block. Output DONE when complete.",
                        current_directive, subgoal.target_entity, subgoal.description
                    )
                } else {
                    format!(
                        "Thinker Directive: {}\nGoal: {}\n\nExecute now with SEARCH: <query> or REPORT: <facts>:",
                        current_directive, subgoal.description
                    )
                };

                let prompt = Self::format_prompt(&self.worker_model, &system_msg, &user_msg);
                let prompt_tokens = self.worker_model.tokenize(&prompt, true)?;
                worker_ctx.eval_tokens(&prompt_tokens, 0)?;

                // CHECKPOINT FREEZE: Anchor cursor position before generation
                checkpoint = worker_ctx.current_cursor();
                println!(
                    "  [WORKER CHECKPOINT] Worker #{} anchored at position {} tokens.",
                    subgoal.id, checkpoint
                );
            } else if let Some(ref nudge) = pending_nudge {
                // NUDGE BRANCH: Worker is frozen at checkpoint!
                let nudge_turn = format!("\nNotice from Thinker: {}\nCorrect the issue and execute now:", nudge);
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

            let action = Self::parse_action(&generated_text);
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
            let (exec_success, exec_output) = match action {
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
                    (false, "Unparsed action / missing ```lua code block".to_string())
                }
            };

            // GROUND TRUTH CHECK
            let vfs_target_exists = self.vfs.exists(&target_path);
            let vfs_target_size = self.vfs.read_file(&target_path).map(|c| c.len()).unwrap_or(0);

            let real_success = if is_build {
                exec_success && vfs_target_exists && vfs_target_size > 30
            } else {
                exec_success && !exec_output.trim().is_empty()
            };

            if real_success {
                println!(
                    "  [VERIFIED] Task verified on attempt #{}. Target: {} ({} bytes)",
                    attempt, subgoal.target_entity, vfs_target_size
                );
                // Worker returns to idle: 100% cache clear
                worker_ctx.kv_cache_clear();
                final_finding = if is_build {
                    format!("Target file '{}' created and verified ({} bytes).", subgoal.target_entity, vfs_target_size)
                } else {
                    format!("Task completed: {}", exec_output)
                };
                break;
            }

            // FAILURE OCCURRED: Trigger Worker Rollback to Checkpoint
            let tokens_before = tokens_at_attempt_end;
            let _ = worker_ctx.rollback_to(checkpoint);
            let tokens_after = worker_ctx.current_cursor();
            let excised = tokens_before.saturating_sub(tokens_after);
            rollbacks_count += 1;
            tokens_saved += excised;

            println!(
                "  [ROLLBACK] Worker #{} rolled back to checkpoint {} (excised {} tokens). Worker frozen.",
                subgoal.id, tokens_after, excised
            );

            // ESCALATE TO THINKER FOR SOVEREIGN DECISION
            let error_desc = if !exec_success {
                exec_output.clone()
            } else if !vfs_target_exists {
                format!("File '{}' was not written to VFS", subgoal.target_entity)
            } else {
                format!("File '{}' is too small ({} bytes)", subgoal.target_entity, vfs_target_size)
            };

            let (decision, ledger_entry) = self.thinker_evaluate_failure(
                subgoal,
                attempt,
                &generated_text,
                &error_desc
            )?;

            println!("  {}", ledger_entry);

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
            }
        }

        if final_finding.is_empty() {
            final_finding = format!("Task '{}' completed attempts budget.", subgoal.description);
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
        let mut ctx = self.orchestrator_model.create_context(4096, 512, 4)?;

        let mut findings_block = String::new();
        for (i, f) in findings.iter().enumerate() {
            findings_block.push_str(&format!(
                "- Task #{}: {} -> {}\n",
                i + 1, f.sub_goal, f.finding
            ));
        }

        let system_msg = "You are the Executive Secretary reporting directly to the user.\n\
Speak directly, clearly, and concisely without embellishments or theatrical language.\n\
Summarize exactly what was accomplished (1, 2, 3), which files were verified, and confirm completion of the request.";
        let user_msg = format!("User Request: {}\n\nCompleted Work:\n{}\nProvide the final report:", original_goal, findings_block);
        let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);

        let final_report = ctx.generate(&prompt, 512)?;
        Ok(final_report)
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
            };
            return SwarmAction::RunLua { script: script.trim().to_string() };
        }

        // 2. Check for general code block ``` ... ```
        if let Some(start) = cleaned.find("```") {
            let after = &cleaned[start + 3..];
            let block = if let Some(end) = after.find("```") {
                &after[..end]
            } else {
                after
            }.trim();

            if block.contains("vfs.write") || block.contains("print(") {
                return SwarmAction::RunLua { script: block.to_string() };
            }
        }

        // 3. Raw Lua call outside code blocks (e.g. vfs.write("...", ...))
        if cleaned.contains("vfs.write(") {
            return SwarmAction::RunLua { script: cleaned.to_string() };
        }

        // 4. Standard commands line by line
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

        // Test that unformatted raw HTML produces None (triggering Causal KV Rollback)
        let txt6 = "```html\n<!DOCTYPE html><html><body><h1>Platformer Game</h1></body></html>\n```";
        assert_eq!(SwarmCoordinator::parse_action(txt6), SwarmAction::None);

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
}
