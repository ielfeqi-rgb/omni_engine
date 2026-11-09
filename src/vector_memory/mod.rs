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

//! Sovereign Vector Memory and HNSW Codebase Atlas.
//!
//! Provides in-memory hierarchical proximity search across multi-file codebases,
//! documentation, and agent interaction histories:
//! - Sub-millisecond vector similarity search using pure Rust HNSW graphs.
//! - Native dense subword semantic embedding projection (384-dimensional).
//! - Semantic codebase chunking preserving functions, structs, and line ranges.
//! - Context assembly pipeline for zero-hallucination agent execution.

pub mod code_chunker;
pub mod embeddings;
pub mod hnsw;

pub use code_chunker::{CodeChunk, CodeChunker};
pub use embeddings::{EmbeddingEngine, EmbeddingError};
pub use hnsw::{HnswConfig, HnswError, HnswIndex, SearchResult};

use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug)]
pub enum VectorError {
    Hnsw(HnswError),
    Embedding(EmbeddingError),
    Io(std::io::Error),
}

impl std::fmt::Display for VectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hnsw(e) => write!(f, "HNSW graph error: {}", e),
            Self::Embedding(e) => write!(f, "Embedding engine error: {}", e),
            Self::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for VectorError {}

impl From<HnswError> for VectorError {
    fn from(e: HnswError) -> Self {
        Self::Hnsw(e)
    }
}

impl From<EmbeddingError> for VectorError {
    fn from(e: EmbeddingError) -> Self {
        Self::Embedding(e)
    }
}

impl From<std::io::Error> for VectorError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Clone)]
pub struct VectorMemoryController {
    index: HnswIndex,
    embedder: Arc<EmbeddingEngine>,
    dimension: usize,
}

impl Default for VectorMemoryController {
    fn default() -> Self {
        Self::new(384)
    }
}

impl VectorMemoryController {
    pub fn new(dimension: usize) -> Self {
        let config = HnswConfig {
            dimension,
            m: 16,
            m0: 32,
            ef_construction: 64,
            ef_search: 32,
            ml: 1.0 / (16.0f64).ln(),
        };

        Self {
            index: HnswIndex::new(config),
            embedder: Arc::new(EmbeddingEngine::new(dimension)),
            dimension,
        }
    }

    pub fn with_embedding_server(dimension: usize, server_url: String) -> Self {
        let config = HnswConfig {
            dimension,
            m: 16,
            m0: 32,
            ef_construction: 64,
            ef_search: 32,
            ml: 1.0 / (16.0f64).ln(),
        };

        Self {
            index: HnswIndex::new(config),
            embedder: Arc::new(EmbeddingEngine::new(dimension).with_server_url(server_url)),
            dimension,
        }
    }

    /// Recursively indexes all matching code and document files in a directory
    pub fn index_directory(&self, root_dir: &Path, extensions: &[&str]) -> Result<usize, VectorError> {
        let mut files_to_index = Vec::new();
        collect_files_recursive(root_dir, extensions, &mut files_to_index)?;

        let mut total_chunks = 0;
        for file_path in files_to_index {
            match CodeChunker::chunk_file(&file_path) {
                Ok(chunks) => {
                    for chunk in chunks {
                        let vec = self.embedder.embed_sync(&chunk.chunk_text);
                        let meta = json!({
                            "symbol": chunk.symbol_name,
                            "file": chunk.file_path,
                            "line_start": chunk.line_start,
                            "line_end": chunk.line_end,
                        });

                        self.index.insert(
                            vec,
                            format!("{}:L{}", chunk.file_path, chunk.line_start),
                            chunk.chunk_text,
                            chunk.file_path,
                            chunk.line_start,
                            chunk.line_end,
                            meta,
                        )?;
                        total_chunks += 1;
                    }
                }
                Err(e) => {
                    warn!("Skipping file {:?}: {}", file_path, e);
                }
            }
        }

        info!("Indexed {} semantic code chunks into HNSW Atlas", total_chunks);
        Ok(total_chunks)
    }

