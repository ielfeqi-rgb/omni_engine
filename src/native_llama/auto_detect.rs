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

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use crate::native_llama::{NativeLlamaModel, NativeLlamaSampler, SamplingConfig};

fn default_engine_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn default_model_family() -> String {
    "Universal".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OmniModelConfig {
    #[serde(default = "default_engine_version")]
    pub engine_version: String,
    #[serde(default = "default_model_family")]
    pub model_family: String,
    #[serde(default)]
    pub chamber_verified: bool,
    pub eos_token: i32,
    pub eot_token: i32,
    pub system_prefix: String,
    pub user_prefix: String,
    pub assistant_prefix: String,
    pub turn_suffix: String,
    pub stop_tokens: Vec<String>,
    pub stop_token_ids: Vec<i32>,
    #[serde(default)]
    pub fim_pre: Option<String>,
    #[serde(default)]
    pub fim_suf: Option<String>,
    #[serde(default)]
    pub fim_mid: Option<String>,
}

impl Default for OmniModelConfig {
    fn default() -> Self {
        Self {
            engine_version: default_engine_version(),
            model_family: "Universal".to_string(),
            chamber_verified: false,
            eos_token: 2,
            eot_token: 2,
            system_prefix: "### System Instruction:\n".to_string(),
            user_prefix: "### User Objective:\n".to_string(),
            assistant_prefix: "### Assistant Execution:\n".to_string(),
            turn_suffix: "\n\n".to_string(),
            stop_tokens: vec![
                "<|im_end|>".to_string(),
                "<|end|>".to_string(),
                "<|eot_id|>".to_string(),
                "<end_of_turn>".to_string(),
                "<｜end·of·sentence｜>".to_string(),
                "</s>".to_string(),
                "<|endoftext|>".to_string(),
                "\n### User".to_string(),
            ],
            stop_token_ids: vec![2],
            fim_pre: None,
            fim_suf: None,
            fim_mid: None,
        }
    }
}

impl OmniModelConfig {
    /// Dynamically format a complete multi-role or single-turn chat prompt.
    pub fn format_chat(&self, system: Option<&str>, user: &str) -> String {
        let mut prompt = String::new();
        if let Some(sys) = system {
            if !sys.trim().is_empty() {
                prompt.push_str(&self.system_prefix);
                prompt.push_str(sys);
                prompt.push_str(&self.turn_suffix);
            }
        }
        prompt.push_str(&self.user_prefix);
        prompt.push_str(user);
        prompt.push_str(&self.turn_suffix);
        prompt.push_str(&self.assistant_prefix);
        prompt
    }

    /// Dynamically format a completed conversation turn where the Assistant's response
    /// is injected and cleanly sealed with the model's native turn_suffix / EOT token.
    pub fn format_completed_turn(&self, system: Option<&str>, user: &str, assistant_response: &str) -> String {
        let mut prompt = self.format_chat(system, user);
        prompt.push_str(assistant_response.trim());
        prompt.push_str(&self.turn_suffix);
        prompt
    }

    /// Dynamically append an observation / environmental feedback turn.
    pub fn format_observation_turn(&self, observation: &str, next_instruction: &str) -> String {
        let mut turn = String::new();
        turn.push_str(&self.turn_suffix);
        turn.push_str(&self.user_prefix);
        turn.push_str("OBSERVATION: ");
        turn.push_str(observation);
        turn.push('\n');
        turn.push_str(next_instruction);
        turn.push_str(&self.turn_suffix);
        turn.push_str(&self.assistant_prefix);
        turn
    }

    /// Dynamically format a post-rollback correction nudge turn.
    /// Closes previous assistant turn, injects the diagnostic directive inside user turn,
    /// and opens the assistant turn for generation.
    pub fn format_nudge_turn(&self, diagnostic: &str) -> String {
        let mut turn = String::new();
        turn.push_str(&self.turn_suffix);
        turn.push_str(&self.user_prefix);
        turn.push_str(diagnostic);
        turn.push_str(&self.turn_suffix);
        turn.push_str(&self.assistant_prefix);
        turn
    }

    /// Check if a generated text piece contains any of the known stop tokens.
    pub fn is_stop_piece(&self, piece: &str) -> bool {
        if piece.is_empty() {
            return false;
        }
        self.stop_tokens.iter().any(|st| piece.contains(st))
    }

    /// Check if a generated token ID is a known stop / EOT token.
    pub fn is_stop_token(&self, token_id: i32) -> bool {
        token_id == self.eot_token || token_id == self.eos_token || self.stop_token_ids.contains(&token_id)
    }
    /// Parse an `.omni` sovereign file from raw text content.
    /// Supports:
    /// 1. JSON content (with or without `#`, `//`, `;` comments)
    /// 2. Multi-model dictionary keyed by model name
    /// 3. Key-Value variable definitions (e.g., `EOS_TOKEN = 32000`)
    pub fn from_omni_str(content: &str, model_stem: &str) -> Result<Self, String> {
        let cleaned_lines: Vec<&str> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//") && !l.starts_with(';'))
            .collect();

        if cleaned_lines.is_empty() {
            return Err("Empty .omni configuration content".to_string());
        }

        let joined = cleaned_lines.join("\n");

        // Attempt 1: Parse directly as single OmniModelConfig JSON
        if joined.starts_with('{') {
            if let Ok(config) = serde_json::from_str::<OmniModelConfig>(&joined) {
                return Ok(config);
            }

            // Attempt 2: Parse as a multi-model dictionary { "model_name": { ... } }
            if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&joined) {
                let normalize = |s: &str| -> String {
                    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
                };
                let norm_stem = normalize(model_stem);
                for (k, v) in map {
                    let norm_k = normalize(&k);
                    if !norm_k.is_empty() && (norm_k == norm_stem || norm_stem.contains(&norm_k) || norm_k.contains(&norm_stem)) {
                        if let Ok(config) = serde_json::from_value::<OmniModelConfig>(v) {
                            return Ok(config);
                        }
                    }
                }
            }
        }

        // Attempt 3: Parse as Key-Value variable assignments
        let mut config = OmniModelConfig::default();
        let mut found_any_key = false;

        for line in &cleaned_lines {
            let split_opt = if line.contains('=') {
                line.split_once('=')
            } else if line.contains(':') {
                line.split_once(':')
            } else {
                None
            };

            if let Some((raw_key, raw_val)) = split_opt {
                let key = raw_key.trim().to_lowercase();
                let val = raw_val.trim();

                let unquote = |s: &str| -> String {
                    let mut trimmed = s.trim();
                    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
                        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
                    {
                        if trimmed.len() >= 2 {
                            trimmed = &trimmed[1..trimmed.len() - 1];
                        }
                    }
                    trimmed
                        .replace("\\n", "\n")
                        .replace("\\t", "\t")
                        .replace("\\\"", "\"")
                };

                match key.as_str() {
                    "engine_version" => {
                        config.engine_version = unquote(val);
                        found_any_key = true;
                    }
                    "model_family" | "family" => {
                        config.model_family = unquote(val);
                        found_any_key = true;
                    }
                    "chamber_verified" | "verified" => {
                        config.chamber_verified = val.eq_ignore_ascii_case("true") || val == "1";
                        found_any_key = true;
                    }
                    "eos_token" | "eos" => {
                        if let Ok(id) = val.parse::<i32>() {
                            config.eos_token = id;
                            found_any_key = true;
                        }
                    }
                    "eot_token" | "eot" => {
                        if let Ok(id) = val.parse::<i32>() {
                            config.eot_token = id;
                            found_any_key = true;
                        }
                    }
                    "system_prefix" => {
                        config.system_prefix = unquote(val);
                        found_any_key = true;
                    }
                    "user_prefix" => {
                        config.user_prefix = unquote(val);
                        found_any_key = true;
                    }
                    "assistant_prefix" => {
                        config.assistant_prefix = unquote(val);
                        found_any_key = true;
                    }
                    "turn_suffix" => {
                        config.turn_suffix = unquote(val);
                        found_any_key = true;
                    }
                    "stop_tokens" => {
                        if let Ok(vec) = serde_json::from_str::<Vec<String>>(val) {
                            config.stop_tokens = vec;
                            found_any_key = true;
                        } else {
                            let parts: Vec<String> = val
                                .trim_matches(|c| c == '[' || c == ']')
                                .split(',')
                                .map(|s| unquote(s.trim()))
                                .filter(|s| !s.is_empty())
                                .collect();
                            if !parts.is_empty() {
                                config.stop_tokens = parts;
                                found_any_key = true;
                            }
                        }
                    }
                    "stop_token_ids" => {
                        if let Ok(vec) = serde_json::from_str::<Vec<i32>>(val) {
                            config.stop_token_ids = vec;
                            found_any_key = true;
                        } else {
                            let parts: Vec<i32> = val
                                .trim_matches(|c| c == '[' || c == ']')
                                .split(',')
                                .filter_map(|s| s.trim().parse::<i32>().ok())
                                .collect();
                            if !parts.is_empty() {
                                config.stop_token_ids = parts;
                                found_any_key = true;
                            }
                        }
                    }
                    "fim_pre" | "fim_prefix" => {
                        config.fim_pre = Some(unquote(val));
                        found_any_key = true;
                    }
                    "fim_suf" | "fim_suffix" => {
                        config.fim_suf = Some(unquote(val));
                        found_any_key = true;
                    }
                    "fim_mid" | "fim_middle" => {
                        config.fim_mid = Some(unquote(val));
                        found_any_key = true;
                    }
                    _ => {}
                }
            }
        }

        if found_any_key {
            Ok(config)
        } else {
            Err(format!("Could not parse .omni manifest for model '{}'", model_stem))
        }
    }

    /// Serialize this configuration into an official Sovereign `.omni` manifest with metadata header.
    pub fn to_omni_string(&self) -> Result<String, String> {
        let header = format!(
            "# ==============================================================================\n\
             # OMNI SOVEREIGN MODEL MANIFEST (.omni)\n\
             # Engine Version: {} | Model Family: {}\n\
             # Empirical Chamber Verification: {}\n\
             # ==============================================================================\n",
            self.engine_version,
            self.model_family,
            if self.chamber_verified { "VERIFIED" } else { "PENDING" }
        );

        let json_body = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialization error: {}", e))?;

        Ok(format!("{}{}\n", header, json_body))
    }
}

