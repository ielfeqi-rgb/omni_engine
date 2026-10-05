#!/bin/bash
# ==============================================================================
# Omni Engine - Sovereign TUI Launcher
# ==============================================================================
# This script wraps the engine in its interactive, animated Terminal User
# Interface mode. It requires `cargo` to compile the engine if not built.
# ==============================================================================

set -e

echo "[Omni] Compiling Sovereign TUI (opt-in feature)..."
cargo build --release 2>/dev/null || cargo build

echo "[Omni] Launching TUI..."
LD_LIBRARY_PATH="$(pwd)/bin" ./target/debug/omni_engine --tui
