# Omni AI Engine v1.0.1 (Standalone Rust Edition)

> **Standalone Project Notice**: This project is a completely independent AI engine written in Rust. It operates autonomously and bears no relationship to any other repository or framework.

---

## 🚀 Technical Architecture Overview

**Omni AI Engine** is a single-binary, zero-runtime-dependency AI execution engine compiled natively in **Rust** wrapping `llama.cpp`. 

It features a full **Command-Line Interface (CLI)**, an embedded web dashboard, an on-demand GGUF model downloader supporting HTTP redirects, an **OpenAI-compatible REST API (`/v1/chat/completions`)**, and an **API Key security middleware layer**.

---

## 📌 Release Policy & License

### Release v1.0.1
Version 1.0.1 introduces a comprehensive native **Terminal CLI & Engine Controller** allowing users to query system diagnostics, start and stop the local engine, run inference prompts directly from the console, manage API keys, and launch the web server.

### Unrestricted Open Source License
This project is licensed under the **Unrestricted MIT / Public Domain License**. You are granted full rights to use, modify, fork, commercialize, or redistribute this software without seeking prior permission.

### Arabic Summary / ملخص باللغة العربية
**مشروع محرك Omni AI Engine v1.0.1**: محرك ذكاء اصطناعي مستقل ومجمع بالكامل بلغة Rust.
- **الترخيص**: حر ومفتوح المصدر بالكامل (MIT / Public Domain). لك مطلق الحرية في استخدامه أو تعديله أو بيعه أو إعادة توزيعه دون إذن من المطور.
- **الجديد في v1.0.1**: إضافة تحكم كامل من سطر الأوامر (Terminal CLI) مع دعم أوامر الفحص (`status`, `models`)، والتشغيل والإيقاف (`start`, `stop`)، وطرح الأسئلة المباشرة أو الدردشة من التيرمينال (`ask`, `chat`)، وإدارة مفاتيح الـ API (`keys`).

---

## 📜 Privacy Statement

We do not collect, process, or sell user data. This is not driven by external privacy mandates, but rather by the architecture of the engine: it operates 100% offline on your local device, and we own no remote servers to receive data even if we attempted to do so. Your conversation history resides and expires strictly within your local system's RAM.

---

## 💻 Native Terminal CLI Usage

Omni AI Engine includes a full CLI controller directly accessible from your terminal:

```bash
# 1. Inspect Hardware Specs & Engine Status
./omni_engine status

# 2. List Installed GGUF Models
./omni_engine models

# 3. Start Local Engine with a Model
./omni_engine start qwen2.5-1.5b.gguf --port 8081 --ctx 2048

# 4. Ask a Quick Question Directly from Terminal
./omni_engine ask "Explain how TCP handshake works in simple terms"

# 5. Interactive Multi-Turn Terminal Chat Session
./omni_engine chat

# 6. Manage API Keys
./omni_engine keys list
./omni_engine keys new "My-Server-Key"
./omni_engine keys revoke <KEY_ID>

# 7. Stop the Running Engine
./omni_engine stop

# 8. Launch the Web UI Dashboard & OpenAI HTTP Proxy
./omni_engine serve --port 8090
```

---

## ✨ Core Features & Specifications

* **Command-Line & Headless First**: Full CLI support for scripting, remote servers, and automation without opening a browser.
* **Bare-Metal Native Execution**: Developed in Rust without Garbage Collection or Virtual Machine overhead. Serves HTTP requests directly via Linux kernel socket polling (`epoll`).
* **100% OpenAI API Compatibility**: Exposes standard `/v1/chat/completions` endpoints supporting Server-Sent Events (SSE) streaming for direct integration with Cursor, Python `openai`, and Open WebUI.
* **Hardware Diagnostics & Parameter Estimator**: Inspects system RAM and CPU capabilities to estimate maximum GGUF model sizes (7B, 13B, 32B, 70B+).
* **Live Terminal Log Streaming**: Embedded auto-scrolling log console (`GET /api/logs`) monitoring server events in real time.
* **API Key Auth Guard**: All external `/v1/*` endpoints require `Authorization: Bearer omni_sk_...` tokens.
* **Trilingual Embedded UI**: Built-in Web UI supporting **Arabic, English, and French** with dynamic `RTL`/`LTR` layout switching.

---

## 📁 Pre-built Releases

Compiled distribution binaries and release packages are published on the GitHub Releases page:

* 🔗 **Official Releases**: [https://github.com/ielfeqi-rgb/omni_engine/releases](https://github.com/ielfeqi-rgb/omni_engine/releases)
* 🐧 **Linux (x86_64)**: `releases/omni_engine_linux_x86_64.tar.gz`
* 🪟 **Windows (x64)**: `releases/omni_engine_windows_x64.zip` *(Contains `omni_engine.exe`)*
* 🍎 **macOS**: `releases/omni_engine_mac.txt` *(Contains official notice: "I don't like Mac")*

---

## 🛠️ Installation & Quick Start

### Build & Run from Source
```bash
cargo build --release
./target/release/omni_engine serve
```

### Access Ports
* **Web UI Dashboard**: `http://127.0.0.1:8090`
* **OpenAI Endpoint**: `http://127.0.0.1:8090/v1`
