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
    /// Records previous reaching writer for written entities to allow O(1) state restoration on rollback
    pub prior_writers: HashMap<String, Option<usize>>,
}

/// Automated entity extractor for runtime error tracebacks
pub struct EntityExtractor;

impl EntityExtractor {
    /// Extracts entity names from standard Python / system runtime errors
    pub fn extract_from_error(error_msg: &str) -> Vec<String> {
        let mut entities = Vec::new();
        // KeyError: 'key' or KeyError: "key"
        if let Some(idx) = error_msg.find("KeyError:") {
            let rest = &error_msg[idx + 9..];
            if let Some(q1) = rest.find(['\'', '"']) {
                let quote = rest.as_bytes()[q1] as char;
                if let Some(q2) = rest[q1 + 1..].find(quote) {
                    entities.push(rest[q1 + 1..q1 + 1 + q2].to_string());
                }
            }
        }
        // FileNotFoundError: ... 'path'
        if let Some(idx) = error_msg.find("FileNotFoundError:") {
            let rest = &error_msg[idx + 18..];
            if let Some(q1) = rest.find(['\'', '"']) {
                let quote = rest.as_bytes()[q1] as char;
                if let Some(q2) = rest[q1 + 1..].find(quote) {
                    entities.push(rest[q1 + 1..q1 + 1 + q2].to_string());
                }
            }
        }
        // NameError: name 'var' is not defined
        if let Some(idx) = error_msg.find("NameError: name ") {
            let rest = &error_msg[idx + 16..];
            if let Some(q1) = rest.find(['\'', '"']) {
                let quote = rest.as_bytes()[q1] as char;
                if let Some(q2) = rest[q1 + 1..].find(quote) {
                    entities.push(rest[q1 + 1..q1 + 1 + q2].to_string());
                }
            }
        }
        entities
    }
}

