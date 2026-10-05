use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::{error::Error, io, time::{Duration, Instant}};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::tui::ui::draw_ui;


impl TuiApp {

    pub fn refresh_sessions(&mut self) {
        let mut sessions = Vec::new();
        let sessions_dir = crate::tui::config::OmniConfig::config_dir().join("sessions");
        let _ = std::fs::create_dir_all(&sessions_dir);
        if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "json" {
                        let name = entry.path().file_stem().unwrap().to_string_lossy().to_string();
                        sessions.push((name, entry.path()));
                    }
                }
            }
        }
        // Sort descending by name (since name is timestamp)
        sessions.sort_by(|a, b| b.0.cmp(&a.0));
        self.saved_sessions = sessions;
    }

    pub fn save_current_session(&self) {
        if self.messages.is_empty() { return; }
        let sessions_dir = crate::tui::config::OmniConfig::config_dir().join("sessions");
        let _ = std::fs::create_dir_all(&sessions_dir);
        let path = sessions_dir.join(format!("{}.json", self.current_session_id));
        if let Ok(json) = serde_json::to_string_pretty(&self.messages) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn load_session(&mut self, idx: usize) {
        if let Some((name, path)) = self.saved_sessions.get(idx) {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(msgs) = serde_json::from_str::<Vec<String>>(&content) {
                    self.messages = msgs;
                    self.current_session_id = name.clone();
                    self.show_sidebar = false;
                }
            }
        }
    }
}

pub enum CurrentView {
    Chat,
    Settings,
}


pub struct SystemMonitor {
    last_idle: f64,
    last_total: f64,
    pub cpu_usage: f64,
    pub ram_used_gb: f64,
}
impl Default for SystemMonitor {
    fn default() -> Self {
        Self { last_idle: 0.0, last_total: 0.0, cpu_usage: 0.0, ram_used_gb: 0.0 }
    }
}
impl SystemMonitor {
    pub fn update(&mut self) {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0.0;
            let mut avail = 0.0;
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") { total = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0.0); }
                if line.starts_with("MemAvailable:") { avail = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0.0); }
            }
            if total > 0.0 { self.ram_used_gb = (total - avail) / 1024.0 / 1024.0; }
        }
        if let Ok(stat) = std::fs::read_to_string("/proc/stat") {
            if let Some(line) = stat.lines().next() {
                let parts: Vec<f64> = line.split_whitespace().skip(1).filter_map(|s| s.parse().ok()).collect();
                if parts.len() >= 4 {
                    let idle = parts[3];
                    let total: f64 = parts.iter().sum();
                    let diff_idle = idle - self.last_idle;
                    let diff_total = total - self.last_total;
                    if diff_total > 0.0 && self.last_total > 0.0 {
                        self.cpu_usage = (1.0 - diff_idle / diff_total) * 100.0;
                    }
                    self.last_idle = idle;
                    self.last_total = total;
                }
            }
        }
    }
}

pub struct TuiApp {
    pub clipboard: Option<arboard::Clipboard>,
    pub sys_monitor: SystemMonitor,
    pub downloader: crate::downloader::ModelDownloader,
    pub active_downloads: Vec<crate::downloader::DownloadTask>,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,
    pub chat_scroll_offset: usize,
    pub attached_files: Vec<String>,
    pub current_task_status: String,
    pub saved_sessions: Vec<(String, std::path::PathBuf)>,
    pub current_session_id: String,
    pub slash_menu_index: usize,
    pub config: crate::tui::config::OmniConfig,
    pub input: Input,
    pub messages: Vec<String>,
    pub tick_count: usize,
    pub show_sidebar: bool,
    pub current_view: CurrentView,
    pub is_generating: bool,
    pub is_global_loading: bool,
    pub global_loading_msg: String,
    pub term_bidi_supported: bool,
    pub term_cjk_supported: bool,
    pub sidebar_selected: usize,
    pub settings_selected: usize,
}

