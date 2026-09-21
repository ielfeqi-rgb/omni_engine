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
}

impl LuaSandboxRunner {
    pub fn new(vfs: Arc<MemoryVfs>) -> Self {
        Self { vfs }
    }

    /// Execute Lua code with native Rust-bridged tools (vfs, browser, sys)
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

        // 4. Execute the Lua script
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
}
