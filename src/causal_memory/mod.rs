//! Dataflow Dependency Graph (Def-Use DAG) and Eviction Store.
//!
//! # Conceptual Architecture:
//! - **Def-Use Dependency DAG (`dag`):** Tracks operational dependencies across
//!   agent steps based on entity read/write sets ($O(S_i) \cap I(S_j) \neq \emptyset$).
//!   Technically a dataflow graph (def-use chains / reaching definitions) rather than
//!   a Pearlian causal Bayesian network. Maps semantic execution step boundaries to physical
//!   token ranges $[p_0, p_1)$ in the inference runtime.
//! - **Physical Memory Interface:** Interfaces with `native_llama` to perform
//!   surgical suffix rollback (`llama_kv_cache_seq_rm`) when an execution step fails.
//! - **Eviction Store (`store`):** In-memory storage for offloaded step artifacts and
//!   token sequences utilizing DEFLATE byte compression.

pub mod dag;
pub mod store;