impl Default for TuiApp {
    fn default() -> Self {
        // Heuristic detection for Terminal capabilities
        let lang = std::env::var("LANG").unwrap_or_default().to_lowercase();
        let term = std::env::var("TERM").unwrap_or_default().to_lowercase();
        let vte = std::env::var("VTE_VERSION").is_ok();
        let konsole = std::env::var("KONSOLE_VERSION").is_ok();
        let wt = std::env::var("WT_SESSION").is_ok();
        
        // UTF-8 is practically required for CJK wide chars
        let cjk = lang.contains("utf-8") || lang.contains("utf8");
        
        // BiDi is supported in Konsole, VTE (Gnome), Windows Terminal, mlterm
        let bidi = konsole || vte || wt || term.contains("mlterm");

        
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| std::path::PathBuf::from("."));
        let base_dir = if exe_dir.join("models").exists() { exe_dir } else { std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")) };
        let downloader = crate::downloader::ModelDownloader::new(base_dir.join("models"));
        
        Self {
            downloader,
            active_downloads: Vec::new(),
            command_history: Vec::new(),
            history_index: None,
            chat_scroll_offset: 0,
            attached_files: Vec::new(),
            current_task_status: "Generating response...".to_string(),
            saved_sessions: Vec::new(),
            current_session_id: chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string(),
            slash_menu_index: 0,
            clipboard: arboard::Clipboard::new().ok(),
            sys_monitor: SystemMonitor::default(),
            config: crate::tui::config::OmniConfig::load(),
            input: Input::default(),
            messages: vec![
                "[System]: Omni Engine v2.2.0 Sovereign Runtime Initialized.".into(),
                "[Thinker]: Awaiting your directives...".into(),
            ],
            tick_count: 0,
            show_sidebar: false,
            current_view: CurrentView::Chat,
            is_generating: false,
            is_global_loading: true, // starts true until initial load finishes
            global_loading_msg: "Initializing Omni Engine...".to_string(),
            term_bidi_supported: bidi,
            term_cjk_supported: cjk,
            sidebar_selected: 0,
            settings_selected: 0,
        }
    }
}

use std::sync::mpsc::{self, Receiver, Sender};
use std::path::PathBuf;
use crate::native_llama::NativeLlamaModel;

pub enum WorkerMessage {
    Token(String),
    Status(String),
    LoadStarted(String),
    LoadComplete(String),
    LoadFailed(String),
    Done,
    Error(String),
}

