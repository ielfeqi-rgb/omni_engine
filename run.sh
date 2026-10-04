#!/bin/bash
set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$SCRIPT_DIR/bin:$LD_LIBRARY_PATH"

if ! command -v bwrap >/dev/null 2>&1; then
    echo "[*] Notice: 'bwrap' (Bubblewrap) is not found. Isolated tests will run in an ephemeral tempfs fallback."
    echo "[*] For full unprivileged container isolation, install bubblewrap: 'sudo dnf install bubblewrap' or 'sudo apt install bubblewrap'."
fi

if [ -f "$SCRIPT_DIR/bin/omni_engine" ]; then
    exec "$SCRIPT_DIR/bin/omni_engine" "$@"
elif [ -f "$SCRIPT_DIR/target/release/omni_engine" ]; then
    exec "$SCRIPT_DIR/target/release/omni_engine" "$@"
else
    echo "[-] Error: omni_engine binary not found."
    exit 1
fi
