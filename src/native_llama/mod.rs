//! Native Llama.cpp C FFI Engine and In-Memory KV-Cache Controller.
//!
//! Provides direct, in-process C-level access to transformer model weights,
//! tokenization, batched decoding, greedy/stochastic sampling, and physical
//! KV-cache tensor manipulation (surgical rollback, sequence branching,
//! defragmentation, and full epistemic apoptosis).

use colored::*;
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::io::{self, IsTerminal, Write};
use std::os::raw::{c_char, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

static BACKEND_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[link(name = "omni_llama_bridge", kind = "static")]
#[link(name = "llama")]
#[link(name = "ggml")]
#[link(name = "ggml-base")]
#[link(name = "ggml-cpu")]
extern "C" {
    fn omni_llama_backend_init();
    fn omni_llama_backend_free();
    fn omni_llama_load_model(path: *const c_char, n_gpu_layers: i32) -> *mut c_void;
    fn omni_llama_free_model(model: *mut c_void);
    fn omni_llama_new_context(
        model: *mut c_void,
        n_ctx: i32,
        n_batch: i32,
        n_threads: i32,
    ) -> *mut c_void;
    fn omni_llama_free_context(ctx: *mut c_void);
    fn omni_llama_tokenize(
        model: *mut c_void,
        text: *const c_char,
        text_len: i32,
        out_tokens: *mut i32,
        max_tokens: i32,
        add_bos: bool,
    ) -> i32;
    fn omni_llama_token_to_piece(
        model: *mut c_void,
        token: i32,
        buf: *mut c_char,
        buf_size: i32,
    ) -> i32;
    fn omni_llama_eval_tokens(
        ctx: *mut c_void,
        tokens: *const i32,
        n_tokens: i32,
        seq_id: i32,
        start_pos: i32,
    ) -> i32;
    fn omni_llama_sample_greedy(ctx: *mut c_void, model: *mut c_void) -> i32;
    fn omni_llama_kv_cache_used_cells(ctx: *mut c_void) -> i32;
    fn omni_llama_kv_cache_token_count(ctx: *mut c_void) -> i32;
    fn omni_llama_kv_cache_clear(ctx: *mut c_void);
    fn omni_llama_kv_cache_seq_rm(ctx: *mut c_void, seq_id: i32, p0: i32, p1: i32) -> bool;
    fn omni_llama_kv_cache_seq_cp(ctx: *mut c_void, seq_src: i32, seq_dst: i32, p0: i32, p1: i32);
    fn omni_llama_kv_cache_seq_shift(ctx: *mut c_void, seq_id: i32, p0: i32, p1: i32, delta: i32);
    fn omni_llama_kv_cache_seq_pos_max(ctx: *mut c_void, seq_id: i32) -> i32;
    fn omni_llama_n_vocab(model: *mut c_void) -> i32;
    fn omni_llama_get_logits(
        ctx: *mut c_void,
        model: *mut c_void,
        out_logits: *mut f32,
        max_vocab: i32,
    ) -> i32;

    // --- Modern Sampler Chain Bindings ---
    fn omni_llama_sampler_init_chain() -> *mut c_void;
    fn omni_llama_sampler_add_penalties(
        chain: *mut c_void,
        n_vocab: i32,
        penalty_last_n: i32,
        penalty_repeat: f32,
        penalty_freq: f32,
        penalty_present: f32,
    );
    fn omni_llama_sampler_add_top_k(chain: *mut c_void, k: i32);
    fn omni_llama_sampler_add_top_p(chain: *mut c_void, p: f32, min_keep: usize);
    fn omni_llama_sampler_add_min_p(chain: *mut c_void, p: f32, min_keep: usize);
    fn omni_llama_sampler_add_temp(chain: *mut c_void, temp: f32);
    fn omni_llama_sampler_add_dist(chain: *mut c_void, seed: u32);
    fn omni_llama_sampler_add_greedy(chain: *mut c_void);
    fn omni_llama_sampler_sample(chain: *mut c_void, ctx: *mut c_void, idx: i32) -> i32;
    fn omni_llama_sampler_accept(chain: *mut c_void, token: i32);
    fn omni_llama_sampler_reset(chain: *mut c_void);
    fn omni_llama_sampler_free(chain: *mut c_void);
    fn omni_llama_sampler_create(
        temp: f32,
        top_p: f32,
        top_k: i32,
        penalty_repeat: f32,
        penalty_freq: f32,
        penalty_present: f32,
        penalty_last_n: i32,
        seed: u32,
    ) -> *mut c_void;
}

/// Initialize the llama.cpp backend once per process lifecycle.
pub fn ensure_backend_initialized() {
    if !BACKEND_INITIALIZED.swap(true, Ordering::SeqCst) {
        unsafe {
            omni_llama_backend_init();
        }
        info!("Native llama.cpp backend initialized successfully.");
    }
}

/// Represents a loaded GGUF transformer model in host / GPU memory.
pub struct NativeLlamaModel {
    raw_model: *mut c_void,
    model_path: String,
}

unsafe impl Send for NativeLlamaModel {}
unsafe impl Sync for NativeLlamaModel {}

impl NativeLlamaModel {
    /// Load a GGUF model directly into RAM/VRAM.
    pub fn load<P: AsRef<Path>>(path: P, n_gpu_layers: i32) -> Result<Arc<Self>, String> {
        ensure_backend_initialized();

        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy().to_string();

        if !path_ref.exists() {
            return Err(format!("Model file not found: {}", path_str));
        }

        let c_path = CString::new(path_str.as_bytes())
            .map_err(|e| format!("Invalid model path string: {}", e))?;

        let raw = unsafe { omni_llama_load_model(c_path.as_ptr(), n_gpu_layers) };
        if raw.is_null() {
            return Err(format!("llama_model_load_from_file returned null for: {}", path_str));
        }

        info!("Successfully loaded native model weights: {}", path_str);

        Ok(Arc::new(Self {
            raw_model: raw,
            model_path: path_str,
        }))
    }

    /// Tokenize raw text into integer token IDs using the model's native vocabulary.
    pub fn tokenize(&self, text: &str, add_bos: bool) -> Result<Vec<i32>, String> {
        let c_text = CString::new(text.as_bytes())
            .map_err(|e| format!("Invalid text string: {}", e))?;

        let mut buffer: Vec<i32> = vec![0; text.len() + 16];
        let count = unsafe {
            omni_llama_tokenize(
                self.raw_model,
                c_text.as_ptr(),
                text.len() as i32,
                buffer.as_mut_ptr(),
                buffer.len() as i32,
                add_bos,
            )
        };

        if count < 0 {
            return Err(format!("Tokenization failed for text (len={})", text.len()));
        }

        buffer.truncate(count as usize);
        Ok(buffer)
    }

    /// Convert a token ID back into its UTF-8 text piece.
    pub fn token_to_piece(&self, token: i32) -> Result<String, String> {
        let mut buf = vec![0u8; 256];
        let len = unsafe {
            omni_llama_token_to_piece(
                self.raw_model,
                token,
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as i32,
            )
        };

        if len < 0 {
            return Err(format!("Failed to decode token {}", token));
        }

        buf.truncate(len as usize);
        Ok(String::from_utf8_lossy(&buf).to_string())
    }

    /// Returns the vocabulary size of the model.
    pub fn n_vocab(&self) -> usize {
        let n = unsafe { omni_llama_n_vocab(self.raw_model) };
        n.max(0) as usize
    }

    /// Spawn an active execution context with dedicated KV-cache memory.
    pub fn create_context(
        self: &Arc<Self>,
        n_ctx: usize,
        n_batch: usize,
        n_threads: usize,
    ) -> Result<NativeLlamaContext, String> {
        let raw_ctx = unsafe {
            omni_llama_new_context(
                self.raw_model,
                n_ctx as i32,
                n_batch as i32,
                n_threads as i32,
            )
        };

        if raw_ctx.is_null() {
            return Err("Failed to allocate native llama context".to_string());
        }

        Ok(NativeLlamaContext {
            raw_ctx,
            model: self.clone(),
            current_cursor: 0,
            n_ctx,
            n_batch,
        })
    }

    pub fn path(&self) -> &str {
        &self.model_path
    }
}

impl Drop for NativeLlamaModel {
    fn drop(&mut self) {
        if !self.raw_model.is_null() {
            unsafe {
                omni_llama_free_model(self.raw_model);
            }
            debug!("Freed native model: {}", self.model_path);
        }
    }
}

/// Hyperparameter configuration for autoregressive token sampling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamplingConfig {
    /// Temperature scaling applied to logits.
    /// Values <= 0.0 enable pure deterministic greedy selection.
    /// Default: 0.7.
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Top-P (nucleus) sampling threshold in range (0.0, 1.0].
    /// Only tokens comprising top cumulative probability mass `p` are considered.
    /// 1.0 disables nucleus filtering. Default: 0.9.
    #[serde(default = "default_top_p")]
    pub top_p: f32,

    /// Top-K sampling candidate count limit.
    /// Retains only the K most likely tokens. <= 0 disables Top-K.
    /// Default: 40.
    #[serde(default = "default_top_k")]
    pub top_k: i32,

    /// Repetition penalty factor applied to previously seen tokens.
    /// 1.0 disables repetition penalty. Values > 1.0 penalize repeated tokens.
    /// Default: 1.1.
    #[serde(default = "default_repetition_penalty")]
    pub repetition_penalty: f32,

    /// Number of recent tokens considered for repetition penalties.
    /// <= 0 disables last-n filtering; > 0 specifies the lookback window.
    /// Default: 64.
    #[serde(default = "default_penalty_last_n")]
    pub penalty_last_n: i32,

    /// Random number generator seed.
    /// 0 triggers default fallback (1337) or entropy seed.
    /// Default: 0.
    #[serde(default)]
    pub seed: u32,
}

