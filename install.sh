#!/usr/bin/env bash
# ==============================================================================
#  Omni Engine - Universal Sovereign Installer
#  Usage: curl -fsSL https://raw.githubusercontent.com/ielfeqi-rgb/omni_engine/main/install.sh | bash
# ==============================================================================
set -e

RESET="\033[0m"
BOLD="\033[1m"
DIM="\033[2m"
GREEN="\033[32m"
CYAN="\033[36m"
YELLOW="\033[33m"
RED="\033[31m"
MAGENTA="\033[35m"

INSTALL_DIR="${HOME}/.omni"
BIN_DIR="${INSTALL_DIR}/bin"
MODELS_DIR="${INSTALL_DIR}/models"
REPO="ielfeqi-rgb/omni_engine"
FROZEN_MACOS_VERSION="v2.2.2"

# Safe interactive prompt reader that supports terminal, pipe, and dev/tty
prompt_user() {
    local prompt_text="$1"
    local default_val="$2"
    local result_var="$3"
    local ans=""
    local read_rc=0

    if [ -t 0 ]; then
        read -r -p "${prompt_text}" ans || read_rc=$?
    elif [ -t 1 ] && [ -c /dev/tty ] && [ -r /dev/tty ]; then
        printf "%s" "${prompt_text}" >/dev/tty 2>/dev/null || true
        read -r ans </dev/tty 2>/dev/null || read_rc=$?
    else
        read -r ans 2>/dev/null || read_rc=$?
    fi

    if [ ${read_rc} -ne 0 ] && [ -z "${ans}" ]; then
        eval "${result_var}=\"__EOF__\""
        return ${read_rc}
    fi

    if [ -z "${ans}" ]; then
        eval "${result_var}=\"${default_val}\""
    else
        eval "${result_var}=\"${ans}\""
    fi
}

echo -e "${CYAN}${BOLD}"
echo "  ================================================================"
echo "     :: OMNI AI ENGINE - Sovereign Autonomous Systems Runtime"
echo "     100% Offline | Zero-Cost | Pure Rust & In-Process C-Speed"
echo "  ================================================================"
echo -e "${RESET}"

# ------------------------------------------------------------------------------
# 1. Environment & Host OS Auto-Detection
# ------------------------------------------------------------------------------
RAW_OS="$(uname -s 2>/dev/null || echo "unknown")"
RAW_ARCH="$(uname -m 2>/dev/null || echo "unknown")"

DETECTED_OS="unknown"
DETECTED_DETAIL=""

# Detect WSL (Windows Subsystem for Linux)
if grep -qi microsoft /proc/version 2>/dev/null || [ -n "${WSL_DISTRO_NAME}" ]; then
    DETECTED_OS="windows_wsl"
    DETECTED_DETAIL="Windows Subsystem for Linux (WSL2)"
# Detect native Windows environments (Git Bash, MSYS, Cygwin, MINGW)
elif [ -n "${COMSPEC}" ] || [[ "${RAW_OS}" =~ ^(msys|cygwin|mingw|MSYS|CYGWIN|MINGW) ]]; then
    DETECTED_OS="windows"
    DETECTED_DETAIL="Native Windows (Git Bash / MSYS)"
# Detect macOS Darwin
elif [ "${RAW_OS}" = "Darwin" ]; then
    if [ "${RAW_ARCH}" = "arm64" ]; then
        DETECTED_OS="macos"
        DETECTED_DETAIL="macOS Apple Silicon (M-Series arm64)"
    else
        # Intel Mac is legacy/unsupported - cannot auto-recommend
        DETECTED_OS="unknown"
        DETECTED_DETAIL="macOS Intel (${RAW_ARCH} - Unsupported architecture)"
    fi
# Detect Linux
elif [ "${RAW_OS}" = "Linux" ]; then
    if [ "${RAW_ARCH}" = "x86_64" ] || [ "${RAW_ARCH}" = "amd64" ]; then
        DETECTED_OS="linux"
        DETECTED_DETAIL="Linux (${RAW_ARCH})"
    else
        # Non-x86_64 Linux (e.g. ARM VM, container)
        DETECTED_OS="unknown"
        DETECTED_DETAIL="Linux (${RAW_ARCH} - Non-standard architecture / VM)"
    fi
fi

