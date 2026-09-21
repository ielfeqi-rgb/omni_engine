use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub model_name: String,
    pub url: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub is_completed: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ModelDownloader {
    models_dir: PathBuf,
    tasks: Arc<Mutex<HashMap<String, DownloadTask>>>,
}

impl ModelDownloader {
    pub fn new(models_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&models_dir);
        Self {
            models_dir,
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_tasks(&self) -> Vec<DownloadTask> {
        let guard = self.tasks.lock().unwrap();
        guard.values().cloned().collect()
    }

    pub fn start_download(&self, url: String, mut filename: String) -> String {
        if filename.trim().is_empty() {
            filename = url
                .split('/')
                .last()
                .unwrap_or("model.gguf")
                .split('?')
                .next()
                .unwrap_or("model.gguf")
                .to_string();
        }

        if !filename.ends_with(".gguf") {
            filename.push_str(".gguf");
        }

        let task_id = uuid::Uuid::new_v4().to_string();
        let initial_task = DownloadTask {
            id: task_id.clone(),
            model_name: filename.clone(),
            url: url.clone(),
            downloaded_bytes: 0,
            total_bytes: 0,
            percent: 0.0,
            is_completed: false,
            error: None,
        };

        self.tasks.lock().unwrap().insert(task_id.clone(), initial_task);

        let tasks_arc = Arc::clone(&self.tasks);
        let target_path = self.models_dir.join(&filename);
        let id_clone = task_id.clone();

        tokio::spawn(async move {
            info!("Starting GGUF download from: {}", url);
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (X11; Linux x86_64)")
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());

            match client.get(&url).send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        let err_msg = format!("HTTP error: {}", response.status());
                        error!("{}", err_msg);
                        let mut guard = tasks_arc.lock().unwrap();
                        if let Some(t) = guard.get_mut(&id_clone) {
                            t.error = Some(err_msg);
                        }
                        return;
                    }

                    let total_bytes = response.content_length().unwrap_or(0);
                    {
                        let mut guard = tasks_arc.lock().unwrap();
                        if let Some(t) = guard.get_mut(&id_clone) {
                            t.total_bytes = total_bytes;
                        }
                    }

                    match tokio::fs::File::create(&target_path).await {
                        Ok(mut file) => {
                            let mut stream = response.bytes_stream();
                            let mut downloaded: u64 = 0;

                            while let Some(chunk_res) = stream.next().await {
                                match chunk_res {
                                    Ok(chunk) => {
                                        if let Err(e) = file.write_all(&chunk).await {
                                            let err = format!("Write error: {}", e);
                                            let mut guard = tasks_arc.lock().unwrap();
                                            if let Some(t) = guard.get_mut(&id_clone) {
                                                t.error = Some(err);
                                            }
                                            return;
                                        }
                                        downloaded += chunk.len() as u64;

                                        let percent = if total_bytes > 0 {
                                            (downloaded as f32 / total_bytes as f32) * 100.0
                                        } else {
                                            0.0
                                        };

                                        let mut guard = tasks_arc.lock().unwrap();
                                        if let Some(t) = guard.get_mut(&id_clone) {
                                            t.downloaded_bytes = downloaded;
                                            t.percent = percent;
                                        }
                                    }
                                    Err(e) => {
                                        let err = format!("Stream error: {}", e);
                                        let mut guard = tasks_arc.lock().unwrap();
                                        if let Some(t) = guard.get_mut(&id_clone) {
                                            t.error = Some(err);
                                        }
                                        return;
                                    }
                                }
                            }

                            let _ = file.flush().await;
                            info!("Completed GGUF download to {:?}", target_path);
                            let mut guard = tasks_arc.lock().unwrap();
                            if let Some(t) = guard.get_mut(&id_clone) {
                                t.percent = 100.0;
                                t.is_completed = true;
                            }
                        }
                        Err(e) => {
                            let err = format!("Failed to create file: {}", e);
                            let mut guard = tasks_arc.lock().unwrap();
                            if let Some(t) = guard.get_mut(&id_clone) {
                                t.error = Some(err);
                            }
                        }
                    }
                }
                Err(e) => {
                    let err = format!("Network request failed: {}", e);
                    let mut guard = tasks_arc.lock().unwrap();
                    if let Some(t) = guard.get_mut(&id_clone) {
                        t.error = Some(err);
                    }
                }
            }
        });

        task_id
    }
}
