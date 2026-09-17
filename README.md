# Omni AI Engine (v1.0.1)

> Independent, zero-dependency local AI execution engine written in Rust over `llama.cpp`.

---

## Technical Overview

**Omni AI Engine** is a single-binary inference backend compiled in **Rust**. It provides high-throughput, low-latency execution with an embedded management dashboard, a CLI controller, and a standard **OpenAI-compatible REST API (`/v1/chat/completions`)** with API key authentication.

---

## What's New in v1.0.1

- **Native CLI & Engine Controller**: Comprehensive terminal interface to manage system hardware status, model states, interactive chat sessions, and service daemons directly from the shell.
- **Improved Process Management**: Automated tracking of child processes with PID persistence and clean signal handling.
- **Port Flexibility**: Runtime binding configuration via `--port` for headless and server environments.

---

## Command-Line Interface (CLI)

The engine can be fully operated without a graphical interface:

```bash
# Display hardware profile and status
./omni_engine status

# List downloaded GGUF models
./omni_engine models

# Launch backend with a specific model
./omni_engine start qwen2.5-1.5b.gguf --port 8081 --ctx 2048

# Execute a one-shot inference query
./omni_engine ask "Explain TCP three-way handshake concisely"

# Start an interactive terminal session
./omni_engine chat

# Manage API tokens
./omni_engine keys list
./omni_engine keys new "Production-Key"
./omni_engine keys revoke <KEY_ID>

# Stop background instance
./omni_engine stop

# Start Web UI dashboard & OpenAI HTTP endpoint
./omni_engine serve --port 8090
```

---

## Core Specifications

* **Zero Runtime Dependencies**: Compiled to a standalone native binary without external runtime or interpreter requirements.
* **OpenAI API Compatibility**: Standard `/v1/chat/completions` supporting streaming (SSE) for easy drop-in use with Cursor, Open WebUI, and standard SDKs.
* **Hardware Sizing Diagnostics**: Real-time inspection of system memory and thread topology to determine optimal context sizes and quantization profiles.
* **Security Layer**: Bearer token middleware (`Authorization: Bearer omni_sk_...`) protecting inference endpoints.
* **Multi-Language Web Dashboard**: Embedded control UI supporting Arabic, English, and French.

---

## Binary Distributions

Pre-built binaries are available in the repository `releases/` directory and on GitHub Releases:

* **Official Releases**: [https://github.com/ielfeqi-rgb/omni_engine/releases](https://github.com/ielfeqi-rgb/omni_engine/releases)
* **Linux (x86_64)**: `releases/omni_engine_linux_x86_64.tar.gz`
* **Windows (x64)**: `releases/omni_engine_windows_x64.zip`

---

## Build from Source

```bash
# Build release target
cargo build --release

# Run web service
./target/release/omni_engine serve
```

### Endpoints
* **Web UI Dashboard**: `http://127.0.0.1:8090`
* **OpenAI API Base**: `http://127.0.0.1:8090/v1`

---

## License

Distributed under the **MIT License**. Permitted for both non-commercial and commercial application with no warranty.

---

## التوثيق باللغة العربية (Arabic Summary)

**محرك Omni AI Engine (v1.0.1)**: محرك ذكاء اصطناعي خفيف ومستقل مبني بلغة Rust بدون أي متطلبات تشغيل خارجية، يلتف حول مكتبة `llama.cpp` لتوفير استدلال سريع وموثوق.

- **الجديد في الإصدار v1.0.1**: إضافة تحكم كامل عبر سطر الأوامر (Terminal CLI) لفحص العتاد، تشغيل وإيقاف النماذج، المحادثة المباشرة من الطرفية، وإدارة مفاتيح الـ API.
- **الترخيص**: ترخيص MIT مفتوح المصدر للاستخدام الحر.
