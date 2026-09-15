use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use tracing::info;

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
    process: Arc<Mutex<Option<Child>>>,
    active_model: Arc<Mutex<Option<String>>>,
}

impl LlamaManager {
    pub fn new(base_dir: PathBuf) -> Self {
        let models_dir = base_dir.join("models");
        let _ = fs::create_dir_all(&models_dir);

        Self {
            base_dir,
            models_dir,
            process: Arc::new(Mutex::new(None)),
            active_model: Arc::new(Mutex::new(None)),
        }
    }

    pub fn locate_binary(&self) -> Option<PathBuf> {
        let candidates = vec![
            self.base_dir.join("bin").join("llama-server"),
            self.base_dir.join("llama-server"),
            PathBuf::from("/home/hema/Downloads/files(1)/omnicontext_v2/bin/llama-server"),
            PathBuf::from("/home/hema/Downloads/files(1)/omnicontext_complete/bin/llama-server"),
            PathBuf::from("./bin/llama-server"),
            PathBuf::from("llama-server"),
            PathBuf::from("/usr/local/bin/llama-server"),
            PathBuf::from("/usr/bin/llama-server"),
        ];

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
        let mut proc_guard = self.process.lock().unwrap();
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

        let bin_opt = self.locate_binary();
        let active_model = self.active_model.lock().unwrap().clone();
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

    pub fn start(&self, model_name: String, port: u16, threads: usize, ctx_size: usize) -> Result<u32, String> {
        let mut proc_guard = self.process.lock().unwrap();
        if proc_guard.is_some() {
            return Err("llama-server engine is already running".to_string());
        }

        let binary = self.locate_binary()
            .ok_or_else(|| "llama-server binary not found on system. Please download/compile llama.cpp binary.".to_string())?;

        let model_path = self.models_dir.join(&model_name);
        if !model_path.exists() {
            return Err(format!("Model file {:?} not found.", model_path));
        }

        let num_threads = if threads == 0 {
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
        } else {
            threads
        };

        let bin_dir = binary.parent().unwrap_or(&self.base_dir);
        let existing_ld = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
        let new_ld = format!("{}:{}:{}", bin_dir.display(), self.base_dir.display(), existing_ld);

        match Command::new(&binary)
            .env("LD_LIBRARY_PATH", new_ld)
            .arg("-m")
            .arg(&model_path)
            .arg("-c")
            .arg(ctx_size.to_string())
            .arg("-t")
            .arg(num_threads.to_string())
            .arg("--host")
            .arg("0.0.0.0")
            .arg("--port")
            .arg(port.to_string())
            .spawn()
        {
            Ok(child) => {
                let id = child.id();
                *proc_guard = Some(child);
                *self.active_model.lock().unwrap() = Some(model_name.clone());
                info!("Started llama-server PID {} with model {}", id, model_name);
                Ok(id)
            }
            Err(e) => Err(format!("Failed to spawn llama-server: {}", e)),
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut proc_guard = self.process.lock().unwrap();
        if let Some(mut child) = proc_guard.take() {
            let _ = child.kill();
            *self.active_model.lock().unwrap() = None;
            info!("Stopped llama-server process");
        }
        Ok(())
    }
}
