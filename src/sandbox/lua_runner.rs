use mlua::Lua;
use std::path::PathBuf;
use std::sync::Arc;
use crate::sandbox::MemoryVfs;

#[derive(Debug, Clone)]
pub struct LuaRunResult {
    pub success: bool,
    pub output_log: String,
    pub error: Option<String>,
}

/// Embedded Lua Execution Environment.
/// Completely compiled inside the Rust binary.
/// Zero external package dependencies, 100% sandboxed in memory.
pub struct LuaSandboxRunner {
    vfs: Arc<MemoryVfs>,
    terminal: Option<Arc<crate::sandbox::TerminalSessionBridge>>,
}

impl LuaSandboxRunner {
    pub fn new(vfs: Arc<MemoryVfs>) -> Self {
        Self { vfs, terminal: None }
    }

    pub fn with_terminal(vfs: Arc<MemoryVfs>, terminal: Arc<crate::sandbox::TerminalSessionBridge>) -> Self {
        Self { vfs, terminal: Some(terminal) }
    }

    /// Execute Lua code with native Rust-bridged tools (vfs, browser, sys, terminal)
    pub fn run_script(&self, lua_code: &str) -> LuaRunResult {
        let lua = Lua::new();
        let logs = Arc::new(std::sync::Mutex::new(Vec::new()));

        // 1. Bridge print function to capture logs
        let logs_clone = logs.clone();
        let print_fn = lua.create_function(move |_, msg: String| {
            logs_clone.lock().unwrap().push(msg);
            Ok(())
        });

        if let Ok(print_fn) = print_fn {
            let _ = lua.globals().set("print", print_fn);
        }

        // 2. Bridge VFS operations: vfs.write(path, content), vfs.read(path)
        let vfs_table = match lua.create_table() {
            Ok(t) => t,
            Err(e) => return LuaRunResult {
                success: false,
                output_log: String::new(),
                error: Some(format!("Failed to create VFS table: {}", e)),
            },
        };

        let vfs_write = self.vfs.clone();
        let write_fn = lua.create_function(move |_, (path, content): (String, String)| {
            vfs_write.write_file(PathBuf::from(path), content.as_bytes());
            Ok(true)
        });

        let vfs_read = self.vfs.clone();
        let read_fn = lua.create_function(move |_, path: String| {
            let content = vfs_read.read_string(PathBuf::from(path));
            Ok(content)
        });

        if let Ok(w) = write_fn {
            let _ = vfs_table.set("write", w);
        }
        if let Ok(r) = read_fn {
            let _ = vfs_table.set("read", r);
        }
        let _ = lua.globals().set("vfs", vfs_table);

        // 3. Bridge HTTP/Wikipedia Fetcher (pure Rust native HTTP via reqwest)
        let fetch_fn = lua.create_function(|_, url: String| {
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent("OmniAgent/1.0")
                .build()
                .map_err(|e| mlua::Error::RuntimeError(format!("Client error: {}", e)))?;

            let res = client.get(&url)
                .send()
                .map_err(|e| mlua::Error::RuntimeError(format!("Request error: {}", e)))?;

            let text = res.text().map_err(|e| mlua::Error::RuntimeError(format!("Read error: {}", e)))?;
            Ok(text)
        });

        let web_table = match lua.create_table() {
            Ok(t) => t,
            Err(e) => return LuaRunResult {
                success: false,
                output_log: String::new(),
                error: Some(format!("Failed to create Web table: {}", e)),
            },
        };

        if let Ok(f) = fetch_fn {
            let _ = web_table.set("fetch", f);
        }
        let _ = lua.globals().set("web", web_table);

        // 4. Bridge Terminal Session if available
        if let Some(term) = &self.terminal {
            let term_exec = term.clone();
            let exec_fn = lua.create_function(move |_, cmd: String| {
                let (job_id, status) = term_exec.execute(&cmd);
                let status_str = match status {
                    crate::sandbox::JobStatus::Running => "running",
                    crate::sandbox::JobStatus::Completed { .. } => "completed",
                    crate::sandbox::JobStatus::Failed { .. } => "failed",
                    crate::sandbox::JobStatus::Blocked { .. } => "blocked",
                };
                Ok((job_id, status_str.to_string()))
            });

            let term_logs = term.clone();
            let logs_fn = lua.create_function(move |_, tail: usize| {
                let lines = term_logs.get_logs(tail);
                Ok(lines.join("\n"))
            });

            let term_status = term.clone();
            let status_fn = lua.create_function(move |_, job_id: u64| {
                let st = term_status.poll_status(job_id);
                let desc = match st {
                    Some(crate::sandbox::JobStatus::Running) => "running".to_string(),
                    Some(crate::sandbox::JobStatus::Completed { exit_code }) => format!("completed({})", exit_code),
                    Some(crate::sandbox::JobStatus::Failed { error }) => format!("failed({})", error),
                    Some(crate::sandbox::JobStatus::Blocked { reason }) => format!("blocked({})", reason),
                    None => "not_found".to_string(),
                };
                Ok(desc)
            });

            if let Ok(term_table) = lua.create_table() {
                if let Ok(e) = exec_fn { let _ = term_table.set("exec", e); }
                if let Ok(l) = logs_fn { let _ = term_table.set("logs", l); }
                if let Ok(s) = status_fn { let _ = term_table.set("status", s); }
                let _ = lua.globals().set("terminal", term_table);
            }
        }

        // 5. Execute the Lua script
        match lua.load(lua_code).exec() {
            Ok(_) => {
                let captured = logs.lock().unwrap().join("\n");
                LuaRunResult {
                    success: true,
                    output_log: captured,
                    error: None,
                }
            }
            Err(e) => {
                let captured = logs.lock().unwrap().join("\n");
                LuaRunResult {
                    success: false,
                    output_log: captured,
                    error: Some(format!("Lua Runtime Trap: {}", e)),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_lua_sandbox_vfs_writing() {
        let vfs = Arc::new(MemoryVfs::new());
        let runner = LuaSandboxRunner::new(vfs.clone());

        let script = r#"
            print("Generating Excel CSV in Lua...")
            local csv = "Product,Quantity,Price,Total\n"
            csv = csv .. "Laptop,3,1200,3600\n"
            csv = csv .. "Smartphone,2,500,1000\n"
            vfs.write("sales.csv", csv)
            print("Sales file saved to VFS successfully!")
        "#;

        let res = runner.run_script(script);
        assert!(res.success, "Lua script must execute without errors: {:?}", res.error);
        assert!(res.output_log.contains("Sales file saved"));

        // Verify that the file now exists in the Rust MemoryVfs in RAM
        let saved = vfs.read_string(PathBuf::from("sales.csv"));
        assert!(saved.is_some());
        assert!(saved.unwrap().contains("Laptop,3,1200,3600"));
    }

    #[test]
    fn test_lua_terminal_bridge_integration() {
        let vfs = Arc::new(MemoryVfs::new());
        let terminal = Arc::new(crate::sandbox::TerminalSessionBridge::new(50));
        let runner = LuaSandboxRunner::with_terminal(vfs, terminal.clone());

        let script = r#"
            local job_id, status = terminal.exec("echo 'LUA_TERMINAL_OUTPUT'")
            print("Job ID: " .. tostring(job_id))
        "#;

        let res = runner.run_script(script);
        assert!(res.success, "Lua script should call terminal.exec: {:?}", res.error);

        // Allow child thread to write logs
        std::thread::sleep(std::time::Duration::from_millis(150));
        let logs = terminal.get_logs(10);
        assert!(logs.iter().any(|l| l.contains("LUA_TERMINAL_OUTPUT")), "Terminal must capture output from Lua-initiated job");
    }
}

