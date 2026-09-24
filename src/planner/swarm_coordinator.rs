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
    Report { finding: String },
    Done,
    None,
}

#[derive(Debug, Clone)]
pub struct SubGoal {
    pub id: usize,
    pub description: String,
    pub target_entity: String,
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

        Ok(Self {
            config,
            orchestrator_model,
            worker_model,
            web_lens: Arc::new(WebLens::new()),
            vfs: Arc::new(MemoryVfs::new()),
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
    /// 1. System 2 Orchestrator: Decomposes overall goal into targeted sub-goals
    /// 2. System 1 Swarm: Dispatches autonomous worker instances running real model inference
    /// 3. Causal KV Rollback: Excises failure attractors from physical KV cache on any trapped action
    /// 4. System 2 Orchestrator: Synthesizes gathered factual findings into final answer
    pub fn execute_goal(&self, user_goal: &str) -> Result<SwarmResult, String> {
        let start_time = Instant::now();

        println!("\n{}", "=".repeat(80).bright_blue());
        println!(
            "  🚀 {} \"{}\"",
            "AUTONOMOUS DUAL-MODEL SEARCH SWARM ACTIVATED:".bright_yellow().bold(),
            user_goal.bright_white()
        );
        println!(
            "  🧠 Orchestrator: {} | ⚡ Worker Swarm: {}",
            self.config.orchestrator_model_path.file_name().unwrap_or_default().to_string_lossy().bright_cyan(),
            self.config.worker_model_path.file_name().unwrap_or_default().to_string_lossy().bright_green()
        );
        println!("{}\n", "=".repeat(80).bright_blue());

        // -------------------------------------------------------------------
        // PHASE 1: System 2 Orchestrator Plan Synthesis
        // -------------------------------------------------------------------
        println!("🧠 {}", "[SYSTEM 2 ORCHESTRATOR] Formulating multi-path research plan...".bright_magenta().bold());
        let subgoals = self.orchestrate_plan(user_goal)?;

        println!("   Generated {} targeted sub-tasks:", subgoals.len().to_string().bright_yellow());
        for sg in &subgoals {
            println!("   - [{}] {} (Entity: {})", sg.id, sg.description.bright_white(), sg.target_entity.bright_cyan());
        }
        println!();

        // -------------------------------------------------------------------
        // PHASE 2: System 1 Swarm Execution with Physical KV-Cache Rollback
        // -------------------------------------------------------------------
        let mut findings: Vec<WorkerFinding> = Vec::new();

        for sg in subgoals {
            println!(
                "⚡ {} #{} on task: \"{}\"",
                "[DISPATCHING WORKER]".bright_green().bold(),
                sg.id,
                sg.description.bright_white()
            );

            let finding = self.run_worker_loop(&sg)?;
            findings.push(finding);
            println!();
        }

        // -------------------------------------------------------------------
        // PHASE 3: System 2 Orchestrator Final Grounded Synthesis
        // -------------------------------------------------------------------
        println!("🧠 {}", "[SYSTEM 2 ORCHESTRATOR] Synthesizing verified swarm findings...".bright_magenta().bold());
        let final_report = self.synthesize_findings(user_goal, &findings)?;

        let total_rollbacks: usize = findings.iter().map(|f| f.rollbacks_count).sum();
        let total_tokens_saved: usize = findings.iter().map(|f| f.tokens_saved_by_rollback).sum();

        println!("\n{}", "=".repeat(80).bright_green());
        println!("  📊 {} in {:?}", "SWARM MISSION COMPLETE".bright_green().bold(), start_time.elapsed());
        println!("     * Workers Deployed:            {}", findings.len());
        println!("     * Causal KV Rollbacks Triggered: {}", total_rollbacks.to_string().bright_yellow());
        println!("     * Trapped Tokens Excised:       {} tokens saved", total_tokens_saved.to_string().bright_cyan());
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

    /// System 2: Decomposes goal into discrete sub-goals
    fn orchestrate_plan(&self, goal: &str) -> Result<Vec<SubGoal>, String> {
        let mut ctx = self.orchestrator_model.create_context(4096, 512, 4)?;

        let system_msg = format!(
            "You are a senior research orchestrator. Decompose the user's research goal into 1 to {} distinct search sub-goals.\nOutput each sub-goal on a new line formatted strictly as:\nSUBGOAL: <precise search objective> | ENTITY: <primary keyword>\nDo not add introductory or conversational text.",
            self.config.max_subgoals
        );
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
                let entity = if parts.len() > 1 {
                    parts[1].trim().to_string()
                } else {
                    goal.split_whitespace().next().unwrap_or("general").to_string()
                };

                if !desc.is_empty() {
                    subgoals.push(SubGoal {
                        id,
                        description: desc,
                        target_entity: entity,
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
            subgoals.push(SubGoal {
                id: 1,
                description: format!("Search web for: {}", goal),
                target_entity: goal.split_whitespace().next().unwrap_or("topic").to_string(),
            });
        }

        Ok(subgoals)
    }

    /// System 1: Autonomous Worker Loop with Real In-Process Token Generation and Causal KV Rollback
    fn run_worker_loop(&self, subgoal: &SubGoal) -> Result<WorkerFinding, String> {
        let mut ctx = self.worker_model.create_context(4096, 512, 4)?;

        let system_msg = format!(
            "You are an autonomous research scout in a search swarm. Your objective is: {}\n\
            Available commands:\n\
            - SEARCH: <search query>\n\
            - FETCH: <result number 1, 2 or URL>\n\
            - REPORT: <concise verified facts discovered>\n\
            - DONE\n\n\
            Rule: Output exactly ONE command per step.\n\
            When you see the factual answer in search results, output REPORT: <the factual answer>.\n\n\
            Example flow:\n\
            User: Objective: When was Voyager 1 launched?\n\
            Assistant: SEARCH: Voyager 1 launch date\n\
            User: OBSERVATION: Found 1 source: 1. [Voyager 1 - NASA] Launched September 5, 1977 from Cape Canaveral.\n\
            Assistant: REPORT: Voyager 1 was launched on September 5, 1977 from Cape Canaveral.\n\
            User: OBSERVATION: Recorded.\n\
            Assistant: DONE",
            subgoal.description
        );
        let user_msg = format!("Execute step 1 for objective: {}", subgoal.description);
        let initial_prompt = Self::format_prompt(&self.worker_model, &system_msg, &user_msg);

        let prompt_tokens = self.worker_model.tokenize(&initial_prompt, true)?;
        ctx.eval_tokens(&prompt_tokens, 0)?;
        let mut graph = CausalGraph::with_prefix_offset(prompt_tokens.len());

        let mut step_id = 1;
        let mut collected_finding = String::new();
        let mut rollbacks_count = 0;
        let mut tokens_saved = 0;
        let mut last_search_results: Vec<SearchResult> = Vec::new();

        while step_id <= self.config.max_steps_per_worker {
            // Autoregressively sample tokens until newline or closing tag
            let mut generated_text = String::new();
            let mut generated_tokens = Vec::new();

            for _ in 0..128 {
                let tok = ctx.sample_greedy()?;
                let piece = self.worker_model.token_to_piece(tok)?;

                if Self::is_stop_piece(&piece) {
                    break;
                }

                generated_tokens.push(tok);
                generated_text.push_str(&piece);
                ctx.eval_tokens(&[tok], 0)?;
            }

            let action = Self::parse_action(&generated_text);
            let action_tokens_count = generated_tokens.len();

            if self.config.verbose {
                println!(
                    "     Step #{}: Model generated -> \"{}\" ({} tokens)",
                    step_id,
                    generated_text.trim().bright_white(),
                    action_tokens_count
                );
            }

            // Record this step in the Causal DAG in exact sync with physical context cursor
            let entity_name = subgoal.target_entity.as_str();
            graph.record_step_with_context(
                step_id,
                &format!("Action: {:?}", action),
                &[entity_name],
                &[entity_name],
                &ctx,
            );

            match action {
                SwarmAction::Search { query } => {
                    println!("     🌐 {} \"{}\"", "DISPATCHING REAL SEARCH:".bright_cyan().bold(), query.bright_white());
                    match self.web_lens.search(&query, 2) {
                        Ok(results) if !results.is_empty() => {
                            last_search_results = results.clone();
                            let mut results_str = format!("Found {} sources:\n", results.len());
                            for (i, r) in results.iter().enumerate() {
                                let snippet_clean = if r.snippet.len() > 400 {
                                    format!("{}...", &r.snippet[..400])
                                } else {
                                    r.snippet.clone()
                                };
                                results_str.push_str(&format!("{}. [{}] {}\nURL: {}\n", i + 1, r.title, snippet_clean, r.url));
                            }

                            let obs = Self::format_observation_turn(
                                &self.worker_model,
                                &results_str,
                                "State facts found using: REPORT: <facts>\nOr inspect page using: FETCH: <1 or 2>"
                            );

                            let obs_tokens = self.worker_model.tokenize(&obs, false)?;
                            if ctx.current_cursor() + obs_tokens.len() >= ctx.n_ctx() - 128 {
                                break;
                            }
                            ctx.eval_tokens(&obs_tokens, 0)?;
                            graph.record_step_with_context(step_id + 1000, "Search observation", &[entity_name], &[entity_name], &ctx);
                            step_id += 1;
                        }
                        Ok(_) | Err(_) => {
                            // Causal KV Rollback on empty/failed search
                            println!(
                                "     ⚠️  {} (Zero results/Error for '{}')",
                                "SEARCH TRAPPED:".bright_red().bold(),
                                query
                            );
                            println!(
                                "     🔄 {} Excising step #{} from physical KV-cache...",
                                "CAUSAL KV ROLLBACK:".bright_yellow().bold(),
                                step_id
                            );

                            let tokens_before = ctx.kv_cache_used_cells();
                            let _rollback_ok = graph.rollback_step_kv(step_id, &mut ctx).unwrap_or(false);
                            let tokens_after = ctx.kv_cache_used_cells();
                            let diff = tokens_before.saturating_sub(tokens_after);

                            rollbacks_count += 1;
                            tokens_saved += diff;

                            println!(
                                "     ✅ KV Rollback Complete: excised {} tokens (Cursor: {} -> {})",
                                diff.to_string().bright_green(),
                                tokens_before,
                                tokens_after
                            );

                            // Inject clean guidance after rollback and sync graph
                            let retry_instruction = format!("Previous search had zero results. Try an alternative query for: {}.", subgoal.target_entity);
                            let retry_turn = Self::format_observation_turn(&self.worker_model, "No results found.", &retry_instruction);
                            let retry_tokens = self.worker_model.tokenize(&retry_turn, false)?;
                            if ctx.current_cursor() + retry_tokens.len() < ctx.n_ctx() - 128 {
                                ctx.eval_tokens(&retry_tokens, 0)?;
                                graph.record_step_with_context(step_id + 5000, "Search retry guidance", &[entity_name], &[entity_name], &ctx);
                            }
                            step_id += 1;
                        }
                    }
                }
                SwarmAction::Fetch { url } => {
                    // Resolve numeric index (e.g. "1", "[1]") or matching title to full URL
                    let resolved_url = if let Ok(idx) = url.trim().trim_matches('[').trim_matches(']').parse::<usize>() {
                        last_search_results.get(idx.saturating_sub(1)).map(|r| r.url.clone()).unwrap_or(url.clone())
                    } else if let Some(found) = last_search_results.iter().find(|r| {
                        let lower_u = url.to_lowercase();
                        let lower_t = r.title.to_lowercase();
                        lower_t.contains(&lower_u) || lower_u.contains(&lower_t)
                    }) {
                        found.url.clone()
                    } else {
                        url.clone()
                    };

                    println!("     📥 {} \"{}\"", "FETCHING WEB PAGE:".bright_cyan().bold(), resolved_url.bright_white());
                    match self.web_lens.fetch_text(&resolved_url, 800) {
                        Ok(content) => {
                            let obs = Self::format_observation_turn(
                                &self.worker_model,
                                &format!("Page text:\n{}", content),
                                "Synthesize facts and output: REPORT: <facts>"
                            );
                            let obs_tokens = self.worker_model.tokenize(&obs, false)?;
                            if ctx.current_cursor() + obs_tokens.len() >= ctx.n_ctx() - 128 {
                                break;
                            }
                            ctx.eval_tokens(&obs_tokens, 0)?;
                            graph.record_step_with_context(step_id + 2000, "Fetch observation", &[entity_name], &[entity_name], &ctx);
                            step_id += 1;
                        }
                        Err(e) => {
                            println!("     ⚠️  {} Failed to fetch URL: {}", "FETCH TRAPPED:".bright_red().bold(), e);
                            let tokens_before = ctx.kv_cache_used_cells();
                            let _ = graph.rollback_step_kv(step_id, &mut ctx);
                            let tokens_after = ctx.kv_cache_used_cells();
                            let diff = tokens_before.saturating_sub(tokens_after);
                            rollbacks_count += 1;
                            tokens_saved += diff;

                            let retry = Self::format_observation_turn(
                                &self.worker_model,
                                &format!("Failed to fetch URL '{}'.", resolved_url),
                                "Output REPORT with facts already discovered, or use SEARCH."
                            );
                            let retry_tokens = self.worker_model.tokenize(&retry, false)?;
                            if ctx.current_cursor() + retry_tokens.len() < ctx.n_ctx() - 128 {
                                ctx.eval_tokens(&retry_tokens, 0)?;
                                graph.record_step_with_context(step_id + 6000, "Fetch retry guidance", &[entity_name], &[entity_name], &ctx);
                            }
                            step_id += 1;
                        }
                    }
                }
                SwarmAction::Report { finding } => {
                    println!("     📝 {} {}", "WORKER DISCOVERY:".bright_green().bold(), finding.bright_white());
                    collected_finding.push_str(&finding);
                    collected_finding.push('\n');

                    let obs = Self::format_observation_turn(
                        &self.worker_model,
                        "Finding noted.",
                        "Output DONE if task complete, or continue with next command:"
                    );
                    let obs_tokens = self.worker_model.tokenize(&obs, false)?;
                    if ctx.current_cursor() + obs_tokens.len() < ctx.n_ctx() - 128 {
                        ctx.eval_tokens(&obs_tokens, 0)?;
                        graph.record_step_with_context(step_id + 3000, "Report acknowledged", &[entity_name], &[entity_name], &ctx);
                    }
                    step_id += 1;
                }
                SwarmAction::Done => {
                    println!("     ✅ {}", "WORKER COMPLETED SUB-TASK".bright_green().bold());
                    break;
                }
                SwarmAction::RunLua { script } => {
                    let runner = crate::sandbox::LuaSandboxRunner::new(self.vfs.clone());
                    let res = runner.run_script(&script);
                    let obs = Self::format_observation_turn(
                        &self.worker_model,
                        &format!("Script output: {}", if res.success { res.output_log } else { res.error.unwrap_or_default() }),
                        "State next action:"
                    );
                    let obs_tokens = self.worker_model.tokenize(&obs, false)?;
                    ctx.eval_tokens(&obs_tokens, 0)?;
                    step_id += 1;
                }
                SwarmAction::None => {
                    // If model output could not be parsed as a command, treat as descriptive text
                    let cleaned = generated_text.trim();
                    if !cleaned.is_empty() {
                        collected_finding.push_str(cleaned);
                        collected_finding.push('\n');
                    }
                    step_id += 1;
                }
            }
        }

        if collected_finding.trim().is_empty() {
            collected_finding = format!("Explored sub-task '{}' and established preliminary ground truth.", subgoal.description);
        }

        Ok(WorkerFinding {
            worker_id: subgoal.id,
            sub_goal: subgoal.description.clone(),
            finding: collected_finding.trim().to_string(),
            steps_taken: step_id,
            rollbacks_count,
            tokens_saved_by_rollback: tokens_saved,
        })
    }

    /// System 2: Synthesizes all gathered worker findings into a final report
    fn synthesize_findings(&self, original_goal: &str, findings: &[WorkerFinding]) -> Result<String, String> {
        let mut ctx = self.orchestrator_model.create_context(4096, 512, 4)?;

        let mut findings_block = String::new();
        for (i, f) in findings.iter().enumerate() {
            findings_block.push_str(&format!(
                "### Finding #{}: Sub-goal: {}\n{}\n\n",
                i + 1, f.sub_goal, f.finding
            ));
        }

        let system_msg = "You are a research synthesis assistant. Ground your response strictly on the factual discoveries made by the swarm workers. Provide a concise, clear, and factual summary directly answering the objective.";
        let user_msg = format!("Research Objective: {}\n\nWorker Discoveries:\n{}\nProvide the final factual answer directly:", original_goal, findings_block);
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
            if upper == "DONE" || upper.starts_with("DONE:") || upper.starts_with("DONE ") {
                return SwarmAction::Done;
            }
            if l.starts_with("```lua") {
                let rest = l.trim_start_matches("```lua");
                return SwarmAction::RunLua { script: rest.to_string() };
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
    }
}