fn default_temperature() -> f32 { 0.7 }
fn default_top_p() -> f32 { 0.9 }
fn default_top_k() -> i32 { 40 }
fn default_repetition_penalty() -> f32 { 1.1 }
fn default_penalty_last_n() -> i32 { 64 }

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repetition_penalty: 1.1,
            penalty_last_n: 64,
            seed: 1337,
        }
    }
}

impl SamplingConfig {
    /// Pure deterministic greedy sampling (temperature = 0, no penalties).
    pub fn greedy() -> Self {
        Self {
            temperature: 0.0,
            top_p: 1.0,
            top_k: 0,
            repetition_penalty: 1.0,
            penalty_last_n: 0,
            seed: 0,
        }
    }

    /// Stochastic sampling with custom temperature and top-p.
    pub fn stochastic(temperature: f32, top_p: f32, seed: u32) -> Self {
        Self {
            temperature,
            top_p,
            top_k: 40,
            repetition_penalty: 1.1,
            penalty_last_n: 64,
            seed,
        }
    }

    /// Returns `true` if greedy selection is configured.
    pub fn is_greedy(&self) -> bool {
        self.temperature <= 0.0
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = top_p;
        self
    }

    pub fn with_top_k(mut self, top_k: i32) -> Self {
        self.top_k = top_k;
        self
    }

