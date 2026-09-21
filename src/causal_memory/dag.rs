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
    pub store: CompressedCacheStore,
    current_token_cursor: usize,
}

impl CausalGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            adjacency: HashMap::new(),
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
        for (prev_id, prev_node) in &self.nodes {
            // Did previous step write an entity that current step reads?
            let intersection: HashSet<_> = prev_node.entities_written.intersection(&entities_read).collect();
            if !intersection.is_empty() {
                self.adjacency.entry(*prev_id).or_default().push(step_id);
            }
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

    /// Query all causal ancestors for an entity involved in an error
    pub fn resolve_dependencies_for_entity(&self, target_entity: &str) -> Vec<usize> {
        let mut relevant_steps = Vec::new();
        for (id, node) in &self.nodes {
            if node.entities_written.contains(target_entity) || node.entities_read.contains(target_entity) {
                relevant_steps.push(*id);
            }
        }
        relevant_steps.sort();
        relevant_steps
    }

    /// Reactive Hydration: Check if any required causal ancestor is evicted, and unpack it immediately
    pub fn hydrate_ancestors_for_error(&self, target_entity: &str) -> Vec<(usize, String)> {
        let mut hydrated_context = Vec::new();
        let ancestors = self.resolve_dependencies_for_entity(target_entity);

        for step_id in ancestors {
            if let Some(node) = self.nodes.get(&step_id) {
                if node.is_evicted {
                    // Hydrate from compressed store!
                    if let Some(content) = self.store.hydrate(step_id) {
                        hydrated_context.push((step_id, content));
                    }
                }
            }
        }
        hydrated_context
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

        // Now, we ask the Causal Graph: Who is related to the crash of 'hello.py'?
        let dependencies = graph.resolve_dependencies_for_entity("hello.py");
        assert_eq!(dependencies, vec![1, 3, 6], "Causal DAG correctly isolated steps 1, 3, and 6!");

        // Reactive Hydration: Step 1 is evicted, so hydrate it!
        let hydrated = graph.hydrate_ancestors_for_error("hello.py");
        assert_eq!(hydrated.len(), 1, "Only Step 1 was evicted and needed hydration");
        assert_eq!(hydrated[0].0, 1);
        assert_eq!(hydrated[0].1, step_1_code, "Hydrated exact byte-for-byte content of Step 1 without context loss!");
    }
}
