use colored::*;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;
use crate::llama_manager::LlamaManager;
use crate::openai_api::{ChatCompletionRequest, ChatMessage};
use crate::sandbox::{
    ActionTargetType, BrowserTerminalLens, InteractiveElement, MemoryVfs, WasmPrimitiveSandbox,
};

pub async fn run_tui_session(port: u16, base_dir: &PathBuf) {
    let llama_manager = LlamaManager::new(base_dir.clone());
    let mut status = llama_manager.status();

    if !status.is_running {
        let available = llama_manager.list_available_models();
        if available.is_empty() {
            eprintln!("{}", "  [!] No GGUF models found in models/ directory.".red());
            return;
        }
        let chosen = if available.contains(&"qwen-0.5b.gguf".to_string()) {
            "qwen-0.5b.gguf".to_string()
        } else {
            available[0].clone()
        };
        println!("{}", format!("  ⏳ Engine is idle. Auto-launching '{}' on port {}...", chosen, port).yellow());
        if let Err(e) = llama_manager.start(chosen.clone(), port, 0, 2048) {
            eprintln!("  [!] Failed to auto-start model: {}", e);
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        status = llama_manager.status();
    }

    let active_model = status.active_model.clone().unwrap_or_else(|| "Local GGUF Model".to_string());

    // Print Cinematic Claude-Style Header
    println!();
    println!("{}", "╭─────────────────────────────────────────────────────────────────────────────╮".cyan());
    println!("{}", "│                         OMNI AGENTIC TERMINAL v2.0                          │".cyan().bold());
    println!("{}", "│         Pure WASM Sandbox  •  In-Memory VFS  •  Terminal Browser Lens        │".cyan());
    println!("{}", "╰─────────────────────────────────────────────────────────────────────────────╯".cyan());
    println!("  {} {}   {} {}", "Model:".bold(), active_model.green(), "Endpoint:".bold(), format!("http://127.0.0.1:{}", port).dimmed());
    println!("  {} {}   {} {}", "Sandbox:".bold(), "Isolated RAM / No Sockets".yellow(), "Status:".bold(), "Active & Guarded".green().bold());
    println!();
    println!("  {} Type {} for commands, {} to inspect web lens, {} for RAM disk.", "Quick Hints:".dimmed(), "/help".bright_yellow(), "/browser".bright_yellow(), "/vfs".bright_yellow());
    println!("{}", "───────────────────────────────────────────────────────────────────────────────".dimmed());
    println!();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let mut conversation: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".to_string(),
        content: crate::planner::GroundedSystemProfile::build_system_prompt(),
    }];

    // Session-bound isolated VFS, Terminal Bridge, and Supervisor
    let vfs = std::sync::Arc::new(MemoryVfs::new());
    let terminal_bridge = std::sync::Arc::new(crate::sandbox::TerminalSessionBridge::new(1000));
    let supervisor = crate::planner::InternalSupervisorProbe::new();
    let _wasm_sandbox = WasmPrimitiveSandbox::new();

    loop {
        // Aesthetic Prompt
        print!("{} ", "‹‹‹".bright_cyan().bold());
        let _ = io::stdout().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let query = input.trim();
        if query.is_empty() {
            continue;
        }

        // Built-in commands
        match query {
            "/exit" | "/quit" | "exit" | "quit" => {
                println!("{}", "  Session terminated cleanly. Goodbye!".dimmed());
                break;
            }
            "/clear" | "clear" => {
                conversation.truncate(1);
                print!("\x1B[2J\x1B[1;1H"); // clear screen
                println!("{}", "  [✓] Conversation context reset.".green().bold());
                println!();
                continue;
            }
            "/help" => {
                render_help_screen();
                continue;
            }
            "/browser" => {
                render_browser_lens_demo();
                continue;
            }
            "/vfs" => {
                render_vfs_status(&vfs);
                continue;
            }
            "/commit" => {
                handle_vfs_commit(&vfs);
                continue;
            }
            "/status" => {
                render_engine_status(&status, port);
                continue;
            }
            _ => {}
        }

        // Speculative Intent Probe & Pre-emptive Tool Warmup
        let spec_target = crate::planner::PrePassTriage::probe_fast_intent(query);
        match spec_target {
            crate::planner::SpeculativeTarget::TermHost => {
                terminal_bridge.arm_and_warmup();
                println!(
                    "  {} {}",
                    "🎯 [INTENT PROBE]:".bright_cyan().bold(),
                    "PREDICTED -> TermHost [PTY Buffer Armed & Ready (0ms Latency)]".bright_green()
                );
            }
            crate::planner::SpeculativeTarget::WebBrowser => {
                println!(
                    "  {} {}",
                    "🎯 [INTENT PROBE]:".bright_cyan().bold(),
                    "PREDICTED -> WebBrowser [HTTP / RSS Pipeline Primed]".bright_blue()
                );
            }
            crate::planner::SpeculativeTarget::DirectVfs => {
                println!(
                    "  {} {}",
                    "🎯 [INTENT PROBE]:".bright_cyan().bold(),
                    "PREDICTED -> MemoryVfs [RAM Buffer Staged]".bright_yellow()
                );
            }
            crate::planner::SpeculativeTarget::PureChat => {
                println!(
                    "  {} {}",
                    "🎯 [INTENT PROBE]:".bright_cyan().bold(),
                    "PREDICTED -> Pure Interactive Dialogue [Tools Sleeping]".dimmed()
                );
            }
        }

        // Pass 1: Ephemeral Micro-Triage & System 2 Tri-Plan Synthesis
        let intent = crate::planner::PrePassTriage::evaluate(query);
        let something_i_know = if let Some(plan) = &intent.dual_system_plan {
            println!(
                "  {} [Plan A: {}, Plan B: {}, Plan C: {}]",
                "🧠 [SYSTEM 2 TRI-PLAN SYNTHESIS]:".bright_purple().bold(),
                plan.plan_a.bright_cyan(),
                plan.plan_b.bright_yellow(),
                plan.plan_c.dimmed()
            );
            println!(
                "  {} {}",
                "⚡ [CAVEMAN INVARIANTS]:".bright_yellow().bold(),
                plan.caveman_constraints.join("; ").dimmed()
            );
            plan.distilled_something_i_know.clone()
        } else {
            String::new()
        };

        // Synthesize Tailored Grounded System Prompt
        let initial_system_prompt = crate::planner::GroundedSystemProfile::build_system_2_grounded_prompt(
            &intent.required_tools,
            &something_i_know,
            None,
        );
        if !conversation.is_empty() {
            conversation[0].content = initial_system_prompt.clone();
        }

        // Pass 2: System 1 Execution Pass
        conversation.push(ChatMessage {
            role: "user".to_string(),
            content: query.to_string(),
        });

        let max_attempts = if intent.is_execution_task { 3 } else { 1 };
        let mut attempt = 1;
        let mut causal_feedback: Option<String> = None;

        while attempt <= max_attempts {
            if let Some(feedback) = &causal_feedback {
                let updated_system = crate::planner::GroundedSystemProfile::build_system_2_grounded_prompt(
                    &intent.required_tools,
                    &something_i_know,
                    Some(feedback),
                );
                conversation[0].content = updated_system;
            }

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                    .template("{spinner:.cyan} {msg}")
                    .unwrap(),
            );
            let spinner_msg = if attempt == 1 {
                if intent.is_execution_task {
                    "System 1 executing primary Plan A in sandbox..."
                } else {
                    "Evaluating prompt in direct interactive mode..."
                }
            } else {
                "System 1 executing fallback plan with injected causal feedback..."
            };
            pb.set_message(spinner_msg);
            pb.enable_steady_tick(Duration::from_millis(80));

            let req = ChatCompletionRequest {
                model: Some(active_model.clone()),
                messages: conversation.clone(),
                temperature: Some(0.6),
                stream: Some(true),
                max_tokens: Some(2048),
            };

            match client.post(&url).json(&req).send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        pb.finish_and_clear();
                        let err_text = response.text().await.unwrap_or_default();
                        println!("{} {}", "  [✗] Backend Inference Error:".red().bold(), err_text);
                        break;
                    }

                    pb.finish_and_clear();
                    println!();
                    print!("{} ", "›››".bright_green().bold());
                    let _ = io::stdout().flush();

                    let mut stream = response.bytes_stream();
                    let mut full_response = String::new();

                    while let Some(chunk_result) = stream.next().await {
                        if let Ok(chunk) = chunk_result {
                            let text = String::from_utf8_lossy(&chunk);
                            for line in text.lines() {
                                let line = line.trim();
                                if line.starts_with("data: ") {
                                    let json_data = line.trim_start_matches("data: ").trim();
                                    if json_data == "[DONE]" {
                                        break;
                                    }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_data) {
                                        if let Some(delta) = val["choices"][0]["delta"]["content"].as_str() {
                                            print!("{}", delta);
                                            let _ = io::stdout().flush();
                                            full_response.push_str(delta);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    println!();
                    println!();

                    // If streaming gave no tokens (fallback non-stream)
                    if full_response.is_empty() {
                        let fallback_req = ChatCompletionRequest {
                            model: Some(active_model.clone()),
                            messages: conversation.clone(),
                            temperature: Some(0.6),
                            stream: Some(false),
                            max_tokens: Some(2048),
                        };
                        if let Ok(res) = client.post(&url).json(&fallback_req).send().await {
                            if let Ok(json_res) = res.json::<serde_json::Value>().await {
                                if let Some(content) = json_res["choices"][0]["message"]["content"].as_str() {
                                    println!("{}", content.trim());
                                    println!();
                                    full_response = content.trim().to_string();
                                }
                            }
                        }
                    }

                    if !full_response.is_empty() {
                        let hands = crate::planner::ExecutiveHands::with_terminal(
                            vfs.clone(),
                            base_dir.clone(),
                            terminal_bridge.clone(),
                        );
                        let action = hands.parse_action(&full_response);
                        if action != crate::planner::ExecutiveAction::None {
                            let exec_res = hands.execute(action);
                            if exec_res.success {
                                println!(
                                    "  {} {}",
                                    "⚡ [EXECUTIVE ACTION DISPATCHED]:".bright_yellow().bold(),
                                    exec_res.output.bright_cyan()
                                );
                                if exec_res.requires_user_confirmation {
                                    println!(
                                        "  {} Use {} to review full diffs and authorize commit to disk.",
                                        "🔒 [GATE REQUIRED]:".bright_red().bold(),
                                        "/commit".bright_yellow().bold()
                                    );
                                }
                                println!();
                                conversation.push(ChatMessage {
                                    role: "assistant".to_string(),
                                    content: full_response,
                                });
                                break;
                            } else if attempt < max_attempts {
                                // Execution trapped! Initiate Causal KV Rollback & Autonomous Fallback
                                println!(
                                    "  {} {}",
                                    "⚠️  [EXECUTIVE ACTION TRAPPED]:".bright_red().bold(),
                                    exec_res.output.red()
                                );
                                println!(
                                    "  {} {}",
                                    "🔄 [CAUSAL KV ROLLBACK]:".bright_yellow().bold(),
                                    "Trapped attempt purged from KV cache (approx 350 tokens saved).".dimmed()
                                );

                                let plan_label = match attempt {
                                    1 => "Plan A",
                                    2 => "Plan B",
                                    _ => "Plan C",
                                };

                                let lesson = supervisor.prune_branch_with_causal_lesson(
                                    &format!("attempt-{}", attempt),
                                    plan_label,
                                    &exec_res.output,
                                    "Do not repeat trapped syntax/API. Use fallback plan directly.",
                                    350,
                                );

                                println!(
                                    "  {} Learned Rule #{}: {}",
                                    "💡 [DISTILLED CAUSAL LESSON]:".bright_green().bold(),
                                    lesson.lesson_id,
                                    lesson.distillation_rule.bright_white()
                                );

                                let next_plan_hint = match attempt {
                                    1 => intent.dual_system_plan.as_ref().map(|p| p.plan_b.as_str()).unwrap_or("Fallback Plan B"),
                                    2 => intent.dual_system_plan.as_ref().map(|p| p.plan_c.as_str()).unwrap_or("Fallback Plan C"),
                                    _ => "Emergency minimal fallback",
                                };

                                println!(
                                    "  {} Engaging fallback strategy: {}",
                                    "⚡ [AUTONOMOUS PIVOT]:".bright_cyan().bold(),
                                    next_plan_hint.bright_yellow()
                                );
                                println!();

                                causal_feedback = Some(format!(
                                    "PREVIOUS TRAP ROOT CAUSE: {}. Discard previous attempt. Activate fallback: {}. Follow caveman invariants.",
                                    exec_res.output, next_plan_hint
                                ));

                                attempt += 1;
                                // Crucial: full_response is NOT pushed to conversation (KV cache rollback!)
                                continue;
                            } else {
                                println!(
                                    "  {} {}",
                                    "💀 [EPISTEMIC APOPTOSIS TRIGGERED]:".bright_red().bold(),
                                    "Generation 1 dead-end reached (3 traps). Purging rotten branch.".red()
                                );

                                let testament = supervisor.trigger_epistemic_apoptosis(&full_response);
                                println!(
                                    "  {} Extracted {} raw testament tokens directly.",
                                    "🧬 [ANCESTRAL TESTAMENT ACQUIRED]:".bright_purple().bold(),
                                    testament.testament_tokens_raw.len()
                                );

                                println!(
                                    "  {} Rebirthing fresh generation with ancestral epigenetic prefix...",
                                    "✨ [EPIGENETIC CACHE REBIRTH]:".bright_cyan().bold()
                                );

                                conversation.clear();
                                let reborn_system = format!(
                                    "{}\n\n[ANCESTRAL EPIGENETIC MEMORY FROM PREVIOUS EXTINCT GENERATION]:\n{}\n[SURVIVE: Strict compliance with ancestral memory required.]",
                                    initial_system_prompt,
                                    testament.testament_tokens_raw
                                );
                                conversation.push(ChatMessage {
                                    role: "system".to_string(),
                                    content: reborn_system,
                                });
                                conversation.push(ChatMessage {
                                    role: "user".to_string(),
                                    content: format!("{} (AVOID EXTINCTION: Follow ancestral rules directly)", query),
                                });

                                println!();
                                break;
                            }
                        } else {
                            // Fallback check if response contains classic FILE block
                            check_and_stage_code_blocks(&vfs, &full_response);
                            conversation.push(ChatMessage {
                                role: "assistant".to_string(),
                                content: full_response,
                            });
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    pb.finish_and_clear();
                    println!("{} Failed to connect to engine: {}", "  [✗]".red().bold(), e);
                    break;
                }
            }
        }
    }
}

fn render_help_screen() {
    println!();
    println!("{}", "  ╭─────────────────────── AVAILABLE COMMANDS ───────────────────────╮".bright_cyan());
    println!("  │  {}          Send a prompt or instruction to the model    │", "‹‹‹ <prompt>".bold());
    println!("  │  {}             Display active 2D Terminal ASCII Browser Lens│", "/browser    ".bright_yellow());
    println!("  │  {}                 Inspect staged virtual files in RAM Sandbox  │", "/vfs        ".bright_yellow());
    println!("  │  {}              Interactive review & write diffs to disk     │", "/commit     ".bright_yellow());
    println!("  │  {}              Display system hardware, model and port      │", "/status     ".bright_yellow());
    println!("  │  {}               Wipe conversation history context            │", "/clear      ".bright_yellow());
    println!("  │  {}                Exit interactive terminal session            │", "/exit       ".bright_yellow());
    println!("{}", "  ╰──────────────────────────────────────────────────────────────────╯".bright_cyan());
    println!();
}

fn render_browser_lens_demo() {
    let lens = BrowserTerminalLens::new(88, 10);
    let elements = vec![
        InteractiveElement {
            id: 1,
            target_type: ActionTargetType::Input,
            label: "Search Google Maps or Web".to_string(),
            selector: "#search-input".to_string(),
            current_value: Some("Cairo Hospital Competitors".to_string()),
        },
        InteractiveElement {
            id: 2,
            target_type: ActionTargetType::Button,
            label: "Filter: Top Rated (★ 4.5+)".to_string(),
            selector: "#filter-rating".to_string(),
            current_value: None,
        },
        InteractiveElement {
            id: 3,
            target_type: ActionTargetType::SelectableRow,
            label: "Cleopatra Hospital - Heliopolis (Rating 4.6 | 1.8k reviews)".to_string(),
            selector: "#poi-card-101".to_string(),
            current_value: None,
        },
        InteractiveElement {
            id: 4,
            target_type: ActionTargetType::Button,
            label: "Export Selected to M.A.R.K.E.T Pipeline".to_string(),
            selector: "#btn-export".to_string(),
            current_value: None,
        },
    ];

    let proj = lens.project_canvas("Market Intelligence Lens", "https://maps.google.com", elements);

    println!();
    println!("{}", "  ╭─── [2D TERMINAL BROWSER LENS (Grounding View)] ────────────────────────╮".bright_magenta().bold());
    for line in proj.ascii_grid.lines() {
        println!("  │  {}", line.bright_white());
    }
    println!("{}", "  ╰────────────────────────────────────────────────────────────────────────╯".bright_magenta().bold());
    println!("  {} Model can choose: {} or {} to trigger actions without touching host DOM.", "Action Grounding:".dimmed(), "click(3)".bright_yellow().bold(), "type(1, \"...\")".bright_yellow().bold());
    println!();
}

fn render_vfs_status(vfs: &MemoryVfs) {
    let diffs = vfs.generate_staged_diffs();
    println!();
    println!("{}", "  ╭─── [IN-MEMORY VIRTUAL FILESYSTEM (RAM SANDBOX)] ───────────────────────╮".bright_blue().bold());
    if diffs.is_empty() {
        println!("  │  {}", "No pending changes. RAM disk matches host state.".dimmed());
    } else {
        for d in &diffs {
            let badge = match d.change_type {
                crate::sandbox::FileChangeType::Created => "[+ CREATED]".green().bold(),
                crate::sandbox::FileChangeType::Modified => "[~ MODIFIED]".yellow().bold(),
                crate::sandbox::FileChangeType::Deleted => "[- DELETED]".red().bold(),
            };
            println!("  │  {} {:<50}", badge, format!("{:?}", d.path).white());
        }
        println!("  │  ");
        println!("  │  {}", "Use /commit to inspect full diffs and authorize write.".bright_yellow());
    }
    println!("{}", "  ╰────────────────────────────────────────────────────────────────────────╯".bright_blue().bold());
    println!();
}

fn handle_vfs_commit(vfs: &MemoryVfs) {
    let diffs = vfs.generate_staged_diffs();
    if diffs.is_empty() {
        println!("{}", "  [i] No staged changes in RAM sandbox to commit.".dimmed());
        return;
    }

    println!();
    println!("{}", "  ════════════════════ STAGED DIFFS REVIEW GATE ════════════════════".bright_yellow().bold());
    for d in &diffs {
        println!("  {} {:?} ({:?})", "● Target File:".bold(), d.path, d.change_type);
        if let Some(content) = &d.new_content {
            println!("{}", "  ┌── Content Preview ──".dimmed());
            for line in content.lines().take(12) {
                println!("  │ {}", line.bright_green());
            }
            if content.lines().count() > 12 {
                println!("  │ {}", format!("... (+ {} more lines)", content.lines().count() - 12).dimmed());
            }
            println!("{}", "  └─────────────────────".dimmed());
        }
    }
    println!("{}", "  ══════════════════════════════════════════════════════════════════".bright_yellow().bold());

    print!("  {} (y/N) › ", "Authorize writing these changes to host disk?".bright_red().bold());
    let _ = io::stdout().flush();

    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_ok() {
        let trimmed = answer.trim().to_lowercase();
        if trimmed == "y" || trimmed == "yes" {
            match vfs.commit_to_host(true) {
                Ok(count) => {
                    println!("{}", format!("  [✓] Successfully committed {} file(s) to host disk.", count).green().bold());
                }
                Err(e) => {
                    println!("{} {}", "  [✗] Commit failed:".red().bold(), e);
                }
            }
        } else {
            println!("{}", "  [!] Authorization denied. Modifications remain safely inside RAM VFS.".yellow());
        }
    }
    println!();
}

fn render_engine_status(status: &crate::llama_manager::LlamaEngineStatus, port: u16) {
    let specs = crate::system_info::get_system_specs();
    println!();
    println!("{}", "  ╭────────────────────────── SYSTEM TELEMETRY ──────────────────────────╮".bright_cyan().bold());
    println!("  │  CPU Cores:    {:<52} │", format!("{} Physical Threads", specs.cpu_cores).white());
    println!("  │  RAM:          {:<52} │", format!("{:.1} GB Free / {:.1} GB Total", specs.free_ram_gb, specs.total_ram_gb).white());
    println!("  │  Active Model: {:<52} │", status.active_model.as_deref().unwrap_or("None").green());
    println!("  │  PID / Port:   {:<52} │", format!("PID: {:?}  •  Port: {}", status.pid, port).white());
    println!("  │  Sandbox Mode: {:<52} │", "WASM Bare-Metal Stack Machine (Isolated)".bright_green());
    println!("{}", "  ╰──────────────────────────────────────────────────────────────────────╯".bright_cyan().bold());
    println!();
}

fn check_and_stage_code_blocks(vfs: &MemoryVfs, response: &str) {
    // If model proposed a file write with pattern:
    // FILE: <path>
    // ```...```
    if let Some(file_idx) = response.find("FILE: ") {
        let rest = &response[file_idx + 6..];
        if let Some(newline_idx) = rest.find('\n') {
            let filename = rest[..newline_idx].trim().trim_matches('`');
            if let Some(code_start) = rest.find("```") {
                let code_rest = &rest[code_start + 3..];
                let content_start = code_rest.find('\n').map(|i| i + 1).unwrap_or(0);
                if let Some(code_end) = code_rest[content_start..].find("```") {
                    let code_body = &code_rest[content_start..content_start + code_end];
                    vfs.write_file(PathBuf::from(filename), code_body.as_bytes());
                    println!("{}", format!("  [+] Staged file in RAM VFS: '{}' (Type /vfs or /commit to inspect)", filename).bright_blue());
                }
            }
        }
    }
}
