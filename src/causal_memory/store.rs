use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::{Read, Write};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HydrationError {
    NotFound,
    DecompressionFailed(String),
    InvalidUtf8,
}

impl std::fmt::Display for HydrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Step ID not found in store"),
            Self::DecompressionFailed(e) => write!(f, "DEFLATE decompression failed: {}", e),
            Self::InvalidUtf8 => write!(f, "Payload is not valid UTF-8"),
        }
    }
}

impl std::error::Error for HydrationError {}

#[derive(Debug, Clone)]
pub struct StoreEntry {
    pub is_compressed: bool,
    pub payload: Vec<u8>,
    pub token_ids: Option<Vec<i32>>,
}

/// In-memory byte store for evicted step artifacts during agent context management.
///
/// NOTE on memory footprint and compression:
/// - Storing raw 32-bit token IDs (`i32`) consumes 4 bytes per token, which can equal
///   or exceed the size of the original text payload. Therefore, `token_ids` are optional
///   and only retained when exact token-boundary reproduction is required.
/// - For small conversational strings (< 100 bytes / ~25 tokens), DEFLATE compression
///   provides negligible byte savings or may slightly expand the payload. It is intended
///   primarily for large tool output payloads, compiler logs, and source files.
pub struct CompressedCacheStore {
    chunks: RwLock<HashMap<usize, StoreEntry>>,
}

impl CompressedCacheStore {
    pub fn new() -> Self {
        Self {
            chunks: RwLock::new(HashMap::new()),
        }
    }

    /// Stores text payload with automatic DEFLATE compression, optionally recording exact token IDs.
    pub fn store_step(
        &self,
        step_id: usize,
        text: &str,
        token_ids: Option<&[i32]>,
    ) -> usize {
        let raw_bytes = text.as_bytes();
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        let (is_compressed, payload) = match encoder.write_all(raw_bytes).and_then(|_| encoder.finish()) {
            Ok(compressed) if compressed.len() < raw_bytes.len() => (true, compressed),
            _ => (false, raw_bytes.to_vec()),
        };

        let stored_len = payload.len() + token_ids.map_or(0, |t| t.len() * 4);
        let entry = StoreEntry {
            is_compressed,
            payload,
            token_ids: token_ids.map(|t| t.to_vec()),
        };

        self.chunks.write().insert(step_id, entry);
        stored_len
    }

    /// Backwards-compatible text-only compression
    pub fn compress_and_store(&self, step_id: usize, text: &str) -> usize {
        self.store_step(step_id, text, None)
    }

    /// Backwards-compatible tokens-and-text compression
    pub fn compress_and_store_tokens(&self, step_id: usize, text: &str, tokens: &[i32]) -> usize {
        self.store_step(step_id, text, if tokens.is_empty() { None } else { Some(tokens) })
    }

    /// Decompresses and restores original UTF-8 payload and optional token IDs.
    /// The entry is cloned inside the read lock and decompression is performed outside
    /// the lock scope to minimize lock duration.
    pub fn hydrate_entry(&self, step_id: usize) -> Result<(String, Option<Vec<i32>>), HydrationError> {
        let entry = {
            let map = self.chunks.read();
            map.get(&step_id).cloned().ok_or(HydrationError::NotFound)?
        };

        let text = if entry.is_compressed {
            let mut decoder = DeflateDecoder::new(&entry.payload[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| HydrationError::DecompressionFailed(e.to_string()))?;
            String::from_utf8(decompressed).map_err(|_| HydrationError::InvalidUtf8)?
        } else {
            String::from_utf8(entry.payload).map_err(|_| HydrationError::InvalidUtf8)?
        };

        Ok((text, entry.token_ids))
    }

    /// Backwards-compatible text-only hydration
    pub fn hydrate(&self, step_id: usize) -> Option<String> {
        self.hydrate_entry(step_id).ok().map(|(text, _)| text)
    }

    pub fn remove(&self, step_id: usize) -> bool {
        self.chunks.write().remove(&step_id).is_some()
    }

    pub fn total_stored_bytes(&self) -> usize {
        self.chunks
            .read()
            .values()
            .map(|v| v.payload.len() + v.token_ids.as_ref().map_or(0, |t| t.len() * 4))
            .sum()
    }
}

