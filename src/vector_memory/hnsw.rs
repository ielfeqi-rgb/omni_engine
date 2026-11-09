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

//! High-Performance In-Memory Hierarchical Navigable Small World (HNSW) Vector Index.
//!
//! Implements Malkov & Yashunin (2018) multi-layer proximity graph:
//! - Multi-layer graph traversal with O(log N) search complexity.
//! - Cosine distance metric with SIMD auto-vectorization over normalized f32 vectors.
//! - Thread-safe multi-reader single-writer synchronization via parking_lot::RwLock.
//! - Disk persistence via JSON / Binary serialization.

use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub enum HnswError {
    DimensionMismatch { expected: usize, actual: usize },
    NodeNotFound(usize),
    EmptyIndex,
    IoError(std::io::Error),
    SerializationError(serde_json::Error),
}

impl std::fmt::Display for HnswError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DimensionMismatch { expected, actual } => {
                write!(f, "Dimension mismatch: expected {}, got {}", expected, actual)
            }
            Self::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            Self::EmptyIndex => write!(f, "Index is empty"),
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::SerializationError(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl std::error::Error for HnswError {}

impl From<std::io::Error> for HnswError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<serde_json::Error> for HnswError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationError(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswNode {
    pub id: usize,
    pub vector: Vec<f32>,
    pub level: usize,
    pub document_id: String,
    pub chunk_text: String,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: usize,
    pub score: f32, // Cosine similarity (1.0 = identical, -1.0 = opposite)
    pub document_id: String,
    pub chunk_text: String,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    pub dimension: usize,
    pub m: usize,              // Max neighbors per node on layers > 0
    pub m0: usize,             // Max neighbors per node on layer 0
    pub ef_construction: usize, // Size of dynamic candidate list during build
    pub ef_search: usize,       // Size of dynamic candidate list during query
    pub ml: f64,               // Normalization factor for level generation (1 / ln(M))
}

impl Default for HnswConfig {
    fn default() -> Self {
        let m = 16;
        Self {
            dimension: 384,
            m,
            m0: m * 2,
            ef_construction: 64,
            ef_search: 32,
            ml: 1.0 / (m as f64).ln(),
        }
    }
}

// Internal candidate for priority queues
#[derive(Debug, Clone, Copy)]
struct Candidate {
    id: usize,
    distance: f32, // Lower distance = closer
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.id == other.id
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// For min-heap / max-heap behavior
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap by default
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HnswIndexData {
    config: HnswConfig,
    nodes: Vec<HnswNode>,
    // adjacency[layer][node_id] = list of neighbor node_ids
    layers: Vec<HashMap<usize, Vec<usize>>>,
    enter_point: Option<usize>,
    max_level: usize,
}

#[derive(Debug, Clone)]
pub struct HnswIndex {
    inner: Arc<RwLock<HnswIndexData>>,
}

impl HnswIndex {
    pub fn new(config: HnswConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HnswIndexData {
                config,
                nodes: Vec::new(),
                layers: Vec::new(),
                enter_point: None,
                max_level: 0,
            })),
        }
    }

    /// Calculate Cosine Distance between two normalized vectors:
    /// Distance = 1.0 - CosineSimilarity (Ranges from 0.0 to 2.0)
    pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for (x, y) in a.iter().zip(b.iter()) {
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        if norm_a <= 0.0 || norm_b <= 0.0 {
            return 1.0;
        }

        let similarity = dot / (norm_a.sqrt() * norm_b.sqrt());
        // Clamp to [-1.0, 1.0] against precision drift
        let clamped = similarity.clamp(-1.0, 1.0);
        1.0 - clamped
    }

    fn sample_level<R: Rng>(rng: &mut R, ml: f64) -> usize {
        let unif: f64 = rng.gen_range(0.0000001..1.0);
        (-unif.ln() * ml).floor() as usize
    }

    /// Inserts a document chunk and its dense vector into the HNSW graph
    pub fn insert(
        &self,
        vector: Vec<f32>,
        document_id: String,
        chunk_text: String,
        file_path: String,
        line_start: usize,
        line_end: usize,
        metadata: serde_json::Value,
    ) -> Result<usize, HnswError> {
        let mut data = self.inner.write();

        if vector.len() != data.config.dimension {
            return Err(HnswError::DimensionMismatch {
                expected: data.config.dimension,
                actual: vector.len(),
            });
        }

        let new_id = data.nodes.len();
        let mut rng = rand::thread_rng();
        let node_level = Self::sample_level(&mut rng, data.config.ml);

        // Ensure layers exist up to max(node_level, max_level)
        let target_layers = node_level.max(data.max_level) + 1;
        while data.layers.len() < target_layers {
            data.layers.push(HashMap::new());
        }

        let new_node = HnswNode {
            id: new_id,
            vector,
            level: node_level,
            document_id,
            chunk_text,
            file_path,
            line_start,
            line_end,
            metadata,
        };
        data.nodes.push(new_node);

        let curr_enter_point = match data.enter_point {
            Some(ep) => ep,
            None => {
                // First node in the graph
                data.enter_point = Some(new_id);
                data.max_level = node_level;
                for l in 0..=node_level {
                    let _ = data.layers[l].insert(new_id, Vec::new());
                }
                return Ok(new_id);
            }
        };

        let mut curr_obj = curr_obj_search(&data, curr_enter_point, &data.nodes[new_id].vector, node_level);

        // From node_level down to 0, connect neighbors
        for l in (0..=node_level.min(data.max_level)).rev() {
            let m_max = if l == 0 { data.config.m0 } else { data.config.m };
            let candidates = search_layer(&data, curr_obj, &data.nodes[new_id].vector, data.config.ef_construction, l);
            
            // Select M neighbors
            let neighbors: Vec<usize> = candidates.iter().take(m_max).map(|c| c.id).collect();

            let _ = data.layers[l].insert(new_id, neighbors.clone());

            // Add reverse edges
            for &n_id in &neighbors {
                let entry = data.layers[l].entry(n_id).or_default();
                if !entry.contains(&new_id) {
                    entry.push(new_id);
                }
            }
            
            // Prune if exceeded m_max
            for &n_id in &neighbors {
                let needs_prune = data.layers[l].get(&n_id).map(|e| e.len() > m_max).unwrap_or(false);
                if needs_prune {
                    let mut sorted_neighbors = data.layers[l][&n_id].clone();
                    let origin_vec = data.nodes[n_id].vector.clone();
                    sorted_neighbors.sort_by(|&a, &b| {
                        let da = HnswIndex::cosine_distance(&origin_vec, &data.nodes[a].vector);
                        let db = HnswIndex::cosine_distance(&origin_vec, &data.nodes[b].vector);
                        da.partial_cmp(&db).unwrap_or(Ordering::Equal)
                    });
                    sorted_neighbors.truncate(m_max);
                    if let Some(entry) = data.layers[l].get_mut(&n_id) {
                        *entry = sorted_neighbors;
                    }
                }
            }

            if !candidates.is_empty() {
                curr_obj = candidates[0].id;
            }
        }

        if node_level > data.max_level {
            data.max_level = node_level;
            data.enter_point = Some(new_id);
        }

        Ok(new_id)
    }

    /// Searches the HNSW graph for the Top-K nearest neighbors
    pub fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<SearchResult>, HnswError> {
        let data = self.inner.read();

        if query.len() != data.config.dimension {
            return Err(HnswError::DimensionMismatch {
                expected: data.config.dimension,
                actual: query.len(),
            });
        }

        let enter_point = match data.enter_point {
            Some(ep) => ep,
            None => return Err(HnswError::EmptyIndex),
        };

        // Greedy traversal from top level down to level 1
        let mut curr_obj = enter_point;
        let mut curr_dist = Self::cosine_distance(query, &data.nodes[curr_obj].vector);

        for l in (1..=data.max_level).rev() {
            let mut changed = true;
            while changed {
                changed = false;
                if let Some(neighbors) = data.layers[l].get(&curr_obj) {
                    for &neighbor in neighbors {
                        let d = Self::cosine_distance(query, &data.nodes[neighbor].vector);
                        if d < curr_dist {
                            curr_dist = d;
                            curr_obj = neighbor;
                            changed = true;
                        }
                    }
                }
            }
        }

        // Layer 0 beam search with ef_search
        let ef = data.config.ef_search.max(top_k);
        let candidates = search_layer(&data, curr_obj, query, ef, 0);

        let mut results = Vec::with_capacity(top_k.min(candidates.len()));
        for c in candidates.iter().take(top_k) {
            let node = &data.nodes[c.id];
            let similarity = 1.0 - c.distance; // Back to cosine similarity
            results.push(SearchResult {
                id: node.id,
                score: similarity,
                document_id: node.document_id.clone(),
                chunk_text: node.chunk_text.clone(),
                file_path: node.file_path.clone(),
                line_start: node.line_start,
                line_end: node.line_end,
                metadata: node.metadata.clone(),
            });
        }

        Ok(results)
    }

    /// Total count of indexed chunks
    pub fn len(&self) -> usize {
        self.inner.read().nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.read().nodes.is_empty()
    }

    /// Persists index to a binary JSON file
    pub fn save_to_file(&self, path: &Path) -> Result<(), HnswError> {
        let data = self.inner.read();
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &*data)?;
        Ok(())
    }

    /// Loads index from a persisted JSON file
    pub fn load_from_file(path: &Path) -> Result<Self, HnswError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let data: HnswIndexData = serde_json::from_reader(reader)?;
        Ok(Self {
            inner: Arc::new(RwLock::new(data)),
        })
    }
}

