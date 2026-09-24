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
    pub token_ids: Vec<i32>,
}

/// In-memory byte store for evicted step artifacts during agent context management.
///
/// NOTE on compression performance:
/// For small conversational strings (< 100 bytes / ~25 tokens), DEFLATE compression provides
/// negligible byte savings or may slightly expand the payload due to header overhead.
/// Compression is primarily beneficial for large tool payloads, compiler logs, and source files.
pub struct CompressedCacheStore {
    chunks: RwLock<HashMap<usize, StoreEntry>>,
}

impl CompressedCacheStore {
    pub fn new() -> Self {
        Self {
            chunks: RwLock::new(HashMap::new()),
        }
    }

    /// Stores text and optional token IDs with automatic DEFLATE compression.
    /// If DEFLATE does not reduce payload size, stores raw bytes with `is_compressed = false`.
    pub fn compress_and_store_tokens(&self, step_id: usize, text: &str, token_ids: &[i32]) -> usize {
        let raw_bytes = text.as_bytes();
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        let (is_compressed, payload) = match encoder.write_all(raw_bytes).and_then(|_| encoder.finish()) {
            Ok(compressed) if compressed.len() < raw_bytes.len() => (true, compressed),
            _ => (false, raw_bytes.to_vec()),
        };

        let stored_len = payload.len();
        let entry = StoreEntry {
            is_compressed,
            payload,
            token_ids: token_ids.to_vec(),
        };

        self.chunks.write().insert(step_id, entry);
        stored_len
    }

    /// Backwards-compatible text-only compression
    pub fn compress_and_store(&self, step_id: usize, text: &str) -> usize {
        self.compress_and_store_tokens(step_id, text, &[])
    }

    /// Decompresses and restores original UTF-8 payload and token IDs.
    /// Clones the entry inside the read lock and performs decompression OUTSIDE the lock
    /// to prevent lock contention across concurrent workers.
    pub fn hydrate_entry(&self, step_id: usize) -> Result<(String, Vec<i32>), HydrationError> {
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
            .map(|v| v.payload.len() + v.token_ids.len() * 4)
            .sum()
    }
}

