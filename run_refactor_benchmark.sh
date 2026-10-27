#!/usr/bin/env bash
# ==============================================================================
# Omni Engine (v2.1.2) - Refactoring & Zero-Wipe Invariant Benchmark
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}"

LOG_FILE="${SCRIPT_DIR}/refactor_run.log"
TELEMETRY_CSV="${SCRIPT_DIR}/telemetry_refactor.csv"
MODEL_PATH="${1:-${SCRIPT_DIR}/models/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf}"

if [ ! -f "${MODEL_PATH}" ]; then
    echo "[-] Model not found at ${MODEL_PATH}. Run './bin/omni_engine download --qwen' first."
    exit 1
fi

GOAL_TEXT="In refactor_enterprise_challenge, perform cross-cutting multi-tenant organization isolation: in models.py add org_id: int = 1 to Task, in repository.py update TaskRepository to filter by org_id and accept org_id in create, in service.py update TaskService to require valid org_id > 0, in test_suite.py add new test_multi_tenant_isolation test and keep all existing tests passing."

echo "[*] Launching Omni Engine v2.1.2 in modify mode..."
export LD_LIBRARY_PATH="${SCRIPT_DIR}/bin:${LD_LIBRARY_PATH}"

ENGINE_BIN="${SCRIPT_DIR}/target/release/omni_engine"
if [ ! -f "${ENGINE_BIN}" ]; then
    ENGINE_BIN="${SCRIPT_DIR}/bin/omni_engine"
fi

# Launch omni_engine in background
"${ENGINE_BIN}" modify \
  --dir refactor_enterprise_challenge \
  --output-dir refactor_enterprise_challenge \
  --model "${MODEL_PATH}" \
  --subgoals 3 \
  --steps 5 \
  --goal "${GOAL_TEXT}" > "${LOG_FILE}" 2>&1 &

OMNI_PID=$!
echo "[+] Omni Engine running with PID: ${OMNI_PID}"

# Launch telemetry monitor if Python script is present
MONITOR_PID=""
if [ -f "${SCRIPT_DIR}/telemetry_monitor.py" ]; then
    python3 "${SCRIPT_DIR}/telemetry_monitor.py" "${OMNI_PID}" "${TELEMETRY_CSV}" &
    MONITOR_PID=$!
    echo "[+] Telemetry monitor running with PID: ${MONITOR_PID}"
fi

# Wait for omni_engine to finish
wait "${OMNI_PID}"
EXIT_CODE=$?

echo "[*] Omni Engine completed with exit code: ${EXIT_CODE}"
if [ -n "${MONITOR_PID}" ]; then
    wait "${MONITOR_PID}" 2>/dev/null || true
    echo "[*] Telemetry captured in ${TELEMETRY_CSV}"
fi
