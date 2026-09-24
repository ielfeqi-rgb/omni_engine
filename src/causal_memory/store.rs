use parking_lot::RwLock;
use std::collections::HashMap;

/// High-speed in-memory store for evicted text chunks during context hydration.
pub struct CompressedCacheStore {
    chunks: RwLock<HashMap<usize, Vec<u8>>>,
}

impl CompressedCacheStore {
    pub fn new() -> Self {
        Self {
            chunks: RwLock::new(HashMap::new()),
        }
    }

    /// Fast storage of evicted text into bytes buffer
    pub fn compress_and_store(&self, step_id: usize, text: &str) {
        let compressed = text.as_bytes().to_vec();
        let mut map = self.chunks.write();
        map.insert(step_id, compressed);
    }

    /// Reactive Hydration: instantly unpack chunk when needed
    pub fn hydrate(&self, step_id: usize) -> Option<String> {
        let map = self.chunks.read();
        map.get(&step_id).and_then(|bytes| String::from_utf8(bytes.clone()).ok())
    }

    pub fn remove(&self, step_id: usize) {
        let mut map = self.chunks.write();
        map.remove(&step_id);
    }

    pub fn total_stored_bytes(&self) -> usize {
        let map = self.chunks.read();
        map.values().map(|v| v.len()).sum()
    }
}

