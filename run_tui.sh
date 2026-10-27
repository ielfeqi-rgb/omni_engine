#!/usr/bin/env bash
# ==============================================================================
# Omni Engine (v2.1.2) - Sovereign TUI Launcher
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="${SCRIPT_DIR}/bin:${LD_LIBRARY_PATH}"

ENGINE_BIN=""
if [ -f "${SCRIPT_DIR}/target/release/omni_engine" ]; then
    ENGINE_BIN="${SCRIPT_DIR}/target/release/omni_engine"
elif [ -f "${SCRIPT_DIR}/bin/omni_engine" ]; then
    ENGINE_BIN="${SCRIPT_DIR}/bin/omni_engine"
elif [ -f "${SCRIPT_DIR}/target/debug/omni_engine" ]; then
    ENGINE_BIN="${SCRIPT_DIR}/target/debug/omni_engine"
else
    echo "[*] Compiling Omni Engine release binary..."
    (cd "${SCRIPT_DIR}" && cargo build --release)
    ENGINE_BIN="${SCRIPT_DIR}/target/release/omni_engine"
fi

echo "[*] Launching Sovereign Terminal UI..."
exec "${ENGINE_BIN}" tui "$@"
