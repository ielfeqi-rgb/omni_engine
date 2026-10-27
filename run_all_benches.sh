#!/usr/bin/env bash
# ==============================================================================
# Omni Engine (v2.1.2) - Comprehensive Systems Benchmark Harness
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="${SCRIPT_DIR}/bin:${LD_LIBRARY_PATH}"

OUTPUT_LOG="${1:-benchmark_results.log}"
ITERATIONS="${2:-30}"

echo "================================================================================"
echo "  Omni Engine Benchmark Suite (Iterations: ${ITERATIONS})"
echo "  Logging output to: ${OUTPUT_LOG}"
echo "================================================================================"

{
    echo "# Omni Engine Benchmark Report"
    echo "Date: $(date -u)"
    echo "Host: $(uname -s) $(uname -m)"
    echo "Iterations: ${ITERATIONS}"
    echo "================================================================================"
} > "${OUTPUT_LOG}"

# 1. Swarm Stress Test & Dynamic DAG Benchmark
if [ -f "${SCRIPT_DIR}/target/release/swarm_stress_test" ]; then
    echo "[*] Running RCPSP Scheduler & Parallel Swarm Stress Test..."
    "${SCRIPT_DIR}/target/release/swarm_stress_test" | tee -a "${OUTPUT_LOG}"
fi

# 2. In-Process Model Latency & KV Benchmark
MODELS_FOUND=0
for MODEL_FILE in "${SCRIPT_DIR}/models"/*.gguf; do
    if [ -f "${MODEL_FILE}" ]; then
        MODELS_FOUND=1
        echo "[*] Benchmarking Model: $(basename "${MODEL_FILE}")"
        echo -e "\n## Model: $(basename "${MODEL_FILE}")" >> "${OUTPUT_LOG}"
        if [ -f "${SCRIPT_DIR}/target/release/omni_bench" ]; then
            "${SCRIPT_DIR}/target/release/omni_bench" --model "${MODEL_FILE}" --iterations "${ITERATIONS}" 2>&1 | tee -a "${OUTPUT_LOG}"
        fi
    fi
done

if [ "${MODELS_FOUND}" -eq 0 ]; then
    echo "[!] No GGUF models found in models/ directory. Run './bin/omni_engine download --qwen' to download default model."
fi

echo "================================================================================"
echo "[+] Benchmarks completed successfully! Results saved in ${OUTPUT_LOG}"
echo "================================================================================"