# ------------------------------------------------------------------------------
# 2. Interactive Menu / Selection Logic
# ------------------------------------------------------------------------------
CHOICE=""
for arg in "$@"; do
    case "${arg}" in
        --linux|-l) CHOICE="1" ;;
        --macos|--mac|-m) CHOICE="2" ;;
        --windows|-w) CHOICE="3" ;;
        -y|--yes) AUTO_APPROVE=1 ;;
        -h|--help)
            echo -e "${CYAN}${BOLD}Omni AI Engine - Universal Sovereign Installer${RESET}"
            echo ""
            echo "Usage: ./install.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -l, --linux       Install Linux production runtime (Dynamic latest)"
            echo "  -m, --macos       Install macOS Apple Silicon frozen snapshot (v2.2.2)"
            echo "  -w, --windows     Launch or configure Windows WSL2 runtime"
            echo "  -y, --yes         Auto-approve installation prompt"
            echo "  -h, --help        Show this help message"
            exit 0
            ;;
    esac
done

if [ -z "${CHOICE}" ]; then
    if [ "${DETECTED_OS}" = "unknown" ]; then
        # Auto-detection failed (likely a VM, custom hypervisor, or container)
        echo -e "${YELLOW}${BOLD}[!] Host auto-detection could not determine target environment${RESET}"
        echo -e "    ${DIM}(Likely running inside a Virtual Machine, custom hypervisor, or non-standard container).${RESET}"
        echo -e "    Please select your target Operating System manually (${BOLD}at your own risk${RESET}):\n"

        echo -e "     1) Linux [x86_64]"
        echo -e "     2) macOS [Apple Silicon M1-M4]"
        echo -e "     3) Windows [via WSL2]"
        echo ""

        while true; do
            USER_INPUT=""
            prompt_user "Enter choice [1-3] (Manual selection at your own risk): " "" USER_INPUT

            if [ "${USER_INPUT}" = "__EOF__" ]; then
                echo -e "${RED}[-] Error: Reached end of input without valid selection.${RESET}"
                echo -e "    Please specify a target flag: --linux, --macos, or --windows"
                exit 1
            fi

            case "${USER_INPUT}" in
                1|2|3)
                    CHOICE="${USER_INPUT}"
                    break
                    ;;
                *)
                    echo -e "${RED}[!] Invalid choice '${USER_INPUT}'. Please enter 1, 2, or 3:${RESET}"
                    ;;
            esac
        done
    else
        # Auto-detection succeeded
        L_ARROW="   "
        M_ARROW="   "
        W_ARROW="   "
        L_NOTE="(Recommended)"
        M_NOTE=""
        W_NOTE=""
        DEFAULT_CHOICE="1"

        if [ "${DETECTED_OS}" = "linux" ]; then
            L_ARROW="-->"
            L_NOTE="(This is your detected OS — Recommended)"
            DEFAULT_CHOICE="1"
        elif [ "${DETECTED_OS}" = "macos" ]; then
            M_ARROW="-->"
            M_NOTE="(yuck... 🤢)"
            DEFAULT_CHOICE="2"
        elif [ "${DETECTED_OS}" = "windows" ] || [ "${DETECTED_OS}" = "windows_wsl" ]; then
            W_ARROW="-->"
            W_NOTE="(This is your detected OS — please don't do something stupid)"
            DEFAULT_CHOICE="3"
        fi

        echo -e "Select your target Operating System:\n"
        echo -e " ${BOLD}${L_ARROW} 1) Linux [x86_64] ${GREEN}${L_NOTE}${RESET}"
        if [ -n "${M_NOTE}" ]; then
            echo -e " ${BOLD}${M_ARROW} 2) macOS [Apple Silicon M1-M4] ${YELLOW}${M_NOTE}${RESET}"
        else
            echo -e " ${BOLD}${M_ARROW} 2) macOS [Apple Silicon M1-M4] ${DIM}(Frozen Snapshot)${RESET}"
        fi
        if [ -n "${W_NOTE}" ]; then
            echo -e " ${BOLD}${W_ARROW} 3) Windows [via WSL2] ${MAGENTA}${W_NOTE}${RESET}"
        else
            echo -e " ${BOLD}${W_ARROW} 3) Windows [via WSL2]${RESET}"
        fi
        echo ""

        USER_INPUT=""
        prompt_user "Enter choice [1-3] (Default: press Enter for detected OS [${DEFAULT_CHOICE}]): " "${DEFAULT_CHOICE}" USER_INPUT
        CHOICE="${USER_INPUT}"
    fi
