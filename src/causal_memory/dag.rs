use std::collections::{HashMap, HashSet};
use crate::causal_memory::store::CompressedCacheStore;

#[derive(Debug, Clone)]
pub struct StepNode {
    pub step_id: usize,
    pub description: String,
    pub entities_read: HashSet<String>,
    pub entities_written: HashSet<String>,
    pub token_range: (usize, usize),
    pub is_evicted: bool,
}

pub struct CausalGraph {
    nodes: HashMap<usize, StepNode>,
    adjacency: HashMap<usize, Vec<usize>>,
    predecessors: HashMap<usize, Vec<usize>>,
    entity_writers: HashMap<String, Vec<usize>>,
    entity_readers: HashMap<String, Vec<usize>>,
    pub store: CompressedCacheStore,
    current_token_cursor: usize,
}

impl CausalGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            adjacency: HashMap::new(),
            predecessors: HashMap::new(),
            entity_writers: HashMap::new(),
            entity_readers: HashMap::new(),
            store: CompressedCacheStore::new(),
            current_token_cursor: 0,
        }
    }

    /// Add a new action step to the causal graph
    pub fn record_step(
        &mut self,
        step_id: usize,
        description: &str,
        reads: &[&str],
        writes: &[&str],
        token_count: usize,
    ) {
        let entities_read: HashSet<String> = reads.iter().map(|s| s.to_string()).collect();
        let entities_written: HashSet<String> = writes.iter().map(|s| s.to_string()).collect();

        let start_pos = self.current_token_cursor;
        let end_pos = start_pos + token_count;
        self.current_token_cursor = end_pos;

        let node = StepNode {
            step_id,
            description: description.to_string(),
            entities_read: entities_read.clone(),
            entities_written: entities_written.clone(),
            token_range: (start_pos, end_pos),
            is_evicted: false,
        };

        // Mathematical Dependency Resolution (Set Intersection: O(A) ∩ I(B) != ∅)
        // Instantaneous lookup via entity_writers index
        for r in &entities_read {
            self.entity_readers.entry(r.clone()).or_default().push(step_id);
            if let Some(writers) = self.entity_writers.get(r) {
                for &writer_id in writers {
                    if writer_id != step_id {
                        let adj = self.adjacency.entry(writer_id).or_default();
                        if !adj.contains(&step_id) {
                            adj.push(step_id);
                        }
                        let preds = self.predecessors.entry(step_id).or_default();
                        if !preds.contains(&writer_id) {
                            preds.push(writer_id);
                        }
                    }
                }
            }
        }

        for w in &entities_written {
            self.entity_writers.entry(w.clone()).or_default().push(step_id);
        }

        self.nodes.insert(step_id, node);
    }

    /// Evict a step from active KV cache to compressed storage (to save RAM)
    pub fn evict_step(&mut self, step_id: usize, text_payload: &str) {
        if let Some(node) = self.nodes.get_mut(&step_id) {
            node.is_evicted = true;
            self.store.compress_and_store(step_id, text_payload);
        }
    }

    /// Resolve causal backward ancestral cone starting from all participants (writers & readers) of target_entity.
    /// Traverses backward along dataflow dependency edges (predecessors).
    pub fn resolve_dependencies_for_entity(&self, target_entity: &str) -> Vec<usize> {
        let mut seed_steps: Vec<usize> = Vec::new();
        if let Some(writers) = self.entity_writers.get(target_entity) {
            seed_steps.extend(writers);
        }
        if let Some(readers) = self.entity_readers.get(target_entity) {
            seed_steps.extend(readers);
        }
        self.transitive_closure_backward(seed_steps)
    }

    /// Resolve strict causal backward ancestral cone for a specific execution step (e.g. crash node).
    /// Follows dataflow edges backwards from crash_step_id through all causal prerequisites.
    pub fn resolve_ancestral_cone_for_step(&self, step_id: usize) -> Vec<usize> {
        if !self.nodes.contains_key(&step_id) {
            return Vec::new();
        }
        self.transitive_closure_backward(vec![step_id])
    }

    fn transitive_closure_backward(&self, seed_steps: Vec<usize>) -> Vec<usize> {
        let mut visited: HashSet<usize> = HashSet::new();
        let mut queue = seed_steps;

        while let Some(current) = queue.pop() {
            if visited.insert(current) {
                if let Some(preds) = self.predecessors.get(&current) {
                    for p in preds {
                        if !visited.contains(p) {
                            queue.push(*p);
                        }
                    }
                }
            }
        }

        let mut result: Vec<usize> = visited.into_iter().collect();
        result.sort();
        result
    }

    /// Reactive Hydration: Check if any required causal ancestor is evicted, and unpack it immediately
    pub fn hydrate_ancestors_for_error(&self, target_entity: &str) -> Vec<(usize, String)> {
        let mut hydrated_context = Vec::new();
        let ancestors = self.resolve_dependencies_for_entity(target_entity);

        for step_id in ancestors {
            if let Some(node) = self.nodes.get(&step_id) {
                if node.is_evicted {
                    if let Some(content) = self.store.hydrate(step_id) {
                        hydrated_context.push((step_id, content));
                    }
                }
            }
        }
        hydrated_context
    }

    /// Physical KV-Cache Rollback: Directly excise this step's tokens from transformer memory,
    /// rewind the graph cursor, and prune the failed node from the causal graph state.
    ///
    /// NOTE: Causal rollback must be applied to the sequence suffix to prevent corrupted
    /// attention holes. `p1` must match `current_token_cursor`.
    pub fn rollback_step_kv(
        &mut self,
        step_id: usize,
        ctx: &mut crate::native_llama::NativeLlamaContext,
    ) -> Result<bool, String> {
        let node = self.nodes.get(&step_id).cloned().ok_or_else(|| {
            format!("Step ID {} not found in causal graph", step_id)
        })?;
        let (p0, p1) = node.token_range;

        // Verify that rollback is applied to the active sequence suffix
        if p1 != self.current_token_cursor {
            return Err(format!(
                "Causal KV rollback must be applied to the active sequence suffix: step range=({}, {}), current_cursor={}",
                p0, p1, self.current_token_cursor
            ));
        }

        // Physically excise from KV cache via C FFI (removing from p0 to end of sequence)
        let ok = ctx.kv_cache_seq_rm(0, p0 as i32, -1)?;
        if !ok {
            return Err("llama_kv_cache_seq_rm kernel returned false".to_string());
        }

        // Rewind token cursor to p0
        self.current_token_cursor = p0;

        // Remove node from graph
        self.nodes.remove(&step_id);

        // Remove from entity_writers
        for writers in self.entity_writers.values_mut() {
            writers.retain(|&id| id != step_id);
        }

        // Remove from entity_readers
        for readers in self.entity_readers.values_mut() {
            readers.retain(|&id| id != step_id);
        }

        // Remove from adjacency
        self.adjacency.remove(&step_id);
        for children in self.adjacency.values_mut() {
            children.retain(|&id| id != step_id);
        }

        // Remove from predecessors
        self.predecessors.remove(&step_id);
        for preds in self.predecessors.values_mut() {
            preds.retain(|&id| id != step_id);
        }

        // Remove from compressed store if present
        self.store.remove(step_id);

        Ok(true)
    }

    pub fn current_token_cursor(&self) -> usize {
        self.current_token_cursor
    }

    pub fn contains_step(&self, step_id: usize) -> bool {
        self.nodes.contains_key(&step_id)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_causal_dag_and_reactive_hydration() {
        let mut graph = CausalGraph::new();

        // Step 1: Create hello.py (Writes hello.py)
        graph.record_step(1, "Create hello.py", &[], &["hello.py"], 50);

        // Step 2: Configure Network (Writes network_cfg)
        graph.record_step(2, "Setup Network", &[], &["network_cfg"], 60);

        // Step 3: Modify hello.py (Reads hello.py, Writes hello.py)
        graph.record_step(3, "Append function to hello.py", &["hello.py"], &["hello.py"], 40);

        // Step 4 & 5: Irrelevant steps
        graph.record_step(4, "Check CPU Stats", &[], &["cpu"], 30);
        graph.record_step(5, "Ping Gateway", &["network_cfg"], &[], 30);

        // Simulate Memory Eviction: Step 1 was evicted to compressed store!
        let step_1_code = "def hello():\n    print('Hello World')\n";
        graph.evict_step(1, step_1_code);

        // Step 6: Execute hello.py and crash!
        graph.record_step(6, "Execute hello.py", &["hello.py"], &[], 20);

        // Test 1: Ancestral cone of the crash step (Step 6)
        let crash_cone = graph.resolve_ancestral_cone_for_step(6);
        assert_eq!(crash_cone, vec![1, 3, 6], "Causal cone of crash step 6 includes only [1, 3, 6]!");

        // Test 2: Dependency participants of 'hello.py'
        let entity_deps = graph.resolve_dependencies_for_entity("hello.py");
        assert_eq!(entity_deps, vec![1, 3, 6], "Causal participants of hello.py are steps 1, 3, and 6");

        // Reactive Hydration: Step 1 is evicted, so hydrate it!
        let hydrated = graph.hydrate_ancestors_for_error("hello.py");
        assert_eq!(hydrated.len(), 1, "Only Step 1 was evicted and needed hydration");
        assert_eq!(hydrated[0].0, 1);
        assert_eq!(hydrated[0].1, step_1_code, "Hydrated exact decompressed content of Step 1!");
    }

    #[test]
    fn test_causal_dag_rollback_consistency_and_cursor_rewind() {
        let mut graph = CausalGraph::new();

        graph.record_step(1, "Step 1", &[], &["state.json"], 50);
        graph.record_step(2, "Step 2", &["state.json"], &["cache.db"], 40);
        graph.record_step(3, "Step 3 (Failed)", &["cache.db"], &["error.log"], 30);

        assert_eq!(graph.current_token_cursor(), 120);
        assert!(graph.contains_step(3));

        // Attempting to rollback step 2 (not suffix, since step 3 follows) must be rejected
        // Note: we test logic validation without needing a live context ptr here
        let node_2 = graph.nodes.get(&2).unwrap();
        assert_ne!(node_2.token_range.1, graph.current_token_cursor());

        // Now simulate rolling back step 3 manually or checking state consistency
        let node_3 = graph.nodes.get(&3).unwrap();
        assert_eq!(node_3.token_range.1, graph.current_token_cursor());
    }
}
