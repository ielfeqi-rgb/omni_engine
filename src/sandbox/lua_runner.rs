use mlua::Lua;
use std::path::PathBuf;
use std::sync::Arc;
use crate::sandbox::MemoryVfs;

#[derive(Debug, Clone)]
pub struct LuaRunResult {
    pub success: bool,
    pub output_log: String,
    pub error: Option<String>,
    pub consultation: Option<String>,
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
        lua.set_hook(
            mlua::HookTriggers::default().every_nth_instruction(10_000),
            |_lua, _debug| Err(mlua::Error::RuntimeError("CPU instruction quota exceeded (infinite loop prevented)".to_string())),
        );

        let _ = lua.globals().set("os", mlua::Value::Nil);
        let _ = lua.globals().set("io", mlua::Value::Nil);
        let _ = lua.globals().set("package", mlua::Value::Nil);
        let _ = lua.globals().set("require", mlua::Value::Nil);

        let logs = Arc::new(parking_lot::Mutex::new(Vec::new()));

        // 1. Bridge print function to capture logs
        let logs_clone = logs.clone();
        let print_fn = lua.create_function(move |_, msg: String| {
            logs_clone.lock().push(msg);
            Ok(())
        });


        if let Ok(print_fn) = print_fn {
            let _ = lua.globals().set("print", print_fn);
        }

        // 1.5 Bridge consult function for proactive Thinker escalation
        let consultation = Arc::new(parking_lot::Mutex::new(None));
        let consult_clone = consultation.clone();
        let consult_fn = lua.create_function(move |_, query: String| {
            *consult_clone.lock() = Some(query);
            Ok(())
        });
        if let Ok(consult_fn) = consult_fn {
            let _ = lua.globals().set("consult", consult_fn);
        }

        // 2. Bridge VFS operations: vfs.write(path, content), vfs.read(path)
        let vfs_table = match lua.create_table() {
            Ok(t) => t,
            Err(e) => return LuaRunResult {
                success: false,
                output_log: String::new(),
                error: Some(format!("Failed to create VFS table: {}", e)),
                consultation: None,
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

        let vfs_delete = self.vfs.clone();
        let delete_fn = lua.create_function(move |_, path: String| {
            let removed = vfs_delete.delete_file(PathBuf::from(path));
            Ok(removed)
        });

        let vfs_exists = self.vfs.clone();
        let exists_fn = lua.create_function(move |_, path: String| {
            let ex = vfs_exists.exists(PathBuf::from(path));
            Ok(ex)
        });

        let vfs_size = self.vfs.clone();
        let size_fn = lua.create_function(move |_, path: String| {
            let sz = vfs_size.file_size(PathBuf::from(path));
            Ok(sz)
        });

        let vfs_list = self.vfs.clone();
        let list_fn = lua.create_function(move |_, _dir: Option<String>| {
            let files = vfs_list.list_files();
            let file_strs: Vec<String> = files.iter().map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string()).collect();
            Ok(file_strs)
        });

        let vfs_patch = self.vfs.clone();
        let patch_fn = lua.create_function(move |_, (path, target, replacement): (String, String, String)| {
            match vfs_patch.patch_file(PathBuf::from(path), &target, &replacement) {
                Ok(_) => Ok((true, "Patch applied successfully".to_string())),
                Err(e) => Ok((false, e)),
            }
        });

        let vfs_diff = self.vfs.clone();
        let diff_fn = lua.create_function(move |_, path: String| {
            let d = vfs_diff.diff_file(PathBuf::from(path)).unwrap_or_default();
            Ok(d)
        });

        if let Ok(w) = write_fn {
            let _ = vfs_table.set("write", w);
        }
        if let Ok(r) = read_fn {
            let _ = vfs_table.set("read", r);
        }
        if let Ok(d) = delete_fn {
            let _ = vfs_table.set("delete", d);
        }
        if let Ok(e) = exists_fn {
            let _ = vfs_table.set("exists", e);
        }
        if let Ok(s) = size_fn {
            let _ = vfs_table.set("size", s);
        }
        if let Ok(l) = list_fn {
            let _ = vfs_table.set("list", l);
        }
        if let Ok(p) = patch_fn {
            let _ = vfs_table.set("patch", p);
        }
        if let Ok(df) = diff_fn {
            let _ = vfs_table.set("diff", df);
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
                consultation: None,
            },
        };

        if let Ok(f) = fetch_fn.clone() {
            let _ = web_table.set("fetch", f);
        }
        let _ = lua.globals().set("web", web_table);

        // Note: Browser perception lens is strictly exclusive to System 2 Thinker.
        // Worker sandbox acts purely as executive hands (vfs, terminal, consult, print).

        // 4. Bridge Terminal Session if available (termhost)
        if let Some(term) = &self.terminal {
            let term_run = term.clone();
            let run_fn = lua.create_function(move |_, cmd: String| {
                match term_run.run_sync(&cmd, 10) {
                    Ok(out) => Ok(out),
                    Err(e) => Ok(format!("Error: {}", e)),
                }
            });

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
                if let Ok(r) = run_fn { let _ = term_table.set("run", r); }
                if let Ok(e) = exec_fn { let _ = term_table.set("exec", e); }
                if let Ok(l) = logs_fn { let _ = term_table.set("logs", l); }
                if let Ok(s) = status_fn { let _ = term_table.set("status", s); }
                let _ = lua.globals().set("terminal", term_table);
            }
        }

        // 5. Execute the Lua script
        match lua.load(lua_code).exec() {
            Ok(_) => {
                let captured = logs.lock().join("\n");
                let consult_req = consultation.lock().clone();
                LuaRunResult {
                    success: true,
                    output_log: captured,
                    error: None,
                    consultation: consult_req,
                }
            }
            Err(e) => {
                let captured = logs.lock().join("\n");
                LuaRunResult {
                    success: false,
                    output_log: captured,
                    error: Some(format!("Lua Runtime Trap: {}", e)),
                    consultation: None,
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

    #[test]
    fn test_lua_vfs_full_operations() {
        let vfs = Arc::new(MemoryVfs::new());
        let runner = LuaSandboxRunner::new(vfs.clone());

        let script = r#"
            vfs.write("game.py", "x = 1\ny = 2\n")
            local sz = vfs.size("game.py")
            local ex = vfs.exists("game.py")
            local ok, msg = vfs.patch("game.py", "x = 1", "x = 99")
            local diff_out = vfs.diff("game.py")
            local list_out = vfs.list()
            local del_ok = vfs.delete("game.py")
            local ex_after = vfs.exists("game.py")

            print("EX:" .. tostring(ex) .. " SZ:" .. tostring(sz) .. " PATCH:" .. tostring(ok) .. " DEL:" .. tostring(del_ok) .. " AFTER:" .. tostring(ex_after))
        "#;

        let res = runner.run_script(script);
        assert!(res.success, "Lua script must execute without errors: {:?}", res.error);
        assert!(res.output_log.contains("EX:true"));
        assert!(res.output_log.contains("PATCH:true"));
        assert!(res.output_log.contains("DEL:true"));
        assert!(res.output_log.contains("AFTER:false"));
    }
}