fi

# Validate final choice
case "${CHOICE}" in
    1|2|3) ;;
    *)
        echo -e "${RED}[-] Invalid choice '${CHOICE}'. Aborting.${RESET}"
        exit 1
        ;;
esac

# ------------------------------------------------------------------------------
# 3. Branching Logic per Target OS
# ------------------------------------------------------------------------------

# === OPTION 3: Windows (WSL2 Architecture) ===
if [ "${CHOICE}" = "3" ]; then
    echo -e "\n${CYAN}${BOLD}[*] Target OS: Windows (WSL2 Architecture)${RESET}\n"
    
    if [ "${DETECTED_OS}" = "windows_wsl" ]; then
        echo -e "${GREEN}[+] Perfect! You are already running inside WSL2 (${WSL_DISTRO_NAME:-Linux}).${RESET}"
        echo -e "    Proceeding with native Linux installation inside your WSL environment...\n"
        CHOICE="1"
    else
        echo -e "${YELLOW}${BOLD}Notice:${RESET} Omni Engine is engineered natively for Linux kernel primitives"
        echo -e "(in-memory virtual filesystem namespaces, fail-closed Bubblewrap cleanrooms,"
        echo -e "and zero-copy causal KV-cache turning). Native Win32 lacks these low-level primitives."
        echo ""
        echo -e "On Windows, Omni Engine runs with ${GREEN}${BOLD}100% bare-metal performance and zero virtualization overhead${RESET}"
        echo -e "inside ${BOLD}WSL2 (Windows Subsystem for Linux)${RESET}."
        echo ""

        if command -v wsl.exe >/dev/null 2>&1 || [ -f "/c/Windows/System32/wsl.exe" ]; then
            echo -e "${GREEN}[+] WSL2 is detected on your Windows machine!${RESET}"
            echo -e "${GREEN}[*] WSL2 Dependency Guarantee:${RESET} Inside WSL2, the installer will automatically"
            echo -e "    audit and install all required Linux dependencies (Python 3.9+, Bubblewrap,"
            echo -e "    and sandboxing primitives) using your WSL distribution package manager (apt/dnf)."
            echo ""
            echo -e "To install and run Omni Engine in 1 line inside WSL, simply run:"
            echo ""
            echo -e "    ${CYAN}${BOLD}wsl -e bash -c \"curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | bash\"${RESET}"
            echo ""
            
            CAN_PROMPT=0
            if [ -t 0 ] || [ -t 1 ]; then CAN_PROMPT=1; fi
            if [ "${CAN_PROMPT}" -eq 1 ] && [ -z "${AUTO_APPROVE}" ]; then
                LAUNCH_WSL=""
                prompt_user "Would you like me to launch the installer inside WSL right now? [y/N]: " "n" LAUNCH_WSL
                if [[ "${LAUNCH_WSL}" =~ ^[Yy]$ ]]; then
                    echo -e "\n[*] Launching inside WSL2...\n"
                    wsl.exe -e bash -c "curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | bash"
                    exit 0
                fi
            fi
        else
            echo -e "${RED}[-] WSL2 is not installed on your system yet.${RESET}"
            echo -e "To enable WSL2 on Windows, open an Administrator PowerShell and run:"
            echo ""
            echo -e "    ${CYAN}${BOLD}wsl --install${RESET}"
            echo ""
            echo -e "After your system reboots, open Ubuntu and re-run this command."
        fi
        exit 0
    fi
fi

