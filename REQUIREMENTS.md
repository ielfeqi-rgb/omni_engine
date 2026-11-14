# Omni Engine - System Requirements & Dependencies

Omni Engine is an autonomous, in-process software engineering runtime written in 100% pure Rust. It is completely self-contained and decoupled from model weights, allowing zero-cost local execution without cloud APIs or external daemons.

---

## 1. Operating System & Hardware Requirements

### 🐧 Linux (x86_64) — Active Production Core
- **Architecture**: `x86_64` / `amd64` (with AVX2 or AVX-512 support)
- **Kernel**: Linux 5.4+ with unprivileged user namespaces enabled
- **Cleanroom Sandboxing**: Bubblewrap (`bwrap`) for hermetic zero-host-write VFS execution (`apt install bubblewrap` / `dnf install bubblewrap`)
- **Acceleration**:
  - **CPU**: Universal CPU binary with AVX2 vectorization
  - **GPU**: NVIDIA GPU (Compute Capability 7.0+, e.g. Tesla T4, RTX 3000/4000 series) with CUDA drivers (`libggml-cuda.so`)
- **Shared Libraries**: `libllama.so`, `libggml.so`, `libggml-base.so`, `libggml-cpu.so` (pre-bundled in release tarballs)
- **Lifecycle**: Continuous production updates (Dynamic Latest)

### 🍏 macOS Apple Silicon (M1–M4, arm64) — Frozen Hardware Snapshot (v2.2.2)
- **Architecture**: Native Apple Silicon (`arm64`, M1, M2, M3, M4)
- **OS**: macOS 12.0+ (Monterey, Ventura, Sonoma, Sequoia)
- **RAM**: 8 GB minimum unified memory (16 GB+ recommended for 7B models)
- **Cleanroom Sandboxing**: Apple Seatbelt (`sandbox-exec`) fail-closed profile with ephemeral APFS RAM disk
- **Acceleration**: Native Apple Metal framework (`libggml-metal.dylib`)
- **Gatekeeper & Apple Tax**:
  - Unsigned/unnotarized open-source build: The author will **not** pay $99/year to Apple for a code-signing certificate.
  - If macOS blocks execution with *"developer cannot be verified"*, strip Apple's quarantine flag:
    ```bash
    xattr -cr ~/.omni
    ```
- **Lifecycle**: **Permanently Frozen Snapshot (`v2.2.2`)**. Handed over 100% to the open-source community. Zero further author updates.

### 🪟 Windows (x86_64) — Supported via WSL2
- **Architecture**: `x86_64`
- **Subsystem**: Windows Subsystem for Linux (WSL2) with Ubuntu or Debian
- **Requirements**: WSL2 enabled via `wsl --install` in PowerShell
- **Performance**: 100% bare-metal Linux kernel performance with native GPU passthrough

---

## 2. Clarification on Target Language Runtimes

> [!IMPORTANT]
> **Omni Engine is NOT a Python application.** It is written in **100% pure Rust** and compiled to a native ELF/Mach-O executable. It requires **zero external language runtimes** to boot, run, and infer.
>
> Runtimes like Python, Cargo (Rust), and Node.js are **Optional Target Compilers**:
> - If you instruct Omni Engine to generate a **Python** project (`omni run "Build a FastAPI backend"`), the sandbox invokes `python3` to run tests and verify AST syntax.
> - If you instruct Omni Engine to build a **Rust** project, it invokes `cargo` and `rustc`.
> - If you instruct Omni Engine to build a **Node.js** project, it invokes `node` or `npm`.
>
> If you do not intend to synthesize Python projects, Python is **not required** on your system.

---

## 3. Official Upstream llama.cpp Dynamic Linkage

Omni Engine links upstream `llama.cpp` directly in-process via C-FFI for microsecond causal KV-cache steering:

- **Upstream Version**: `v0.6.0-dev`
- **Build Number**: `11429`
- **Git Commit**: `d81235049`

### Pre-Bundled Release Assets:
End-users do **not** need to compile `llama.cpp`. The universal 1-line installer and official GitHub release tarballs bundle pre-verified shared libraries inside `bin/`:
- **Linux CPU**: `libllama.so`, `libggml.so`, `libggml-cpu.so`
- **Linux CUDA**: `libllama.so`, `libggml.so`, `libggml-cuda.so`
- **macOS Metal**: `libllama.dylib`, `libggml.dylib`, `libggml-metal.dylib`

### Building from Source (Custom Flags / Contributors Only):
```bash
git clone https://github.com/ggerganov/llama.cpp.git
cd llama.cpp
cmake -B build -DBUILD_SHARED_LIBS=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j$(nproc)
export LD_LIBRARY_PATH="$(pwd)/build/bin:${LD_LIBRARY_PATH}"
```

---

## 4. Supported GGUF Model Families

Omni Engine automatically probes model vocabulary, prompt templates, stop tokens, and FIM code infill signatures at runtime:

| Model Architecture | Recommended Quant | Context Window | Best Use Case |
| :--- | :--- | :--- | :--- |
| **Qwen 2.5 Coder** (1.5B / 7B) | `Q4_K_M` / `Q6_K` | 4,096 – 16,384 tokens | Native FIM code infill, high AST accuracy |
| **Microsoft Phi-4** (Mini) | `Q4_K_M` | 4,096 – 8,192 tokens | High logic density, compact reasoning |
| **Google Gemma 2** (2B / 9B) | `Q4_K_M` | 4,096 – 8,192 tokens | Precise instruction following |
| **DeepSeek Coder v2** | `Q4_K_M` | 4,096 – 16,384 tokens | Multi-file algorithmic architecture |
