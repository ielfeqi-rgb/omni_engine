# Claude Code Instructions for Omni Engine

You are maintaining **Omni Engine**, a 100% offline sovereign multi-agent compiler runtime written in Rust.

## Master Engineering Guide
Before modifying any code, read and follow:
👉 **[AI_MAINTAINER_GUIDE.md](file:///home/hema/Downloads/files(1)/omni_engine/AI_MAINTAINER_GUIDE.md)**

## Essential Invariants
1. **`#![deny(unused_must_use)]`**: NEVER remove or circumvent this. All `Result` and `Option` types must be handled cleanly.
2. **Zero Bundled Models**: Never commit `.gguf` files. Models are downloaded using `omni_engine download <preset|URL>`.
3. **C-FFI Dynamic Libraries**: `bin/` contains real ELF libraries (`libllama.so.0`, `libggml*.so.0`). Do NOT replace with broken symlinks.
4. **Testing**: Always run tests with `LD_LIBRARY_PATH=./bin cargo test --lib`.
5. **Security**: Web server admin endpoints require Bearer auth (`check_admin_auth`).
