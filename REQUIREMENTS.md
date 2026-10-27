# Omni Engine - System Requirements & Dependencies

Omni Engine is completely decoupled from model weights and external library builds. It does not bundle heavy model weights or custom `llama.cpp` builds, preserving full freedom to update upstream libraries.

---

## 1. System Requirements

### Host Dependencies
- **Rust Toolchain**: `rustc` / `cargo` (1.80+ recommended)
- **C/C++ Compiler**: `clang` or `gcc` (with C11 / C++17 support)
- **CMake & Ninja**: (for building upstream `llama.cpp`)
- **Bubblewrap (`bwrap`)**: Linux user namespace container isolation for zero-write sandbox cleanrooms (`sudo apt install bubblewrap` / `sudo dnf install bubblewrap`)

### ⚠️ Important Notice: llama.cpp and Models are NOT Bundled
Omni Engine is completely decoupled from model weights and external library builds. **It does not bundle `llama.cpp` binaries or model weights** in this repository. It links dynamically against official upstream `llama.cpp` shared libraries, ensuring:
- Full freedom to update upstream `llama.cpp` versions without waiting for engine patches.
- Native compiler optimization for your exact CPU/GPU architecture (AVX, AVX2, AVX-512, CUDA, Metal, ROCm, Vulkan).
- Clean, lightweight source repository without platform-specific binary or model bloat.

### Verified Upstream llama.cpp Build
The current production release (`v2.1.2`) is verified against:
- **Upstream Version:** `v0.6.0-dev`
- **Build Number:** `11429`
- **Git Commit:** `d81235049`
- **Dynamic Libraries Required:** `libllama.so.0.6.0`, `libggml.so.0.26.0`, `libggml-base.so.0.26.0`, `libggml-cpu.so`

### llama.cpp Shared Libraries Setup

Omni Engine dynamically links against `llama.cpp` (`libllama.so`, `libggml.so`, `libggml-base.so`, `libggml-cpu.so`).

#### Option A: Quick Build from Source (Recommended, ~2 minutes)
```bash
# 1. Clone official upstream llama.cpp
git clone https://github.com/ggerganov/llama.cpp.git
cd llama.cpp

# 2. Build shared libraries (CPU or with GPU acceleration flags)
cmake -B build -DBUILD_SHARED_LIBS=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release -j$(nproc)

# 3. Export LLAMA_CPP_DIR or copy shared libraries
export LLAMA_CPP_DIR=$(pwd)/build/bin
export LD_LIBRARY_PATH="${LLAMA_CPP_DIR}:${LD_LIBRARY_PATH}"
```

#### Option B: Local Project Directory
Place your compiled shared libraries directly into `./bin/` or `~/.omni/bin/`:
- `libllama.so`
- `libggml.so`
- `libggml-base.so`
- `libggml-cpu.so`

#### Option C: System-Wide Installation
Install the shared libraries into `/usr/local/lib` or `/usr/lib`, where Omni Engine's linker searches automatically.

---

## 2. Recommended GGUF Models

Omni Engine automatically detects model vocabulary, prompt framing (ChatML, Llama-3, Phi-4, Gemma, Spark-X2.5, or fallback), stop tokens, and FIM infill capabilities using its **Universal Triangulated Detection Protocol & Empirical Chamber Verification**.

You can run Omni Engine with any of the following models:

| Model Architecture | Recommended GGUF File | Family | Strengths |
| :--- | :--- | :--- | :--- |
| **Qwen 2.5 Coder** | `qwen2.5-coder-1.5b-instruct-q4_k_m.gguf` | ChatML | Ultrafast 1.5B agent, native FIM infill, highly accurate syntax generation |
| **Microsoft Phi-4** | `phi-4-mini-instruct-q4_k_m.gguf` | Phi-4 | High reasoning density, compact math and logic |
| **iFlytek Spark-X** | `spark-x2.5-4b-instruct-q4_k_m.gguf` | Spark / Hybrid | Sliding-window hybrid attention |
| **Google Gemma 2** | `gemma-2-2b-it-q4_k_m.gguf` | Gemma | Clean instruction adherence, lightweight footprint |

---

## 3. How Omni Engine Adapts Dynamically

When a model is loaded:
1. **Layer 1: Structural Vocabulary Signature Probing**: Probes tokenizer single-token IDs for signature tokens (`<|im_start|>`, `<|start_header_id|>`, `<|user|>`, `<start_of_turn>`, `<｜start·of·sentence｜>`).
2. **Layer 2: Assistant-Prefill Chamber Verification**: Executes a zero-temperature prefill test (`Echo exactly: TEST`) to empirically capture the model's exact native EOT / EOS token ID.
3. **Layer 3: FIM Code Infill Signature**: Identifies code infilling tokens (`<|fim_prefix|>` vs `<fim_prefix>`).
4. **Layer 4: Manifest Caching**: Writes a cached descriptor to `<model_path>.omni.json` for zero-overhead subsequent startups.
