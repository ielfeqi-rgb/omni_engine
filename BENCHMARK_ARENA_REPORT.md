# Omni Engine Multi-Model Arena Benchmark Report

Empirical evaluation of all 5 local models on logic-rich Interactive Web UI and Python GUI generation with Cleanroom verification.

---

## 1. Comparative Performance Matrix

| Model | Parameters | Quantization | Architecture | Task 1: Web Canvas (tok/s / Valid) | Task 2: Python GUI (tok/s / Valid) | Overall Score | Verdict |
| :--- | :---: | :---: | :--- | :---: | :---: | :---: | :--- |
| **Qwen2.5-Coder-1.5B** | 1.5B | Q4_K_M | ChatML | **3.8 tok/s** (5/5) ✅ | **3.8 tok/s** (4/5) ✅ | **9/10** 🏆 | **Best Coding Engine**: Instant start, concise working code, zero truncation |
| **Qwen3-4B** | 4.0B | Q6_K | ChatML (Reasoning) | 1.9 tok/s (3/5) ⚠️ | 1.9 tok/s (3/5) ⚠️ | **6/10** | **Deep Reasoning Model**: Uses `<think>` chain-of-thought; needs higher token budget |
| **Phi-4-mini-instruct** | 3.8B | Q6_K | Phi-4 | 1.3 tok/s (4/5) ✅ | 1.3 tok/s (1/5) ⚠️ | **5/10** | **Solid Generalist**: High quality Web UI, slower execution on 4-core CPU |
| **Spark-X2.5-4B** | 4.1B | Q4_K_M | Spark-X2.5 (SWA) | 2.3 tok/s (2/5) ⚠️ | 1.7 tok/s (2/5) ⚠️ | **4/10** | **Advanced UI Layout**: Highly detailed CSS and OOP structure, truncated at 650 tok |
| **Google-Gemma-2B** | 2.0B | Q4_K_M | Gemma | 3.1 tok/s (2/5) ❌ | 3.0 tok/s (1/5) ❌ | **3/10** | **Underperformer**: Fast generation but frequent syntax truncations |

---

## 2. Detailed Task Breakdown & Code Analysis

### 🥇 1. Qwen2.5-Coder-1.5B
- **Weights File**: `models/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf` (1.1 GB)
- **Profile**: `models/qwen2.5-coder-1.5b-instruct-q4_k_m.omni` (Chamber Verified)
- **HTML5 Retro Canvas Game**: Generated **421 tokens in 111.4s (3.8 tok/s)**. Syntax: **VALID**. Feature Score: **5/5**.
  - Produced a self-contained, playable arcade breakout game with keyboard controls, ball bouncing physics, score counter, and game-over restart loop.
- **Python Tkinter Snake Game**: Generated **440 tokens in 116.1s (3.8 tok/s)**. Syntax: **VALID**. Feature Score: **4/5**.
  - Produced a standalone executable Python Tkinter GUI application with arrow key bindings, food spawning, collision detection, and score rendering.

### 🥈 2. Qwen3-4B
- **Weights File**: `models/Qwen3-4B-Q6_K.gguf` (3.1 GB)
- **Profile**: `models/Qwen3-4B-Q6_K.omni` (Upgraded llama.cpp `b11429` runtime)
- **HTML5 Retro Canvas Game**: Generated **650 tokens in 349.5s (1.9 tok/s)**. Feature Score: **3/5**.
- **Python Tkinter Snake Game**: Generated **650 tokens in 346.4s (1.9 tok/s)**. Feature Score: **3/5**.
- **Key Discovery**: Qwen3-4B natively operates as a **Reasoning / Thinking Model** (similar to DeepSeek-R1). It opens a `<think>` block to analyze physics equations, coordinate systems, and edge cases before outputting code. Because the arena was capped at 650 tokens, the token budget was partially consumed by internal chain-of-thought reasoning.

### 🥉 3. Phi-4-mini-instruct
- **Weights File**: `models/Phi-4-mini-instruct-Q6_K.gguf` (3.0 GB)
- **Profile**: `models/Phi-4-mini-instruct-Q6_K.omni` (Chamber Verified)
- **HTML5 Retro Canvas Game**: Generated **650 tokens in 486.2s (1.3 tok/s)**. Syntax: **VALID**. Feature Score: **4/5**.
  - Excellent styling and clean code separation.
- **Python Tkinter Snake Game**: Generated **250 tokens in 189.4s (1.3 tok/s)**. Feature Score: **1/5**.

### 4. Spark-X2.5-4B
- **Weights File**: `models/Spark-X2.5-4B-Q4_K_M.gguf` (2.5 GB)
- **Profile**: `models/Spark-X2.5-4B-Q4_K_M.omni` (Hybrid Sliding Window Attention)
- **HTML5 Retro Canvas Game**: Generated **650 tokens in 284.1s (2.3 tok/s)**. Feature Score: **2/5**.
  - Created retro neon styling with orange glowing borders and overlays.
- **Python Tkinter Snake Game**: Generated **650 tokens in 385.8s (1.7 tok/s)**. Feature Score: **2/5**.
  - Wrote a modular, clean object-oriented `SnakeGame` class.

### 5. Google-Gemma-2B
- **Weights File**: `models/gemma-2b.gguf` (1.7 GB)
- **Profile**: `models/gemma-2b.omni`
- **HTML5 Retro Canvas Game**: Generated **650 tokens in 209.6s (3.1 tok/s)**. Syntax: **FAILED**.
- **Python Tkinter Snake Game**: Generated **650 tokens in 216.7s (3.0 tok/s)**. Syntax: **FAILED**.
  - Generated code was cut off prematurely and failed Cleanroom AST validation.

---

## 3. Sovereign Profiles Generated & Cached

All models now have persistent sovereign `.omni` profiles in the `models/` directory:
- `models/qwen2.5-coder-1.5b-instruct-q4_k_m.omni`
- `models/Qwen3-4B-Q6_K.omni`
- `models/Phi-4-mini-instruct-Q6_K.omni`
- `models/Spark-X2.5-4B-Q4_K_M.omni`
- `models/gemma-2b.omni`
