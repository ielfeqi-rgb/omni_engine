// [GUIDANCE] NAMING: This is a step-dependency DAG, NOT a causal attention graph.
// It tracks which steps read/wrote which entities. It has no connection to the model's
// internal KV-cache or attention mechanism. Consider renaming to StepDependencyGraph
// to avoid confusion with actual causal inference concepts.
//
// The "eviction" in store.rs does .to_vec() -- it's not compression, just moving bytes
// to a separate HashMap. Consider renaming CompressedCacheStore to EvictionStore.

pub mod dag;
pub mod store;
