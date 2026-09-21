use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistilledCausalLesson {
    pub lesson_id: usize,
    pub branch_id: String,
    pub failed_hypothesis: String,
    pub root_cause: String,
    pub distillation_rule: String,
    pub active_tokens_saved: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BranchStatus {
    Hypothesized,
    ActiveEvaluating,
    TrappedAndPruned,
    ValidatedAndMerged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubExperiment {
    pub exp_id: usize,
    pub parent_branch: String,
    pub purpose: String,
    pub isolated_code_snippet: String,
    pub execution_success: bool,
    pub discovery_outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchExecutionPlan {
    pub branch_id: String,
    pub strategy_name: String,
    pub file_targets: Vec<String>,
    pub status: BranchStatus,
    pub sub_experiments: Vec<SubExperiment>,
}

/// Internal Agent Supervisory & Telemetry Engine.
/// Private to supervisor model: evaluates thinking, prunes dead branches,
/// harvests distilled causal lessons, and executes isolated sub-experiments.
pub struct InternalSupervisorProbe {
    pub session_start: Instant,
    pub active_goal: Mutex<Option<String>>,
    pub branches: Mutex<Vec<BranchExecutionPlan>>,
    pub lessons: Mutex<Vec<DistilledCausalLesson>>,
    pub sub_experiments_counter: Mutex<usize>,
    pub probe_telemetry_stream: Mutex<Vec<String>>,
}

impl InternalSupervisorProbe {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            session_start: Instant::now(),
            active_goal: Mutex::new(None),
            branches: Mutex::new(Vec::new()),
            lessons: Mutex::new(Vec::new()),
            sub_experiments_counter: Mutex::new(0),
            probe_telemetry_stream: Mutex::new(Vec::new()),
        })
    }

    pub fn set_goal(&self, goal: &str) {
        let mut g = self.active_goal.lock().unwrap_or_else(|e| e.into_inner());
        *g = Some(goal.to_string());
        self.record_telemetry(&format!("GOAL_INITIALIZED: '{}'", goal));
    }

    pub fn register_branch(&self, branch_id: &str, strategy_name: &str, file_targets: Vec<String>) {
        let mut b = self.branches.lock().unwrap();
        b.push(BranchExecutionPlan {
            branch_id: branch_id.to_string(),
            strategy_name: strategy_name.to_string(),
            file_targets,
            status: BranchStatus::Hypothesized,
            sub_experiments: Vec::new(),
        });
        self.record_telemetry(&format!("BRANCH_REGISTERED: id='{}' strategy='{}'", branch_id, strategy_name));
    }

    pub fn set_branch_active(&self, branch_id: &str) {
        let mut b = self.branches.lock().unwrap();
        for branch in b.iter_mut() {
            if branch.branch_id == branch_id {
                branch.status = BranchStatus::ActiveEvaluating;
                self.record_telemetry(&format!("BRANCH_ACTIVATED: id='{}'", branch_id));
            }
        }
    }

    /// Trap detection + Causal Distillation + Surgical KV-Pruning signal
    pub fn prune_branch_with_causal_lesson(
        &self,
        branch_id: &str,
        failed_hypothesis: &str,
        root_cause: &str,
        distillation_rule: &str,
        tokens_purged: usize,
    ) -> DistilledCausalLesson {
        let mut lessons = self.lessons.lock().unwrap();
        let lesson_id = lessons.len() + 1;

        let lesson = DistilledCausalLesson {
            lesson_id,
            branch_id: branch_id.to_string(),
            failed_hypothesis: failed_hypothesis.to_string(),
            root_cause: root_cause.to_string(),
            distillation_rule: distillation_rule.to_string(),
            active_tokens_saved: tokens_purged,
        };

        lessons.push(lesson.clone());

        // Update branch status
        let mut b = self.branches.lock().unwrap();
        for branch in b.iter_mut() {
            if branch.branch_id == branch_id {
                branch.status = BranchStatus::TrappedAndPruned;
            }
        }

        self.record_telemetry(&format!(
            "BRANCH_PRUNED: id='{}' tokens_purged={} distilled_rule='{}'",
            branch_id, tokens_purged, distillation_rule
        ));

        lesson
    }

    /// Spawn isolated sub-experiment in scratch space (Experiment-inside-Experiment)
    pub fn trigger_sub_experiment(
        &self,
        parent_branch: &str,
        purpose: &str,
        code_snippet: &str,
        success: bool,
        outcome: Option<String>,
    ) -> SubExperiment {
        let mut counter = self.sub_experiments_counter.lock().unwrap();
        *counter += 1;
        let exp_id = *counter;

        let sub_exp = SubExperiment {
            exp_id,
            parent_branch: parent_branch.to_string(),
            purpose: purpose.to_string(),
            isolated_code_snippet: code_snippet.to_string(),
            execution_success: success,
            discovery_outcome: outcome.clone(),
        };

        let mut b = self.branches.lock().unwrap();
        for branch in b.iter_mut() {
            if branch.branch_id == parent_branch {
                branch.sub_experiments.push(sub_exp.clone());
            }
        }

        self.record_telemetry(&format!(
            "SUB_EXPERIMENT: id={} parent='{}' success={} outcome='{:?}'",
            exp_id, parent_branch, success, outcome
        ));

        sub_exp
    }

    /// Synthesize compact prompt advice for the next active branch (Zero Context Bloat)
    pub fn format_causal_guidance_prompt(&self) -> String {
        let lessons = self.lessons.lock().unwrap();
        if lessons.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("\n[CRITICAL CAUSAL CONSTRAINTS FROM PRIOR PRUNED BRANCHES]:\n");
        for l in lessons.iter() {
            prompt.push_str(&format!("- Rule #{}: {}\n", l.lesson_id, l.distillation_rule));
        }
        prompt.push_str("[Do NOT repeat the architecture or APIs flagged above.]\n");
        prompt
    }

    pub fn dump_supervisor_dense_telemetry(&self) -> String {
        let elapsed_ms = self.session_start.elapsed().as_millis();
        let logs = self.probe_telemetry_stream.lock().unwrap();
        let branches = self.branches.lock().unwrap();
        let lessons = self.lessons.lock().unwrap();

        format!(
            "PROBE_METRICS: elapsed_ms={} branches_total={} active_lessons={} logs_count={}",
            elapsed_ms,
            branches.len(),
            lessons.len(),
            logs.len()
        )
    }

    fn record_telemetry(&self, entry: &str) {
        let mut stream = self.probe_telemetry_stream.lock().unwrap();
        let ts = self.session_start.elapsed().as_millis();
        stream.push(format!("[{:#06}ms] {}", ts, entry));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervisor_branch_pruning_and_distilled_lessons() {
        let supervisor = InternalSupervisorProbe::new();
        supervisor.set_goal("Build WebGL Mini FPS");

        // 1. Register 2 speculative branches
        supervisor.register_branch("B1_CANVAS", "2D Raycaster Engine", vec!["world.js".to_string()]);
        supervisor.register_branch("B2_THREEJS", "Three.js WebGL Engine", vec!["renderer.js".to_string()]);

        supervisor.set_branch_active("B1_CANVAS");

        // 2. Trigger sub-experiment in scratch space
        let sub = supervisor.trigger_sub_experiment(
            "B1_CANVAS",
            "Testing 2D Raycast Wall Traversal at 60 FPS",
            "for (let x = 0; x < width; x++) { castRay(x); }",
            false,
            Some("Raycast computation exceeds 16ms CPU frame budget on small models".to_string()),
        );
        assert!(!sub.execution_success);

        // 3. Prune B1_CANVAS and distill causal rule
        let lesson = supervisor.prune_branch_with_causal_lesson(
            "B1_CANVAS",
            "Pure 2D Canvas CPU Raycasting",
            "CPU frame rendering dropped to 14 FPS with 300 rays",
            "Avoid software 2D Canvas raycasting for 3D worlds; utilize hardware-accelerated WebGL/Three.js instead.",
            850, // 850 failed tokens purged from KV cache
        );

        assert_eq!(lesson.lesson_id, 1);
        assert_eq!(lesson.active_tokens_saved, 850);

        // 4. Verify that compact advice for B2_THREEJS is generated cleanly
        let advice = supervisor.format_causal_guidance_prompt();
        assert!(advice.contains("Rule #1"));
        assert!(advice.contains("Avoid software 2D Canvas raycasting"));

        // 5. Dense telemetry dump check
        let dump = supervisor.dump_supervisor_dense_telemetry();
        assert!(dump.contains("branches_total=2"));
        assert!(dump.contains("active_lessons=1"));
    }
}
