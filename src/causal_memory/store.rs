use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::{Read, Write};

/// High-speed compressed in-memory store for evicted text chunks during context hydration.
/// Utilizes genuine DEFLATE compression to physically minimize byte footprint in RAM.
pub struct CompressedCacheStore {
    chunks: RwLock<HashMap<usize, Vec<u8>>>,
}

impl CompressedCacheStore {
    pub fn new() -> Self {
        Self {
            chunks: RwLock::new(HashMap::new()),
        }
    }

    /// Genuine DEFLATE compression of evicted text into bytes buffer
    pub fn compress_and_store(&self, step_id: usize, text: &str) -> usize {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        let _ = encoder.write_all(text.as_bytes());
        let compressed = encoder.finish().unwrap_or_else(|_| text.as_bytes().to_vec());
        let size = compressed.len();
        self.chunks.write().insert(step_id, compressed);
        size
    }

    /// Reactive Hydration: decompress and restore original UTF-8 payload on demand
    pub fn hydrate(&self, step_id: usize) -> Option<String> {
        let map = self.chunks.read();
        let bytes = map.get(&step_id)?;
        let mut decoder = DeflateDecoder::new(&bytes[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).ok()?;
        Some(decompressed)
    }

    pub fn remove(&self, step_id: usize) -> bool {
        self.chunks.write().remove(&step_id).is_some()
    }

    pub fn total_stored_bytes(&self) -> usize {
        self.chunks.read().values().map(|v| v.len()).sum()
    }
}

