use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct KeyManager {
    config_path: PathBuf,
    keys: Arc<Mutex<Vec<ApiKey>>>,
}

impl KeyManager {
    pub fn new(base_dir: PathBuf) -> Self {
        let config_dir = base_dir.join("config");
        let _ = fs::create_dir_all(&config_dir);
        let config_path = config_dir.join("apikeys.json");

        let mut keys = Vec::new();
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(parsed) = serde_json::from_str::<Vec<ApiKey>>(&content) {
                    keys = parsed;
                }
            }
        }

        // If no keys exist, generate a primary key
        if keys.is_empty() {
            let primary_key = ApiKey {
                id: Uuid::new_v4().to_string(),
                name: "Default Master Key".to_string(),
                key: format!("omni_sk_{}", Uuid::new_v4().to_string().replace("-", "")),
                created_at: chrono_like_timestamp(),
            };
            info!("🗝️  Generated Master API Key: {}", primary_key.key);
            keys.push(primary_key);
            let _ = save_keys_to_disk(&config_path, &keys);
        }

        Self {
            config_path,
            keys: Arc::new(Mutex::new(keys)),
        }
    }

    pub fn validate_key(&self, token: &str) -> bool {
        let clean_token = token.trim_start_matches("Bearer ").trim();
        let guard = self.keys.lock().unwrap();
        guard.iter().any(|k| k.key == clean_token)
    }

    pub fn list_keys(&self) -> Vec<ApiKey> {
        let guard = self.keys.lock().unwrap();
        guard.clone()
    }

    pub fn create_key(&self, name: String) -> ApiKey {
        let new_key = ApiKey {
            id: Uuid::new_v4().to_string(),
            name: if name.is_empty() { "API Key".to_string() } else { name },
            key: format!("omni_sk_{}", Uuid::new_v4().to_string().replace("-", "")),
            created_at: chrono_like_timestamp(),
        };

        let mut guard = self.keys.lock().unwrap();
        guard.push(new_key.clone());
        let _ = save_keys_to_disk(&self.config_path, &guard);
        new_key
    }

    pub fn revoke_key(&self, id: &str) -> bool {
        let mut guard = self.keys.lock().unwrap();
        let initial_len = guard.len();
        guard.retain(|k| k.id != id);
        let removed = guard.len() < initial_len;
        if removed {
            let _ = save_keys_to_disk(&self.config_path, &guard);
        }
        removed
    }
}

fn save_keys_to_disk(path: &PathBuf, keys: &[ApiKey]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(keys).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

fn chrono_like_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    format!("Epoch {}", since_epoch.as_secs())
}
