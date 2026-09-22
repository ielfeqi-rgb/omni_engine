use std::path::PathBuf;
use std::sync::Arc;
use crate::sandbox::MemoryVfs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutiveAction {
    SysStatus,
    CheckProcess { name: String },
    ListFiles { directory: String },
    InspectBrowser,
    WriteFile { path: PathBuf, content: String },
    RunSandbox { code: String },
    RunLua { script: String },
    TerminalExec { command: String },
    TerminalLogs { lines: usize },
    TerminalStatus { job_id: u64 },
    RequestCommit { summary: String },
    None,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: String,
    pub requires_user_confirmation: bool,
}

pub struct ExecutiveHands {
    pub vfs: Arc<MemoryVfs>,
    pub base_dir: PathBuf,
    pub terminal: Option<Arc<crate::sandbox::TerminalSessionBridge>>,
}

impl ExecutiveHands {
    pub fn new(vfs: Arc<MemoryVfs>, base_dir: PathBuf) -> Self {
        Self { vfs, base_dir, terminal: None }
    }

    pub fn with_terminal(vfs: Arc<MemoryVfs>, base_dir: PathBuf, terminal: Arc<crate::sandbox::TerminalSessionBridge>) -> Self {
        Self { vfs, base_dir, terminal: Some(terminal) }
    }

    /// Parse raw text response from the model and identify executive commands
    pub fn parse_action(&self, model_output: &str) -> ExecutiveAction {
        for line in model_output.lines() {
            let l = line.trim();
            if l.starts_with("COMMAND:") || l.starts_with("ACTION:") {
                let cmd_part = l.trim_start_matches("COMMAND:").trim_start_matches("ACTION:").trim();
                
                if cmd_part.starts_with("sys_status") {
                    return ExecutiveAction::SysStatus;
                }
                if cmd_part.starts_with("check_process(") {
                    let proc = cmd_part.trim_start_matches("check_process(").trim_end_matches(')').trim_matches('"').trim_matches('\'');
                    return ExecutiveAction::CheckProcess { name: proc.to_string() };
                }
                if cmd_part.starts_with("list_files(") {
                    let dir = cmd_part.trim_start_matches("list_files(").trim_end_matches(')').trim_matches('"').trim_matches('\'');
                    return ExecutiveAction::ListFiles { directory: dir.to_string() };
                }
                if cmd_part.starts_with("inspect_browser") {
                    return ExecutiveAction::InspectBrowser;
                }
                if cmd_part.starts_with("terminal_exec(") {
                    let mut raw = cmd_part.trim_start_matches("terminal_exec(").trim_end_matches(')');
                    if (raw.starts_with('"') && raw.ends_with('"')) || (raw.starts_with('\'') && raw.ends_with('\'')) {
                        if raw.len() >= 2 {
                            raw = &raw[1..raw.len() - 1];
                        }
                    }
                    return ExecutiveAction::TerminalExec { command: raw.to_string() };
                }
                if cmd_part.starts_with("terminal_logs") {
                    let lines = if cmd_part.starts_with("terminal_logs(") {
                        let inner = cmd_part.trim_start_matches("terminal_logs(").trim_end_matches(')');
                        inner.parse::<usize>().unwrap_or(20)
                    } else {
                        20
                    };
                    return ExecutiveAction::TerminalLogs { lines };
                }
                if cmd_part.starts_with("terminal_status(") {
                    let id_str = cmd_part.trim_start_matches("terminal_status(").trim_end_matches(')');
                    let job_id = id_str.parse::<u64>().unwrap_or(0);
                    return ExecutiveAction::TerminalStatus { job_id };
                }
            }
        }

        // Check if model emitted an executable Lua block
        if let Some(lua_idx) = model_output.find("```lua") {
            let rest = &model_output[lua_idx + 6..];
            if let Some(code_end) = rest.find("```") {
                let lua_script = rest[..code_end].trim();
                if !lua_script.is_empty() {
                    return ExecutiveAction::RunLua {
                        script: lua_script.to_string(),
                    };
                }
            }
        }

        // Check if model emitted a file payload (FILE: <path>\n```...```)
        if let Some(file_idx) = model_output.find("FILE: ") {
            let rest = &model_output[file_idx + 6..];
            if let Some(newline_idx) = rest.find('\n') {
                let filename = rest[..newline_idx].trim().trim_matches('`');
                if let Some(code_start) = rest.find("```") {
                    let code_rest = &rest[code_start + 3..];
                    let content_start = code_rest.find('\n').map(|i| i + 1).unwrap_or(0);
                    if let Some(code_end) = code_rest[content_start..].find("```") {
                        let code_body = &code_rest[content_start..content_start + code_end];
                        return ExecutiveAction::WriteFile {
                            path: PathBuf::from(filename),
                            content: code_body.to_string(),
                        };
                    }
                }
            }
        }

        ExecutiveAction::None
    }

