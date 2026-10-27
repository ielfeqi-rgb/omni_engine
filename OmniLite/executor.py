import time
import subprocess
from dataclasses import dataclass, field
from typing import List, Optional
from models import Task, TaskStatus
from storage import TaskStore
from graph import DependencyGraph, CycleError

@dataclass
class PipelineResult:
    total_tasks: int
    succeeded: int
    failed: int
    duration_ms: float
    tasks: List[Task] = field(default_factory=list)

class PipelineExecutor:
    def __init__(self, store: Optional[TaskStore] = None):
        self.store = store

    def execute_task(self, task: Task) -> Task:
        task.status = TaskStatus.RUNNING
        start = time.perf_counter()
        proc = subprocess.run(task.command, shell=True, capture_output=True, text=True)
        if proc.returncode == 0:
            task.status = TaskStatus.COMPLETED
            task.output = proc.stdout.strip()
            task.error = None
        else:
            task.status = TaskStatus.FAILED
            task.output = proc.stdout.strip()
            task.error = proc.stderr.strip() or f"Process exited with code {proc.returncode}"
        return task

    def run_pipeline(self, tasks: List[Task]) -> PipelineResult:
        start_time = time.perf_counter()
        graph = DependencyGraph(tasks)
        batches = graph.get_execution_batches()
        failed_ids = set()
        all_tasks = {t.id: t for t in tasks}
        for batch in batches:
            for task in batch:
                has_failed_dep = any(dep in failed_ids for dep in task.depends_on)
                if has_failed_dep:
                    task.status = TaskStatus.FAILED
                    task.error = "Prerequisite dependency failed"
                    failed_ids.add(task.id)
                else:
                    self.execute_task(task)
                    if task.status == TaskStatus.FAILED:
                        failed_ids.add(task.id)
        if self.store:
            self.store.save_tasks(list(all_tasks.values()))
        duration_ms = (time.perf_counter() - start_time) * 1000.0
        succeeded = sum(1 for t in all_tasks.values() if t.status == TaskStatus.COMPLETED)
        failed = sum(1 for t in all_tasks.values() if t.status == TaskStatus.FAILED)
        return PipelineResult(total_tasks=len(all_tasks), succeeded=succeeded, failed=failed, duration_ms=duration_ms, tasks=list(all_tasks.values()))