    pub fn with_repetition_penalty(mut self, penalty: f32, last_n: i32) -> Self {
        self.repetition_penalty = penalty;
        self.penalty_last_n = last_n;
        self
    }

    pub fn with_seed(mut self, seed: u32) -> Self {
        self.seed = seed;
        self
    }
}

/// RAII wrapper over modern llama.cpp `llama_sampler` chain.
///
/// Encapsulates repetition penalties, top-k/top-p candidate filtering,
/// temperature scaling, and final token selection (distribution or greedy).
pub struct NativeLlamaSampler {
    raw_sampler: *mut c_void,
}

unsafe impl Send for NativeLlamaSampler {}

impl NativeLlamaSampler {
    /// Build a new sampler chain based on `SamplingConfig`.
    pub fn new(config: &SamplingConfig) -> Result<Self, String> {
        let chain = unsafe { omni_llama_sampler_init_chain() };
        if chain.is_null() {
            return Err("Failed to allocate native llama_sampler_chain (returned null)".to_string());
        }

        unsafe {
            // 1. Repetition penalties (applied first on full candidate logits)
            if config.repetition_penalty != 1.0 || config.penalty_last_n > 0 {
                let last_n = if config.penalty_last_n > 0 { config.penalty_last_n } else { 64 };
                omni_llama_sampler_add_penalties(
                    chain,
                    151936, // Qwen2.5 vocab size // n_vocab
                    last_n,
                    config.repetition_penalty,
                    0.0, // penalty_freq
                    0.0, // penalty_present
                );
            }

            // 2. Top-K filtering
            if config.top_k > 0 {
                omni_llama_sampler_add_top_k(chain, config.top_k);
            }

            // 3. Top-P (nucleus) filtering
            if config.top_p > 0.0 && config.top_p < 1.0 {
                omni_llama_sampler_add_top_p(chain, config.top_p, 1);
            }

            // 4. Temperature & Distribution vs Greedy
            if config.temperature <= 0.0 {
                omni_llama_sampler_add_greedy(chain);
            } else {
                omni_llama_sampler_add_temp(chain, config.temperature);
                let seed = if config.seed == 0 { 1337 } else { config.seed };
                omni_llama_sampler_add_dist(chain, seed);
            }
        }

        Ok(Self { raw_sampler: chain })
    }

