//! Native Llama.cpp C FFI Engine and In-Memory KV-Cache Controller.
//!
//! Provides direct, in-process C-level access to transformer model weights,
//! tokenization, batched decoding, greedy/stochastic sampling, and physical
//! KV-cache tensor manipulation (surgical rollback, sequence branching,
//! defragmentation, and full epistemic apoptosis).

use colored::*;
use std::ffi::CString;
use std::io::{self, IsTerminal, Write};
use std::os::raw::{c_char, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info};

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

    /// Autoregressively generate up to `max_tokens` from a prompt with live visual CLI telemetry.
    pub fn generate(&mut self, prompt: &str, max_tokens: usize) -> Result<String, String> {
        let is_term = io::stdout().is_terminal();
        let prompt_tokens = self.model.tokenize(prompt, true)?;

        let start_time = std::time::Instant::now();
        if is_term {
            print!("     [*] Evaluating context ({} tokens)...", prompt_tokens.len());
            let _ = io::stdout().flush();
        }

        self.eval_tokens(&prompt_tokens, 0)?;

        let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let mut generated = String::new();

        for i in 0..max_tokens {
            let next_tok = self.sample_greedy()?;
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

            // Guard against autoregressive loop repetition
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

    /// SURGICAL CAUSAL KV ROLLBACK (Suffix Excision):
    /// Physically excises cached key/value tensors for tokens in range `[p0, p1)`.
    /// When applied to sequence suffix (`p1 < 0` or `p1 >= current_cursor`), rewinds `current_cursor` to `p0`.
    ///
    /// CAUTION: For middle excision (`p1 < current_cursor`), llama.cpp retains subsequent token positions
    /// without shifting unless `kv_cache_seq_rm_and_shift` is used.
    pub fn kv_cache_seq_rm(&mut self, seq_id: i32, p0: i32, p1: i32) -> Result<bool, String> {
        let ok = unsafe { omni_llama_kv_cache_seq_rm(self.raw_ctx, seq_id, p0, p1) };
        if ok {
            if p1 < 0 || (p1 as usize) >= self.current_cursor {
                self.current_cursor = p0.max(0) as usize;
            } else {
                // Middle excision without shift: tokens after p1 retain their original positional
                // indices, so current_cursor (the append position) cannot be decremented without collision.
                info!(
                    "Middle KV cells excised [{}..{}) for seq {}: trailing tokens remain at unshifted positions",
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

    /// Checkpoint Rollback (Tail Truncation):
    /// Excises all KV cells from `checkpoint` to the current cursor, resetting cursor to `checkpoint`.
    pub fn rollback_to(&mut self, checkpoint: usize) -> Result<bool, String> {
        self.kv_cache_seq_rm(0, checkpoint as i32, -1)
    }

    /// SURGICAL MIDDLE EXCISION WITH AUTOMATIC POSITION SHIFT:
    /// Excises `[p0, p1)` and physically shifts trailing tokens in `[p1, current_cursor)` left
    /// by `-(p1 - p0)` to maintain continuous positional alignment and safely decrement `current_cursor`.
    pub fn kv_cache_seq_rm_and_shift(&mut self, seq_id: i32, p0: i32, p1: i32) -> Result<bool, String> {
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
