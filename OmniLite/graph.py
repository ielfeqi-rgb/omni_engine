from collections import defaultdict, deque
from typing import List, Dict, Set
from models import Task

class CycleError(Exception):
    pass

class DependencyGraph:
    def __init__(self, tasks: List[Task]):
        self.tasks = {t.id: t for t in tasks}
        self.adj = defaultdict(list)
        self.in_degree = {t.id: 0 for t in tasks}
        for task in tasks:
            for dep in task.depends_on:
                if dep in self.tasks:
                    self.adj[dep].append(task.id)
                    self.in_degree[task.id] += 1

    def has_cycle(self) -> bool:
        in_deg = dict(self.in_degree)
        queue = deque([tid for tid, deg in in_deg.items() if deg == 0])
        visited_count = 0
        while queue:
            curr = queue.popleft()
            visited_count += 1
            for neighbor in self.adj[curr]:
                in_deg[neighbor] -= 1
                if in_deg[neighbor] == 0:
                    queue.append(neighbor)
        return visited_count != len(self.tasks)

    def get_execution_batches(self) -> List[List[Task]]:
        if self.has_cycle():
            raise CycleError("Cycle detected in dependency graph")
        in_deg = dict(self.in_degree)
        queue = deque([tid for tid, deg in in_deg.items() if deg == 0])
        batches = []
        while queue:
            current_batch = list(queue)
            queue.clear()
            batch_tasks = [self.tasks[tid] for tid in current_batch]
            batches.append(batch_tasks)
            for tid in current_batch:
                for neighbor in self.adj[tid]:
                    in_deg[neighbor] -= 1
                    if in_deg[neighbor] == 0:
                        queue.append(neighbor)
        return batches

if __name__ == '__main__':
    # Example usage
    tasks = [
        Task(id="A", name="Task A", command="echo 'Hello, World!'"),
        Task(id="B", name="Task B", command="sleep 2", depends_on=["A"]),
        Task(id="C", name="Task C", command="echo 'Goodbye, World!'")
    ]
    graph = DependencyGraph(tasks)
    try:
        batches = graph.get_execution_batches()
        print("Execution Batches:", batches)
    except CycleError as e:
        print(e)