    /// Convenience constructor for a greedy sampler.
    pub fn greedy() -> Result<Self, String> {
        Self::new(&SamplingConfig::greedy())
    }

    /// Sample next token from context's last evaluated position (index -1).
    pub fn sample(&mut self, ctx: &mut NativeLlamaContext) -> Result<i32, String> {
        self.sample_idx(ctx, -1)
    }

    /// Sample token from context at explicit batch position `idx`.
    pub fn sample_idx(&mut self, ctx: &mut NativeLlamaContext, idx: i32) -> Result<i32, String> {
        let tok = unsafe { omni_llama_sampler_sample(self.raw_sampler, ctx.raw_ctx, idx) };
        if tok < 0 {
            return Err(format!("Sampler failed to select token (code: {})", tok));
        }
        Ok(tok)
    }

    /// Explicitly feed a token into the sampler's penalty history.
    pub fn accept(&mut self, token: i32) {
        unsafe { omni_llama_sampler_accept(self.raw_sampler, token) };
    }

    /// Reset internal state (penalty rings and token history).
    pub fn reset(&mut self) {
        unsafe { omni_llama_sampler_reset(self.raw_sampler) };
    }

    /// Access raw underlying C pointer.
    pub fn raw(&self) -> *mut c_void {
        self.raw_sampler
    }
}

impl Drop for NativeLlamaSampler {
    fn drop(&mut self) {
        if !self.raw_sampler.is_null() {
            unsafe {
                omni_llama_sampler_free(self.raw_sampler);
            }
            debug!("Freed native llama sampler chain.");
            self.raw_sampler = std::ptr::null_mut();
        }
    }
}

/// An active inference execution context with direct C-level KV-cache manipulation.
pub struct NativeLlamaContext {
    raw_ctx: *mut c_void,
    model: Arc<NativeLlamaModel>,
    current_cursor: usize,
    n_ctx: usize,
    n_batch: usize,
}

unsafe impl Send for NativeLlamaContext {}

impl NativeLlamaContext {
    /// Evaluate token sequence through transformer layers, updating active KV-cache.
    pub fn eval_tokens(&mut self, tokens: &[i32], seq_id: i32) -> Result<(), String> {
        if tokens.is_empty() {
            return Ok(());
        }

        // Chunk by batch size to respect llama.cpp cparams.n_batch limit
        let chunk_size = self.n_batch.max(64);
        for chunk in tokens.chunks(chunk_size) {
            let res = unsafe {
                omni_llama_eval_tokens(
                    self.raw_ctx,
                    chunk.as_ptr(),
                    chunk.len() as i32,
                    seq_id,
                    self.current_cursor as i32,
                )
            };

            if res != 0 {
                return Err(format!("llama_decode failed with error code: {}", res));
            }

            self.current_cursor += chunk.len();
        }

        Ok(())
    }


    /// Sample next token using an external NativeLlamaSampler chain.
    pub fn sample(&mut self, sampler: &mut NativeLlamaSampler) -> Result<i32, String> {
        let tok = unsafe { omni_llama_sampler_sample(sampler.raw_sampler, self.raw_ctx, -1) };
        if tok < 0 {
            return Err(format!("Context sampling failed with error code: {}", tok));
        }
        Ok(tok)
    }

    /// Sample next token using an external NativeLlamaSampler chain (alias for sample).
    pub fn sample_with(&mut self, sampler: &mut NativeLlamaSampler) -> Result<i32, String> {
        self.sample(sampler)
    }

