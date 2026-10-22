#!/usr/bin/env bash
# ==============================================================================
#  Omni Engine - Universal 1-Line Sovereign Installer
#  Usage: curl -fsSL https://raw.githubusercontent.com/ielfeqi-rgb/omni_engine/main/install.sh | bash
# ==============================================================================
set -e

RESET="\033[0m"
BOLD="\033[1m"
GREEN="\033[32m"
CYAN="\033[36m"
YELLOW="\033[33m"
RED="\033[31m"

echo -e "${CYAN}${BOLD}"
echo "  ================================================================"
echo "     :: OMNI AI ENGINE - Sovereign Autonomous Coding Runtime"
echo "     100% Offline | Zero-Cost | Pure Rust & In-Process C-Speed"
echo "  ================================================================"
echo -e "${RESET}"

INSTALL_DIR="${HOME}/.omni"
BIN_DIR="${INSTALL_DIR}/bin"
MODELS_DIR="${INSTALL_DIR}/models"

mkdir -p "${BIN_DIR}" "${MODELS_DIR}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

echo -e "[*] Detected Platform: ${BOLD}${OS} (${ARCH})${RESET}"

if [ "${OS}" != "linux" ] && [ "${OS}" != "darwin" ]; then
    echo -e "${RED}[-] Error: Omni Engine currently supports Linux and macOS.${RESET}"
    exit 1
fi

# 1. Check for Bubblewrap (Sandboxing)
if command -v bwrap >/dev/null 2>&1; then
    echo -e "${GREEN}[+] Bubblewrap (bwrap) sandbox isolation: ACTIVE (W = ∅ Cleanroom ready)${RESET}"
else
    echo -e "${YELLOW}[!] Notice: 'bwrap' not detected. Isolated cleanroom tests will use ephemeral tempfs fallback.${RESET}"
    echo -e "${YELLOW}    To enable maximum containerized sandboxing, install bubblewrap: 'sudo apt install bubblewrap' or 'sudo dnf install bubblewrap'.${RESET}"
fi

# 2. Local Model Setup (Default: Qwen2.5-Coder-1.5B Instruct Q4_K_M)
MODEL_NAME="qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"
MODEL_PATH="${MODELS_DIR}/${MODEL_NAME}"
MODEL_URL="https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF/resolve/main/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"

# Check if model already exists in local repo
if [ -f "./models/${MODEL_NAME}" ]; then
    echo -e "[*] Found local model in workspace. Linking to ${MODEL_PATH}..."
    ln -sf "$(pwd)/models/${MODEL_NAME}" "${MODEL_PATH}"
elif [ ! -f "${MODEL_PATH}" ]; then
    echo -e "[*] Preparing sovereign coding model (${MODEL_NAME}, ~1.1GB)..."
    echo -e "    Downloading lightweight model for CPU inference..."
    if command -v curl >/dev/null 2>&1; then
        curl -L --progress-bar -o "${MODEL_PATH}" "${MODEL_URL}"
    elif command -v wget >/dev/null 2>&1; then
        wget --show-progress -O "${MODEL_PATH}" "${MODEL_URL}"
    else
        echo -e "${RED}[-] Error: Neither curl nor wget found. Please place a GGUF model in ${MODELS_DIR}.${RESET}"
    fi
else
    echo -e "${GREEN}[+] Local model verified at: ${MODEL_PATH}${RESET}"
fi

# 3. Binary Setup
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "${SCRIPT_DIR}/target/release/omni_engine" ]; then
    echo -e "[*] Installing pre-built release binary from workspace..."
    cp -f "${SCRIPT_DIR}/target/release/omni_engine" "${BIN_DIR}/omni"
    if [ -d "${SCRIPT_DIR}/bin" ]; then
        cp -rf "${SCRIPT_DIR}/bin/"* "${BIN_DIR}/" 2>/dev/null || true
    fi
elif [ -f "${SCRIPT_DIR}/bin/omni_engine" ]; then
    cp -f "${SCRIPT_DIR}/bin/omni_engine" "${BIN_DIR}/omni"
    if [ -d "${SCRIPT_DIR}/bin" ]; then
        cp -rf "${SCRIPT_DIR}/bin/"* "${BIN_DIR}/" 2>/dev/null || true
    fi
else
    echo -e "[*] Compiling Omni Engine optimized release binary via cargo..."
    if command -v cargo >/dev/null 2>&1; then
        (cd "${SCRIPT_DIR}" && cargo build --release)
        cp -f "${SCRIPT_DIR}/target/release/omni_engine" "${BIN_DIR}/omni"
        if [ -d "${SCRIPT_DIR}/bin" ]; then
            cp -rf "${SCRIPT_DIR}/bin/"* "${BIN_DIR}/" 2>/dev/null || true
        fi
    else
        echo -e "${RED}[-] Rust cargo not found. Please install Rust (https://rustup.rs) or download a release binary.${RESET}"
        exit 1
    fi
fi

chmod +x "${BIN_DIR}/omni"

# 4. Wrapper Script for LD_LIBRARY_PATH
cat << 'EOF' > "${BIN_DIR}/omni_run"
#!/usr/bin/env bash
INSTALL_DIR="${HOME}/.omni"
export LD_LIBRARY_PATH="${INSTALL_DIR}/bin:${LD_LIBRARY_PATH}"
exec "${INSTALL_DIR}/bin/omni" "$@"
EOF
chmod +x "${BIN_DIR}/omni_run"
mv "${BIN_DIR}/omni_run" "${BIN_DIR}/omni"

# 5. Shell PATH Configuration
SHELL_RC=""
case "${SHELL}" in
    */zsh) SHELL_RC="${HOME}/.zshrc" ;;
    */bash) SHELL_RC="${HOME}/.bashrc" ;;
    *) SHELL_RC="${HOME}/.profile" ;;
esac

if [ -f "${SHELL_RC}" ]; then
    if ! grep -q ".omni/bin" "${SHELL_RC}"; then
        echo -e "\n# Omni AI Engine" >> "${SHELL_RC}"
        echo "export PATH=\"\${HOME}/.omni/bin:\${PATH}\"" >> "${SHELL_RC}"
        echo -e "${GREEN}[+] Added ~/.omni/bin to ${SHELL_RC}${RESET}"
    fi
fi

echo -e "\n${GREEN}${BOLD}================================================================${RESET}"
echo -e "${GREEN}${BOLD}   :: OMNI AI ENGINE SUCCESSFULLY INSTALLED!${RESET}"
echo -e "${GREEN}${BOLD}================================================================${RESET}\n"
echo -e "You can now run Omni Engine from any terminal:\n"
echo -e "  ${CYAN}omni --tui${RESET}                           # Open Sovereign Terminal Dashboard"
echo -e "  ${CYAN}omni swarm \"build a todo CLI\"${RESET}        # Run Autonomous Multi-Agent Swarm"
echo -e "  ${CYAN}omni status${RESET}                          # Inspect System Hardware & KV-Cache"
echo -e "\nTo reload your shell environment right now, run:"
echo -e "  ${YELLOW}source ${SHELL_RC}${RESET}\n"