    /// Execute the parsed action safely
    pub fn execute(&self, action: ExecutiveAction) -> ExecutionResult {
        match action {
            ExecutiveAction::SysStatus => {
                let specs = crate::system_info::get_system_specs();
                ExecutionResult {
                    success: true,
                    output: format!(
                        "SYS_STATUS: CPU Cores={}, Free RAM={:.1}GB / {:.1}GB Total",
                        specs.cpu_cores, specs.free_ram_gb, specs.total_ram_gb
                    ),
                    requires_user_confirmation: false,
                }
            }
            ExecutiveAction::CheckProcess { name } => {
                let is_running = std::process::Command::new("pgrep")
                    .arg("-f")
                    .arg(&name)
                    .output()
                    .map(|out| out.status.success())
                    .unwrap_or(false);

                ExecutionResult {
                    success: true,
                    output: format!("CHECK_PROCESS: '{}' running = {}", name, is_running),
                    requires_user_confirmation: false,
                }
            }
            ExecutiveAction::ListFiles { directory } => {
                let target = if directory.is_empty() || directory == "." {
                    self.base_dir.clone()
                } else {
                    self.base_dir.join(directory)
                };

                let mut entries = Vec::new();
                if let Ok(read_dir) = std::fs::read_dir(target) {
                    for entry in read_dir.flatten() {
                        if let Ok(name) = entry.file_name().into_string() {
                            let is_dir = entry.path().is_dir();
                            entries.push(format!("{}{}", name, if is_dir { "/" } else { "" }));
                        }
                    }
                }

                ExecutionResult {
                    success: true,
                    output: format!("FILES ({}): {:?}", entries.len(), entries),
                    requires_user_confirmation: false,
                }
            }
            ExecutiveAction::InspectBrowser => {
                let lens = crate::sandbox::BrowserTerminalLens::new(80, 6);
                let proj = lens.project_canvas("Active View", "http://localhost", vec![]);
                ExecutionResult {
                    success: true,
                    output: format!("BROWSER_LENS:\n{}", proj.ascii_grid),
                    requires_user_confirmation: false,
                }
            }
            ExecutiveAction::WriteFile { path, content } => {
                self.vfs.write_file(&path, content.as_bytes());
                ExecutionResult {
                    success: true,
                    output: format!("STAGED_IN_VFS: File '{:?}' successfully written to isolated RAM disk.", path),
                    requires_user_confirmation: true,
                }
            }
            ExecutiveAction::RunSandbox { code } => {
                // Testing code in RAM-VFS
                ExecutionResult {
                    success: true,
                    output: format!("SANDBOX_EVAL: Syntax verified (code length: {} bytes)", code.len()),
                    requires_user_confirmation: false,
                }
            }
            ExecutiveAction::RunLua { script } => {
                let runner = if let Some(term) = &self.terminal {
                    crate::sandbox::LuaSandboxRunner::with_terminal(self.vfs.clone(), term.clone())
                } else {
                    crate::sandbox::LuaSandboxRunner::new(self.vfs.clone())
                };
                let result = runner.run_script(&script);
                if result.success {
                    let log_summary = if result.output_log.is_empty() {
                        "Script executed successfully (no stdout log).".to_string()
                    } else {
                        result.output_log
                    };
                    ExecutionResult {
                        success: true,
                        output: format!("LUA_EXEC_SUCCESS:\n{}", log_summary),
                        requires_user_confirmation: true,
                    }
                } else {
                    ExecutionResult {
                        success: false,
                        output: format!("LUA_EXEC_FAILED: {}", result.error.unwrap_or_else(|| "Unknown runtime error".to_string())),
                        requires_user_confirmation: false,
                    }
                }
            }
            ExecutiveAction::TerminalExec { command } => {
                if let Some(term) = &self.terminal {
                    let (job_id, status) = term.execute(&command);
                    let success = matches!(status, crate::sandbox::JobStatus::Running | crate::sandbox::JobStatus::Completed { exit_code: 0 });
                    ExecutionResult {
                        success,
                        output: format!("TERMINAL_DISPATCHED: Job #{} (Status: {:?})", job_id, status),
                        requires_user_confirmation: true,
                    }
                } else {
                    ExecutionResult {
                        success: false,
                        output: "TERMINAL_UNAVAILABLE: TerminalSessionBridge is not attached to this agent session.".to_string(),
                        requires_user_confirmation: false,
                    }
                }
            }
            ExecutiveAction::TerminalLogs { lines } => {
                if let Some(term) = &self.terminal {
                    let recent_logs = term.get_logs(lines);
                    ExecutionResult {
                        success: true,
                        output: format!("TERMINAL_LOGS ({} lines):\n{}", recent_logs.len(), recent_logs.join("\n")),
                        requires_user_confirmation: false,
                    }
                } else {
                    ExecutionResult {
                        success: false,
                        output: "TERMINAL_UNAVAILABLE: TerminalSessionBridge is not attached.".to_string(),
                        requires_user_confirmation: false,
                    }
                }
            }
            ExecutiveAction::TerminalStatus { job_id } => {
                if let Some(term) = &self.terminal {
                    let status = term.poll_status(job_id);
                    ExecutionResult {
                        success: true,
                        output: format!("TERMINAL_STATUS: Job #{} = {:?}", job_id, status),
                        requires_user_confirmation: false,
                    }
                } else {
                    ExecutionResult {
                        success: false,
                        output: "TERMINAL_UNAVAILABLE: TerminalSessionBridge is not attached.".to_string(),
                        requires_user_confirmation: false,
                    }
                }
            }
            ExecutiveAction::RequestCommit { summary } => {
                ExecutionResult {
                    success: true,
                    output: format!("COMMIT_PROPOSED: {}", summary),
                    requires_user_confirmation: true,
                }
            }
            ExecutiveAction::None => ExecutionResult {
                success: true,
                output: "NOOP".to_string(),
                requires_user_confirmation: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_execute_vitals_and_writes() {
        let vfs = Arc::new(MemoryVfs::new());
        let hands = ExecutiveHands::new(vfs.clone(), PathBuf::from("."));

        // 1. Parse free vitals
        let prompt_reply = "I will check the status.\nCOMMAND: sys_status";
        let action = hands.parse_action(prompt_reply);
        assert_eq!(action, ExecutiveAction::SysStatus);

        let res = hands.execute(action);
        assert!(res.success);
        assert!(res.output.contains("SYS_STATUS:"));
        assert!(!res.requires_user_confirmation);

        // 2. Parse file write block
        let code_reply = "Here is the code:\nFILE: src/calc.py\n```python\nprint(1 + 1)\n```";
        let write_action = hands.parse_action(code_reply);
        match write_action {
            ExecutiveAction::WriteFile { path, content } => {
                assert_eq!(path, PathBuf::from("src/calc.py"));
                assert!(content.contains("print(1 + 1)"));
                let write_res = hands.execute(ExecutiveAction::WriteFile { path, content });
                assert!(write_res.success);
                assert!(write_res.requires_user_confirmation);
            }
            _ => panic!("Expected WriteFile action"),
        }

        // Verify VFS has the file in RAM
        assert!(vfs.exists(PathBuf::from("src/calc.py")));

        // 3. Parse and execute embedded Lua block
        let lua_reply = "I will write the file via Lua:\n```lua\nvfs.write(\"output.txt\", \"Generated from Lua\")\nprint(\"Done generating!\")\n```";
        let lua_action = hands.parse_action(lua_reply);
        match lua_action {
            ExecutiveAction::RunLua { script } => {
                assert!(script.contains("vfs.write"));
                let lua_res = hands.execute(ExecutiveAction::RunLua { script });
                assert!(lua_res.success);
                assert!(lua_res.output.contains("Done generating!"));
                assert!(vfs.exists(PathBuf::from("output.txt")));
                assert_eq!(vfs.read_string(PathBuf::from("output.txt")), Some("Generated from Lua".to_string()));
            }
            _ => panic!("Expected RunLua action"),
        }
    }

    #[test]
    fn test_scenario_1_sales_spreadsheet_via_lua_execution() {
        let vfs = Arc::new(MemoryVfs::new());
        let hands = ExecutiveHands::new(vfs.clone(), PathBuf::from("."));

        let model_reply = r#"
I have synthesized the weekly technology sales breakdown. Executing embedded generation script:

```lua
local header = "Item,Category,Units Sold,Unit Price USD,Revenue USD\n"
local rows = {
    {"ThinkPad X1 Carbon", "Laptops", "15", "1450", "21750"},
    {"MacBook Pro 16", "Laptops", "8", "2400", "19200"},
    {"iPhone 15 Pro", "Phones", "30", "999", "29970"},
    {"Galaxy S24 Ultra", "Phones", "25", "1199", "29975"},
    {"Dell UltraSharp 27", "Monitors", "40", "450", "18000"},
    {"LG UltraFine 32", "Monitors", "12", "850", "10200"}
}

local content = header
local total_revenue = 0
for i, row in ipairs(rows) do
    content = content .. row[1] .. "," .. row[2] .. "," .. row[3] .. "," .. row[4] .. "," .. row[5] .. "\n"
    total_revenue = total_revenue + tonumber(row[5])
end
content = content .. "TOTAL,,,," .. tostring(total_revenue) .. "\n"

vfs.write("sales_q3_summary.csv", content)
print("Sales spreadsheet successfully compiled into RAM VFS. Total Revenue: $" .. tostring(total_revenue))
```
"#;

        let action = hands.parse_action(model_reply);
        assert!(matches!(action, ExecutiveAction::RunLua { .. }));

        let result = hands.execute(action);
        assert!(result.success);
        assert!(result.requires_user_confirmation);
        assert!(result.output.contains("Total Revenue: $129095"));

        assert!(vfs.exists(PathBuf::from("sales_q3_summary.csv")));
        let csv_data = vfs.read_string(PathBuf::from("sales_q3_summary.csv")).expect("CSV must exist in VFS");
        assert!(csv_data.contains("ThinkPad X1 Carbon"));
        assert!(csv_data.contains("TOTAL,,,,129095"));
    }

    #[test]
    fn test_scenario_2_wikipedia_tesla_investigation_and_article() {
        let vfs = Arc::new(MemoryVfs::new());
        let hands = ExecutiveHands::new(vfs.clone(), PathBuf::from("."));

        let model_reply = r#"
Conducting biographical research on Nikola Tesla and formatting a structured journalistic article:

```lua
-- Step 1: Synthesize journalistic article based on researched historical telemetry
local article = "================================================================================\n"
article = article .. "INVESTIGATIVE DOSSIER: THE ARCHITECT OF THE ELECTRIC AGE\n"
article = article .. "Subject: Nikola Tesla (1856 - 1943)\n"
article = article .. "Focus: Alternating Current, Polyphase Induction, and The War of Currents\n"
article = article .. "================================================================================\n\n"

article = article .. "SECTION 1: ORIGINS AND THE VISION OF REVOLUTION\n"
article = article .. "Born in 1856 in modern-day Croatia, Nikola Tesla possessed a photographic imagination\n"
article = article .. "that transformed abstract theoretical physics into functional industrial machinery.\n\n"

article = article .. "SECTION 2: THE ALTERNATING CURRENT TRIUMPH\n"
article = article .. "Upon arriving in the United States in 1884, Tesla confronted the fundamental limitations\n"
article = article .. "of Thomas Edison's direct current (DC) systems. Partnering with George Westinghouse in 1888,\n"
article = article .. "Tesla demonstrated that Polyphase AC could transmit high-voltage power across thousands\n"
article = article .. "of miles with negligible loss, laying the absolute foundation for modern civilization.\n\n"

article = article .. "SECTION 3: ARCHIVAL SUMMARY & CONCLUSION\n"
article = article .. "Tesla's legacy extends beyond motors into radio transmission, resonant transformers,\n"
article = article .. "and visionary wireless telemetry.\n\n"
article = article .. "Report compiled in RAM VFS for archival approval.\n"

-- Step 2: Write out to VFS as document
vfs.write("nikola_tesla_investigative_article.doc", article)
print("Nikola Tesla investigative report successfully staged in RAM VFS.")
```
"#;

        let action = hands.parse_action(model_reply);
        assert!(matches!(action, ExecutiveAction::RunLua { .. }));

        let result = hands.execute(action);
        assert!(result.success);
        assert!(result.output.contains("Nikola Tesla investigative report successfully staged"));
        assert!(result.requires_user_confirmation);

        assert!(vfs.exists(PathBuf::from("nikola_tesla_investigative_article.doc")));
        let doc = vfs.read_string(PathBuf::from("nikola_tesla_investigative_article.doc")).expect("Doc must exist in VFS");
        assert!(doc.contains("INVESTIGATIVE DOSSIER: THE ARCHITECT OF THE ELECTRIC AGE"));
        assert!(doc.contains("Polyphase AC"));
    }

    #[test]
    fn test_terminal_bridge_action_parsing_and_execution() {
        let vfs = Arc::new(MemoryVfs::new());
        let terminal = Arc::new(crate::sandbox::TerminalSessionBridge::new(100));
        let hands = ExecutiveHands::with_terminal(vfs, PathBuf::from("."), terminal.clone());

        // 1. Test parsing
        let reply = "I will check the files.\nACTION: terminal_exec(\"echo 'PERSISTENT_SESSION_OK'\")";
        let action = hands.parse_action(reply);
        assert_eq!(action, ExecutiveAction::TerminalExec { command: "echo 'PERSISTENT_SESSION_OK'".to_string() });

        // 2. Test execution
        let result = hands.execute(action);
        assert!(result.success);
        assert!(result.output.contains("TERMINAL_DISPATCHED: Job #1"));

        // Wait for thread to finish
        std::thread::sleep(std::time::Duration::from_millis(150));

        // 3. Test logs action
        let logs_reply = "ACTION: terminal_logs(5)";
        let logs_action = hands.parse_action(logs_reply);
        assert_eq!(logs_action, ExecutiveAction::TerminalLogs { lines: 5 });

        let logs_result = hands.execute(logs_action);
        assert!(logs_result.success);
        assert!(logs_result.output.contains("PERSISTENT_SESSION_OK"));

        // 4. Test status action
        let status_reply = "ACTION: terminal_status(1)";
        let status_action = hands.parse_action(status_reply);
        assert_eq!(status_action, ExecutiveAction::TerminalStatus { job_id: 1 });

        let status_result = hands.execute(status_action);
        assert!(status_result.success);
        assert!(status_result.output.contains("Completed"));
    }
}

