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
    /// Records previous reaching writer for written entities to allow state restoration on rollback
    pub prior_writers: HashMap<String, Option<usize>>,
}

/// Automated entity extractor for runtime error tracebacks across multiple languages and runtimes.
pub struct EntityExtractor;

impl EntityExtractor {
    /// Extracts candidate entity names (files, modules, variables, keys) from error tracebacks.
    pub fn extract_from_error(error_msg: &str) -> Vec<String> {
        let mut entities = Vec::new();

        // 1. Python KeyError: 'key' or "key"
        if let Some(idx) = error_msg.find("KeyError:") {
            let rest = &error_msg[idx + 9..];
            if let Some(q1) = rest.find(['\'', '"']) {
                let quote = rest.as_bytes()[q1] as char;
                if let Some(q2) = rest[q1 + 1..].find(quote) {
                    entities.push(rest[q1 + 1..q1 + 1 + q2].to_string());
                }
            }
        }

        // 2. Python FileNotFoundError / IOError: [Errno 2] ... 'path'
        if let Some(idx) = error_msg.find("FileNotFoundError:") {
            let rest = &error_msg[idx + 18..];
            if let Some(q1) = rest.find(['\'', '"']) {
                let quote = rest.as_bytes()[q1] as char;
                if let Some(q2) = rest[q1 + 1..].find(quote) {
                    entities.push(rest[q1 + 1..q1 + 1 + q2].to_string());
                }
            }
        }

        // 3. Python NameError: name 'var' is not defined
        if let Some(idx) = error_msg.find("NameError: name ") {
            let rest = &error_msg[idx + 16..];
            if let Some(q1) = rest.find(['\'', '"']) {
                let quote = rest.as_bytes()[q1] as char;
                if let Some(q2) = rest[q1 + 1..].find(quote) {
                    entities.push(rest[q1 + 1..q1 + 1 + q2].to_string());
                }
            }
        }

        // 4. Python AttributeError: 'object' has no attribute 'attr'
        if let Some(idx) = error_msg.find("has no attribute ") {
            let rest = &error_msg[idx + 17..];
            if let Some(q1) = rest.find(['\'', '"']) {
                let quote = rest.as_bytes()[q1] as char;
                if let Some(q2) = rest[q1 + 1..].find(quote) {
                    entities.push(rest[q1 + 1..q1 + 1 + q2].to_string());
                }
            }
        }

        // 5. Python ImportError / ModuleNotFoundError: No module named 'mod' / cannot import name 'name'
        if let Some(idx) = error_msg.find("No module named ") {
            let rest = &error_msg[idx + 16..];
            if let Some(q1) = rest.find(['\'', '"']) {
                let quote = rest.as_bytes()[q1] as char;
                if let Some(q2) = rest[q1 + 1..].find(quote) {
                    entities.push(rest[q1 + 1..q1 + 1 + q2].to_string());
                }
            }
        }

        // 6. Rust error[E0425]: cannot find value `val` in this scope
        if let Some(idx) = error_msg.find("cannot find value `") {
            let rest = &error_msg[idx + 19..];
            if let Some(q2) = rest.find('`') {
                entities.push(rest[..q2].to_string());
            }
        }

        // 7. Shell / OS: command not found: cmd / No such file or directory
        if let Some(idx) = error_msg.find("command not found: ") {
            let rest = error_msg[idx + 19..].trim();
            let cmd = rest.split_whitespace().next().unwrap_or(rest);
            entities.push(cmd.to_string());
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
    /// Tracks explicit execution order to uniquely determine the active sequence suffix
    execution_order: Vec<usize>,
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
            execution_order: Vec::new(),
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

    /// Checks if a step is the true active suffix of the execution sequence.
    /// Uniquely validated via execution order to prevent 0-token step collisions.
    pub fn is_active_suffix(&self, step_id: usize) -> bool {
        if self.execution_order.last() != Some(&step_id) {
            return false;
        }
        if let Some(node) = self.nodes.get(&step_id) {
            node.token_range.1 == self.current_token_cursor
        } else {
            false
        }
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

        // 1. Dataflow Read Dependency Resolution:
        // A read depends on the MOST RECENT writer of that entity.
        for r in &entities_read {
            self.entity_readers.entry(r.clone()).or_default().push(step_id);
            if let Some(&writer_id) = self.latest_writer.get(r) {
                if writer_id != step_id {
                    self.adjacency.entry(writer_id).or_default().insert(step_id);
                    self.predecessors.entry(step_id).or_default().insert(writer_id);
                }
            }
        }

        // 2. Dataflow Write Resolution:
        // A pure write is a Definition-Kill (overwriting previous definition).
        // If the step also reads the entity (Read-Modify-Write), the causal chain is
        // naturally preserved through the read dependency above!
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
        self.execution_order.push(step_id);
        true
    }

    /// Synchronizes step recording with live physical context position
    pub fn record_step_with_context(
        &mut self,
        step_id: usize,
        description: &str,
        reads: &[&str],
        writes: &[&str],
        ctx: &crate::native_llama::NativeLlamaContext,
    ) -> bool {
        let current_pos = ctx.current_cursor();
        let token_count = current_pos.saturating_sub(self.current_token_cursor);
        self.record_step(step_id, description, reads, writes, token_count)
    }

    /// Evict a step from active memory to compressed storage
    pub fn evict_step(&mut self, step_id: usize, text_payload: &str) -> bool {
        self.evict_step_with_context(step_id, text_payload, None, None).unwrap_or(false)
    }

    /// Evict a step from active memory with optional physical KV cache excision and shifting.
    /// - If suffix: excises directly via `kv_cache_seq_rm`.
    /// - If middle step: excises and shifts subsequent tokens via `kv_cache_seq_rm_and_shift`,
    ///   adjusting token ranges of all subsequent steps to maintain continuous positional RoPE.
    pub fn evict_step_with_context(
        &mut self,
        step_id: usize,
        text_payload: &str,
        token_ids: Option<&[i32]>,
        mut ctx: Option<&mut crate::native_llama::NativeLlamaContext>,
    ) -> Result<bool, String> {
        let (p0, p1) = {
            let node = self.nodes.get_mut(&step_id).ok_or_else(|| format!("Step {} not found", step_id))?;
            node.is_evicted = true;
            node.token_range
        };

        self.store.store_step(step_id, text_payload, token_ids);

        if let Some(c) = ctx.as_mut() {
            if self.is_active_suffix(step_id) {
                let ok = c.kv_cache_seq_rm(0, p0 as i32, -1)?;
                if !ok {
                    return Err("Failed to excise suffix in kv_cache_seq_rm".to_string());
                }
                self.current_token_cursor = p0;
                if let Some(n) = self.nodes.get_mut(&step_id) {
                    n.token_range = (0, 0);
                }
            } else {
                // Middle step excision with automatic position shift
                let ok = c.kv_cache_seq_rm_and_shift(0, p0 as i32, p1 as i32)?;
                if !ok {
                    return Err("Failed to excise and shift middle KV cells".to_string());
                }
                let shift = p1 - p0;
                self.current_token_cursor = self.current_token_cursor.saturating_sub(shift);
                if let Some(n) = self.nodes.get_mut(&step_id) {
                    n.token_range = (0, 0);
                }
                for other in self.nodes.values_mut() {
                    if other.step_id != step_id && other.token_range.0 >= p1 {
                        other.token_range.0 -= shift;
                        other.token_range.1 -= shift;
                    }
                }
            }
        }
        Ok(true)
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

    /// Resolve ancestral cone for target_entity restricted to steps occurring up to `up_to_step`.
    /// Excludes post-crash steps and avoids read-read over-expansion.
    pub fn resolve_ancestors_for_entity_up_to(&self, target_entity: &str, up_to_step: usize) -> Vec<usize> {
        let mut seed_steps: Vec<usize> = Vec::new();
        if let Some(writers) = self.entity_writers.get(target_entity) {
            for &w in writers {
                if w <= up_to_step {
                    seed_steps.push(w);
                }
            }
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

    /// Resolves cone from error message: matches candidates with graph entities or falls back to crash step.
    pub fn resolve_error_cone(&self, crash_step_id: usize, error_msg: Option<&str>) -> Vec<usize> {
        if let Some(msg) = error_msg {
            let candidates = EntityExtractor::extract_from_error(msg);
            for cand in candidates {
                if self.latest_writer.contains_key(&cand) || self.entity_writers.contains_key(&cand) {
                    return self.resolve_ancestors_for_entity_up_to(&cand, crash_step_id);
                }
            }
        }
        self.resolve_ancestral_cone_for_step(crash_step_id)
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

    /// Unified, Atomic Self-Healing:
    /// 1. Pre-validates that all required evicted ancestors exist in the store (Atomicity).
    /// 2. Rolls back the failed crash step from the active suffix.
    /// 3. Hydrates any missing evicted ancestors sequentially at the rewound cursor,
    ///    ensuring definitions strictly precede the repair generation!
    pub fn rollback_and_recover(
        &mut self,
        crash_step_id: usize,
        error_entity: Option<&str>,
        mut ctx: Option<&mut crate::native_llama::NativeLlamaContext>,
        model: Option<&crate::native_llama::NativeLlamaModel>,
    ) -> Result<Vec<(usize, String)>, String> {
        // Step 1: Determine required ancestral cone
        let required_cone = match error_entity {
            Some(ent) => self.resolve_ancestors_for_entity_up_to(ent, crash_step_id),
            None => self.resolve_ancestral_cone_for_step(crash_step_id),
        };

        // Step 2: Atomic Pre-validation: ensure all evicted ancestors can be hydrated
        let mut evicted_ancestors = Vec::new();
        for &step_id in &required_cone {
            if step_id != crash_step_id {
                if let Some(node) = self.nodes.get(&step_id) {
                    if node.is_evicted {
                        let _ = self.store.hydrate_entry(step_id).map_err(|e| {
                            format!("Atomicity failure: ancestor {} cannot be hydrated: {}", step_id, e)
                        })?;
                        evicted_ancestors.push(step_id);
                    }
                }
            }
        }

        // Step 3: Rollback the crash step from suffix
        if let Some(c) = ctx.as_mut() {
            self.rollback_step_kv(crash_step_id, c)?;
        } else {
            self.prune_step_graph_state(crash_step_id)?;
        }

        // Step 4: Sequentially hydrate missing ancestors at the rewound cursor position
        let mut hydrated_payloads = Vec::new();
        for step_id in evicted_ancestors {
            let (text, tokens) = self.store.hydrate_entry(step_id).map_err(|e| e.to_string())?;

            if let (Some(c), Some(m)) = (ctx.as_mut(), model) {
                let eval_toks = if let Some(toks) = tokens {
                    toks
                } else {
                    m.tokenize(&text, false)?
                };
                let start_pos = self.current_token_cursor;
                c.eval_tokens(&eval_toks, 0)?;
                let end_pos = c.current_cursor();
                self.current_token_cursor = end_pos;
                if let Some(node) = self.nodes.get_mut(&step_id) {
                    node.token_range = (start_pos, end_pos);
                    node.is_evicted = false;
                }
            } else if let Some(node) = self.nodes.get_mut(&step_id) {
                node.is_evicted = false;
            }

            hydrated_payloads.push((step_id, text));
        }

        Ok(hydrated_payloads)
    }

    /// Reactive Hydration & Re-Prefill for backward compatibility
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
                        let eval_toks = if let Some(toks) = tokens {
                            toks
                        } else {
                            m.tokenize(&text, false)?
                        };
                        let start_pos = self.current_token_cursor;
                        c.eval_tokens(&eval_toks, 0)?;
                        let end_pos = c.current_cursor();
                        self.current_token_cursor = end_pos;
                        node.token_range = (start_pos, end_pos);
                        node.is_evicted = false;
                    } else {
                        // In graph-only mode without live context, leave is_evicted = false for simulation
                        node.is_evicted = false;
                    }

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
    /// rewinds the cursor, restores reaching definitions in O(K_w) time,
    /// and prunes the node from graph structures.
    pub fn rollback_step_kv(
        &mut self,
        step_id: usize,
        ctx: &mut crate::native_llama::NativeLlamaContext,
    ) -> Result<bool, String> {
        // Point 13: Validate cursor synchronization between Graph and Physical Context
        if self.current_token_cursor != ctx.current_cursor() {
            return Err(format!(
                "Cursor desynchronization: CausalGraph cursor={}, NativeLlamaContext cursor={}",
                self.current_token_cursor,
                ctx.current_cursor()
            ));
        }

        // Point 11: Validate true suffix using execution order
        if !self.is_active_suffix(step_id) {
            return Err(format!(
                "Causal KV rollback must be applied to active sequence suffix (last step={:?}, target={})",
                self.execution_order.last(),
                step_id
            ));
        }

        let node = self.nodes.get(&step_id).cloned().ok_or_else(|| {
            format!("Step ID {} not found in causal graph", step_id)
        })?;
        let (p0, _) = node.token_range;

        // Physically excise from KV cache via C FFI
        let ok = ctx.kv_cache_seq_rm(0, p0 as i32, -1)?;
        if !ok {
            return Err("llama_kv_cache_seq_rm kernel returned false".to_string());
        }

        self.prune_step_graph_state(step_id)?;
        Ok(true)
    }

    /// Internal graph state pruning executed after physical KV rollback (or during logical simulation)
    fn prune_step_graph_state(&mut self, step_id: usize) -> Result<bool, String> {
        let node = self.nodes.get(&step_id).cloned().ok_or_else(|| {
            format!("Step ID {} not found in causal graph", step_id)
        })?;
        let (p0, _) = node.token_range;

        self.current_token_cursor = p0;
        self.nodes.remove(&step_id);

        if let Some(pos) = self.execution_order.iter().rposition(|&id| id == step_id) {
            self.execution_order.remove(pos);
        }

        // Clean up entity_writers and restore reaching definition in O(K_w)
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

        // Clean up entity_readers
        for r in &node.entities_read {
            if let Some(readers) = self.entity_readers.get_mut(r) {
                readers.retain(|&id| id != step_id);
            }
        }

        // Clean up graph edges
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
        if !self.is_active_suffix(step_id) {
            return Err(format!(
                "Causal rollback must be applied to active sequence suffix: step_id={}",
                step_id
            ));
        }
        self.prune_step_graph_state(step_id)
    }

    pub fn current_token_cursor(&self) -> usize {
        self.current_token_cursor
    }

    pub fn contains_step(&self, step_id: usize) -> bool {
        self.nodes.contains_key(&step_id)
    }

    pub fn is_step_evicted(&self, step_id: usize) -> Option<bool> {
        self.nodes.get(&step_id).map(|n| n.is_evicted)
    }

    pub fn step_token_range(&self, step_id: usize) -> Option<(usize, usize)> {
        self.nodes.get(&step_id).map(|n| n.token_range)
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
    fn test_entity_extractor_expanded_patterns() {
        let err1 = "AttributeError: 'NoneType' object has no attribute 'secret_key'";
        assert_eq!(EntityExtractor::extract_from_error(err1), vec!["secret_key"]);

        let err2 = "ModuleNotFoundError: No module named 'jwt_helper'";
        assert_eq!(EntityExtractor::extract_from_error(err2), vec!["jwt_helper"]);

        let err3 = "error[E0425]: cannot find value `token_ring` in this scope";
        assert_eq!(EntityExtractor::extract_from_error(err3), vec!["token_ring"]);

        let err4 = "/bin/sh: command not found: pg_dump";
        assert_eq!(EntityExtractor::extract_from_error(err4), vec!["pg_dump"]);

        // Negative test case: no entity in string
        let err5 = "ZeroDivisionError: division by zero";
        assert!(EntityExtractor::extract_from_error(err5).is_empty());
    }

    #[test]
    fn test_causal_dag_zero_token_steps_suffix_uniqueness() {
        let mut graph = CausalGraph::new();
        graph.record_step(1, "Step 1", &[], &["a"], 10);
        graph.record_step(2, "Step 2 (0 tok)", &["a"], &["b"], 0);
        graph.record_step(3, "Step 3 (0 tok)", &["b"], &["c"], 0);

        // Step 3 is the active suffix, Step 2 is NOT
        assert!(!graph.is_active_suffix(2));
        assert!(graph.is_active_suffix(3));

        // Attempting to rollback Step 2 must fail
        assert!(graph.rollback_step_logical(2).is_err());

        // Rolling back Step 3 succeeds, making Step 2 the active suffix
        assert!(graph.rollback_step_logical(3).is_ok());
        assert!(graph.is_active_suffix(2));
    }

    #[test]
    fn test_causal_dag_edge_cleanup_and_new_reader_links_to_restored_writer() {
        let mut graph = CausalGraph::new();
        graph.record_step(1, "Write X v1", &[], &["X"], 10);
        graph.record_step(2, "Write X v2", &["X"], &["X"], 10);
        graph.record_step(3, "Failed Write X v3", &["X"], &["X"], 10);

        // Rollback failed Step 3
        assert!(graph.rollback_step_logical(3).is_ok());

        // Step 4 reads X -> must connect to restored latest_writer (Step 2), NOT Step 3!
        graph.record_step(4, "Read X", &["X"], &[], 10);
        let cone_4 = graph.resolve_ancestral_cone_for_step(4);
        assert_eq!(cone_4, vec![1, 2, 4]);
        assert!(!cone_4.contains(&3));
    }

    #[test]
    fn test_causal_dag_resolve_ancestors_for_entity_up_to() {
        let mut graph = CausalGraph::new();
        graph.record_step(1, "Write X v1", &[], &["X"], 10);
        graph.record_step(2, "Crash Step", &["X"], &[], 10);
        graph.record_step(3, "Post-crash Write X", &[], &["X"], 10);

        // Ancestors up to step 2 strictly excludes step 3
        let ancestors = graph.resolve_ancestors_for_entity_up_to("X", 2);
        assert_eq!(ancestors, vec![1]);
    }

    #[test]
    fn test_causal_dag_atomic_rollback_and_recovery() {
        let mut graph = CausalGraph::new();
        graph.record_step(1, "Write Auth Code", &[], &["auth.py"], 50);
        let code = "def verify(): return True\n";
        graph.evict_step(1, code);
        assert!(graph.nodes.get(&1).unwrap().is_evicted);

        // Step 2 is the crash step (active suffix)
        graph.record_step(2, "Crash executing auth", &["auth.py"], &[], 20);

        // Execute atomic rollback and recovery
        let recovered = graph
            .rollback_and_recover(2, Some("auth.py"), None, None)
            .expect("Atomic recovery must succeed");

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].0, 1);
        assert_eq!(recovered[0].1, code);
        assert!(!graph.contains_step(2));
        assert!(!graph.nodes.get(&1).unwrap().is_evicted);
        assert_eq!(graph.current_token_cursor(), 50);
    }
}
