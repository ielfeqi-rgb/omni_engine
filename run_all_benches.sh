#!/bin/bash
set -e

echo "# 🧪 Omni Engine - Multi-Model Benchmark Report (Rigorous)" > BENCHMARK_REPORT.md
echo "Date: $(date)" >> BENCHMARK_REPORT.md
echo "N=30 iterations per test." >> BENCHMARK_REPORT.md
echo "" >> BENCHMARK_REPORT.md

MODELS=("models/qwen-0.5b.gguf" "models/gemma-2b.gguf" "models/qwen2.5-coder-1.5b-instruct-q4_k_m.gguf")

for MODEL in "${MODELS[@]}"; do
    if [ -f "$MODEL" ]; then
        echo "Running benchmark for $MODEL..."
        echo "## Model: $(basename $MODEL)" >> BENCHMARK_REPORT.md
        echo '```text' >> BENCHMARK_REPORT.md
        ./target/release/omni_bench --model "$MODEL" --iterations 30 >> BENCHMARK_REPORT.md
        echo '```' >> BENCHMARK_REPORT.md
        echo "" >> BENCHMARK_REPORT.md
    else
        echo "Model $MODEL not found, skipping."
    fi
done

echo "Benchmark complete! Check BENCHMARK_REPORT.md"
