# Omni AI Engine v1.0 (Standalone Rust Edition)

> **Note**: This is an independent, standalone AI engine project. It is completely separate from M.A.R.K.E.T AI and operates as a standalone zero-dependency Rust executable.

---

## 🚀 Overview

**Omni AI Engine** is a high-performance, standalone AI Engine written in **Rust** that wraps `llama.cpp`. It compiles into a single, self-contained executable binary with zero external runtime dependencies. 

It provides an embedded browser Web UI for local chat, an on-demand GGUF model downloader from custom URLs (e.g. HuggingFace), exposes an **OpenAI-compatible REST API (`/v1/chat/completions`)**, and enforces **API Key Security Middleware** for network accessibility.

---

## 📌 Release Policy & License

* **Single Final Release (v1.0)**: This project is released as a complete, standalone single-version release. No further updates or maintenance are planned.
* **License**: **Unrestricted Free & Open Source (MIT / Public Domain)**. Free for everyone to use, modify, distribute, or integrate into any personal, educational, or commercial applications without restrictions.

---

## ✨ Features

1. **Bare-Metal Performance**:
   - Written in Rust with no Garbage Collector or Virtual Machine overhead.
   - Axum + Tokio server operating directly on native Linux kernel sockets (`epoll`).
   - Native C/C++ `llama-server` backend utilizing AVX2/AVX-512 SIMD tensor acceleration.

2. **100% OpenAI API Compatible**:
   - Endpoints: `POST /v1/chat/completions` and `GET /v1/models`.
   - Supports Server-Sent Events (SSE) streaming (`text/event-stream`).
   - Integrates seamlessly with Python `openai`, JS SDKs, Cursor, and Open WebUI.

3. **API Key Security Middleware**:
   - Protects `/v1/*` endpoints with `Authorization: Bearer omni_sk_...` tokens.
   - Allows safe binding to local network and internet endpoints (`0.0.0.0:8090`).
   - Generate and revoke API keys via the Web UI.

4. **GGUF Model Downloader**:
   - Download any GGUF model directly from a URL inside the browser dashboard.
   - Real-time download progress bar and status tracking.

5. **Single-Binary Web UI**:
   - Embedded single-page Web UI inside the compiled binary.
   - Interactive local chat, model selection, engine controls (Start/Stop), and API key management.

---

## 📁 Release Packages

Pre-built release packages are located in the `releases/` folder:

* 🐧 **Linux (x86_64)**: `releases/omni_engine_linux_x86_64.tar.gz`
* 🪟 **Windows (x64)**: `releases/omni_engine_windows_x64.tar.gz`
* 🍎 **Mac (macOS)**: `releases/omni_engine_mac.txt` *(Note: "I don't like Mac")*

---

## 🛠️ Quick Start

### Running the Binary
```bash
./target/release/omni_engine
```

### Accessing Dashboard & Endpoints
* **Web UI Dashboard**: `http://127.0.0.1:8090`
* **OpenAI API Base**: `http://127.0.0.1:8090/v1`

---

## 🌐 Arabic Summary / ملخص عربي

**محرك Omni AI Engine v1.0**: محرك ذكاء اصطناعي مستقل ومجمع بالكامل بلغة **Rust**.
- المشروع منفصل ومستقل تماماً وله ريبو خاص به دون أي ارتباط بمشاريع أخرى.
- **الترخيص**: مفتوح ومجاني بالكامل للجميع بدون أي قيود استخدام تجارية أو شخصية.
- **الإصدار**: هذا هو الإصدار النهائي الكامل v1.0.
- **الخصائص**: يعمل بأقصى سرعة مباشرة على المعالج (Bare-Metal)، يتضمن واجهة متصفح مدمجة داخل الملف التنفيذي، تنزيل الموديلات بصيغة GGUF من أي رابط مباشر، توافقية 100% مع معايير OpenAI API، ونظام حماية بمفاتيح الـ API Keys.
