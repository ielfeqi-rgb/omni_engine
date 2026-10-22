# Omni Engine - System Requirements & Dependencies

Omni Engine is completely decoupled from model weights and external library builds. It does not bundle heavy model weights or custom `llama.cpp` builds, preserving full freedom to update upstream libraries.

---

## 1. System Requirements

### Host Dependencies
- **Rust Toolchain**: `rustc` / `cargo` (1.80+ recommended)
- **C/C++ Compiler**: `clang` or `gcc` (with C11 / C++17 support)
- **CMake & Ninja**: (for building upstream `llama.cpp`)
- **Bubblewrap (`bwrap`)**: Linux user namespace container isolation for zero-write sandbox cleanrooms (`sudo apt install bubblewrap` / `sudo dnf install bubblewrap`)

### llama.cpp Shared Libraries
Omni Engine links dynamically against `llama.cpp`. You can either:
1. Place the compiled shared libraries (`libllama.so`, `libggml.so`, `libggml-base.so`, `libggml-cpu.so`) in `./bin/` or `~/.omni/bin/`.
2. Install them system-wide into `/usr/local/lib` or `/usr/lib`.
3. Set the `LLAMA_CPP_DIR` environment variable to your custom `llama.cpp` build directory:
   ```bash
   export LLAMA_CPP_DIR=/path/to/llama.cpp/build/bin
   ```

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
