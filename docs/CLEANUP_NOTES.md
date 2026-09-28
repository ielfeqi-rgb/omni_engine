# Cleanup Notes -- What to Remove and Why

> These are items that should be removed or heavily refactored.
> Nothing has been deleted -- this file documents what and why.

---

## 1. REMOVE: `#![allow(dead_code)]` from main.rs:1 and lib.rs:1

**Why**: This single line hides ALL dead-code warnings across the entire project.
You have no idea what functions are actually used vs orphaned.
Remove it, then fix each warning individually.

**Files**:
- `src/main.rs` line 1
- `src/lib.rs` line 1

---

## 2. REMOVE: Hardcoded absolute paths in llama_manager.rs:44-47

```rust
// DELETE these 3 lines:
PathBuf::from("/home/hema/Downloads/files(1)/M.A.R.K.E.T/bin/llama-server"),
PathBuf::from("/home/hema/Downloads/files(1)/omnicontext_v2/bin/llama-server"),
PathBuf::from("/home/hema/Downloads/files(1)/omnicontext_complete/bin/llama-server"),
```

**Why**: These only work on YOUR laptop. Replace with:
```rust
if let Ok(home) = std::env::var("HOME") {
    // or better: read from a config file / env var LLAMA_SERVER_PATH
}
```

---

## 3. REMOVE: Stale version strings

**Files & what to change**:
- `src/main.rs:78` -- says "v1.0.1" but Cargo.toml says "2.0.0"
- `src/cli.rs:23` -- says "v1.0.1"

**Fix**: Replace both with `env!("CARGO_PKG_VERSION")`

---

## 4. RECONSIDER: Benchmark naming in tests/

The 7 benchmark files test REAL infrastructure, but their NAMES claim to test AI capabilities:

| File | Name Claims | Actually Tests |
|------|-------------|----------------|
| `benchmark_complete_swarm_gauntlet.rs` | Byzantine consensus, elastic scaling | tokio tasks doing arithmetic, HashMap writes |
| `benchmark_snake_swarm_apoptosis.rs` | Swarm discovery across generations | Hardcoded match statement running python3 |
| `benchmark_jailbreak_escape_containment.rs` | Model escape attempts | Hardcoded Lua scripts vs sandbox |
| `benchmark_speed_efficiency.rs` | Token throughput | Keyword string matching speed |
| `benchmark_torture_adversarial.rs` | Breaking point | Sandbox stress test |

**Recommendation**: Either rename them honestly (e.g. `test_sandbox_security.rs`, `test_vfs_concurrent_stress.rs`) or add clear comments at the top of each file stating what they ACTUALLY test.

---

## 5. REMOVE: Emoji from production output

- `src/main.rs:69` -- rocket emoji
- `src/main.rs:93` -- globe emoji
- `src/main.rs:94` -- speech emoji
- `src/main.rs:95` -- lightning emoji

**Why**: Breaks on terminals without Unicode. Use plain text banners.

---

## 6. RECONSIDER: docs/DECLARATION_NEW_ERA_AGENTIC_AI.md

This is a manifesto document, not technical documentation.
Move to `tools/deprecated/` or rename to make clear it's aspirational, not implemented.

---

## 7. REVIEW: CausalGraph and CompressedCacheStore naming

These are fine Rust data structures but their names imply they touch the model's KV-cache:
- `src/causal_memory/dag.rs` -- it's a step-dependency DAG, not a causal attention graph
- `src/causal_memory/store.rs` -- it does `.to_vec()`, not compression

**Recommendation**: Rename to `StepDependencyGraph` and `EvictionStore` or similar,
OR add prominent doc comments clarifying what they actually do.
