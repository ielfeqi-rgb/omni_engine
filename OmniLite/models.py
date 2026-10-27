from dataclasses import dataclass, field
from enum import Enum
from typing import List, Optional, Dict, Any

class TaskStatus(Enum):
    PENDING = "PENDING"
    RUNNING = "RUNNING"
    COMPLETED = "COMPLETED"
    FAILED = "FAILED"

@dataclass
class Task:
    id: str
    name: str
    command: str
    depends_on: List[str] = field(default_factory=list)
    status: TaskStatus = TaskStatus.PENDING
    output: Optional[str] = None
    error: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {"id": self.id, "name": self.name, "command": self.command, "depends_on": self.depends_on, "status": self.status.value, "output": self.output, "error": self.error}

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "Task":
        return cls(id=data["id"], name=data["name"], command=data["command"], depends_on=data.get("depends_on", []), status=TaskStatus(data.get("status", "PENDING")), output=data.get("output"), error=data.get("error"))