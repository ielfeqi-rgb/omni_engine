import sys
import argparse
import json
from typing import List, Optional
from models import Task, TaskStatus
from storage import TaskStore
from graph import DependencyGraph, CycleError
from executor import PipelineExecutor, PipelineResult

def load_pipeline_file(filepath: str) -> List[Task]:
    with open(filepath, 'r', encoding='utf-8') as f:
        raw = json.load(f)
    return [Task.from_dict(item) for item in raw]

def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="OmniLite Task Pipeline Runner")
    subparsers = parser.add_subparsers(dest="subcommand", help="Commands")

    val_parser = subparsers.add_parser("validate", help="Validate pipeline DAG")
    val_parser.add_argument("--file", required=True, help="Path to pipeline JSON file")

    run_parser = subparsers.add_parser("run", help="Execute pipeline DAG")
    run_parser.add_argument("--file", required=True, help="Path to pipeline JSON file")
    run_parser.add_argument("--storage", default=None, help="Optional persistence path")

    status_parser = subparsers.add_parser("status", help="Show saved tasks state")
    status_parser.add_argument("--storage", required=True, help="Path to storage JSON file")

    args = parser.parse_args(argv if argv is not None else sys.argv[1:])
    if args.subcommand == "validate":
        tasks = load_pipeline_file(args.file)
        graph = DependencyGraph(tasks)
        batches = graph.get_execution_batches()
        print(f"Validation SUCCESS: {len(tasks)} tasks in {len(batches)} batches.")
        return 0
    elif args.subcommand == "run":
        tasks = load_pipeline_file(args.file)
        store = TaskStore(args.storage) if args.storage else None
        executor = PipelineExecutor(store=store)
        res = executor.run_pipeline(tasks)
        print(f"Execution finished: {res.succeeded}/{res.total_tasks} passed in {res.duration_ms:.1f}ms")
        return 0 if res.failed == 0 else 1
    elif args.subcommand == "status":
        store = TaskStore(args.storage)
        tasks = store.load_tasks()
        print(f"Loaded {len(tasks)} tasks from {args.storage}")
        for t in tasks:
            print(f" - [{t.status.value}] {t.id}: {t.name}")
        return 0
    else:
        parser.print_help()
        return 0

if __name__ == "__main__":
    sys.exit(main())