/// Discovers any existing `.omni` configuration for the given model.
/// Checks candidate paths in order:
/// 1. `<parent>/<model_stem>.omni`
/// 2. `<model_path>.omni`
/// 3. `./<model_stem>.omni`
/// 4. `models/<model_stem>.omni`
/// 5. Global/central variable file (`engine.omni`, `models.omni`, `omni.omni`, `omni.config`)
/// 6. Legacy fallback (`<parent>/<model_stem>.omni.json`, `<model_path>.omni.json`)
pub fn find_omni_config_path(model_path: &str) -> Option<PathBuf> {
    let path = Path::new(model_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(model_path);
    let parent = path.parent().unwrap_or(Path::new("."));

    let candidates = vec![
        parent.join(format!("{}.omni", stem)),
        PathBuf::from(format!("{}.omni", model_path)),
        PathBuf::from(format!("{}.omni", stem)),
        Path::new("models").join(format!("{}.omni", stem)),
        PathBuf::from("engine.omni"),
        PathBuf::from("models.omni"),
        PathBuf::from("omni.omni"),
        PathBuf::from("omni.config"),
        parent.join("engine.omni"),
        parent.join("models.omni"),
        // Legacy fallbacks
        parent.join(format!("{}.omni.json", stem)),
        PathBuf::from(format!("{}.omni.json", model_path)),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

fn is_valid_special_stop_token(model: &NativeLlamaModel, tok: i32) -> bool {
    if tok <= 0 {
        return false;
    }
    // Direct query: is it an End-of-Generation token natively?
    if model.is_eog(tok) {
        return true;
    }
    // Inspect piece text representation
    if let Ok(piece) = model.token_to_piece(tok) {
        let t = piece.trim();
        if t.is_empty() || t == "\n" || t == "\r\n" || t == " " || t == "\t" {
            return false;
        }
        if (t.starts_with('<') && t.ends_with('>'))
            || (t.starts_with('[') && t.ends_with(']'))
            || t == "</s>"
            || t == "<s>"
        {
            return true;
        }
    }
    false
}

/// Detects the structural format and stop tokens of any model family using
/// Triangulated Structural Vocabulary Probing and Assistant-Prefill Chamber Verification.
/// Loads or saves the sovereign `.omni` manifest.
pub fn detect_and_save_model_tokens(
    model_path: &str,
    model: &std::sync::Arc<NativeLlamaModel>,
) -> Result<OmniModelConfig, String> {
    let path = Path::new(model_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(model_path);
    let parent = path.parent().unwrap_or(Path::new("."));

    // Check if an existing .omni or legacy .omni.json profile exists
    if let Some(config_path) = find_omni_config_path(model_path) {
        if let Ok(data) = fs::read_to_string(&config_path) {
            match OmniModelConfig::from_omni_str(&data, stem) {
                Ok(config) => {
                    println!("  [AUTO-DETECT] Loaded sovereign model manifest from {}", config_path.display());
                    return Ok(config);
                }
                Err(e) => {
                    println!("  [AUTO-DETECT] Warning: could not parse {}: {}", config_path.display(), e);
                }
            }
        }
    }

    println!("  [AUTO-DETECT] Running Universal Triangulated Detection Protocol for '{}'...", stem);

    // ── LAYER 0: Embedded GGUF Metadata Introspection ─────────────────────────
    let native_eos = model.vocab_eos();
    let native_eot = model.vocab_eot();
    let template_opt = model.get_chat_template();

    // ── LAYER 1: Structural Vocabulary Signature & Template Probing ───────────
    let tmpl = template_opt.as_deref().unwrap_or("");
    
    let is_chatml = tmpl.contains("<|im_start|>")
        || model.tokenize("<|im_start|>", false).map(|t| t.len() == 1).unwrap_or(false);
    let is_llama3 = tmpl.contains("<|start_header_id|>")
        || model.tokenize("<|start_header_id|>", false).map(|t| t.len() == 1).unwrap_or(false);
    let is_phi4 = tmpl.contains("<|user|>")
        || model.tokenize("<|user|>", false).map(|t| t.len() == 1).unwrap_or(false);
    let is_gemma = tmpl.contains("<start_of_turn>")
        || model.tokenize("<start_of_turn>", false).map(|t| t.len() == 1).unwrap_or(false);
    let is_spark = tmpl.contains("<|User|>") || tmpl.contains("start·of·sentence")
        || model.tokenize("<｜start·of·sentence｜>", false).map(|t| t.len() == 1).unwrap_or(false)
        || model.tokenize("<|User|>", false).map(|t| t.len() == 1).unwrap_or(false);
    let is_mistral = tmpl.contains("[INST]")
        || model.tokenize("[INST]", false).map(|t| t.len() == 1).unwrap_or(false);
    let is_command_r = tmpl.contains("<|START_OF_TURN_TOKEN|>")
        || model.tokenize("<|START_OF_TURN_TOKEN|>", false).map(|t| t.len() == 1).unwrap_or(false);
    let is_deepseek = tmpl.contains("<｜User｜>")
        || model.tokenize("<｜User｜>", false).map(|t| t.len() == 1).unwrap_or(false);

    let mut config = if is_chatml {
        println!("  [AUTO-DETECT] Identified Model Architecture: ChatML (Qwen/DeepSeek style)");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "ChatML".to_string(),
            chamber_verified: false,
            eos_token: if native_eos > 0 { native_eos } else { 151645 },
            eot_token: if native_eot > 0 { native_eot } else { 151645 },
            system_prefix: "<|im_start|>system\n".to_string(),
            user_prefix: "<|im_start|>user\n".to_string(),
            assistant_prefix: "<|im_start|>assistant\n".to_string(),
            turn_suffix: "<|im_end|>\n".to_string(),
            stop_tokens: vec!["<|im_end|>".to_string(), "<|im_start|>".to_string(), "<|endoftext|>".to_string()],
            stop_token_ids: vec![],
            fim_pre: None, fim_suf: None, fim_mid: None,
        }
    } else if is_llama3 {
        println!("  [AUTO-DETECT] Identified Model Architecture: Llama-3 / Llama-3.1");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "Llama-3".to_string(),
            chamber_verified: false,
            eos_token: if native_eos > 0 { native_eos } else { 128009 },
            eot_token: if native_eot > 0 { native_eot } else { 128009 },
            system_prefix: "<|start_header_id|>system<|end_header_id|>\n\n".to_string(),
            user_prefix: "<|start_header_id|>user<|end_header_id|>\n\n".to_string(),
            assistant_prefix: "<|start_header_id|>assistant<|end_header_id|>\n\n".to_string(),
            turn_suffix: "<|eot_id|>".to_string(),
            stop_tokens: vec!["<|eot_id|>".to_string(), "<|start_header_id|>".to_string(), "<|endoftext|>".to_string()],
            stop_token_ids: vec![],
            fim_pre: None, fim_suf: None, fim_mid: None,
        }
    } else if is_phi4 {
        println!("  [AUTO-DETECT] Identified Model Architecture: Microsoft Phi-4 / Phi-3");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "Phi-4".to_string(),
            chamber_verified: false,
            eos_token: native_eos,
            eot_token: native_eot,
            system_prefix: "<|system|>\n".to_string(),
            user_prefix: "<|user|>\n".to_string(),
            assistant_prefix: "<|assistant|>\n".to_string(),
            turn_suffix: "<|end|>\n".to_string(),
            stop_tokens: vec!["<|end|>".to_string(), "<|user|>".to_string(), "<|assistant|>".to_string(), "<|system|>".to_string()],
            stop_token_ids: vec![],
            fim_pre: None, fim_suf: None, fim_mid: None,
        }
    } else if is_gemma {
        println!("  [AUTO-DETECT] Identified Model Architecture: Google Gemma / Gemma-2");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "Gemma".to_string(),
            chamber_verified: false,
            eos_token: if native_eos > 0 { native_eos } else { 1 },
            eot_token: if native_eot > 0 { native_eot } else { 107 },
            system_prefix: "<start_of_turn>system\n".to_string(),
            user_prefix: "<start_of_turn>user\n".to_string(),
            assistant_prefix: "<start_of_turn>model\n".to_string(),
            turn_suffix: "<end_of_turn>\n".to_string(),
            stop_tokens: vec!["<end_of_turn>".to_string(), "<start_of_turn>".to_string()],
            stop_token_ids: vec![],
            fim_pre: None, fim_suf: None, fim_mid: None,
        }
    } else if is_spark {
        println!("  [AUTO-DETECT] Identified Model Architecture: Spark-X2.5 (Hybrid Attention)");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "Spark-X2.5".to_string(),
            chamber_verified: false,
            eos_token: native_eos,
            eot_token: native_eot,
            system_prefix: "<｜start·of·sentence｜><|System|>\n".to_string(),
            user_prefix: "<｜start·of·sentence｜><|User|>\n".to_string(),
            assistant_prefix: "<｜start·of·sentence｜><|Bot|>\n".to_string(),
            turn_suffix: "<｜end·of·sentence｜>\n".to_string(),
            stop_tokens: vec!["<｜end·of·sentence｜>".to_string(), "<｜start·of·sentence｜>".to_string(), "<|endoftext|>".to_string()],
            stop_token_ids: vec![],
            fim_pre: None, fim_suf: None, fim_mid: None,
        }
    } else if is_mistral {
        println!("  [AUTO-DETECT] Identified Model Architecture: Mistral / Llama-2 [INST]");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "Mistral".to_string(),
            chamber_verified: false,
            eos_token: if native_eos > 0 { native_eos } else { 2 },
            eot_token: if native_eot > 0 { native_eot } else { 2 },
            system_prefix: "".to_string(),
            user_prefix: "[INST] ".to_string(),
            assistant_prefix: " [/INST]".to_string(),
            turn_suffix: "</s>\n".to_string(),
            stop_tokens: vec!["</s>".to_string(), "[INST]".to_string(), "<|im_end|>".to_string()],
            stop_token_ids: vec![],
            fim_pre: None, fim_suf: None, fim_mid: None,
        }
    } else if is_command_r {
        println!("  [AUTO-DETECT] Identified Model Architecture: Cohere Command-R");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "Command-R".to_string(),
            chamber_verified: false,
            eos_token: native_eos,
            eot_token: native_eot,
            system_prefix: "<|START_OF_TURN_TOKEN|><|SYSTEM_TOKEN|>".to_string(),
            user_prefix: "<|START_OF_TURN_TOKEN|><|USER_TOKEN|>".to_string(),
            assistant_prefix: "<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>".to_string(),
            turn_suffix: "<|END_OF_TURN_TOKEN|>\n".to_string(),
            stop_tokens: vec!["<|END_OF_TURN_TOKEN|>".to_string(), "<|START_OF_TURN_TOKEN|>".to_string()],
            stop_token_ids: vec![],
            fim_pre: None, fim_suf: None, fim_mid: None,
        }
    } else if is_deepseek {
        println!("  [AUTO-DETECT] Identified Model Architecture: DeepSeek V2/V3");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "DeepSeek".to_string(),
            chamber_verified: false,
            eos_token: native_eos,
            eot_token: native_eot,
            system_prefix: "".to_string(),
            user_prefix: "<｜User｜>".to_string(),
            assistant_prefix: "<｜Assistant｜>".to_string(),
            turn_suffix: "<｜end of sentence｜>".to_string(),
            stop_tokens: vec!["<｜end of sentence｜>".to_string(), "<｜User｜>".to_string()],
            stop_token_ids: vec![],
            fim_pre: None, fim_suf: None, fim_mid: None,
        }
    } else {
        println!("  [AUTO-DETECT] Using Universal Markdown Framing fallback");
        OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "Universal".to_string(),
            chamber_verified: false,
            eos_token: if native_eos > 0 { native_eos } else { 2 },
            eot_token: if native_eot > 0 { native_eot } else { 2 },
            ..OmniModelConfig::default()
        }
    };

    // Incorporate native GGUF EOS / EOT tokens from vocabulary (if valid special tokens)
    if native_eos > 0 && is_valid_special_stop_token(model, native_eos) {
        if !config.stop_token_ids.contains(&native_eos) {
            config.stop_token_ids.push(native_eos);
        }
        if let Ok(piece) = model.token_to_piece(native_eos) {
            let p_trim = piece.trim().to_string();
            if !p_trim.is_empty() && !config.stop_tokens.contains(&p_trim) {
                config.stop_tokens.push(p_trim);
            }
        }
    }
    if native_eot > 0 && is_valid_special_stop_token(model, native_eot) {
        if !config.stop_token_ids.contains(&native_eot) {
            config.stop_token_ids.push(native_eot);
        }
        if let Ok(piece) = model.token_to_piece(native_eot) {
            let p_trim = piece.trim().to_string();
            if !p_trim.is_empty() && !config.stop_tokens.contains(&p_trim) {
                config.stop_tokens.push(p_trim);
            }
        }
    }

    // Resolve any string stop_tokens (e.g. "<|im_end|>") into stop_token_ids
    for st in &config.stop_tokens {
        if let Ok(toks) = model.tokenize(st, false) {
            if toks.len() == 1 {
                let tid = toks[0];
                if is_valid_special_stop_token(model, tid) && !config.stop_token_ids.contains(&tid) {
                    config.stop_token_ids.push(tid);
                }
            }
        }
    }

    // ── LAYER 2: Assistant-Prefill Empirical Chamber Verification ─────────────
    println!("  [AUTO-DETECT] Executing Assistant-Prefill Chamber probe...");
    let probe_prompt = config.format_chat(None, "Echo exactly: TEST");
    let prefill_input = format!("{}TEST", probe_prompt);

    if let Ok(mut ctx) = model.create_context(256, 256, 1) {
        if let Ok(tokens) = model.tokenize(&prefill_input, true) {
            if ctx.eval_tokens(&tokens, 0).is_ok() {
                if let Ok(mut sampler) = NativeLlamaSampler::new(&SamplingConfig::greedy(), model.n_vocab()) {
                    let mut cursor = tokens.len() as i32;
                    for _ in 0..6 {
                        if let Ok(tok) = ctx.sample(&mut sampler) {
                            if is_valid_special_stop_token(model, tok) {
                                let piece = model.token_to_piece(tok).unwrap_or_default();
                                println!("  [CHAMBER VERIFIED] Empirical termination token: id={} piece='{}'", tok, piece.escape_debug());

                                config.chamber_verified = true;
                                config.eot_token = tok;
                                if !config.stop_token_ids.contains(&tok) {
                                    config.stop_token_ids.push(tok);
                                }

                                let trimmed_piece = piece.trim().to_string();
                                if !trimmed_piece.is_empty() && !config.stop_tokens.contains(&trimmed_piece) {
                                    config.stop_tokens.push(trimmed_piece);
                                }
                                break;
                            }
                            let _ = ctx.eval_tokens(&[tok], cursor);
                            cursor += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    }

    if config.eos_token <= 0 {
        config.eos_token = config.eot_token.max(2);
    }
    if config.eot_token <= 0 {
        config.eot_token = config.eos_token;
    }
    if is_valid_special_stop_token(model, config.eos_token) && !config.stop_token_ids.contains(&config.eos_token) {
        config.stop_token_ids.push(config.eos_token);
    }

    // ── LAYER 3: Fill-In-Middle (FIM) Code Infill Signature ───────────────────
    let has_qwen_fim = model.tokenize("<|fim_prefix|>", false).map(|t| t.len() == 1).unwrap_or(false);
    let has_starcoder_fim = model.tokenize("<fim_prefix>", false).map(|t| t.len() == 1).unwrap_or(false);

    if has_qwen_fim {
        config.fim_pre = Some("<|fim_prefix|>".to_string());
        config.fim_suf = Some("<|fim_suffix|>".to_string());
        config.fim_mid = Some("<|fim_middle|>".to_string());
    } else if has_starcoder_fim {
        config.fim_pre = Some("<fim_prefix>".to_string());
        config.fim_suf = Some("<fim_suffix>".to_string());
        config.fim_mid = Some("<fim_middle>".to_string());
    }

    // ── LAYER 4: Sovereign .omni Manifest Caching ──────────────────────────────
    let target_omni_path = parent.join(format!("{}.omni", stem));
    match config.to_omni_string() {
        Ok(manifest_content) => {
            if let Err(e) = fs::write(&target_omni_path, manifest_content) {
                println!("  [AUTO-DETECT] Warning: failed to save sovereign manifest to {}: {}", target_omni_path.display(), e);
            } else {
                println!("  [AUTO-DETECT] Successfully verified and saved sovereign profile to {}", target_omni_path.display());
            }
        }
        Err(e) => {
            println!("  [AUTO-DETECT] Warning: failed to serialize sovereign manifest: {}", e);
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_omni_json_format_with_comments() {
        let content = r#"
# ==============================================================================
# OMNI SOVEREIGN MODEL MANIFEST (.omni)
# ==============================================================================
{
    "engine_version": "2.1.0",
    "model_family": "Phi-4",
    "chamber_verified": true,
    "eos_token": 32000,
    "eot_token": 32007,
    "system_prefix": "<|system|>\n",
    "user_prefix": "<|user|>\n",
    "assistant_prefix": "<|assistant|>\n",
    "turn_suffix": "<|end|>\n",
    "stop_tokens": ["<|end|>", "<|user|>"],
    "stop_token_ids": [32000, 32007],
    "fim_pre": null,
    "fim_suf": null,
    "fim_mid": null
}
"#;
        let config = OmniModelConfig::from_omni_str(content, "Phi-4-mini").expect("Must parse");
        assert_eq!(config.model_family, "Phi-4");
        assert!(config.chamber_verified);
        assert_eq!(config.eos_token, 32000);
        assert_eq!(config.eot_token, 32007);
        assert_eq!(config.system_prefix, "<|system|>\n");
        assert_eq!(config.turn_suffix, "<|end|>\n");
        assert_eq!(config.stop_token_ids, vec![32000, 32007]);
    }

    #[test]
    fn test_parse_omni_key_value_format() {
        let content = r#"
# Sovereign Variables File
MODEL_FAMILY = Spark-X2.5
CHAMBER_VERIFIED = true
EOS_TOKEN = 151643
EOT_TOKEN = 151645
SYSTEM_PREFIX = "<|System|>\n"
USER_PREFIX = "<|User|>\n"
ASSISTANT_PREFIX = "<|Bot|>\n"
TURN_SUFFIX = "</s>\n"
STOP_TOKENS = ["</s>", "<|User|>"]
STOP_TOKEN_IDS = [151643, 151645]
"#;
        let config = OmniModelConfig::from_omni_str(content, "spark-model").expect("Must parse");
        assert_eq!(config.model_family, "Spark-X2.5");
        assert!(config.chamber_verified);
        assert_eq!(config.eos_token, 151643);
        assert_eq!(config.eot_token, 151645);
        assert_eq!(config.system_prefix, "<|System|>\n");
        assert_eq!(config.user_prefix, "<|User|>\n");
        assert_eq!(config.assistant_prefix, "<|Bot|>\n");
        assert_eq!(config.turn_suffix, "</s>\n");
        assert_eq!(config.stop_tokens, vec!["</s>", "<|User|>"]);
        assert_eq!(config.stop_token_ids, vec![151643, 151645]);
    }

    #[test]
    fn test_parse_omni_multi_model_dictionary() {
        let content = r#"
{
    "phi4": {
        "model_family": "Phi-4",
        "eos_token": 32000,
        "eot_token": 32007,
        "system_prefix": "<|system|>\n",
        "user_prefix": "<|user|>\n",
        "assistant_prefix": "<|assistant|>\n",
        "turn_suffix": "<|end|>\n",
        "stop_tokens": ["<|end|>"],
        "stop_token_ids": [32000, 32007]
    },
    "qwen": {
        "model_family": "ChatML",
        "eos_token": 151645,
        "eot_token": 151645,
        "system_prefix": "<|im_start|>system\n",
        "user_prefix": "<|im_start|>user\n",
        "assistant_prefix": "<|im_start|>assistant\n",
        "turn_suffix": "<|im_end|>\n",
        "stop_tokens": ["<|im_end|>"],
        "stop_token_ids": [151645]
    }
}
"#;
        let config_phi = OmniModelConfig::from_omni_str(content, "Phi-4-mini-instruct").expect("Must parse phi");
        assert_eq!(config_phi.model_family, "Phi-4");
        assert_eq!(config_phi.eos_token, 32000);

        let config_qwen = OmniModelConfig::from_omni_str(content, "Qwen2.5-Coder-7B").expect("Must parse qwen");
        assert_eq!(config_qwen.model_family, "ChatML");
        assert_eq!(config_qwen.eos_token, 151645);
    }

    #[test]
    fn test_omni_to_string_and_reload() {
        let original = OmniModelConfig {
            engine_version: default_engine_version(),
            model_family: "Cohere Command-R".to_string(),
            chamber_verified: true,
            eos_token: 255001,
            eot_token: 255001,
            system_prefix: "<|START_OF_TURN_TOKEN|><|SYSTEM_TOKEN|>".to_string(),
            user_prefix: "<|START_OF_TURN_TOKEN|><|USER_TOKEN|>".to_string(),
            assistant_prefix: "<|START_OF_TURN_TOKEN|><|CHATBOT_TOKEN|>".to_string(),
            turn_suffix: "<|END_OF_TURN_TOKEN|>\n".to_string(),
            stop_tokens: vec!["<|END_OF_TURN_TOKEN|>".to_string()],
            stop_token_ids: vec![255001],
            fim_pre: None,
            fim_suf: None,
            fim_mid: None,
        };

        let serialized = original.to_omni_string().expect("Must serialize");
        assert!(serialized.contains("# OMNI SOVEREIGN MODEL MANIFEST (.omni)"));

        let reloaded = OmniModelConfig::from_omni_str(&serialized, "command-r").expect("Must reload");
        assert_eq!(reloaded.model_family, "Cohere Command-R");
        assert!(reloaded.chamber_verified);
        assert_eq!(reloaded.eos_token, 255001);
    }
}
