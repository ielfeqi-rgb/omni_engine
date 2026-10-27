import unittest
import tempfile
import os
import json
from models import Task, TaskStatus
from storage import TaskStore
from graph import DependencyGraph, CycleError
from executor import PipelineExecutor, PipelineResult
from cli import main, load_pipeline_file

class TestModels(unittest.TestCase):
    def test_task_serialization(self):
        t = Task(id="1", name="test", command="echo hi")
        self.assertEqual(t.status, TaskStatus.PENDING)
        d = t.to_dict()
        self.assertEqual(d["status"], "PENDING")
        t2 = Task.from_dict(d)
        self.assertEqual(t, t2)

class TestStorage(unittest.TestCase):
    def test_save_and_load(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            p = os.path.join(tmpdir, "tasks.json")
            store = TaskStore(p)
            self.assertEqual(store.load_tasks(), [])
            t = Task(id="t1", name="T1", command="echo test")
            store.save_tasks([t])
            loaded = store.load_tasks()
            self.assertEqual(len(loaded), 1)
            self.assertEqual(loaded[0].id, "t1")

class TestGraph(unittest.TestCase):
    def test_batches(self):
        t1 = Task(id="a", name="A", command="echo a")
        t2 = Task(id="b", name="B", command="echo b", depends_on=["a"])
        graph = DependencyGraph([t1, t2])
        batches = graph.get_execution_batches()
        self.assertEqual(len(batches), 2)
        self.assertEqual(batches[0][0].id, "a")
        self.assertEqual(batches[1][0].id, "b")

    def test_cycle(self):
        t1 = Task(id="a", name="A", command="echo a", depends_on=["b"])
        t2 = Task(id="b", name="B", command="echo b", depends_on=["a"])
        graph = DependencyGraph([t1, t2])
        self.assertTrue(graph.has_cycle())
        with self.assertRaises(CycleError):
            graph.get_execution_batches()

class TestExecutor(unittest.TestCase):
    def test_pipeline_execution(self):
        t1 = Task(id="a", name="A", command="echo 'hello world'")
        t2 = Task(id="b", name="B", command="echo 'task b'", depends_on=["a"])
        executor = PipelineExecutor()
        res = executor.run_pipeline([t1, t2])
        self.assertEqual(res.succeeded, 2)
        self.assertEqual(res.failed, 0)
        self.assertEqual(t1.status, TaskStatus.COMPLETED)
        self.assertEqual(t1.output, "hello world")

    def test_pipeline_failure_cascade(self):
        t1 = Task(id="fail", name="F", command="exit 1")
        t2 = Task(id="dep", name="D", command="echo ok", depends_on=["fail"])
        executor = PipelineExecutor()
        res = executor.run_pipeline([t1, t2])
        self.assertEqual(res.failed, 2)
        self.assertEqual(t1.status, TaskStatus.FAILED)
        self.assertEqual(t2.status, TaskStatus.FAILED)

class TestCli(unittest.TestCase):
    def test_help(self):
        self.assertEqual(main([]), 0)

if __name__ == "__main__":
    unittest.main()