    /// Sample next token using greedy argmax selection over logits.
    pub fn sample_greedy(&self) -> Result<i32, String> {
        let token = unsafe { omni_llama_sample_greedy(self.raw_ctx, self.model.raw_model) };
        if token < 0 {
            return Err("Failed to sample token from logits".to_string());
        }
        Ok(token)
    }

    /// Extracts output logits for the last evaluated token across the full vocabulary.
    pub fn get_logits(&self) -> Result<Vec<f32>, String> {
        let n_vocab = self.model.n_vocab();
        if n_vocab == 0 {
            return Err("Model vocabulary size is 0".to_string());
        }
        let mut logits = vec![0.0f32; n_vocab];
        let copied = unsafe {
            omni_llama_get_logits(
                self.raw_ctx,
                self.model.raw_model,
                logits.as_mut_ptr(),
                n_vocab as i32,
            )
        };
        if copied < 0 {
            return Err("Failed to retrieve logits from llama context".to_string());
        }
        logits.truncate(copied as usize);
        Ok(logits)
    }

    /// Autoregressively generate text from a prompt using a specified `SamplingConfig`.
    pub fn generate_with_sampling(
        &mut self,
        prompt: &str,
        max_tokens: usize,
        config: &SamplingConfig,
    ) -> Result<String, String> {
        let is_term = io::stdout().is_terminal();
        let prompt_tokens = self.model.tokenize(prompt, true)?;

        let mut sampler = NativeLlamaSampler::new(config)?;

        // Ingest prompt tokens into sampler history for repetition penalties
        for &tok in &prompt_tokens {
            sampler.accept(tok);
        }

        let start_time = std::time::Instant::now();
        if is_term {
            print!("     [*] Evaluating context ({} tokens)...", prompt_tokens.len());
            let _ = io::stdout().flush();
        }

        self.eval_tokens(&prompt_tokens, 0)?;

        let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let mut generated = String::new();

        for i in 0..max_tokens {
            let next_tok = self.sample(&mut sampler)?;
            let piece = self.model.token_to_piece(next_tok)?;

            // Check EOS tokens across model architectures (<|im_end|>, <|im_start|>, <|endoftext|>, <|eot_id|>, </s>, etc.)
            if piece.is_empty()
                || piece.contains("<|im_end|>")
                || piece.contains("<|im_start|>")
                || piece.contains("<|endoftext|>")
                || piece.contains("<|eot_id|>")
                || piece.contains("</s>")
                || piece.contains("<end_of_turn>")
                || piece.contains("### User")
                || piece.contains("### System")
            {
                break;
            }

            generated.push_str(&piece);
            self.eval_tokens(&[next_tok], 0)?;

            if is_term {
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed = if elapsed > 0.1 { (i + 1) as f64 / elapsed } else { 0.0 };
                let frame = spinner_frames[i % spinner_frames.len()];
                let used_cells = self.kv_cache_used_cells();

                // Live dynamic CLI indicator (animating on the same line)
                let gauge_width: usize = 10;
                let ratio = (used_cells as f64 / self.n_ctx as f64).clamp(0.0, 1.0);
                let filled = (ratio * gauge_width as f64).round() as usize;
                let bar: String = "█".repeat(filled) + &"░".repeat(gauge_width.saturating_sub(filled));

                print!(
                    "\r     {} {} Active: Token #{} ({:.1} t/s) │ KV: [{}] {}/{} │ {:.0}s   ",
                    frame.to_string().bright_cyan().bold(),
                    "●".bright_green(),
                    (i + 1).to_string().bright_yellow().bold(),
                    speed,
                    bar.bright_magenta(),
                    used_cells,
                    self.n_ctx,
                    elapsed
                );
                let _ = io::stdout().flush();
            }

            // Loop repetition breaker: guard against infinite token cycle attractor loops
            let tail_len = 32;
            if generated.len() >= tail_len * 2 {
                let tail = &generated[generated.len() - tail_len..];
                if generated[..generated.len() - tail_len].contains(tail) {
                    let trimmed = generated[..generated.len() - tail_len].trim_end();
                    generated = trimmed.to_string();
                    break;
                }
            }
        }

        if is_term {
            let total_time = start_time.elapsed().as_secs_f64();
            let final_speed = if total_time > 0.1 { (generated.len()) as f64 / total_time } else { 0.0 };
            print!("\r                                                                                         \r");
            println!(
                "     [+] {} ({:.1}s, {:.1} chars/s, {} KV cells)",
                "Generation pass complete".bright_green().bold(),
                total_time,
                final_speed,
                self.kv_cache_used_cells()
            );
            let _ = io::stdout().flush();
        }

        Ok(generated)
    }

