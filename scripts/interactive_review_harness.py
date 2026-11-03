#!/usr/bin/env python3
"""
Omni Engine v2.1.2 - Interactive Plan Review & Steering Daemon Harness
Enables human-in-the-loop plan review, critique steering, causal KV-cache rollback,
and cleanroom delivery tracking in multi-turn interactive environments.
"""

import os
import sys
import time
import json
import argparse
import subprocess
import select

def run_harness():
    parser = argparse.ArgumentParser(description="Omni Engine Interactive Review Harness")
    parser.add_argument("--engine-bin", default="./target/release/omni_engine", help="Path to omni_engine binary")
    parser.add_argument("--model", required=True, help="Path to GGUF model")
    parser.add_argument("--goal", required=True, help="User goal / prompt")
    parser.add_argument("--output-dir", default="./workspace/task_tracker", help="Deliverables output directory")
    parser.add_argument("--session-dir", default="./workspace/review_session", help="Session state directory")
    parser.add_argument("--modify", action="store_true", help="Run in modify / incremental mode")
    parser.add_argument("--subgoals", type=int, default=3, help="Max subgoals")
    parser.add_argument("--steps", type=int, default=5, help="Max steps per worker")
    args = parser.parse_args()

    session_dir = os.path.abspath(args.session_dir)
    os.makedirs(session_dir, exist_ok=True)
    os.makedirs(os.path.abspath(args.output_dir), exist_ok=True)

    status_file = os.path.join(session_dir, "status.json")
    command_file = os.path.join(session_dir, "command.txt")
    log_file = os.path.join(session_dir, "engine.log")

    if os.path.exists(command_file):
        os.remove(command_file)

    def write_status(data):
        tmp_path = status_file + ".tmp"
        with open(tmp_path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
        os.replace(tmp_path, status_file)

    write_status({
        "state": "STARTING",
        "goal": args.goal,
        "model": args.model,
        "output_dir": args.output_dir,
        "timestamp": time.time()
    })

    env = os.environ.copy()
    base_dir = os.path.abspath(os.path.dirname(os.path.dirname(__file__)))
    lib_path = os.path.join(base_dir, "bin")
    env["LD_LIBRARY_PATH"] = f"{lib_path}:{env.get('LD_LIBRARY_PATH', '')}"

    cmd = [
        args.engine_bin,
        "modify" if args.modify else "swarm",
        args.goal,
        "--review",
        "--model", args.model,
        "--output-dir", args.output_dir,
        "--subgoals", str(args.subgoals),
        "--steps", str(args.steps),
    ]
    if args.modify:
        cmd.extend(["--input-dir", args.output_dir])

    print(f"[*] Launching: {' '.join(cmd)}")
    log_fp = open(log_file, "w", encoding="utf-8", buffering=1)

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        bufsize=0,
        env=env,
        cwd=base_dir
    )

    iteration = 1
    buffer = ""
    start_time = time.time()

    try:
        while True:
            r, _, _ = select.select([proc.stdout], [], [], 0.1)
            if r:
                chunk_bytes = os.read(proc.stdout.fileno(), 4096)
                if not chunk_bytes:
                    if proc.poll() is not None:
                        break
                    time.sleep(0.05)
                    continue

                chunk = chunk_bytes.decode("utf-8", errors="replace")
                log_fp.write(chunk)
                log_fp.flush()
                sys.stdout.write(chunk)
                sys.stdout.flush()
                buffer += chunk

                # Check if review prompt has appeared in buffer
                if "Approve? [yes / no <critique>]:" in buffer:
                    parts = buffer.split("[PLAN REVIEW v")
                    if len(parts) > 1:
                        v_num = parts[-1].split("]")[0]
                        try:
                            iteration = int(v_num)
                        except Exception:
                            iteration += 1

                    box = ""
                    box_start = buffer.rfind("┌──────────────────────────────────────────────")
                    box_end = buffer.rfind("└──────────────────────────────────────────────")
                    if box_start != -1 and box_end != -1 and box_end > box_start:
                        # find the end of line after the box end
                        eol = buffer.find("\n", box_end)
                        if eol != -1:
                            box = buffer[box_start:eol]
                        else:
                            box = buffer[box_start:]
                    else:
                        box = buffer

                    print(f"\n[HARNESS] Detected Plan v{iteration}! Waiting for user command in {command_file}...")

                    write_status({
                        "state": "AWAITING_REVIEW",
                        "iteration": iteration,
                        "plan_preview": box,
                        "prompt": f"[PLAN REVIEW v{iteration}] Approve? [yes / no <critique>]:",
                        "timestamp": time.time()
                    })

                    buffer = ""

                    user_cmd = None
                    while True:
                        if proc.poll() is not None:
                            break
                        if os.path.exists(command_file):
                            try:
                                with open(command_file, "r", encoding="utf-8") as cf:
                                    content = cf.read().strip()
                                    if content:
                                        user_cmd = content
                                        break
                            except Exception:
                                pass
                        time.sleep(0.1)

                    if user_cmd:
                        print(f"\n[HARNESS] Delivering user command to engine: '{user_cmd}'")
                        try:
                            os.remove(command_file)
                        except Exception:
                            pass

                        write_status({
                            "state": "STEERING_DELIVERED",
                            "iteration": iteration,
                            "command": user_cmd,
                            "timestamp": time.time()
                        })

                        os.write(proc.stdin.fileno(), (user_cmd + "\n").encode("utf-8"))
            else:
                if proc.poll() is not None:
                    break

        ret = proc.wait()
        elapsed = time.time() - start_time
        print(f"\n[*] Omni Engine exited with code {ret} in {elapsed:.1f}s")
        write_status({
            "state": "COMPLETED",
            "exit_code": ret,
            "duration": elapsed,
            "timestamp": time.time()
        })
    finally:
        log_fp.close()

if __name__ == "__main__":
    run_harness()