    /// Indexes a single code or text chunk into memory
    pub fn index_chunk(
        &self,
        document_id: String,
        chunk_text: String,
        file_path: String,
        line_start: usize,
        line_end: usize,
    ) -> Result<usize, VectorError> {
        let vec = self.embedder.embed_sync(&chunk_text);
        let meta = json!({
            "file": file_path,
            "line_start": line_start,
            "line_end": line_end,
        });

        let id = self.index.insert(vec, document_id, chunk_text, file_path, line_start, line_end, meta)?;
        Ok(id)
    }

    /// Performs an instantaneous O(log N) semantic search over the codebase
    pub fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>, VectorError> {
        if self.index.is_empty() {
            return Ok(Vec::new());
        }

        let q_vec = self.embedder.embed_sync(query);
        let results = self.index.search(&q_vec, top_k)?;
        Ok(results)
    }

    /// Formats retrieved search results into a clean, injection-ready context block
    pub fn format_rag_context(&self, results: &[SearchResult]) -> String {
        if results.is_empty() {
            return String::new();
        }

        let mut out = String::from("### [RELEVANT CODEBASE CONTEXT (HNSW ATLAS)]\n");
        for (i, res) in results.iter().enumerate() {
            out.push_str(&format!(
                "--- [{}] File: {} (Lines {}-{}) | Score: {:.3} ---\n{}\n\n",
                i + 1,
                res.file_path,
                res.line_start,
                res.line_end,
                res.score,
                res.chunk_text
            ));
        }
        out
    }

    pub fn total_chunks(&self) -> usize {
        self.index.len()
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), VectorError> {
        self.index.save_to_file(path)?;
        Ok(())
    }

    pub fn load_from_file(&mut self, path: &Path) -> Result<(), VectorError> {
        self.index = HnswIndex::load_from_file(path)?;
        Ok(())
    }
}

fn collect_files_recursive(
    dir: &Path,
    extensions: &[&str],
    out: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    if !dir.exists() || !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Skip target, .git, and trash
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name == "target" || name == ".git" || name == "trash" || name == "node_modules" {
                    continue;
                }
            }
            collect_files_recursive(&path, extensions, out)?;
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    out.push(path);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_codebase_indexing_and_search() {
        let controller = VectorMemoryController::new(384);

        // Index 3 distinct code snippets
        let res1 = controller.index_chunk(
            "auth.rs:L10".to_string(),
            "pub fn verify_api_key(key: &str) -> bool { key.starts_with(\"sk-omni\") }".to_string(),
            "src/auth.rs".to_string(),
            10,
            12,
        );
        assert!(res1.is_ok());

        let res2 = controller.index_chunk(
            "physics.rs:L40".to_string(),
            "pub fn calculate_entropy(temperature: f64) -> f64 { 1.38e-23 * temperature.ln() }".to_string(),
            "src/physics.rs".to_string(),
            40,
            42,
        );
        assert!(res2.is_ok());

        let res3 = controller.index_chunk(
            "market.rs:L100".to_string(),
            "pub fn process_order(item_id: u64, amount: u32) -> Result<Invoice, MarketError>".to_string(),
            "src/market.rs".to_string(),
            100,
            102,
        );
        assert!(res3.is_ok());

        assert_eq!(controller.total_chunks(), 3);

        // Search for Authentication
        let search_results = controller.search("How to verify user API key and credentials?", 1).unwrap();
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].file_path, "src/auth.rs");
        assert!(search_results[0].score > 0.0);

        // Search for Physics/Entropy
        let physics_results = controller.search("Thermodynamics entropy calculation", 1).unwrap();
        assert_eq!(physics_results.len(), 1);
        assert_eq!(physics_results[0].file_path, "src/physics.rs");

        // Format RAG context
        let context = controller.format_rag_context(&search_results);
        assert!(context.contains("auth.rs"));
        assert!(context.contains("verify_api_key"));
    }
}