pub struct CausalGraph {
    nodes: HashMap<usize, StepNode>,
    adjacency: HashMap<usize, HashSet<usize>>,
    predecessors: HashMap<usize, HashSet<usize>>,
    entity_writers: HashMap<String, Vec<usize>>,
    entity_readers: HashMap<String, Vec<usize>>,
    /// Tracks reaching definitions: latest writer of each entity
    latest_writer: HashMap<String, usize>,
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
            latest_writer: HashMap::new(),
            store: CompressedCacheStore::new(),
            current_token_cursor: 0,
        }
    }

    /// Initializes graph with a pre-existing prefix token offset (e.g. system prompt)
    pub fn with_prefix_offset(offset: usize) -> Self {
        let mut graph = Self::new();
        graph.current_token_cursor = offset;
        graph
    }

    /// Add a new action step to the causal graph.
    /// Returns false if `step_id` already exists to prevent duplicate overwrite corruption.
    pub fn record_step(
        &mut self,
        step_id: usize,
        description: &str,
        reads: &[&str],
        writes: &[&str],
        token_count: usize,
    ) -> bool {
        if self.nodes.contains_key(&step_id) {
            return false;
        }

        let entities_read: HashSet<String> = reads.iter().map(|s| s.to_string()).collect();
        let entities_written: HashSet<String> = writes.iter().map(|s| s.to_string()).collect();

        let start_pos = self.current_token_cursor;
        let end_pos = start_pos + token_count;
        self.current_token_cursor = end_pos;

        let mut prior_writers = HashMap::new();

        // Reaching Definitions Dataflow Resolution:
        // A read depends on the MOST RECENT writer of that entity (preventing WAW cone explosion).
        for r in &entities_read {
            self.entity_readers.entry(r.clone()).or_default().push(step_id);
            if let Some(&writer_id) = self.latest_writer.get(r) {
                if writer_id != step_id {
                    self.adjacency.entry(writer_id).or_default().insert(step_id);
                    self.predecessors.entry(step_id).or_default().insert(writer_id);
                }
            }
        }

        for w in &entities_written {
            self.entity_writers.entry(w.clone()).or_default().push(step_id);
            prior_writers.insert(w.clone(), self.latest_writer.get(w).copied());
            self.latest_writer.insert(w.clone(), step_id);
        }

        let node = StepNode {
            step_id,
            description: description.to_string(),
            entities_read,
            entities_written,
            token_range: (start_pos, end_pos),
            is_evicted: false,
            prior_writers,
        };

        self.nodes.insert(step_id, node);
        true
    }

    /// Synchronizes step recording with live physical context telemetry
    pub fn record_step_with_context(
        &mut self,
        step_id: usize,
        description: &str,
        reads: &[&str],
        writes: &[&str],
        ctx: &crate::native_llama::NativeLlamaContext,
    ) -> bool {
        let current_phys = ctx.kv_cache_used_cells();
        let token_count = current_phys.saturating_sub(self.current_token_cursor);
        self.record_step(step_id, description, reads, writes, token_count)
    }

    /// Evict a step from active memory to compressed storage
    pub fn evict_step(&mut self, step_id: usize, text_payload: &str) -> bool {
        self.evict_step_with_context(step_id, text_payload, None, None)
    }

    /// Evict a step with optional physical suffix KV excision
    pub fn evict_step_with_context(
        &mut self,
        step_id: usize,
        text_payload: &str,
        token_ids: Option<&[i32]>,
        mut ctx: Option<&mut crate::native_llama::NativeLlamaContext>,
    ) -> bool {
        let (p0, p1) = match self.nodes.get(&step_id) {
            Some(n) => n.token_range,
            None => return false,
        };

        if let Some(node) = self.nodes.get_mut(&step_id) {
            node.is_evicted = true;
        }

        self.store.compress_and_store_tokens(step_id, text_payload, token_ids.unwrap_or(&[]));

        // If physical context provided and step is suffix, physically excise from KV cache
        if let Some(c) = ctx.as_mut() {
            if p1 == self.current_token_cursor {
                let _ = c.kv_cache_seq_rm(0, p0 as i32, -1);
                self.current_token_cursor = p0;
            }
        }
        true
    }

    /// Resolve backward ancestral cone starting from all participants (writers & readers) of target_entity.
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

    /// Reactive Hydration & Re-Prefill: Hydrates all evicted ancestors of crash_step_id,
    /// sets is_evicted = false, and optionally executes forward-pass re-prefill.
    pub fn hydrate_and_reprefill_ancestors(
        &mut self,
        crash_step_id: usize,
        mut ctx: Option<&mut crate::native_llama::NativeLlamaContext>,
        model: Option<&crate::native_llama::NativeLlamaModel>,
    ) -> Result<Vec<(usize, String)>, String> {
        let mut hydrated = Vec::new();
        let ancestors = self.resolve_ancestral_cone_for_step(crash_step_id);

        for step_id in ancestors {
            if let Some(node) = self.nodes.get_mut(&step_id) {
                if node.is_evicted {
                    let (text, tokens) = self
                        .store
                        .hydrate_entry(step_id)
                        .map_err(|e| format!("Hydration failed for step {}: {}", step_id, e))?;

                    if let (Some(c), Some(m)) = (ctx.as_mut(), model) {
                        let eval_toks = if !tokens.is_empty() {
                            tokens
                        } else {
                            m.tokenize(&text, false)?
                        };
                        let start_pos = self.current_token_cursor;
                        c.eval_tokens(&eval_toks, 0)?;
                        let end_pos = c.current_cursor();
                        self.current_token_cursor = end_pos;
                        node.token_range = (start_pos, end_pos);
                    }

                    node.is_evicted = false;
                    hydrated.push((step_id, text));
                }
            }
        }
        Ok(hydrated)
    }

    /// Backward-compatible query for entity dependencies
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

    /// Physical KV-Cache Suffix Rollback: Excises step's tokens from transformer memory,
    /// rewinds the cursor, restores reaching definitions in O(1) amortized time,
    /// and prunes the node from graph structures.
    pub fn rollback_step_kv(
        &mut self,
        step_id: usize,
        ctx: &mut crate::native_llama::NativeLlamaContext,
    ) -> Result<bool, String> {
        let node = self.nodes.get(&step_id).cloned().ok_or_else(|| {
            format!("Step ID {} not found in causal graph", step_id)
        })?;
        let (p0, p1) = node.token_range;

        // Verify that rollback is applied to active sequence suffix
        if p1 != self.current_token_cursor {
            return Err(format!(
                "Causal KV rollback must be applied to the active sequence suffix: step range=({}, {}), current_cursor={}",
                p0, p1, self.current_token_cursor
            ));
        }

        // Physically excise from KV cache via C FFI
        let ok = ctx.kv_cache_seq_rm(0, p0 as i32, -1)?;
        if !ok {
            return Err("llama_kv_cache_seq_rm kernel returned false".to_string());
        }

        self.current_token_cursor = p0;
        self.nodes.remove(&step_id);

        // Targeted O(|entities_written|) cleanup & restoring latest_writer
        for w in &node.entities_written {
            if let Some(writers) = self.entity_writers.get_mut(w) {
                writers.retain(|&id| id != step_id);
            }
            if let Some(prev) = node.prior_writers.get(w).copied().flatten() {
                self.latest_writer.insert(w.clone(), prev);
            } else {
                self.latest_writer.remove(w);
            }
        }

        // Targeted O(|entities_read|) cleanup
        for r in &node.entities_read {
            if let Some(readers) = self.entity_readers.get_mut(r) {
                readers.retain(|&id| id != step_id);
            }
        }

        // Targeted O(|deg|) adjacency and predecessors cleanup
        if let Some(preds) = self.predecessors.remove(&step_id) {
            for p in preds {
                if let Some(adj) = self.adjacency.get_mut(&p) {
                    adj.remove(&step_id);
                }
            }
        }
        if let Some(succs) = self.adjacency.remove(&step_id) {
            for s in succs {
                if let Some(p) = self.predecessors.get_mut(&s) {
                    p.remove(&step_id);
                }
            }
        }

        self.store.remove(step_id);
        Ok(true)
    }

    /// Logical graph-only suffix rollback without requiring physical llama context
    pub fn rollback_step_logical(&mut self, step_id: usize) -> Result<bool, String> {
        let node = self.nodes.get(&step_id).cloned().ok_or_else(|| {
            format!("Step ID {} not found in causal graph", step_id)
        })?;
        let (p0, p1) = node.token_range;

        if p1 != self.current_token_cursor {
            return Err(format!(
                "Causal rollback must be applied to the active sequence suffix: step range=({}, {}), current_cursor={}",
                p0, p1, self.current_token_cursor
            ));
        }

        self.current_token_cursor = p0;
        self.nodes.remove(&step_id);

        for w in &node.entities_written {
            if let Some(writers) = self.entity_writers.get_mut(w) {
                writers.retain(|&id| id != step_id);
            }
            if let Some(prev) = node.prior_writers.get(w).copied().flatten() {
                self.latest_writer.insert(w.clone(), prev);
            } else {
                self.latest_writer.remove(w);
            }
        }

        for r in &node.entities_read {
            if let Some(readers) = self.entity_readers.get_mut(r) {
                readers.retain(|&id| id != step_id);
            }
        }

        if let Some(preds) = self.predecessors.remove(&step_id) {
            for p in preds {
                if let Some(adj) = self.adjacency.get_mut(&p) {
                    adj.remove(&step_id);
                }
            }
        }
        if let Some(succs) = self.adjacency.remove(&step_id) {
            for s in succs {
                if let Some(p) = self.predecessors.get_mut(&s) {
                    p.remove(&step_id);
                }
            }
        }

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
    fn test_entity_extractor_from_standard_errors() {
        let err1 = "KeyError: 'auth_token'";
        assert_eq!(EntityExtractor::extract_from_error(err1), vec!["auth_token"]);

        let err2 = "FileNotFoundError: [Errno 2] No such file or directory: 'config.json'";
        assert_eq!(EntityExtractor::extract_from_error(err2), vec!["config.json"]);

        let err3 = "NameError: name 'user_session' is not defined";
        assert_eq!(EntityExtractor::extract_from_error(err3), vec!["user_session"]);
    }

    #[test]
    fn test_causal_dag_duplicate_step_id_rejected() {
        let mut graph = CausalGraph::new();
        assert!(graph.record_step(1, "Step 1", &[], &["file.txt"], 50));
        assert!(!graph.record_step(1, "Duplicate Step 1", &[], &["file.txt"], 50));
    }

    #[test]
    fn test_causal_dag_reaching_definitions_and_no_waw_explosion() {
        let mut graph = CausalGraph::new();
        graph.record_step(1, "Write X v1", &[], &["X"], 10);
        graph.record_step(2, "Write X v2", &[], &["X"], 10);
        graph.record_step(3, "Read X", &["X"], &[], 10);

        // Step 3 depends only on reaching definition Step 2, NOT on Step 1!
        let cone_3 = graph.resolve_ancestral_cone_for_step(3);
        assert_eq!(cone_3, vec![2, 3]);
    }

    #[test]
    fn test_causal_dag_post_crash_nodes_excluded() {
        let mut graph = CausalGraph::new();
        graph.record_step(1, "Write A", &[], &["A"], 10);
        graph.record_step(2, "Crash Step", &["A"], &[], 10);
        graph.record_step(3, "Post-crash Step", &["A"], &[], 10);

        // Ancestral cone of crash step 2 strictly excludes step 3
        let cone_2 = graph.resolve_ancestral_cone_for_step(2);
        assert_eq!(cone_2, vec![1, 2]);
    }

    #[test]
    fn test_causal_dag_non_suffix_rollback_rejected() {
        let mut graph = CausalGraph::new();
        graph.record_step(1, "Step 1", &[], &["A"], 10);
        graph.record_step(2, "Step 2", &["A"], &["B"], 10);
        graph.record_step(3, "Step 3", &["B"], &["C"], 10);

        // Attempting to rollback step 2 (non-suffix) must be rejected
        let res = graph.rollback_step_logical(2);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("active sequence suffix"));
    }

    #[test]
    fn test_causal_dag_suffix_rollback_and_graph_cleanup() {
        let mut graph = CausalGraph::new();
        graph.record_step(1, "Write X", &[], &["X"], 50);
        graph.record_step(2, "Write X v2", &["X"], &["X"], 40);
        graph.record_step(3, "Failed Write X v3", &["X"], &["X"], 30);

        assert_eq!(graph.current_token_cursor(), 120);
        assert!(graph.contains_step(3));

        // Rollback suffix step 3
        let ok = graph.rollback_step_logical(3).expect("Suffix rollback must succeed");
        assert!(ok);

        // Verify cursor rewound
        assert_eq!(graph.current_token_cursor(), 90);
        assert!(!graph.contains_step(3));

        // Verify reaching definition for X was restored to Step 2!
        assert_eq!(graph.latest_writer.get("X"), Some(&2));

        // Verify indices cleaned up
        assert_eq!(graph.entity_writers.get("X"), Some(&vec![1, 2]));
    }

    #[test]
    fn test_causal_dag_and_reactive_hydration() {
        let mut graph = CausalGraph::new();

        graph.record_step(1, "Create hello.py", &[], &["hello.py"], 50);
        graph.record_step(2, "Setup Network", &[], &["network_cfg"], 60);
        graph.record_step(3, "Append function to hello.py", &["hello.py"], &["hello.py"], 40);
        graph.record_step(4, "Check CPU Stats", &[], &["cpu"], 30);
        graph.record_step(5, "Ping Gateway", &["network_cfg"], &[], 30);

        let step_1_code = "def hello():\n    print('Hello World')\n";
        graph.evict_step(1, step_1_code);

        graph.record_step(6, "Execute hello.py", &["hello.py"], &[], 20);

        let crash_cone = graph.resolve_ancestral_cone_for_step(6);
        assert_eq!(crash_cone, vec![1, 3, 6], "Causal cone of crash step 6 includes only [1, 3, 6]!");

        let entity_deps = graph.resolve_dependencies_for_entity("hello.py");
        assert_eq!(entity_deps, vec![1, 3, 6], "Causal participants of hello.py are steps 1, 3, and 6");

        // Hydration unmarks is_evicted
        let hydrated = graph.hydrate_and_reprefill_ancestors(6, None, None).unwrap();
        assert_eq!(hydrated.len(), 1);
        assert_eq!(hydrated[0].0, 1);
        assert_eq!(hydrated[0].1, step_1_code);
        assert!(!graph.nodes.get(&1).unwrap().is_evicted);
    }
}
