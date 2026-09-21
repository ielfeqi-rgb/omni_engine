use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::planner::supervisor::{DistilledCausalLesson, InternalSupervisorProbe};
use crate::sandbox::MemoryVfs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepGoalStage {
    pub stage_index: usize,
    pub description: String,
    pub target_filename: String,
    pub is_completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepReasoningState {
    pub goal: String,
    pub stages: Vec<DeepGoalStage>,
    pub current_stage_idx: usize,
    pub current_branch_idx: usize,
    pub lessons: Vec<DistilledCausalLesson>,
    pub is_finished: bool,
}

pub struct DeepReasoningEngine {
    pub supervisor: Arc<InternalSupervisorProbe>,
    pub vfs: Arc<MemoryVfs>,
}

impl DeepReasoningEngine {
    pub fn new(supervisor: Arc<InternalSupervisorProbe>, vfs: Arc<MemoryVfs>) -> Self {
        Self { supervisor, vfs }
    }

    /// Initialize a multi-stage goal and speculate branches
    pub fn initialize_deep_goal(&self, goal: &str, branches: Vec<(String, String)>) -> DeepReasoningState {
        self.supervisor.set_goal(goal);

        for (b_id, strategy) in &branches {
            self.supervisor.register_branch(b_id, strategy, vec![]);
        }

        if let Some((first_id, _)) = branches.first() {
            self.supervisor.set_branch_active(first_id);
        }

        DeepReasoningState {
            goal: goal.to_string(),
            stages: Vec::new(),
            current_stage_idx: 0,
            current_branch_idx: 0,
            lessons: Vec::new(),
            is_finished: false,
        }
    }

    /// Decompose goal into discrete, isolated stages
    pub fn plan_stages(&self, state: &mut DeepReasoningState, stage_descriptions: Vec<(String, String)>) {
        for (idx, (desc, file)) in stage_descriptions.into_iter().enumerate() {
            state.stages.push(DeepGoalStage {
                stage_index: idx + 1,
                description: desc,
                target_filename: file,
                is_completed: false,
            });
        }
    }

    /// Execute rollback on current branch, record lesson, and switch to next branch
    pub fn fail_branch_and_pivot(
        &self,
        state: &mut DeepReasoningState,
        branch_id: &str,
        cause: &str,
        lesson_rule: &str,
        next_branch_id: &str,
    ) -> DistilledCausalLesson {
        let lesson = self.supervisor.prune_branch_with_causal_lesson(
            branch_id,
            "Active Architecture Hypothesis",
            cause,
            lesson_rule,
            450, // surgical KV tokens purged
        );

        state.lessons.push(lesson.clone());
        self.supervisor.set_branch_active(next_branch_id);
        lesson
    }

    /// Mark current stage complete and advance
    pub fn complete_current_stage(&self, state: &mut DeepReasoningState) -> bool {
        if state.current_stage_idx < state.stages.len() {
            state.stages[state.current_stage_idx].is_completed = true;
            state.current_stage_idx += 1;
        }

        if state.current_stage_idx >= state.stages.len() {
            state.is_finished = true;
        }

        state.is_finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_reasoning_lifecycle() {
        let supervisor = InternalSupervisorProbe::new();
        let vfs = Arc::new(MemoryVfs::new());
        let engine = DeepReasoningEngine::new(supervisor, vfs);

        let mut state = engine.initialize_deep_goal(
            "Build Mini FPS",
            vec![
                ("BRANCH_CANVAS".to_string(), "2D Canvas Raycaster".to_string()),
                ("BRANCH_THREEJS".to_string(), "Three.js WebGL Engine".to_string()),
            ],
        );

        engine.plan_stages(
            &mut state,
            vec![
                ("Setup 3D Canvas Context".to_string(), "renderer.js".to_string()),
                ("Player WASD & Mouse Movement".to_string(), "controls.js".to_string()),
            ],
        );

        assert_eq!(state.stages.len(), 2);

        // Fail Canvas branch, pivot to ThreeJS
        let lesson = engine.fail_branch_and_pivot(
            &mut state,
            "BRANCH_CANVAS",
            "FPS dropped below 20",
            "Canvas 2D lacks GPU depth testing; must use WebGL/Three.js",
            "BRANCH_THREEJS",
        );

        assert_eq!(lesson.lesson_id, 1);
        assert_eq!(state.lessons.len(), 1);

        // Complete stages
        assert!(!engine.complete_current_stage(&mut state));
        assert!(engine.complete_current_stage(&mut state));
        assert!(state.is_finished);
    }
}
