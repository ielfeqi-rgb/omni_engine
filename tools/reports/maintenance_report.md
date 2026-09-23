# Omni Engine v2.0.0 -- Maintenance Report

> **Auditor**: Antigravity (Claude Opus 4.6)
> **Date**: 2026-09-23
> **Codebase**: 5,208 lines Rust (src/) + 1,464 lines tests/ + Cargo.toml

---

## Health Dashboard

| Metric | Value | Status |
|--------|-------|--------|
| `cargo check` | 0 warnings, 0 errors | OK |
| `#![allow(dead_code)]` | Active in both main.rs and lib.rs | MASKING |
| `.unwrap()` calls in production code | **76** | HIGH RISK |
| `let _ = ...` (swallowed errors) | **52** | MEDIUM RISK |
| Hardcoded absolute paths | **3** user-specific paths | MUST FIX |
| Version string drift | v2.0.0, v1.0.1, v1.0.1 | INCONSISTENT |
| `unsafe` blocks | 0 | OK |
| TODO/FIXME comments | 0 | OK |
| Total public functions | 93 | -- |
| Inline unit tests | 30 | -- |
| Integration test files | 7 | -- |
| Public functions with ZERO test coverage | **27** (29%) | GAP |
| Source code size | 524 KB | OK |
| Build artifacts (`target/`) | **3.3 GB** | BLOATED |
| Lock objects (`Mutex`/`RwLock`) | 16 | REVIEW |

---

## CRITICAL: Issues That Will Cause Production Crashes

### C1. Mutex Poison Panic Risk (76 `.unwrap()` on `.lock()`)

> [!CAUTION]
> Every `.lock().unwrap()` will **panic and crash** the entire process if any thread holding that mutex has previously panicked. In a web server handling concurrent requests, one bad request can cascade-kill all connections.

**Affected files and count:**

