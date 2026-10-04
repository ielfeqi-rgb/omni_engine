# Omni Engine Development Roadmap

This document outlines the strategic architectural milestones for Omni Engine, following the stabilization of the v2.1.0 Sovereign Runtime. The roadmap strictly adheres to the "Opt-in Complexity" philosophy; core automation stability will not be compromised for presentation or peripheral features.

---

## Phase 1: Observability & Presentation (v2.2.0)
*Targeting human-computer interaction (HCI) and swarm observability.*

- [ ] **Sovereign TUI (Terminal User Interface)**
  - Implement a highly concurrent `ratatui`-based interface running on a detached presentation thread.
  - Architecture: Zero-overhead opt-in design triggered exclusively via `--tui` flag (or `run_tui.sh`), preserving the strict `stdout` pipeline for legacy scripts.
- [ ] **Real-Time Swarm Telemetry**
  - Live TUI sidebar mapping the execution tree of System 1 Workers.
  - Hardware & Causal-DAG KV-cache usage gauges (VRAM/RAM visualization).
- [ ] **Async I/O Decoupling**
  - Physical separation of the user input prompt from standard output streams to prevent async text collisions during active swarm generation.

---

## Phase 2: Memory & Modality (v2.3.0)
*Targeting context horizon limits and data ingestion capabilities.*

- [ ] **Long-term Episodic Memory (RAG)**
  - Implement a lightweight, embedded Vector Database (e.g., SQLite-VSS or local LanceDB) for cross-session persistent memory.
  - Enable the Thinker to autonomously query past successful strategies (Ancestral Inheritance across reboots).
- [ ] **Multi-Modal Vision Hooks**
  - Seamless integration with Vision-Language Models (e.g., LLaVA natively via `llama.cpp` bindings).
  - Allow the sandbox to capture screenshots or ingest user images, expanding the Thinker's sensory input beyond pure text.

---

## Phase 3: Distributed Intelligence (v3.0.0)
*Targeting compute bottlenecks and enterprise scalability.*

- [ ] **Networked Distributed Swarm**
  - Evolve the Swarm Coordinator to dispatch System 1 Workers over a network layer (gRPC or WebSockets) to physically separate machines/nodes.
  - Enables massive parallel execution without being bottlenecked by the local host's CPU/RAM.
- [ ] **Third-Party Plugin Ecosystem**
  - Stabilize an official Sandbox SDK.
  - Allow developers to write untrusted `.lua` or compiled `.so` plugins that can be safely loaded into the Bubblewrap jail dynamically.

---
> **Architectural Principle:** *If it ain't broke, don't fix it.* Every feature above must be designed as a decoupled layer. The core text-in/text-out autonomous loop established in v2.1.0 remains the immutable heart of the engine.
