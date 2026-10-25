# Omni Engine — AI Lead Maintainer & Engineering Handover Guide
**Target Audience:** Any AI Coding Agent (Antigravity, Claude Code, Cursor, Copilot, ChatGPT) or Human Lead Engineer taking over maintenance, evolution, or refactoring of the `omni_engine` codebase.  
**Repository:** `https://github.com/ielfeqi-rgb/omni_engine`  
**Current Release:** `v2.1.0` (Sovereign Autonomous Runtime)  
**Encoding:** UTF-8 • Markdown Specification

---

## 1. Executive Identity & System Philosophy

Omni Engine is **NOT** a conventional wrapper around cloud LLM APIs. It is a **100% sovereign, offline, multi-agent compiler runtime** written in high-performance Rust.

### Core Architectural Axioms:
1. **Zero External Cloud Telemetry**: Core runtime operations (code synthesis, refactoring, Cleanroom testing, and causal rollback) execute completely offline.
2. **In-Process C-FFI Inference**: Inference is performed in-process by linking `llama.cpp` and `ggml` dynamic shared objects (`libllama.so`, `libggml*.so`) located in `bin/` directly into memory.
3. **RAM Virtual File System (VFS)**: Code synthesis and multi-file DAG steps occur in an in-memory virtual filesystem before committing verified deliverables to the host storage.
4. **Cleanroom & Zero-Wipe Invariant**: During codebase modifications (`omni_engine modify`), existing AST definitions (functions, classes, schemas) are strictly preserved (`+lines / -0 regressions`). Output code undergoes strict compilation checks (`py_compile`, `cargo check`) inside a Bubblewrap container sandbox.

---

## 2. Inviolable Engineering Invariants (Hard Rules)

Any AI assistant modifying this repository **MUST** adhere to the following rules without exception:

### ⚠️ RULE 1: `#![deny(unused_must_use)]` Is Absolute
- **File**: `src/main.rs` (and all Rust modules).
- **Mandate**: Every single `Result` and `Option` must be explicitly handled via pattern matching, `?`, or structured error handling.
- **Forbidden**: Never add `#![allow(unused_must_use)]` or use `let _ = ...` to swallow unexpected I/O, IPC, or inference failures.

### ⚠️ RULE 2: Zero Bundled Model Weights in Repository
- The Git repository must **NEVER** contain `.gguf` files or model weights.
- Models are downloaded dynamically on-demand using the built-in downloader:
  ```bash
  omni_engine download <preset|URL> [filename]
  # Examples:
  omni_engine download --qwen       # 1.04GB baseline model
  omni_engine download --qwen-7b    # 4.68GB coding model
  omni_engine download --phi-4      # 9.12GB high-reasoning model
  ```
- Any model files in `models/` must remain in `.gitignore`.

### ⚠️ RULE 3: Dynamic Library (`bin/`) & Linker Integrity
- `bin/` contains real, dereferenced ELF dynamic libraries (`libllama.so`, `libggml.so`, etc.).
- **DO NOT** replace these with relative dangling symlinks (`libllama.so -> libllama.so.0.6.0`). In clean CI/CD environments (like GitHub Actions), relative symlinks to uncommitted targets break the linker with `rust-lld: error: unable to find library -lllama`.
- When running `cargo test` or executing the binary, always configure `LD_LIBRARY_PATH`:
  ```bash
  LD_LIBRARY_PATH=./bin cargo test --lib
  LD_LIBRARY_PATH=./bin ./bin/omni_engine --help
  ```

### ⚠️ RULE 4: Sandbox & Container Isolation Compatibility
- Process isolation is implemented in `src/sandbox/isolated_jail.rs` using Bubblewrap (`bwrap`).
- **Container Environment Pitfall**: Inside Docker containers or GitHub Actions runners, unprivileged network namespaces are forbidden by kernel security profiles (`bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted`).
- **Mandate**: `is_bwrap_available()` must probe real namespace creation (`bwrap --ro-bind / / --unshare-all /usr/bin/true`), NOT just check `bwrap --version`. If `bwrap` fails with `Operation not permitted` or is missing, `run_in_jail()` must gracefully fall back to `run_fallback` without failing tests or crashing the host.

### ⚠️ RULE 5: Web Server Security & Host Isolation
- The Web Server (`src/web_server.rs`) provides OpenAI-compatible REST endpoints (`/v1/chat/completions`) and a Web UI.
- **Admin Endpoints**: `/api/shutdown`, `/api/server/*`, `/api/keys*`, and `/api/models/download` must strictly enforce Bearer Token authentication via `check_admin_auth()`.
- **CSRF & Origin Blocking**: Reject non-loopback origins or cross-site requests (`sec-fetch-site: cross-site`) unless validated by a valid Bearer API key.
- By default, bind only to `127.0.0.1`.

### ⚠️ RULE 6: Thread-Safety & C-FFI Invariants
- `src/native_llama/mod.rs` wraps raw pointers `*mut llama_model` and `*mut llama_context`.
- `unsafe impl Send` and `unsafe impl Sync` for `NativeLlamaModel` are safe **ONLY** because:
  1. `llama_model` weights are read-only in memory.
  2. Tokenization is serialized using `tokenize_lock: Mutex<()>`.
  3. Inference contexts are allocated per-worker or guarded against concurrent evaluation.
