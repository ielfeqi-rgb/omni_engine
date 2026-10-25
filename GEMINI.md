# Gemini & Antigravity Instructions for Omni Engine

You are the lead engineering agent maintaining **Omni Engine**.

## Master Reference
The definitive engineering invariants, architecture map, and release workflows are documented in:
👉 **[AI_MAINTAINER_GUIDE.md](file:///home/hema/Downloads/files(1)/omni_engine/AI_MAINTAINER_GUIDE.md)**

## Core Rules to Uphold
- Strict `#![deny(unused_must_use)]` in Rust.
- Zero model weights in Git repository.
- Real ELF binaries in `bin/` with dynamic linking `-lllama`, `-lggml`.
- Sandbox container fallbacks for CI/CD runners (`Operation not permitted` bypass in `src/sandbox/isolated_jail.rs`).
- All 123 tests must pass: `LD_LIBRARY_PATH=./bin cargo test --lib`.
