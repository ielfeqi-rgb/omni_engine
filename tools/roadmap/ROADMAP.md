# Omni Engine -- The Right Path Forward

> This is not a wish list. This is the minimum viable sequence to get
> from "infrastructure wrapper" to "engine that actually touches the model."

---

## Where You Are Now

```
[Rust Web Server] --HTTP--> [llama-server subprocess] --internally--> [Model + KV-Cache]
       ^                            ^                                        ^
    YOU CONTROL              YOU SPAWN/KILL                           YOU CANNOT TOUCH
```

You built a solid sandbox layer (VFS, Lua, Terminal). That's real and works.
But the AI brain is behind an HTTP wall. You send text in, get text out.

---

## Phase 0: Stabilize (Before anything else)

**Time**: 3-5 days
**Goal**: Make the existing code production-safe

1. Remove `#![allow(dead_code)]` -- see what's actually dead
2. Replace `.lock().unwrap()` with `.lock().unwrap_or_else(|e| e.into_inner())`
   or add `parking_lot` crate (drop-in, never poisons)
3. Remove hardcoded `/home/hema/` paths
4. Fix version strings
5. Default bind to `127.0.0.1`
6. Add signal handler for graceful shutdown

**Validation**: `cargo clippy` with zero warnings, manual test of start/stop/ask

---

## Phase 1: First Real Model Test

**Time**: 1 week
**Goal**: Prove the engine works end-to-end with a real model

Write ONE test that:
1. Starts llama-server with a real GGUF model
2. Sends a prompt through the OpenAI API proxy
3. Receives a response
4. Validates the response is coherent text
5. Stops llama-server cleanly

This test does not exist today. It should be the FIRST thing you build.

```rust
// tests/test_real_inference.rs
#[tokio::test]
async fn test_real_model_inference_roundtrip() {
    let manager = LlamaManager::new(base_dir);
    manager.start("qwen2-0.5b-q4.gguf", 8091, 4, 2048).unwrap();
    
    // Wait for server to be ready
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    let client = reqwest::Client::new();
    let resp = client.post("http://127.0.0.1:8091/v1/chat/completions")
        .json(&json!({
            "model": "qwen2-0.5b",
            "messages": [{"role": "user", "content": "What is 2+2?"}]
        }))
        .send().await.unwrap();
    
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.contains("4"));
    
    manager.stop().unwrap();
}
```

---

## Phase 2: Python KV-Cache Proof of Concept

**Time**: 1 week
**Goal**: Test if KV-cache manipulation actually improves output quality

Install `llama-cpp-python` and write a Python script that:

```python
from llama_cpp import Llama

llm = Llama(model_path="qwen2-0.5b-q4.gguf", n_ctx=2048)

# Step 1: Generate with a bad prefix (simulate "poisoned" context)
tokens_bad = llm.tokenize(b"The capital of France is Berlin. ")
llm.eval(tokens_bad)

# Step 2: ROLLBACK -- remove the bad tokens from KV cache
llm._ctx.kv_cache_seq_rm(-1, 0, len(tokens_bad))

# Step 3: Generate with clean prefix
tokens_good = llm.tokenize(b"The capital of France is ")
llm.eval(tokens_good)

# Step 4: Sample -- does it say "Paris" now?
output = llm.create_completion("", max_tokens=5)
print(output)  # Should say "Paris" not "Berlin"
```

If this works: KV rollback theory is validated.
If this doesn't work: you know before investing weeks in Rust FFI.

---

## Phase 3: Native KV Access (Only if Phase 2 succeeds)

**Time**: 3-4 weeks
**Goal**: Bring KV-cache control into the Rust engine

Add `llama-cpp-2` crate to Cargo.toml:

```toml
[dependencies]
llama-cpp-2 = "0.1"  # Check latest version
```

Replace `LlamaManager` (subprocess approach) with direct model loading:

```rust
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::context::LlamaContext;

struct NativeModelManager {
    model: LlamaModel,
    context: LlamaContext,
}

impl NativeModelManager {
    fn kv_rollback(&mut self, from_pos: u32, to_pos: u32) {
        self.context.kv_cache_seq_rm(0, from_pos as i32, to_pos as i32);
    }
    
    fn kv_branch(&mut self, src_seq: i32, dst_seq: i32, len: u32) {
        self.context.kv_cache_seq_cp(src_seq, dst_seq, 0, len as i32);
    }
}
```

---

## Phase 4: Real Swarm Architecture (Only if Phase 3 succeeds)

**Time**: 4-6 weeks
**Goal**: Multiple models running in parallel with real coordination

This is where the "Leader + Workers" swarm can become real:

```
Leader (1.5B model, full context, owns the plan)
  |
  +-- Worker 1 (0.5B, subtask A, own KV-cache)
  +-- Worker 2 (0.5B, subtask B, own KV-cache)
  +-- Worker 3 (0.5B, subtask C, own KV-cache)
```

Each worker = separate `LlamaContext` sharing the same `LlamaModel` weights.
KV-cache is per-context, so each worker has its own memory.
Leader aggregates results, detects bad outputs, triggers rollback on specific workers.

**Key insight**: `LlamaModel` can be shared (read-only weights).
`LlamaContext` is per-conversation (read-write KV-cache).
Multiple contexts on one model = real swarm with shared knowledge.

---

## What NOT to Build

| Idea | Why Not |
|------|---------|
| Byzantine fault tolerance | You're not running on untrusted hardware. Simple majority voting is enough. |
| Custom tokenizer | Use the model's built-in tokenizer. Don't reinvent this. |
| Custom attention implementation | Use llama.cpp's. It's optimized for years. |
| "Epistemic apoptosis" as killing knowledge | KV-cache is per-session. Clearing it just forgets the conversation, not knowledge. |
| Compressed cache with custom compression | llama.cpp already has KV quantization (Q4/Q8 cache). Use it. |

---

## Summary: The Shortest Path

```
You are here:  [HTTP Wrapper + Sandbox]
               |
               v
Phase 0:       [Fix the 76 unwraps and hardcoded paths]     -- 3 days
               |
               v  
Phase 1:       [One real end-to-end test with a model]       -- 3 days
               |
               v
Phase 2:       [Python KV-cache experiment]                  -- 1 week
               |
               v
           Does KV rollback actually improve output?
              / \
            NO   YES
            |      |
            v      v
         STOP    Phase 3: [Rust FFI native KV access]        -- 3 weeks
         HERE              |
                           v
                  Phase 4: [Real multi-context swarm]         -- 4 weeks
```

Each phase validates the next. Don't skip ahead.