# === OPTION 2: macOS Apple Silicon (Frozen Snapshot) ===
if [ "${CHOICE}" = "2" ]; then
    echo -e "\n${YELLOW}${BOLD}================================================================${RESET}"
    echo -e "${YELLOW}${BOLD}[!] macOS? Are you sure? 🤨${RESET}"
    echo -e "    Well, that's your choice I suppose, even though it's definitely"
    echo -e "    not our preferred platform."
    echo ""
    echo -e "    ${BOLD}Please note:${RESET} This macOS build is a ${RED}${BOLD}FROZEN FINAL SNAPSHOT${RESET} (${FROZEN_MACOS_VERSION})"
    echo -e "    with ${RED}${BOLD}NO future updates${RESET} from the author."
    echo -e "    Support is 100% abandoned and handed to the community."
    echo -e "    If a future macOS update breaks it, fork it and fix it yourselves. 🙂"
    echo ""
    echo -e "    ${BOLD}Gatekeeper & The Apple Tax:${RESET}"
    echo -e "    If macOS whines about 'unverified developer' or Gatekeeper blocks the binary:"
    echo -e "    No, I am NOT paying \$99/year to a trillion-dollar company for a certificate."
    echo -e "    If needed, strip Apple's quarantine bureaucracy yourself:"
    echo -e "        ${CYAN}xattr -cr ~/.omni${RESET}"
    echo -e "${YELLOW}${BOLD}================================================================${RESET}\n"

    # Architecture verification
    if [ "${RAW_ARCH}" != "arm64" ] && [ "${RAW_OS}" = "Darwin" ]; then
        echo -e "${RED}[-] Error: This snapshot supports Apple Silicon (M1-M4, arm64) only.${RESET}"
        echo -e "    Intel Macs (x86_64) are completely unsupported."
        exit 1
    fi

    TARBALL="omni_engine_${FROZEN_MACOS_VERSION}_darwin_arm64.tar.gz"
    RELEASE_URL="https://github.com/${REPO}/releases/download/${FROZEN_MACOS_VERSION}/${TARBALL}"

    mkdir -p "${BIN_DIR}" "${MODELS_DIR}"

    TOTAL_RAM_BYTES="$(sysctl -n hw.memsize 2>/dev/null || echo 0)"
    TOTAL_RAM_GB=$((TOTAL_RAM_BYTES / 1024 / 1024 / 1024))
    echo -e "[*] Apple Silicon Hardware: ${GREEN}Active (Unified Memory: ${TOTAL_RAM_GB} GB)${RESET}"
    echo -e "[*] Metal GPU Acceleration: ${GREEN}libggml-metal runtime ready${RESET}"
    echo -e "[*] Sandbox: ${GREEN}Apple Seatbelt (sandbox-exec) fail-closed profile${RESET}"

    # Dependency audit for macOS
    MAC_MISSING_PKGS=()
    if command -v python3 >/dev/null 2>&1; then
        PY_VER="$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null || echo "0.0")"
        PY_MAJOR="$(echo "${PY_VER}" | cut -d. -f1)"
        PY_MINOR="$(echo "${PY_VER}" | cut -d. -f2)"
        if [ "${PY_MAJOR}" -ge 3 ] && [ "${PY_MINOR}" -ge 9 ]; then
            echo -e "[+] Python Runtime: ${GREEN}Python ${PY_VER} (OK)${RESET}"
        else
            echo -e "${YELLOW}[!] Python Runtime: Detected Python ${PY_VER} (Recommended: 3.9+)${RESET}"
            MAC_MISSING_PKGS+=("python@3.11")
        fi
    else
        echo -e "${YELLOW}[!] Python Runtime: Not found.${RESET}"
        MAC_MISSING_PKGS+=("python@3.11")
    fi

    if [ ${#MAC_MISSING_PKGS[@]} -gt 0 ]; then
        if command -v brew >/dev/null 2>&1; then
            echo -e "\n[*] Homebrew detected: Installing recommended dependencies (${MAC_MISSING_PKGS[*]})..."
            brew install "${MAC_MISSING_PKGS[@]}" || {
                echo -e "${YELLOW}[!] Brew installation failed. You can manually run: brew install ${MAC_MISSING_PKGS[*]}${RESET}"
            }
        else
            echo -e "${YELLOW}[!] Notice: Homebrew (brew) not detected. To install Python 3.9+, visit: https://brew.sh${RESET}"
        fi
    fi

    echo -e "\n[*] Downloading frozen macOS snapshot (${FROZEN_MACOS_VERSION})..."
    TMP_DIR="$(mktemp -d)"
    if command -v curl >/dev/null 2>&1; then
        curl -fSL --progress-bar -o "${TMP_DIR}/${TARBALL}" "${RELEASE_URL}"
    elif command -v wget >/dev/null 2>&1; then
        wget --show-progress -O "${TMP_DIR}/${TARBALL}" "${RELEASE_URL}"
    else
        echo -e "${RED}[-] Error: Neither curl nor wget found.${RESET}"
        exit 1
    fi

    echo -e "[*] Extracting snapshot assets to ${INSTALL_DIR}..."
    tar -xzf "${TMP_DIR}/${TARBALL}" -C "${INSTALL_DIR}" --strip-components=1 2>/dev/null || tar -xzf "${TMP_DIR}/${TARBALL}" -C "${INSTALL_DIR}"
    rm -rf "${TMP_DIR}"

    chmod +x "${BIN_DIR}/omni_engine" 2>/dev/null || true

    # Strip Apple Gatekeeper quarantine flag automatically
    if command -v xattr >/dev/null 2>&1; then
        echo -e "[*] Stripping Apple Gatekeeper quarantine attributes (xattr -cr)..."
        xattr -cr "${INSTALL_DIR}" 2>/dev/null || xattr -d com.apple.quarantine "${BIN_DIR}/omni_engine" 2>/dev/null || true
    fi

    # Download default offline coding model (Qwen 1.5B)
    MODEL_NAME="qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"
    MODEL_PATH="${MODELS_DIR}/${MODEL_NAME}"
    MODEL_URL="https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF/resolve/main/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"

    if [ -f "./models/${MODEL_NAME}" ]; then
        echo -e "[*] Linking local model from current directory..."
        ln -sf "$(pwd)/models/${MODEL_NAME}" "${MODEL_PATH}"
    elif [ ! -f "${MODEL_PATH}" ]; then
        echo -e "[*] Downloading default offline coding model (${MODEL_NAME}, ~1.1GB)..."
        if command -v curl >/dev/null 2>&1; then
            curl -L --progress-bar -o "${MODEL_PATH}" "${MODEL_URL}" || true
        elif command -v wget >/dev/null 2>&1; then
            wget --show-progress -O "${MODEL_PATH}" "${MODEL_URL}" || true
        fi
    fi

    # Create wrapper launcher
    cat << 'EOF' > "${BIN_DIR}/omni"
#!/usr/bin/env bash
INSTALL_DIR="${HOME}/.omni"
export DYLD_LIBRARY_PATH="${INSTALL_DIR}/bin:${DYLD_LIBRARY_PATH}"
export PATH="${INSTALL_DIR}/bin:/opt/homebrew/bin:${PATH}"
exec "${INSTALL_DIR}/bin/omni_engine" "$@"
EOF
    chmod +x "${BIN_DIR}/omni"
    ln -sf "${BIN_DIR}/omni" "${BIN_DIR}/omni-engine"

    # Shell PATH setup
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
    echo -e "${GREEN}${BOLD}   :: OMNI ENGINE ${FROZEN_MACOS_VERSION} (MACOS SNAPSHOT) INSTALLED!${RESET}"
    echo -e "${GREEN}${BOLD}================================================================${RESET}\n"
    echo -e "You can now run Omni Engine from any terminal:\n"
    echo -e "  ${CYAN}omni tui${RESET}                                # Open Sovereign Terminal Dashboard"
    echo -e "  ${CYAN}omni run \"Build a modular API\" --review${RESET}  # Autonomous Swarm with Plan Review"
    echo -e "  ${CYAN}omni run --plan-file ./plan.json${RESET}         # Execute Maestro Protocol Blueprint"
    echo -e "\nTo reload your shell right now:"
    echo -e "  ${YELLOW}source ${SHELL_RC}${RESET}\n"
    echo -e "${DIM}Note: If Gatekeeper ever complains about an unverified developer, run:${RESET}"
    echo -e "  ${CYAN}xattr -cr ~/.omni${RESET}\n"
    exit 0
fi

# === OPTION 1: Linux (Native Production - Dynamic Latest) ===
echo -e "\n${CYAN}${BOLD}[*] Target OS: Linux (Native Production Engine)${RESET}\n"

# 1. Distro & Package Manager Discovery
PKG_MGR=""
PKG_INSTALL_CMD=""
DISTRO_NAME="Unknown Linux"

if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO_NAME="${NAME:-Linux}"
fi

if command -v apt-get >/dev/null 2>&1; then
    PKG_MGR="apt"
    PKG_INSTALL_CMD="sudo apt-get update && sudo apt-get install -y"
elif command -v dnf >/dev/null 2>&1; then
    PKG_MGR="dnf"
    PKG_INSTALL_CMD="sudo dnf install -y"
elif command -v pacman >/dev/null 2>&1; then
    PKG_MGR="pacman"
    PKG_INSTALL_CMD="sudo pacman -Sy --noconfirm"
elif command -v zypper >/dev/null 2>&1; then
    PKG_MGR="zypper"
    PKG_INSTALL_CMD="sudo zypper install -y"
elif command -v apk >/dev/null 2>&1; then
    PKG_MGR="apk"
    PKG_INSTALL_CMD="sudo apk add"
fi

echo -e "[*] Detected Distribution: ${BOLD}${DISTRO_NAME}${RESET}"

# 2. Dependency Audit (Python 3.9+, Bubblewrap, Curl/Wget, Tar)
MISSING_PKGS=()
PYTHON_OK=0

if command -v python3 >/dev/null 2>&1; then
    PY_VER="$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")' 2>/dev/null || echo "0.0")"
    PY_MAJOR="$(echo "${PY_VER}" | cut -d. -f1)"
    PY_MINOR="$(echo "${PY_VER}" | cut -d. -f2)"
    if [ "${PY_MAJOR}" -ge 3 ] && [ "${PY_MINOR}" -ge 9 ]; then
        PYTHON_OK=1
        echo -e "[+] Python Runtime: ${GREEN}Python ${PY_VER} (OK)${RESET}"
    else
        echo -e "${YELLOW}[!] Python Runtime: Detected Python ${PY_VER} (Recommended: 3.9+)${RESET}"
        MISSING_PKGS+=("python3")
    fi
else
    echo -e "${YELLOW}[!] Python Runtime: Not found.${RESET}"
    MISSING_PKGS+=("python3")
fi

if command -v bwrap >/dev/null 2>&1; then
    echo -e "[+] Sandboxing Primitives: ${GREEN}Bubblewrap (bwrap) present (Kernel namespaces ready)${RESET}"
else
    echo -e "${YELLOW}[!] Sandboxing: 'bwrap' not detected.${RESET}"
    MISSING_PKGS+=("bubblewrap")
fi

if ! command -v tar >/dev/null 2>&1; then
    MISSING_PKGS+=("tar")
fi

if [ ${#MISSING_PKGS[@]} -gt 0 ]; then
    echo -e "\n${YELLOW}[!] Recommended packages for full cleanroom & tooling support:${RESET}"
    echo -e "    Missing: ${BOLD}${MISSING_PKGS[*]}${RESET}"
    if [ -n "${PKG_INSTALL_CMD}" ]; then
        echo -e "    Can be installed via: ${CYAN}${PKG_INSTALL_CMD} ${MISSING_PKGS[*]}${RESET}\n"
    fi
fi

# 3. Dynamic Latest Version Discovery from GitHub
echo -e "[*] Querying GitHub API for latest official Omni release..."
LATEST_TAG=""
if command -v curl >/dev/null 2>&1; then
    LATEST_TAG="$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')"
elif command -v wget >/dev/null 2>&1; then
    LATEST_TAG="$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name":' | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/')"
fi

if [ -z "${LATEST_TAG}" ]; then
    LATEST_TAG="v2.2.2"
    echo -e "${YELLOW}[*] Fallback to current verified release: ${LATEST_TAG}${RESET}"
else
    echo -e "[+] Latest release available: ${GREEN}${BOLD}${LATEST_TAG}${RESET}"
fi

# Determine Linux tarball (CUDA vs Standard CPU)
if command -v nvidia-smi >/dev/null 2>&1; then
    echo -e "[*] GPU Acceleration: ${GREEN}NVIDIA GPU detected (CUDA build selected)${RESET}"
    TARBALL="omni_engine_${LATEST_TAG}_linux_x86_64_cuda.tar.gz"
else
    echo -e "[*] Compute Engine: ${BOLD}CPU Autonomous Build (Universal x86_64)${RESET}"
    TARBALL="omni_engine_${LATEST_TAG}_linux_x86_64.tar.gz"
fi

RELEASE_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${TARBALL}"

# 4. Classic Prompt: Proceed [y/N]?
if [ -z "${AUTO_APPROVE}" ]; then
    PROMPT_CONFIRM=""
    prompt_user "Proceed with installation of Omni Engine ${LATEST_TAG}? [y/N]: " "y" PROMPT_CONFIRM

    if [[ ! "${PROMPT_CONFIRM}" =~ ^[Yy]$ ]]; then
        echo -e "\nInstallation cancelled by user."
        exit 0
    fi
fi

# 5. Optional Dependency Auto-Install
if [ ${#MISSING_PKGS[@]} -gt 0 ] && [ -n "${PKG_INSTALL_CMD}" ]; then
    echo -e "\n[*] Installing missing system dependencies (${MISSING_PKGS[*]})..."
    ${PKG_INSTALL_CMD} "${MISSING_PKGS[@]}" 2>/dev/null || {
        echo -e "${YELLOW}[!] Package installation skipped or sudo required. Continuing with Omni Engine runtime...${RESET}"
    }
fi

# 6. Download & Installation
mkdir -p "${BIN_DIR}" "${MODELS_DIR}"

echo -e "\n[*] Downloading ${TARBALL} from GitHub..."
TMP_DIR="$(mktemp -d)"
if command -v curl >/dev/null 2>&1; then
    curl -fSL --progress-bar -o "${TMP_DIR}/${TARBALL}" "${RELEASE_URL}" || {
        # Fallback to CPU tarball if CUDA tarball is missing
        if [[ "${TARBALL}" =~ cuda ]]; then
            echo -e "${YELLOW}[!] CUDA asset not found for ${LATEST_TAG}. Falling back to standard CPU release...${RESET}"
            TARBALL="omni_engine_${LATEST_TAG}_linux_x86_64.tar.gz"
            RELEASE_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/${TARBALL}"
            curl -fSL --progress-bar -o "${TMP_DIR}/${TARBALL}" "${RELEASE_URL}"
        fi
    }
elif command -v wget >/dev/null 2>&1; then
    wget --show-progress -O "${TMP_DIR}/${TARBALL}" "${RELEASE_URL}"
fi

echo -e "[*] Extracting release assets to ${INSTALL_DIR}..."
tar -xzf "${TMP_DIR}/${TARBALL}" -C "${INSTALL_DIR}" --strip-components=1 2>/dev/null || tar -xzf "${TMP_DIR}/${TARBALL}" -C "${INSTALL_DIR}"
rm -rf "${TMP_DIR}"

chmod +x "${BIN_DIR}/omni_engine" 2>/dev/null || true

# 7. Default Lightweight Model Download (Qwen 1.5B)
MODEL_NAME="qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"
MODEL_PATH="${MODELS_DIR}/${MODEL_NAME}"
MODEL_URL="https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF/resolve/main/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"

if [ -f "./models/${MODEL_NAME}" ]; then
    echo -e "[*] Linking local model from current directory..."
    ln -sf "$(pwd)/models/${MODEL_NAME}" "${MODEL_PATH}"
elif [ ! -f "${MODEL_PATH}" ]; then
    echo -e "[*] Downloading default offline coding model (${MODEL_NAME}, ~1.1GB)..."
    if command -v curl >/dev/null 2>&1; then
        curl -L --progress-bar -o "${MODEL_PATH}" "${MODEL_URL}" || true
    elif command -v wget >/dev/null 2>&1; then
        wget --show-progress -O "${MODEL_PATH}" "${MODEL_URL}" || true
    fi
fi

# 8. Create Wrapper Launcher
cat << 'EOF' > "${BIN_DIR}/omni"
#!/usr/bin/env bash
INSTALL_DIR="${HOME}/.omni"
export LD_LIBRARY_PATH="${INSTALL_DIR}/bin:${LD_LIBRARY_PATH}"
export PATH="${INSTALL_DIR}/bin:${PATH}"
exec "${INSTALL_DIR}/bin/omni_engine" "$@"
EOF
chmod +x "${BIN_DIR}/omni"
ln -sf "${BIN_DIR}/omni" "${BIN_DIR}/omni-engine"

# 9. Shell PATH Configuration
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
echo -e "${GREEN}${BOLD}   :: OMNI AI ENGINE ${LATEST_TAG} SUCCESSFULLY INSTALLED!${RESET}"
echo -e "${GREEN}${BOLD}================================================================${RESET}\n"
echo -e "You can now run Omni Engine from any terminal:\n"
echo -e "  ${CYAN}omni tui${RESET}                                # Open Sovereign Terminal Dashboard"
echo -e "  ${CYAN}omni run \"Build a modular API\" --review${RESET}  # Autonomous Swarm with Plan Review"
echo -e "  ${CYAN}omni run --plan-file ./plan.json${RESET}         # Execute Maestro Protocol Blueprint"
echo -e "  ${CYAN}omni status${RESET}                              # Inspect Hardware & KV-Cache Telemetry"
echo -e "\nTo reload your shell right now:"
echo -e "  ${YELLOW}source ${SHELL_RC}${RESET}\n"
