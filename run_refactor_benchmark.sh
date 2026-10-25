#!/usr/bin/env bash
set -e

WORKSPACE_DIR="/home/hema/Downloads/files(1)/omni_engine"
cd "${WORKSPACE_DIR}"

LOG_FILE="${WORKSPACE_DIR}/refactor_run.log"
TELEMETRY_CSV="${WORKSPACE_DIR}/telemetry_refactor.csv"
MODEL_PATH="${WORKSPACE_DIR}/models/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf"

GOAL_TEXT="In refactor_enterprise_challenge, perform cross-cutting multi-tenant organization isolation: in models.py add org_id: int = 1 to Task, in repository.py update TaskRepository to filter by org_id and accept org_id in create, in service.py update TaskService to require valid org_id > 0, in test_suite.py add new test_multi_tenant_isolation test and keep all existing tests passing."

echo "[*] Launching Omni Engine v2.1.0 in modify mode..."
export LD_LIBRARY_PATH="${WORKSPACE_DIR}/bin:${LD_LIBRARY_PATH}"

# Launch omni_engine in background
./target/release/omni_engine modify \
  --dir refactor_enterprise_challenge \
  --output-dir refactor_enterprise_challenge \
  --model "${MODEL_PATH}" \
  --subgoals 3 \
  --steps 5 \
  --goal "${GOAL_TEXT}" > "${LOG_FILE}" 2>&1 &

OMNI_PID=$!
echo "[+] Omni Engine running with PID: ${OMNI_PID}"

# Launch telemetry monitor
python3 telemetry_monitor.py "${OMNI_PID}" "${TELEMETRY_CSV}" &
MONITOR_PID=$!
echo "[+] Telemetry monitor running with PID: ${MONITOR_PID}"

# Wait for omni_engine to finish
wait "${OMNI_PID}"
EXIT_CODE=$?

echo "[*] Omni Engine completed with exit code: ${EXIT_CODE}"
wait "${MONITOR_PID}" 2>/dev/null || true

echo "[*] Telemetry captured in ${TELEMETRY_CSV}"
