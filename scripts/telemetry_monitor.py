#!/usr/bin/env python3
import time
import sys
import os
import subprocess

def monitor(target_pid, output_csv):
    with open(output_csv, 'w') as f:
        f.write("timestamp,elapsed_sec,rss_mb,vsz_mb,cpu_pct,swap_used_mb\n")
        start_time = time.time()
        while True:
            # Check if process is still alive
            if not os.path.exists(f"/proc/{target_pid}"):
                break
            
            try:
                # Get process stats
                res = subprocess.check_output(
                    ["ps", "-p", str(target_pid), "-o", "rss,vsz,%cpu", "--no-headers"],
                    text=True
                ).strip().split()
                rss_kb = int(res[0])
                vsz_kb = int(res[1])
                cpu_pct = float(res[2])
                
                # Get swap stats
                free_out = subprocess.check_output(["free", "-m"], text=True)
                swap_used = 0
                for line in free_out.splitlines():
                    if line.startswith("Swap:"):
                        swap_used = int(line.split()[2])
                
                elapsed = time.time() - start_time
                f.write(f"{time.time()},{elapsed:.1f},{rss_kb/1024:.2f},{vsz_kb/1024:.2f},{cpu_pct},{swap_used}\n")
                f.flush()
            except Exception:
                pass
            time.sleep(1.0)

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: telemetry_monitor.py <PID> <OUTPUT_CSV>")
        sys.exit(1)
    monitor(int(sys.argv[1]), sys.argv[2])