// --- Helper Functions ---

fn curr_obj_search(data: &HnswIndexData, start_ep: usize, query: &[f32], target_level: usize) -> usize {
    let mut curr_obj = start_ep;
    let mut curr_dist = HnswIndex::cosine_distance(query, &data.nodes[curr_obj].vector);

    for l in (target_level + 1..=data.max_level).rev() {
        let mut changed = true;
        while changed {
            changed = false;
            if let Some(neighbors) = data.layers[l].get(&curr_obj) {
                for &neighbor in neighbors {
                    let d = HnswIndex::cosine_distance(query, &data.nodes[neighbor].vector);
                    if d < curr_dist {
                        curr_dist = d;
                        curr_obj = neighbor;
                        changed = true;
                    }
                }
            }
        }
    }
    curr_obj
}

fn search_layer(
    data: &HnswIndexData,
    enter_point: usize,
    query: &[f32],
    ef: usize,
    level: usize,
) -> Vec<Candidate> {
    let mut visited = HashSet::new();
    let mut candidates_min_heap = BinaryHeap::new(); // Explores closest candidates
    let mut results_max_heap = BinaryHeap::new();    // Keeps `ef` closest (furthest on top for eviction)

    let initial_dist = HnswIndex::cosine_distance(query, &data.nodes[enter_point].vector);
    let _ = visited.insert(enter_point);

    candidates_min_heap.push(Candidate { id: enter_point, distance: initial_dist });
    results_max_heap.push(ReverseCandidate { id: enter_point, distance: initial_dist });

    while let Some(current) = candidates_min_heap.pop() {
        // If current candidate is further than the furthest in results, stop early
        if let Some(furthest) = results_max_heap.peek() {
            if current.distance > furthest.distance {
                break;
            }
        }

        if let Some(neighbors) = data.layers[level].get(&current.id) {
            for &n_id in neighbors {
                if visited.insert(n_id) {
                    let d = HnswIndex::cosine_distance(query, &data.nodes[n_id].vector);
                    let should_insert = match results_max_heap.peek() {
                        Some(furthest) => d < furthest.distance || results_max_heap.len() < ef,
                        None => true,
                    };

                    if should_insert {
                        candidates_min_heap.push(Candidate { id: n_id, distance: d });
                        results_max_heap.push(ReverseCandidate { id: n_id, distance: d });

                        if results_max_heap.len() > ef {
                            let _ = results_max_heap.pop();
                        }
                    }
                }
            }
        }
    }

    // Convert max heap to sorted list of closest candidates
    let mut sorted_results: Vec<Candidate> = results_max_heap
        .into_iter()
        .map(|rc| Candidate { id: rc.id, distance: rc.distance })
        .collect();
    sorted_results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
    sorted_results
}


// Wrapper for max-heap where highest distance is on top
#[derive(Debug, Clone, Copy, PartialEq)]
struct ReverseCandidate {
    id: usize,
    distance: f32,
}

impl Eq for ReverseCandidate {}

impl PartialOrd for ReverseCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReverseCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance.partial_cmp(&other.distance).unwrap_or(Ordering::Equal)
    }
}
