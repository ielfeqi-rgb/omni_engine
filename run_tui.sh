#!/bin/bash
# ==============================================================================
# Omni Engine - Sovereign TUI Launcher
# ==============================================================================
# This script wraps the engine in its interactive, animated Terminal User
# Interface mode. It requires `cargo` to compile the engine if not built.
# ==============================================================================

set -e

echo "[Omni] Compiling Sovereign TUI (opt-in feature)..."
cargo build --release

echo "[Omni] Launching TUI..."
if [ -f "./target/release/omni_engine" ]; then
    LD_LIBRARY_PATH="$(pwd)/bin:$LD_LIBRARY_PATH" ./target/release/omni_engine tui
else
    LD_LIBRARY_PATH="$(pwd)/bin:$LD_LIBRARY_PATH" ./target/debug/omni_engine tui
fi