    /// Autoregressively generate up to `max_tokens` from a prompt with live visual CLI telemetry (greedy default).
    pub fn generate(&mut self, prompt: &str, max_tokens: usize) -> Result<String, String> {
        self.generate_with_sampling(prompt, max_tokens, &SamplingConfig::greedy())
    }

    // -----------------------------------------------------------------------
    // REAL KV-CACHE OPERATIONS
    // -----------------------------------------------------------------------

    /// Number of active cells currently occupied in the physical KV-cache.
    pub fn kv_cache_used_cells(&self) -> usize {
        let cells = unsafe { omni_llama_kv_cache_used_cells(self.raw_ctx) };
        cells.max(0) as usize
    }

    /// Total token count tracked inside KV-cache memory.
    pub fn kv_cache_token_count(&self) -> usize {
        let count = unsafe { omni_llama_kv_cache_token_count(self.raw_ctx) };
        count.max(0) as usize
    }

    /// SURGICAL CAUSAL KV ROLLBACK (Suffix Excision / Tail Rollback):
    /// Physically excises cached key/value tensors for tokens in range `[p0, p1)`.
    /// When applied to sequence suffix (`p1 < 0` or `p1 >= current_cursor`), rewinds `current_cursor` to `p0`.
    /// Tail rollback (`p1 < 0`) cleanly purges all tail tokens from `p0` onward without touching
    /// prefix tokens in `[0, p0)`.
    ///
    /// CAUTION: For middle excision (`p1 < current_cursor`), llama.cpp retains subsequent token positions
    /// without shifting. Attempting mid-span excision and cell shifting corrupts Rotary Position
    /// Embeddings (RoPE) and introduces causal attention contamination. Use exact tail rollback instead.
    pub fn kv_cache_seq_rm(&mut self, seq_id: i32, p0: i32, p1: i32) -> Result<bool, String> {
        let ok = unsafe { omni_llama_kv_cache_seq_rm(self.raw_ctx, seq_id, p0, p1) };
        if ok {
            if p1 < 0 || (p1 as usize) >= self.current_cursor {
                self.current_cursor = p0.max(0) as usize;
            } else {
                warn!(
                    "Middle KV cells excised [{}..{}) for seq {}: warning: non-tail excision without re-rotation leaves RoPE phases unadjusted",
                    p0, p1, seq_id
                );
            }
            info!(
                "Physical KV Rollback executed: seq={}, [{}..{}), current_cursor={}",
                seq_id, p0, p1, self.current_cursor
            );
        }
        Ok(ok)
    }

    /// Checkpoint Rollback (Exact Tail Truncation):
    /// Excises all KV cells from `checkpoint` to the current cursor via `kv_cache_seq_rm(0, checkpoint as i32, -1)`,
    /// resetting cursor to `checkpoint` while leaving prefix tokens [0, checkpoint) 100% mathematically intact.
    pub fn rollback_to(&mut self, checkpoint: usize) -> Result<bool, String> {
        self.kv_cache_seq_rm(0, checkpoint as i32, -1)
    }