- Never remove `tokenize_lock` or execute unsynchronized tokenization across multiple threads.

---

## 3. Subsystem Architecture & Code Map

```
omni_engine/
├── src/
│   ├── main.rs                 # CLI entry point, signal handlers, error handling
│   ├── cli.rs                  # CLI parser, console loop, native GGUF downloader
│   ├── orchestration/          # Swarm coordinator (Thinker, Planner, Coder, Critic)
│   ├── causal_memory/          # Turn sealing, KV-cache rewinding, prompt excision
│   ├── native_llama/           # C-FFI bindings to llama.cpp / ggml
│   ├── cleanroom/              # Zero-wipe AST verification & compiler checks
│   ├── sandbox/                # Bubblewrap process isolation & container fallbacks
│   ├── vfs/                    # In-memory RAM Virtual File System
│   └── web_server.rs           # Axum REST server (OpenAI API + Web Admin UI)
├── bin/                        # Pre-compiled ELF dynamic libraries & release binary
│   ├── libllama.so / .so.0     # Dynamic shared library for llama.cpp
│   ├── libggml*.so / .so.0     # Dynamic shared libraries for ggml acceleration
│   └── omni_engine             # Stripped production release binary
├── build.rs                    # Rust linker configuration (-lllama, -lggml)
├── Cargo.toml                  # Dependency specifications
└── .github/workflows/rust.yml  # GitHub Actions CI/CD matrix
```

---

## 4. Engineering & Development Workflow

### 4.1 Running Tests
Omni Engine maintains a **100% test pass rate** (123+ unit and invariant tests).
Always execute tests with `LD_LIBRARY_PATH` set:
```bash
LD_LIBRARY_PATH=./bin cargo test --lib
```
To run integration tests:
```bash
LD_LIBRARY_PATH=./bin cargo test --tests
```

### 4.2 Building Production Release
```bash
cargo build --release
strip target/release/omni_engine
cp target/release/omni_engine bin/omni_engine
```

### 4.3 Testing the Built-in Downloader
```bash
# Test help & presets
./target/release/omni_engine download --help
# Test interactive console download command
./target/release/omni_engine
omni❯ download --qwen
```

---

## 5. Release & Distribution Packaging Pipeline

When publishing a new release (e.g., `v2.1.0`):
1. **Verify All Tests Pass**:
   ```bash
   LD_LIBRARY_PATH=./bin cargo test --lib
   ```
2. **Build Release Binary**:
   ```bash
   cargo build --release
   strip target/release/omni_engine
   ```
3. **Assemble Release Package**:
   ```bash
   mkdir -p target/release_pkg/bin
   cp target/release/omni_engine target/release_pkg/bin/
   cp bin/libllama.so* target/release_pkg/bin/
   cp bin/libggml*.so* target/release_pkg/bin/
   cp README.md LICENSE install.sh target/release_pkg/
   cd target/release_pkg
   tar -czvf ../omni_engine_v2.1.0_linux_x86_64.tar.gz bin README.md LICENSE install.sh
   cd ..
   sha256sum omni_engine_v2.1.0_linux_x86_64.tar.gz > omni_engine_v2.1.0_linux_x86_64.tar.gz.sha256
   ```
4. **Publish to GitHub Release**:
   - Upload tarball and sha256 to GitHub Release assets via GitHub API.
   - Tag release with `git tag v2.1.0` and push to `origin`.

---

## 6. Historical Lessons & "Do Not Touch" Pitfalls

| Subsystem | Historical Incident | Permanent Fix / Lesson |
| :--- | :--- | :--- |
| **C-FFI Dynamic Linking** | `rust-lld: error: unable to find library -lllama` in CI. | `bin/` previously had relative symlinks that pointed to ignored files. Now dereferenced into real ELF binaries (`lib*.so.0`) tracked in Git. Never reintroduce unverified symlinks. |
| **Bubblewrap Sandbox** | GitHub Actions runner failed with `loopback: Failed RTM_NEWADDR: Operation not permitted`. | Container environments block loopback creation. Fixed by checking actual sandbox execution (`bwrap ... /usr/bin/true`) and falling back gracefully without failing test suite. |
| **Admin Web Endpoints** | Audit flagged `/api/shutdown` and `/api/server/*` without auth. | Protected with `check_admin_auth()`, CSRF origin validation, and Bearer API keys. Never expose unauthenticated mutation endpoints. |
| **Model Weights** | Repository bloat with large GGUF files in Git history. | Zero models in git. Always use native `omni_engine download <preset|URL>`. |
| **License Ambiguity** | Audit flagged conflicting MIT / third-party copyright headers. | Root `LICENSE` clearly establishes Omni AI Team & Mohamed Elfeqi as copyright holders with explicit third-party attribution to `llama.cpp` and `bubblewrap`. |

---

## 7. Handover Checklist for the Incoming AI Maintainer

Before making any commits or opening pull requests:
- [ ] Read `AI_MAINTAINER_GUIDE.md` (this file) and `SECURITY_AUDIT.md`.
- [ ] Confirm `#![deny(unused_must_use)]` is intact in `src/main.rs`.
- [ ] Run `LD_LIBRARY_PATH=./bin cargo test --lib` (must report 0 failures).
- [ ] Ensure no `.gguf` files, model weights, or API keys are committed.
- [ ] Verify that changes to `src/sandbox/` do not break containerized CI execution.
