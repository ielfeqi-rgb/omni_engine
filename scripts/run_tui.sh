#!/usr/bin/env bash
# ==============================================================================
# Omni Engine (v2.2.1) - Sovereign TUI Launcher
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"
export LD_LIBRARY_PATH="${PROJECT_ROOT}/bin:${LD_LIBRARY_PATH}"

ENGINE_BIN=""
if [ -f "${PROJECT_ROOT}/target/release/omni_engine" ]; then
    ENGINE_BIN="${PROJECT_ROOT}/target/release/omni_engine"
elif [ -f "${PROJECT_ROOT}/bin/omni_engine" ]; then
    ENGINE_BIN="${PROJECT_ROOT}/bin/omni_engine"
elif [ -f "${PROJECT_ROOT}/target/debug/omni_engine" ]; then
    ENGINE_BIN="${PROJECT_ROOT}/target/debug/omni_engine"
else
    echo "[*] Compiling Omni Engine release binary..."
    (cd "${PROJECT_ROOT}" && cargo build --release)
    ENGINE_BIN="${PROJECT_ROOT}/target/release/omni_engine"
fi

echo "[*] Launching Sovereign Terminal UI..."
exec "${ENGINE_BIN}" tui "$@"