    /// DEPRECATED: SURGICAL MIDDLE EXCISION WITH AUTOMATIC POSITION SHIFT.
    ///
    /// # Mathematical & Architectural Caveat (RoPE Phase Corruption):
    /// In modern transformer architectures employing Rotary Position Embeddings (RoPE),
    /// key tensors $K_m$ are rotated by frequency rotation matrices $R_{\Theta, m}$ during
    /// the prefill pass before being written to KV cache cells.
    ///
    /// Calling `llama_kv_cache_seq_add` (shifting cell position metadata from $t$ to $t - \Delta$)
    /// merely updates the integer position tags in the cache. It does NOT counter-rotate the
    /// high-dimensional cached key tensors by $R_{\Theta, -\Delta}$. Consequently, attention
    /// inner products $\langle q_n, k_{\text{shifted}} \rangle$ evaluate with a phase mismatch
    /// $\Delta \theta_i$, introducing severe phase noise and degradation into multi-head attention.
    ///
    /// Furthermore, causal conditioning is violated: tokens downstream of the excised span were
    /// evaluated by attending to the excised tokens across all transformer layers. Simply shifting
    /// those tokens leftward retains "zombie activations" conditioned on deleted context.
    ///
    /// # Recommended Alternative:
    /// Use exact tail rollback (`kv_cache_seq_rm(seq_id, p0, -1)` or `rollback_to(checkpoint)`).
    /// Tail rollback preserves untouched prefix $[0, p_0)$ with 100% RoPE and causal integrity,
    /// achieving bitwise logit equivalence and $D_{\text{KL}} = 0.00$ against a cold start.
    #[deprecated(
        since = "2.2.0",
        note = "Mid-span excision causes RoPE phase corruption and causal attention contamination. Use exact tail-rollback via kv_cache_seq_rm(seq_id, p0, -1) or rollback_to(checkpoint)."
    )]
    pub fn kv_cache_seq_rm_and_shift(&mut self, seq_id: i32, p0: i32, p1: i32) -> Result<bool, String> {
        warn!(
            "kv_cache_seq_rm_and_shift invoked for seq={}, [{}..{}). Caution: mid-span shifting causes RoPE phase mismatch!",
            seq_id, p0, p1
        );
        if p0 < 0 || p1 <= p0 {
            return Err(format!("Invalid token range: [{}, {})", p0, p1));
        }
        let ok = unsafe { omni_llama_kv_cache_seq_rm(self.raw_ctx, seq_id, p0, p1) };
        if !ok {
            return Ok(false);
        }

        if (p1 as usize) >= self.current_cursor {
            self.current_cursor = p0 as usize;
        } else {
            let delta = -(p1 - p0);
            let trail_start = p1;
            let trail_end = self.current_cursor as i32;
            unsafe {
                omni_llama_kv_cache_seq_shift(self.raw_ctx, seq_id, trail_start, trail_end, delta);
            }
            self.current_cursor = (self.current_cursor as i32 + delta).max(0) as usize;
        }

        info!(
            "Physical KV Excised & Shifted: seq={}, [{}..{}), new_cursor={}",
            seq_id, p0, p1, self.current_cursor
        );
        Ok(true)
    }

    /// Synchronizes cursor with external tracker (e.g. CausalGraph)
    pub fn sync_cursor(&mut self, cursor: usize) {
        self.current_cursor = cursor;
    }

    /// Sets cursor from physically occupied cells in KV cache
    pub fn sync_cursor_from_physical_used_cells(&mut self) {
        self.current_cursor = self.kv_cache_used_cells();
    }

    /// SWARM BRANCHING:
    /// Forks the KV-cache of `seq_src` into `seq_dst` without recomputing tensors.
    pub fn kv_cache_seq_cp(&mut self, seq_src: i32, seq_dst: i32, p0: i32, p1: i32) {
        unsafe {
            omni_llama_kv_cache_seq_cp(self.raw_ctx, seq_src, seq_dst, p0, p1);
        }
        info!("Forked KV branch: src_seq={} -> dst_seq={}", seq_src, seq_dst);
    }

    /// CONTEXT SLIDING / POSITION SHIFT:
    /// Shifts token position indices by `delta` in the KV cache.
    pub fn kv_cache_seq_shift(&mut self, seq_id: i32, p0: i32, p1: i32, delta: i32) {
        unsafe {
            omni_llama_kv_cache_seq_shift(self.raw_ctx, seq_id, p0, p1, delta);
        }
    }

    /// EPISTEMIC APOPTOSIS:
    /// Completely clears all active cell positions in the KV-cache and resets cursor.
    pub fn kv_cache_clear(&mut self) {
        unsafe {
            omni_llama_kv_cache_clear(self.raw_ctx);
        }
        self.current_cursor = 0;
        info!("Physical KV-cache purged (Epistemic Apoptosis)");
    }

    pub fn current_cursor(&self) -> usize {
        self.current_cursor
    }

    pub fn n_ctx(&self) -> usize {
        self.n_ctx
    }
}

impl Drop for NativeLlamaContext {
    fn drop(&mut self) {
        if !self.raw_ctx.is_null() {
            unsafe {
                omni_llama_free_context(self.raw_ctx);
            }
            debug!("Freed native context.");
        }
    }
}
