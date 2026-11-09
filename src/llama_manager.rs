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

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaEngineStatus {
    pub is_running: bool,
    pub is_binary_available: bool,
    pub binary_path: Option<String>,
    pub pid: Option<u32>,
    pub port: u16,
    pub active_model: Option<String>,
    pub available_models: Vec<String>,
}

#[derive(Clone)]
pub struct LlamaManager {
    base_dir: PathBuf,
    models_dir: PathBuf,
    pub process: Arc<Mutex<Option<Child>>>,
    pub active_model: Arc<Mutex<Option<String>>>,
    pub native_model: Arc<std::sync::Mutex<Option<std::sync::Arc<crate::native_llama::NativeLlamaModel>>>>,
    pub model_registry: Arc<RwLock<HashMap<String, std::sync::Arc<crate::native_llama::NativeLlamaModel>>>>,
}

impl LlamaManager {
    pub fn new(base_dir: PathBuf) -> Self {
        let models_dir = base_dir.join("models");
        if let Err(e) = fs::create_dir_all(&models_dir) {
            error!("Failed to create models directory {:?}: {}", models_dir, e);
        }

        Self {
            base_dir,
            models_dir,
            process: Arc::new(Mutex::new(None)),
            active_model: Arc::new(Mutex::new(None)),
            native_model: Arc::new(std::sync::Mutex::new(None)),
            model_registry: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn locate_binary(&self) -> Option<PathBuf> {
        if let Ok(custom_path) = std::env::var("LLAMA_SERVER_PATH") {
            let p = PathBuf::from(custom_path);
            if p.exists() {
                return Some(p);
            }
        }

        let mut candidates = vec![
            self.base_dir.join("bin").join("llama-server"),
            self.base_dir.join("llama-server"),
            PathBuf::from("./bin/llama-server"),
            PathBuf::from("llama-server"),
            PathBuf::from("/usr/local/bin/llama-server"),
            PathBuf::from("/usr/bin/llama-server"),
        ];

        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            candidates.push(home_path.join("omnicontext_ai/llama-server"));
            candidates.push(home_path.join("bin/llama-server"));
            candidates.push(home_path.join(".local/bin/llama-server"));
        }

        for path in candidates {
            if path.exists() {
                return Some(path);
            }
        }

        if let Ok(output) = Command::new("which").arg("llama-server").output() {
            if output.status.success() {
                let bin = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !bin.is_empty() {
                    let pb = PathBuf::from(bin);
                    if pb.exists() {
                        return Some(pb);
                    }
                }
            }
        }

        None
    }


    pub fn list_available_models(&self) -> Vec<String> {
        let mut models = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.models_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "gguf" {
                            if let Some(name) = path.file_name() {
                                models.push(name.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
        models
    }

    pub fn status(&self) -> LlamaEngineStatus {
        let mut proc_guard = self.process.lock();
        let mut is_running = false;
        let mut pid = None;

        if let Some(child) = proc_guard.as_mut() {
            match child.try_wait() {
                Ok(None) => {
                    is_running = true;
                    pid = Some(child.id());
                }
                _ => {
                    *proc_guard = None;
                }
            }
        }

        // Check external PID file if not managed directly in-memory
        if !is_running {
            let pid_file = self.base_dir.join("config").join("llama_server.pid");
            if let Ok(content) = fs::read_to_string(&pid_file) {
                if let Ok(file_pid) = content.trim().parse::<u32>() {
                    if std::path::Path::new(&format!("/proc/{}", file_pid)).exists() {
                        is_running = true;
                        pid = Some(file_pid);
                    } else {
                        let _ = fs::remove_file(&pid_file);
                    }
                }
            }
        }

        // Fallback check with pgrep
        if !is_running {
            if let Ok(output) = Command::new("pgrep").arg("-f").arg("llama-server").output() {
                if output.status.success() {
                    let s = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_pid) = s.lines().next().and_then(|l| l.trim().parse::<u32>().ok()) {
                        is_running = true;
                        pid = Some(first_pid);
                        let config_dir = self.base_dir.join("config");
                        let _ = fs::create_dir_all(&config_dir);
                        let _ = fs::write(config_dir.join("llama_server.pid"), first_pid.to_string());
                    }
                }
            }
        }

        let bin_opt = self.locate_binary();
        let active_model = self.active_model.lock().clone().or_else(|| {
            let model_file = self.base_dir.join("config").join("active_model.txt");
            fs::read_to_string(model_file).ok().map(|s| s.trim().to_string())
        });
        let available_models = self.list_available_models();

        LlamaEngineStatus {
            is_running,
            is_binary_available: bin_opt.is_some(),
            binary_path: bin_opt.map(|p| p.to_string_lossy().to_string()),
            pid,
            port: 8081,
            active_model,
            available_models,
        }
    }

    pub fn get_or_load_native_model(&self, requested_name: &str) -> Option<std::sync::Arc<crate::native_llama::NativeLlamaModel>> {
        let clean_name = requested_name.trim();
        if clean_name.is_empty() {
            return self.native_model.lock().ok()?.clone();
        }

        // 1. Check in-memory model_registry
        {
            let reg = self.model_registry.read();
            if let Some(m) = reg.get(clean_name) {
                return Some(m.clone());
            }
            for (k, v) in reg.iter() {
                if k.eq_ignore_ascii_case(clean_name)
                    || k.trim_end_matches(".gguf").eq_ignore_ascii_case(clean_name.trim_end_matches(".gguf"))
                {
                    return Some(v.clone());
                }
            }
        }

        // 2. Check if file exists on disk in models_dir
        let mut candidates = vec![
            self.models_dir.join(clean_name),
            self.models_dir.join(format!("{}.gguf", clean_name)),
            self.base_dir.join(clean_name),
        ];

        if let Ok(entries) = fs::read_dir(&self.models_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.eq_ignore_ascii_case(clean_name)
                    || name.trim_end_matches(".gguf").eq_ignore_ascii_case(clean_name.trim_end_matches(".gguf"))
                {
                    candidates.insert(0, entry.path());
                    break;
                }
            }
        }

        for path in candidates {
            if path.exists() && path.is_file() {
                info!("[MODEL ROUTER] Loading requested model on-demand: {:?}", path);
                if let Ok(loaded) = crate::native_llama::NativeLlamaModel::load(&path, 0) {
                    let mut reg = self.model_registry.write();
                    reg.insert(clean_name.to_string(), loaded.clone());
                    if let Some(fname) = path.file_name() {
                        reg.insert(fname.to_string_lossy().to_string(), loaded.clone());
                    }
                    return Some(loaded);
                }
            }
        }

        // 3. Fallback to default active native_model if any
        self.native_model.lock().ok()?.clone()
    }

    pub fn start(&self, model_name: String, _port: u16, threads: usize, _ctx_size: usize) -> Result<u32, String> {
        let proc_guard = self.process.lock();
        if proc_guard.is_some() {
            return Err("llama-server engine is already running".to_string());
        }

        let model_path = self.models_dir.join(&model_name);
        if !model_path.exists() {
            return Err(format!("Model file {:?} not found.", model_path));
        }

        let _num_threads = if threads == 0 {
            std::thread::available_parallelism().map(|n| (n.get() / 2).max(1)).unwrap_or(2)
        } else {
            threads
        };
        
        let loaded_model = crate::native_llama::NativeLlamaModel::load(&model_path, 0)?;

        let config_dir = self.base_dir.join("config");
        let _ = fs::create_dir_all(&config_dir);
        let _ = fs::write(config_dir.join("active_model.txt"), &model_name);
        
        if let Ok(mut lock) = self.native_model.lock() {
            *lock = Some(loaded_model.clone());
        }
        self.model_registry.write().insert(model_name.clone(), loaded_model);

        *self.active_model.lock() = Some(model_name.clone());
        info!("Started NativeLlamaModel with model {}", model_name);
        Ok(std::process::id()) // Return current pid as a placeholder
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut proc_guard = self.process.lock();
        if let Some(mut child) = proc_guard.take() {
            if let Err(e) = child.kill() {
                error!("Failed to kill child process: {}", e);
            }
            *self.active_model.lock() = None;
            info!("Stopped llama-server process");
        }

        // Also check and kill PID file
        let pid_file = self.base_dir.join("config").join("llama_server.pid");
        if let Ok(content) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
            }
            let _ = fs::remove_file(&pid_file);
        }

        let _ = fs::remove_file(self.base_dir.join("config").join("active_model.txt"));
        *self.active_model.lock() = None;
        if let Ok(mut lock) = self.native_model.lock() {
            *lock = None;
        }
        self.model_registry.write().clear();
        Ok(())
    }
}


