# GitHub Copilot Instructions for Omni Engine

Omni Engine is an offline, sovereign multi-agent Rust runtime linking `llama.cpp` via C-FFI.

### Mandatory Rules
1. Refer to `AI_MAINTAINER_GUIDE.md` for architectural invariants and test workflows.
2. Keep `#![deny(unused_must_use)]` enabled at all times.
3. Keep the repository free of any `.gguf` model weights.
4. Always test using `LD_LIBRARY_PATH=./bin cargo test --lib`.
5. Keep admin endpoints on the web server protected with `check_admin_auth()`.