pub async fn run_tui() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::default();
    let tick_rate = Duration::from_millis(60);
    let mut last_tick = Instant::now();

    // Setup communication channels
    let (tx_ui, rx_ui) = mpsc::channel::<WorkerMessage>();
        let (tx_cmd, rx_cmd) = mpsc::channel::<String>();
    
    // Initial Load
    let initial_model = app.config.default_model.clone();
    let tx_cmd_clone = tx_cmd.clone();
    std::thread::spawn(move || {
        let _ = tx_cmd_clone.send(format!("/LOAD_MODEL {}", initial_model));
    });
    
    

    // Spawn the background worker thread for AI
    std::thread::spawn(move || {
        crate::native_llama::NativeLlamaModel::disable_logs();
        
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let base_dir = if exe_dir.join("models").exists() { exe_dir } else { std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")) };

        let mut current_model: Option<std::sync::Arc<crate::native_llama::NativeLlamaModel>> = None;
        let mut current_ctx: Option<crate::native_llama::NativeLlamaContext> = None;
        let mut active_model_name = String::new();

        while let Ok(cmd) = rx_cmd.recv() {
            if cmd.starts_with("/LOAD_MODEL ") {
                let model_name = cmd.trim_start_matches("/LOAD_MODEL ").trim();
                let _ = tx_ui.send(WorkerMessage::LoadStarted(format!("Loading {}...", model_name)));
                
                // Free RAM first
                current_ctx = None;
                current_model = None;
                
                let path = base_dir.join("models").join(model_name);
                if !path.exists() {
                    let _ = tx_ui.send(WorkerMessage::LoadFailed(format!("Model file not found: {:?}", path)));
                    continue;
                }

                match crate::native_llama::NativeLlamaModel::load(&path, 0) {
                    Ok(m) => {
                        match m.create_context(8192, 512, 6) {
                            Ok(c) => {
                                current_ctx = Some(c);
                                current_model = Some(m);
                                active_model_name = model_name.to_string();
                                let _ = tx_ui.send(WorkerMessage::LoadComplete(format!("Model {} loaded successfully.", model_name)));
                            },
                            Err(e) => {
                                let _ = tx_ui.send(WorkerMessage::LoadFailed(format!("Context failed: {}", e)));
                            }
                        }
                    },
                    Err(e) => {
                        let _ = tx_ui.send(WorkerMessage::LoadFailed(format!("Load failed: {}", e)));
                    }
                }
                continue;
            }

            if cmd.starts_with("/goal ") {
                let goal_text = cmd.trim_start_matches("/goal ").trim();
                let _ = tx_ui.send(WorkerMessage::Status("Disengaging Chat... Initializing Swarm...".into()));
                
                // 1. Free RAM!
                current_ctx = None;
                current_model = None;
                
                let _ = tx_ui.send(WorkerMessage::Token(format!("\n[System]: 🚀 Launching Swarm to execute goal: {}\n", goal_text)));
                
                // 2. Spawn Child Process
                let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("./omni_engine"));
                let mut child = std::process::Command::new(exe)
                    .arg("swarm")
                    .arg(goal_text)
                    .arg("--model")
                    .arg(base_dir.join("models").join(&active_model_name))
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .expect("Failed to spawn swarm process");
                    
                if let Some(stdout) = child.stdout.take() {
                    use std::io::BufRead;
                    let reader = std::io::BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        let _ = tx_ui.send(WorkerMessage::Token(format!("{}\n", line)));
                    }
                }
                
                let status = child.wait().unwrap();
                let _ = tx_ui.send(WorkerMessage::Token(format!("\n[System]: Swarm execution completed with status {}. Reloading chat model...\n", status)));
                let _ = tx_ui.send(WorkerMessage::Status(format!("Restoring {}", active_model_name)));
                
                // 3. Reload Model
                let path = base_dir.join("models").join(&active_model_name);
                if path.exists() {
                    if let Ok(m) = crate::native_llama::NativeLlamaModel::load(&path, 0) {
                        if let Ok(c) = m.create_context(8192, 512, 6) {
                            current_ctx = Some(c);
                            current_model = Some(m);
                            let _ = tx_ui.send(WorkerMessage::LoadComplete(format!("Model {} restored successfully.", active_model_name)));
                            let _ = tx_ui.send(WorkerMessage::Status("".into()));
                        } else {
                            let _ = tx_ui.send(WorkerMessage::LoadFailed("Failed to restore context.".into()));
                        }
                    } else {
                        let _ = tx_ui.send(WorkerMessage::LoadFailed("Failed to load model weights.".into()));
                    }
                }
                let _ = tx_ui.send(WorkerMessage::Done);
                continue;
            }

            if cmd.starts_with("/SYSTEM ") || cmd.starts_with("/CLEAR_CACHE") {
                if cmd.starts_with("/CLEAR_CACHE") {
                    current_ctx = None;
                    current_model = None;
                    let _ = tx_ui.send(WorkerMessage::LoadComplete("VRAM Cleared successfully.".into()));
                }
                continue;
            }

            // Normal Prompt handling
            if let (Some(model), Some(ctx)) = (current_model.as_ref(), current_ctx.as_mut()) {
                let model_lower = active_model_name.to_lowercase();
                let formatted_prompt = if model_lower.contains("gemma") {
                    format!("<start_of_turn>user\n{}<end_of_turn>\n<start_of_turn>model\n", cmd.trim())
                } else if model_lower.contains("llama-3") || model_lower.contains("llama3") {
                    format!("<|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n", cmd.trim())
                } else {
                    format!("<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", cmd.trim())
                };
                
                let prompt_tokens = match model.tokenize(&formatted_prompt, true) {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                if ctx.eval_tokens(&prompt_tokens, 0).is_err() { continue; }

                let mut generated = 0;
                while generated < 512 {
                    if let Ok(cancel) = rx_cmd.try_recv() {
                        if cancel == "/stop" {
                            let _ = tx_ui.send(WorkerMessage::Token("\n\n[System]: 🛑 Generation stopped by user.\n".into()));
                            break;
                        }
                    }
                    let next_tok = match ctx.sample_greedy() {
                        Ok(t) => t,
                        Err(_) => break,
                    };
                    let piece = match model.token_to_piece(next_tok) {
                        Ok(p) => p,
                        Err(_) => break,
                    };
                    
                    let stop_tokens = ["<|im_end|>", "<|endoftext|>", "</s>", "<eos>", "<end_of_turn>", "<|eot_id|>", "<step_end>"];
                    if piece.is_empty() || stop_tokens.iter().any(|&s| piece.contains(s)) {
                        break;
                    }

                    let _ = tx_ui.send(WorkerMessage::Token(piece));
                    
                    if ctx.eval_tokens(&[next_tok], 0).is_err() { break; }
                    generated += 1;
                }
                let _ = tx_ui.send(WorkerMessage::Done);
            } else {
                let _ = tx_ui.send(WorkerMessage::Error("No model is loaded. Use /models to load one.".into()));
                let _ = tx_ui.send(WorkerMessage::Done);
            }
        }
    });

    loop {
        terminal.draw(|f| draw_ui(f, &app))?;

        // Process any messages from the background worker
        while let Ok(msg) = rx_ui.try_recv() {
            match msg {
                WorkerMessage::Status(s) => {
                    app.current_task_status = s;
                }
                WorkerMessage::LoadStarted(s) => {
                    app.is_global_loading = true;
                    app.global_loading_msg = s;
                }
                WorkerMessage::LoadComplete(s) => {
                    app.is_global_loading = false;
                    app.messages.push(format!("[System]: ✨ {}", s));
                }
                WorkerMessage::LoadFailed(s) => {
                    app.is_global_loading = false;
                    app.messages.push(format!("[System]: ❌ Load Error: {}", s));
                }
                WorkerMessage::Error(e) => {
                    app.messages.push(format!("❌ [Error]: {}", e));
                    app.is_generating = false;
                }
                WorkerMessage::Token(t) => {
                    if let Some(last) = app.messages.last_mut() {
                        last.push_str(&t);
                        let cleaned = last
                            .replace("<|im_end|>", "")
                            .replace("<|im_start|>", "")
                            .replace("<end_of_turn>", "")
                            .replace("<start_of_turn>", "")
                            .replace("<|eot_id|>", "")
                            .replace("<eos>", "")
                            .replace("</s>", "");
                        if *last != cleaned {
                            *last = cleaned;
                        }
                    }
                }
                WorkerMessage::Done => {
                    app.is_generating = false;
                }
            }
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            let evt = event::read()?;
            if let Event::Resize(_, _) = evt {
                let _ = terminal.autoresize();
                let _ = terminal.clear();
            } else if let Event::Key(key) = evt {
                match key.code {
                    KeyCode::Esc => {
                        if app.show_sidebar {
                            app.show_sidebar = false;
                        } else if matches!(app.current_view, CurrentView::Settings) {
                            app.current_view = CurrentView::Chat;
                        } else if app.is_generating {
                            let _ = tx_cmd.send("/stop".into());
                        } else {
                            break;
                        }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        if app.is_generating {
                            let _ = tx_cmd.send("/stop".into());
                        } else {
                            break;
                        }
                    }
                    KeyCode::Char('q') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::Tab => {
                        let val = app.input.value().to_lowercase();
                        if val.starts_with('/') {
                            let commands = ["/models", "/download ", "/read ", "/goal ", "/copy", "/reset", "/system ", "/theme ", "/exit"];
                            let typed = val.replace("/", "");
                            let filtered: Vec<_> = commands.iter().filter(|c| c.replace("/", "").starts_with(&typed) || typed.is_empty()).collect();
                            
                            if !filtered.is_empty() {
                                // If the current input is already one of the exact commands, cycle to the next
                                if let Some(pos) = filtered.iter().position(|&&c| c == &val) {
                                    let next_pos = (pos + 1) % filtered.len();
                                    app.input = tui_input::Input::new(filtered[next_pos].to_string());
                                } else {
                                    app.input = tui_input::Input::new(filtered[0].to_string());
                                }
                            }
                        } else {
                            app.show_sidebar = !app.show_sidebar;
                            if app.show_sidebar {
                                app.refresh_sessions();
                            }
                        }
                    }
                    KeyCode::F(2) => {
                        disable_raw_mode()?;
                        // Disable mouse capture so it doesn't bleed into Nano/Vim!
                        execute!(std::io::stdout(), LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
                        
                        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
                        let _ = std::process::Command::new(editor).arg(crate::tui::config::OmniConfig::config_path()).status();
                        
                        enable_raw_mode()?;
                        execute!(std::io::stdout(), EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
                        
                        app.config = crate::tui::config::OmniConfig::load();
                        app.messages.push("[System]: Configuration Hot-Reloaded!".into());
                    }
                    KeyCode::Up => {
                        if app.show_sidebar {
                            if app.sidebar_selected > 0 { app.sidebar_selected -= 1; }
                        } else if matches!(app.current_view, CurrentView::Settings) {
                            if app.settings_selected > 0 { app.settings_selected -= 1; }
                        } else if matches!(app.current_view, CurrentView::Chat) {
                            if !app.command_history.is_empty() {
                                let new_idx = match app.history_index {
                                    Some(i) => if i > 0 { i - 1 } else { 0 },
                                    None => app.command_history.len() - 1,
                                };
                                app.history_index = Some(new_idx);
                                app.input = tui_input::Input::new(app.command_history[new_idx].clone());
                            }
                        }
                    }
                    KeyCode::Down => {
                        if app.show_sidebar {
                            if app.sidebar_selected < app.saved_sessions.len() + 3 { app.sidebar_selected += 1; }
                        } else if matches!(app.current_view, CurrentView::Settings) {
                            if app.settings_selected < 3 { app.settings_selected += 1; }
                        } else if matches!(app.current_view, CurrentView::Chat) {
                            if let Some(i) = app.history_index {
                                if i + 1 < app.command_history.len() {
                                    app.history_index = Some(i + 1);
                                    app.input = tui_input::Input::new(app.command_history[i + 1].clone());
                                } else {
                                    app.history_index = None;
                                    app.input = tui_input::Input::new(String::new());
                                }
                            }
                        }
                    }
                    KeyCode::PageUp => { app.chat_scroll_offset = app.chat_scroll_offset.saturating_add(3); }
                    KeyCode::PageDown => { app.chat_scroll_offset = app.chat_scroll_offset.saturating_sub(3); }
                    KeyCode::Enter => {
                        if app.show_sidebar {
                            let tools_start = app.saved_sessions.len();
                            if app.sidebar_selected < tools_start {
                                app.load_session(app.sidebar_selected);
                            } else {
                                let tool_idx = app.sidebar_selected - tools_start;
                                match tool_idx {
                                    0 => { 
                                        app.save_current_session();
                                        app.messages.clear();
                                        app.current_session_id = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                                        app.messages.push("[System]: ✨ New Session Started.".into());
                                        app.show_sidebar = false;
                                        app.refresh_sessions();
                                    }
                                    1 => {
                                        app.show_sidebar = false;
                                        app.current_view = CurrentView::Settings;
                                    }
                                    2 => {
                                        app.messages.push(format!("[System]: ⚡ Performance - CPU: {:.1}%, RAM: {:.2} GB", app.sys_monitor.cpu_usage, app.sys_monitor.ram_used_gb));
                                        app.show_sidebar = false;
                                    }
                                    3 => {
                                        app.messages.push("[System]: 🧹 VRAM KV-Cache flush requested...".into());
                                        let _ = tx_cmd.send("/CLEAR_CACHE".into());
                                        app.show_sidebar = false;
                                    }
                                    _ => {}
                                }
                            }
                        } else if matches!(app.current_view, CurrentView::Settings) {
                            // Settings Mock
                        } else if matches!(app.current_view, CurrentView::Chat) && !app.is_generating {
                            let msg = app.input.value().to_string();
                            if !msg.is_empty() {
                                app.command_history.push(msg.clone());
                                app.history_index = None;
                                app.chat_scroll_offset = 0;
                                app.messages.push(format!("❯ [You]: {}", msg));
                                
                                if msg.trim() == "/models" {
                                    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| std::path::PathBuf::from("."));
                                    let base_dir = if exe_dir.join("models").exists() { exe_dir } else { std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")) };
                                    let mut found = Vec::new();
                                    if let Ok(entries) = std::fs::read_dir(base_dir.join("models")) {
                                        for entry in entries.flatten() {
                                            if let Some(ext) = entry.path().extension() {
                                                if ext == "gguf" {
                                                    found.push(entry.file_name().to_string_lossy().to_string());
                                                }
                                            }
                                        }
                                    }
                                    if found.is_empty() {
                                        app.messages.push("[System]: No .gguf models found in ./models/ directory.".into());
                                    } else {
                                        app.messages.push(format!("[System]: Available models:\n - {}", found.join("\n - ")));
                                    }
                                    app.input.reset();
                                } else if msg.trim() == "/copy" {
                                    let last_ai_msg = app.messages.iter().rev().find(|m| m.starts_with("[Thinker]:")).cloned();
                                    match last_ai_msg {
                                        Some(msg_text) => {
                                            let text = msg_text.replacen("[Thinker]: ", "", 1).trim().to_string();
                                            if let Some(clipboard) = &mut app.clipboard {
                                                if clipboard.set_text(text).is_ok() {
                                                    app.messages.push("[System]: 📋 Copied last AI response to clipboard!".into());
                                                } else {
                                                    app.messages.push("[System]: ❌ Failed to write to clipboard.".into());
                                                }
                                            } else {
                                                app.messages.push("[System]: ❌ Clipboard system unavailable on this machine.".into());
                                            }
                                        }
                                        None => {
                                            app.messages.push("[System]: ❌ Nothing to copy.".into());
                                        }
                                    }
                                    app.input.reset();
                                } else if msg.starts_with("/model ") || msg.starts_with("/models ") {
                                    let parts: Vec<&str> = msg.split_whitespace().collect();
                                    if parts.len() > 1 {
                                        let new_model = parts[1].to_string();
                                        app.config.default_model = new_model.clone();
                                        
                                        // Save to config file
                                        let config_path = crate::tui::config::OmniConfig::config_path();
                                        if let Ok(content) = std::fs::read_to_string(&config_path) {
                                            let mut new_content = String::new();
                                            let mut replaced = false;
                                            for line in content.lines() {
                                                if line.trim().starts_with("default_model") {
                                                    new_content.push_str(&format!("default_model = \"{}\"\n", new_model));
                                                    replaced = true;
                                                } else {
                                                    new_content.push_str(line);
                                                    new_content.push('\n');
                                                }
                                            }
                                            if !replaced {
                                                new_content.push_str(&format!("\ndefault_model = \"{}\"\n", new_model));
                                            }
                                            let _ = std::fs::write(config_path, new_content);
                                        }
                                                                                app.messages.push(format!("[System]: Active model updated to '{}' in config.", new_model));
                                        let _ = tx_cmd.send(format!("/LOAD_MODEL {}", new_model));
                                    }
                                } else if msg.starts_with("/download ") {
                                    let url = msg.trim_start_matches("/download ").trim().to_string();
                                    app.messages.push(format!("[System]: Starting download from: {}", url));
                                    app.downloader.start_download(url, String::new());
                                    app.input.reset();

                                } else if msg.trim() == "/clear" || msg.trim() == "/reset" {
                                    app.attached_files.clear();
                                    app.messages.clear();
                                    app.messages.push("[System]: Chat history cleared.".into());
                                    app.input.reset();
                                } else if msg.trim() == "/exit" {
                                    break;
} else {
                                    app.messages.push("[Thinker]: ".to_string());
                                    let _ = tx_cmd.send(msg);
                                    app.is_generating = true;
                                    app.input.reset();
                                }
                            }
                        }
                    }
                    _ => {
                        if matches!(app.current_view, CurrentView::Chat) && !app.is_generating {
                            app.input.handle_event(&Event::Key(key));
                        }
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick_count = app.tick_count.wrapping_add(1);
            if app.tick_count % 16 == 0 {
                app.sys_monitor.update();
            }

            last_tick = Instant::now();
            
            // Sync downloads
            let tasks = app.downloader.get_tasks();
            app.active_downloads.clear();
            for t in tasks {
                if !t.is_completed && t.error.is_none() {
                    app.active_downloads.push(t);
                } else if t.is_completed && t.percent == 100.0 {
                    // Quick hack to prevent spam: remove it from tasks in Downloader?
                    // Downloader keeps tasks forever currently. Let's just track if we notified.
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}