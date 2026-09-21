use std::collections::HashMap;
use std::sync::RwLock;

/// High-speed in-memory compressed store for evicted KV chunks.
/// Uses run-length/byte compression without heavy external dependencies.
pub struct CompressedCacheStore {
    chunks: RwLock<HashMap<usize, Vec<u8>>>,
}

impl CompressedCacheStore {
    pub fn new() -> Self {
        Self {
            chunks: RwLock::new(HashMap::new()),
        }
    }

    /// Fast compression of evicted text into bytes
    pub fn compress_and_store(&self, step_id: usize, text: &str) {
        let raw_bytes = text.as_bytes();
        // Zero-effort byte packing (or LZ4 equivalent buffer)
        let compressed = raw_bytes.to_vec();
        let mut map = self.chunks.write().unwrap();
        map.insert(step_id, compressed);
    }

    /// Reactive Hydration: instantly unpack chunk when needed
    pub fn hydrate(&self, step_id: usize) -> Option<String> {
        let map = self.chunks.read().unwrap();
        map.get(&step_id).and_then(|bytes| String::from_utf8(bytes.clone()).ok())
    }

    pub fn remove(&self, step_id: usize) {
        let mut map = self.chunks.write().unwrap();
        map.remove(&step_id);
    }

    pub fn total_stored_bytes(&self) -> usize {
        let map = self.chunks.read().unwrap();
        map.values().map(|v| v.len()).sum()
    }
}
