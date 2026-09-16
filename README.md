# Omni AI Engine v1.0 (Standalone Rust Edition)

> **Official Notice**: This project is a completely standalone AI engine written in Rust. It bears zero relationship to any other project and operates independently.

---

## 🚀 Technical Architecture Overview

**Omni AI Engine** is a single-binary, zero-runtime-dependency AI execution engine compiled natively in **Rust** wrapping `llama.cpp`. 

It includes an embedded browser dashboard, an on-demand GGUF model downloader with HTTP redirect support, an **OpenAI-compatible REST API (`/v1/chat/completions`)**, and an **API Key security layer**.

---

## 🧐 Formally Sarcastic Disclaimer & Policies (السياسات وإخلاء المسؤولية)

### 1. Versioning & Future Maintenance Policy
* **Single Release Rule (v1.0)**: This software is provided as version 1.0 and constitutes the first, last, and final release. 
* **Maintenance Notice**: No feature requests will be reviewed, no roadmaps will be created, and no bug fixes will be issued. The code is provided "as is".

### 2. Licensing
* **Unrestricted Open Source (MIT / Public Domain)**: You are granted unconditional rights to use, modify, redistribute, commercialize, or completely disregard this codebase for any purpose whatsoever without needing permission.

### 3. Privacy Statement
* **Privacy Policy**: We do not harvest, store, or sell your personal data — not out of an overwhelming reverence for your privacy, but simply because we do not own remote cloud servers to transmit it to. Your data resides and expires exclusively within your system's local RAM.

---

## ✨ Features & Hardware Capabilities

* **Bare-Metal Native Execution**: Developed in Rust without Garbage Collection or Virtual Machine overhead. Serves requests directly via Linux kernel socket polling (`epoll`).
* **100% OpenAI API Compatibility**: Exposes standard `/v1/chat/completions` endpoints supporting Server-Sent Events (SSE) streaming for direct integration with Cursor, Python `openai`, and Open WebUI.
* **Hardware Diagnostics & Parameter Estimator**: Inspects system RAM and CPU capabilities to estimate maximum GGUF model sizes (7B, 13B, 32B, 70B+).
* **Live Terminal Log Streaming**: Embedded auto-scrolling log console (`GET /api/logs`) monitoring server events in real time.
* **API Key Auth Guard**: All external `/v1/*` endpoints require `Authorization: Bearer omni_sk_...` tokens.
* **Trilingual Embedded UI**: Built-in Web UI supporting **Arabic, English, and French** with dynamic `RTL`/`LTR` layout switching.

---

## 📁 Release Artifacts (حزم الإصدارات)

The compiled distribution binaries are stored in the `releases/` directory:

* 🐧 **Linux (x86_64)**: `releases/omni_engine_linux_x86_64.tar.gz`
* 🪟 **Windows (x64)**: `releases/omni_engine_windows_x64.tar.gz`
* 🍎 **macOS**: `releases/omni_engine_mac.txt` *(Contains official notice: "I don't like Mac")*

---

## 🛠️ Installation & Quick Start

### Build & Run from Source
```bash
cargo build --release
./target/release/omni_engine
```

### Access Ports
* **Web UI Dashboard**: `http://127.0.0.1:8090`
* **OpenAI Endpoint**: `http://127.0.0.1:8090/v1`

---

## 🌐 Arabic Formal Sarcastic Summary / ملخص رسمي ساخر بالعربية

**محرك Omni AI Engine v1.0**: محرك ذكاء اصطناعي مكتوب بلغة Rust ومستقل تماماً.

* **سياسة التحديثات**: هذا هو الإصدار الأول والنهائي v1.0. لن يتم الاستماع لاقتراحات التحديث، ولن تُطرح خريطة طريق مستقبلية، ولن تُصلح أي أخطاء قد تكتشفها مستقبلاً.
* **الترخيص**: حر ومفتوح المصدر بالكامل (MIT / Public Domain). لك مطلق الحرية في استخدامه أو تعديله أو بيعه أو تجاهله تماماً دون الحاجة لإذن المطور.
* **سياسة الخصوصية**: لا نقوم بجمع أو تتبع بياناتك الشخصية، ليس بدافع الاحترام المفرط لخصوصيتك، بل لعدم امتلاكنا خوادم سحابية خارجية لنقل البيانات إليها أساساً. تعيش بياناتك وتموت داخل الذاكرة العشوائية لرقاقتك المحلية.
* **إصدار ماك (macOS)**: نأسف لإبلاغ مستخدمي أجهزة أبل بعدم توفر نسخة تنفيدية لنظام ماك، وذلك تفضيلاً شخصياً من المطور بعدم التعامل مع هذا النظام.
* **المطور المباشر**: ielfeqi-rgb ([https://github.com/ielfeqi-rgb](https://github.com/ielfeqi-rgb)).
