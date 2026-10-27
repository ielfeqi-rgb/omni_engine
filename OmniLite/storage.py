import os
import json
import tempfile
from typing import List, Optional
from models import Task

class TaskStore:
    def __init__(self, storage_path: str):
        self.storage_path = storage_path

    def save_tasks(self, tasks: List[Task]) -> None:
        target_dir = os.path.dirname(os.path.abspath(self.storage_path)) or "."
        os.makedirs(target_dir, exist_ok=True)
        fd, temp_path = tempfile.mkstemp(dir=target_dir, prefix="store_", suffix=".tmp")
        try:
            with os.fdopen(fd, 'w', encoding='utf-8') as f:
                data = [t.to_dict() for t in tasks]
                json.dump(data, f, indent=2)
                f.flush()
                os.fsync(f.fileno())
            os.replace(temp_path, self.storage_path)
        except Exception:
            if os.path.exists(temp_path):
                os.remove(temp_path)
            raise

    def load_tasks(self) -> List[Task]:
        if not os.path.exists(self.storage_path):
            return []
        with open(self.storage_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            return [Task.from_dict(item) for item in data]

    def get_task(self, task_id: str) -> Optional[Task]:
        for task in self.load_tasks():
            if task.id == task_id:
                return task
        return None