| File | `.lock().unwrap()` Count |
|------|--------------------------|
| [supervisor.rs](file:///home/hema/Downloads/omni_engine_package/src/planner/supervisor.rs) | 14 |
| [terminal_bridge.rs](file:///home/hema/Downloads/omni_engine_package/src/sandbox/terminal_bridge.rs) | 13 |
| [downloader.rs](file:///home/hema/Downloads/omni_engine_package/src/downloader.rs) | 10 |
| [llama_manager.rs](file:///home/hema/Downloads/omni_engine_package/src/llama_manager.rs) | 8 |
| [lua_runner.rs](file:///home/hema/Downloads/omni_engine_package/src/sandbox/lua_runner.rs) | 4 |
| [vfs.rs](file:///home/hema/Downloads/omni_engine_package/src/sandbox/vfs.rs) | 8 |
| [auth.rs](file:///home/hema/Downloads/omni_engine_package/src/auth.rs) | 4 |
| [causal_memory/store.rs](file:///home/hema/Downloads/omni_engine_package/src/causal_memory/store.rs) | 4 |
| [logger.rs](file:///home/hema/Downloads/omni_engine_package/src/logger.rs) | 2 |

**Fix**: Replace all `.lock().unwrap()` with either:
```rust
// Option A: Recover from poison
.lock().unwrap_or_else(|e| e.into_inner())

// Option B: Use parking_lot::Mutex (never poisons)
use parking_lot::Mutex;  // Drop-in replacement, no unwrap needed
```

---

### C2. Hardcoded User-Specific Paths

> [!CAUTION]
> 3 absolute paths in [llama_manager.rs](file:///home/hema/Downloads/omni_engine_package/src/llama_manager.rs#L44-L47) will fail on ANY machine other than the developer's laptop.

```rust
// Line 44 - hardcoded to specific user
PathBuf::from("/home/hema/Downloads/files(1)/M.A.R.K.E.T/bin/llama-server"),
// Line 46
PathBuf::from("/home/hema/Downloads/files(1)/omnicontext_v2/bin/llama-server"),
// Line 47
PathBuf::from("/home/hema/Downloads/files(1)/omnicontext_complete/bin/llama-server"),
```

**Fix**: Replace with `$HOME`-relative or config-file-based paths:
```rust
if let Ok(home) = std::env::var("HOME") {
    candidates.push(PathBuf::from(format!("{}/Downloads/files(1)/M.A.R.K.E.T/bin/llama-server", home)));
}
```

Or better: add a `LLAMA_SERVER_PATH` env var / config file.

---

### C3. Version String Drift

Three different version strings in the codebase:

| Location | String |
|----------|--------|
| [Cargo.toml](file:///home/hema/Downloads/omni_engine_package/Cargo.toml) | `version = "2.0.0"` |
| [main.rs:69](file:///home/hema/Downloads/omni_engine_package/src/main.rs#L69) | `"v2.0.0"` |
| [main.rs:78](file:///home/hema/Downloads/omni_engine_package/src/main.rs#L78) | `"v1.0.1"` |
| [cli.rs:23](file:///home/hema/Downloads/omni_engine_package/src/cli.rs#L23) | `"v1.0.1"` |

**Fix**: Use `env!("CARGO_PKG_VERSION")` macro everywhere:
```rust
println!("OMNI AI ENGINE v{} initialized.", env!("CARGO_PKG_VERSION"));
```

---

## HIGH: Issues That Degrade Reliability

### H1. `#![allow(dead_code)]` Hides Unused Code

> [!WARNING]
> Both [main.rs:1](file:///home/hema/Downloads/omni_engine_package/src/main.rs#L1) and [lib.rs:1](file:///home/hema/Downloads/omni_engine_package/src/lib.rs#L1) suppress all dead-code warnings globally. This means entire functions and structs could be orphaned without any compiler notification.

**Fix**: Remove `#![allow(dead_code)]` and address each warning individually. Functions truly needed for future use can get `#[allow(dead_code)]` per-item.

---

### H2. Silent Error Swallowing (52 instances of `let _ = ...`)

Key concerning patterns:

| File | Line | What's Swallowed |
|------|------|-----------------|
| [auth.rs:25](file:///home/hema/Downloads/omni_engine_package/src/auth.rs#L25) | `let _ = fs::create_dir_all(...)` | Config directory creation failure |
| [auth.rs:47](file:///home/hema/Downloads/omni_engine_package/src/auth.rs#L47) | `let _ = save_keys_to_disk(...)` | API key persistence failure |
| [llama_manager.rs:137](file:///home/hema/Downloads/omni_engine_package/src/llama_manager.rs#L137) | `let _ = fs::write(...pid...)` | PID file write failure (orphan processes) |
| [llama_manager.rs:236](file:///home/hema/Downloads/omni_engine_package/src/llama_manager.rs#L236) | `let _ = child.kill()` | Process kill failure |
| [web_server.rs:129](file:///home/hema/Downloads/omni_engine_package/src/web_server.rs#L129) | `let _ = state.llama_manager.stop()` | Server shutdown cleanup failure |
| [lua_runner.rs:33](file:///home/hema/Downloads/omni_engine_package/src/sandbox/lua_runner.rs#L33) | `let _ = lua.set_hook(...)` | Lua safety hook installation failure |

The Lua safety hook at line 33 is especially dangerous -- if `set_hook` fails silently, the infinite-loop protection is gone and hostile Lua scripts can hang the process forever.

**Fix**: At minimum, log errors. For critical paths (safety hook, PID file), return `Result` or `panic!`.

---

### H3. Server Binds to `0.0.0.0` by Default

[main.rs:90](file:///home/hema/Downloads/omni_engine_package/src/main.rs#L90) and [llama_manager.rs:205](file:///home/hema/Downloads/omni_engine_package/src/llama_manager.rs#L205) bind to all network interfaces. On a laptop connected to public Wi-Fi, the LLM inference endpoint is exposed to the entire network with no authentication enforced by default.

**Fix**: Default to `127.0.0.1` (localhost only). Add `--bind` CLI flag for explicit network exposure.

---

### H4. Zero Test Coverage on Critical Modules

These modules have **no unit tests and no integration test coverage**:

| Module | Lines | Risk |
|--------|-------|------|
| [openai_api.rs](file:///home/hema/Downloads/omni_engine_package/src/openai_api.rs) | 141 | Main API endpoint -- untested |
| [web_server.rs](file:///home/hema/Downloads/omni_engine_package/src/web_server.rs) | 207 | HTTP router and handlers -- untested |
| [cli.rs](file:///home/hema/Downloads/omni_engine_package/src/cli.rs) | 422 | All CLI commands -- untested |
| [tui_agent.rs](file:///home/hema/Downloads/omni_engine_package/src/tui_agent.rs) | 578 | Terminal UI agent -- untested |
| [downloader.rs](file:///home/hema/Downloads/omni_engine_package/src/downloader.rs) | 174 | Model downloads -- untested |
| [llama_manager.rs](file:///home/hema/Downloads/omni_engine_package/src/llama_manager.rs) | 255 | Process lifecycle -- untested |
| [auth.rs](file:///home/hema/Downloads/omni_engine_package/src/auth.rs) | 102 | API key management -- untested |
| [main.rs](file:///home/hema/Downloads/omni_engine_package/src/main.rs) | 101 | Entry point -- untested |

**Total untested**: 1,980 lines / 5,208 = **38% of codebase has zero tests**

These are the modules that handle real user interaction. The modules that ARE tested (sandbox, planner, causal_memory) are internal infrastructure.

---

## MEDIUM: Issues That Cause Maintenance Pain

### M1. Build Artifact Bloat

```
Source code:    524 KB
Build target/:  3.3 GB  (6,300x the source)
```

**Fix**: `cargo clean` periodically, or add `target/` to `.gitignore` (already done) and run `cargo clean --release` after testing.

---

### M2. Lock Contention Architecture

16 separate `Mutex`/`RwLock` objects across the codebase, all using `std::sync::Mutex` (not tokio's async mutex). In an async Axum web server, holding a `std::sync::Mutex` across `.await` points can block the tokio runtime.

**Affected**: Any `Mutex::lock()` call inside `async fn` handlers.

**Fix**: Audit each lock. For short critical sections in sync code, `std::sync::Mutex` is fine. For locks held across async boundaries, switch to `tokio::sync::Mutex`.

---

### M3. No Graceful Shutdown

[web_server.rs:129](file:///home/hema/Downloads/omni_engine_package/src/web_server.rs#L129) calls `llama_manager.stop()` but only on the `/api/model/stop` endpoint. If the main process is killed (Ctrl+C, SIGTERM), the llama-server child process is **orphaned**.

**Fix**: Add `tokio::signal` handler:
```rust
tokio::select! {
    _ = axum::serve(listener, router) => {},
    _ = tokio::signal::ctrl_c() => {
        tracing::info!("Shutting down, stopping llama-server...");
        state.llama_manager.stop().ok();
    }
}
```

---

### M4. No Request Timeouts or Rate Limiting

The OpenAI API proxy in [openai_api.rs](file:///home/hema/Downloads/omni_engine_package/src/openai_api.rs) has no timeout on the `reqwest` call to llama-server. A hung model will hold the HTTP connection indefinitely.

**Fix**: Add `.timeout(Duration::from_secs(300))` to the reqwest client.

---

## LOW: Cleanup and Polish

### L1. Emoji Usage in Production Output

[main.rs:69](file:///home/hema/Downloads/omni_engine_package/src/main.rs#L69), [93-95](file:///home/hema/Downloads/omni_engine_package/src/main.rs#L93-L95) use emoji in stdout. This can break on terminals without Unicode support and may cause issues with log aggregation tools.

### L2. `Cargo.toml` Author Field

```toml
authors = ["Omni AI Team"]
```

Should match actual authorship: `"Ibrahim Elfeqi <ielfeqi@gmail.com>"`.

### L3. No `.env` or Configuration File Support

All configuration is through CLI flags or hardcoded values. No support for `.env` files, `config.toml`, or environment variable overrides for port, model path, bind address, etc.

---

## Prioritized Fix Roadmap

### Phase 1: Safety (Week 1)
1. Replace all 76 `.lock().unwrap()` with poison-safe alternatives
2. Remove 3 hardcoded `/home/hema/` paths
3. Fix Lua safety hook to `panic!` on failure instead of `let _ = ...`
4. Default bind to `127.0.0.1`
5. Add graceful shutdown with signal handler

### Phase 2: Reliability (Week 2)
6. Fix version strings (use `env!("CARGO_PKG_VERSION")`)
7. Add request timeout to OpenAI proxy
8. Log swallowed errors instead of `let _ = ...`
9. Remove `#![allow(dead_code)]` and audit dead code

### Phase 3: Coverage (Week 3-4)
10. Add unit tests for `auth.rs` (key create/validate/revoke)
11. Add integration test for `openai_api.rs` (mock llama-server)
12. Add CLI command tests
13. Add one end-to-end test that actually loads a model and runs inference

### Phase 4: Polish (Week 4+)
14. Add config file support (`.env` or `config.toml`)
15. Audit async/sync lock usage in handlers
16. Clean build artifacts
17. Update Cargo.toml author field
